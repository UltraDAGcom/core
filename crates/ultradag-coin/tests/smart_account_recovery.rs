/// SmartAccount social recovery integration tests.
///
/// Tests the full guardian-based account recovery lifecycle:
/// set guardians → guardian approvals → time-lock → execution/cancellation.

use ultradag_coin::address::{Address, SecretKey, Signature};
use ultradag_coin::state::StateEngine;
use ultradag_coin::tx::smart_account::*;

fn setup_engine_with_account(sk: &SecretKey, balance: u64) -> StateEngine {
    let mut engine = StateEngine::new_with_genesis();
    engine.faucet_credit(&sk.address(), balance).unwrap();
    engine
}

fn make_add_key_tx(sk: &SecretKey, new_key: AuthorizedKey, nonce: u64) -> AddKeyTx {
    let mut tx = AddKeyTx {
        from: sk.address(),
        new_key,
        fee: 10_000,
        nonce,
        pub_key: sk.verifying_key().to_bytes(),
        signature: Signature([0u8; 64]),
    };
    tx.signature = sk.sign(&tx.signable_bytes());
    tx
}

fn make_set_recovery_tx(
    sk: &SecretKey, guardians: Vec<Address>, threshold: u8, delay: u64, nonce: u64,
) -> SetRecoveryTx {
    let mut tx = SetRecoveryTx {
        from: sk.address(),
        guardians,
        threshold,
        delay_rounds: delay,
        fee: 10_000,
        nonce,
        pub_key: sk.verifying_key().to_bytes(),
        signature: Signature([0u8; 64]),
    };
    tx.signature = sk.sign(&tx.signable_bytes());
    tx
}

fn make_recover_tx(
    guardian_sk: &SecretKey, target: Address, new_key: AuthorizedKey,
    revoke: bool, nonce: u64,
) -> RecoverAccountTx {
    let mut tx = RecoverAccountTx {
        target_account: target,
        from: guardian_sk.address(),
        new_key,
        revoke_existing: revoke,
        nonce,
        pub_key: guardian_sk.verifying_key().to_bytes(),
        signature: Signature([0u8; 64]),
    };
    tx.signature = guardian_sk.sign(&tx.signable_bytes());
    tx
}

fn make_cancel_recovery_tx(sk: &SecretKey, nonce: u64) -> CancelRecoveryTx {
    let mut tx = CancelRecoveryTx {
        from: sk.address(),
        nonce,
        pub_key: sk.verifying_key().to_bytes(),
        signature: Signature([0u8; 64]),
    };
    tx.signature = sk.sign(&tx.signable_bytes());
    tx
}

fn make_ed25519_authorized_key(sk: &SecretKey, label: &str) -> AuthorizedKey {
    let pubkey = sk.verifying_key().to_bytes();
    AuthorizedKey {
        key_id: AuthorizedKey::compute_key_id(KeyType::Ed25519, &pubkey),
        key_type: KeyType::Ed25519,
        pubkey: pubkey.to_vec(),
        label: label.to_string(),
        daily_limit: None,
        daily_spent: (0, 0),
    }
}

#[test]
fn test_set_recovery_guardians() {
    let owner = SecretKey::from_bytes([0x01; 32]);
    let g1 = SecretKey::from_bytes([0x10; 32]);
    let g2 = SecretKey::from_bytes([0x11; 32]);
    let g3 = SecretKey::from_bytes([0x12; 32]);

    let mut engine = setup_engine_with_account(&owner, 1_000_000_000);

    let tx = make_set_recovery_tx(
        &owner,
        vec![g1.address(), g2.address(), g3.address()],
        2, // 2-of-3
        DEFAULT_RECOVERY_DELAY_ROUNDS,
        0,
    );
    engine.apply_set_recovery_tx(&tx).unwrap();

    let config = engine.smart_account(&owner.address()).unwrap();
    let recovery = config.recovery.as_ref().unwrap();
    assert_eq!(recovery.guardians.len(), 3);
    assert_eq!(recovery.threshold, 2);
    assert_eq!(recovery.delay_rounds, DEFAULT_RECOVERY_DELAY_ROUNDS);
}

#[test]
fn test_cannot_set_self_as_guardian() {
    let owner = SecretKey::from_bytes([0x02; 32]);
    let mut engine = setup_engine_with_account(&owner, 1_000_000_000);

    let tx = make_set_recovery_tx(&owner, vec![owner.address()], 1, MIN_RECOVERY_DELAY_ROUNDS, 0);
    let result = engine.apply_set_recovery_tx(&tx);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("cannot set self as guardian"));
}

#[test]
fn test_recovery_full_lifecycle() {
    let owner = SecretKey::from_bytes([0x03; 32]);
    let g1 = SecretKey::from_bytes([0x20; 32]);
    let g2 = SecretKey::from_bytes([0x21; 32]);
    let g3 = SecretKey::from_bytes([0x22; 32]);
    let new_device = SecretKey::from_bytes([0x30; 32]);

    let mut engine = setup_engine_with_account(&owner, 1_000_000_000);
    // Fund guardians so they can have nonces
    engine.faucet_credit(&g1.address(), 100_000_000).unwrap();
    engine.faucet_credit(&g2.address(), 100_000_000).unwrap();

    // Step 1: Owner sets up 2-of-3 recovery with 100-round delay
    let set_tx = make_set_recovery_tx(
        &owner,
        vec![g1.address(), g2.address(), g3.address()],
        2, MIN_RECOVERY_DELAY_ROUNDS, 0,
    );
    engine.apply_set_recovery_tx(&set_tx).unwrap();

    // Step 2: Guardian 1 submits recovery approval
    let new_key = make_ed25519_authorized_key(&new_device, "recovery_key");
    let recover_tx1 = make_recover_tx(&g1, owner.address(), new_key.clone(), true, 0);
    engine.apply_recover_account_tx(&recover_tx1).unwrap();

    // Pending recovery exists but threshold not reached — no time-lock yet
    let config = engine.smart_account(&owner.address()).unwrap();
    let pending = config.pending_recovery.as_ref().unwrap();
    assert_eq!(pending.approvals.len(), 1);
    assert_eq!(pending.executes_at_round, u64::MAX); // Not yet

    // Step 3: Guardian 2 submits matching approval — threshold reached
    let recover_tx2 = make_recover_tx(&g2, owner.address(), new_key.clone(), true, 0);
    engine.apply_recover_account_tx(&recover_tx2).unwrap();

    let config = engine.smart_account(&owner.address()).unwrap();
    let pending = config.pending_recovery.as_ref().unwrap();
    assert_eq!(pending.approvals.len(), 2);
    assert_ne!(pending.executes_at_round, u64::MAX); // Time-lock started

    let executes_at = pending.executes_at_round;

    // Step 4: Process before time-lock — nothing happens
    engine.process_pending_recoveries(executes_at - 1);
    assert!(engine.smart_account(&owner.address()).unwrap().pending_recovery.is_some());

    // Step 5: Process at time-lock — recovery executes
    engine.process_pending_recoveries(executes_at);

    let config = engine.smart_account(&owner.address()).unwrap();
    assert!(config.pending_recovery.is_none()); // Cleared
    assert_eq!(config.authorized_keys.len(), 1); // Old keys revoked, new key added
    assert_eq!(config.authorized_keys[0].key_id, new_key.key_id);
}

#[test]
fn test_owner_cancels_recovery_during_timelock() {
    let owner = SecretKey::from_bytes([0x04; 32]);
    let g1 = SecretKey::from_bytes([0x40; 32]);
    let new_device = SecretKey::from_bytes([0x50; 32]);

    let mut engine = setup_engine_with_account(&owner, 1_000_000_000);
    engine.faucet_credit(&g1.address(), 100_000_000).unwrap();

    // Setup 1-of-1 recovery (threshold=1, immediate time-lock start)
    let set_tx = make_set_recovery_tx(&owner, vec![g1.address()], 1, MIN_RECOVERY_DELAY_ROUNDS, 0);
    engine.apply_set_recovery_tx(&set_tx).unwrap();

    // Guardian submits recovery
    let new_key = make_ed25519_authorized_key(&new_device, "recovery_key");
    let recover_tx = make_recover_tx(&g1, owner.address(), new_key.clone(), true, 0);
    engine.apply_recover_account_tx(&recover_tx).unwrap();

    // Recovery is pending
    assert!(engine.smart_account(&owner.address()).unwrap().pending_recovery.is_some());

    // Owner cancels
    let cancel_tx = make_cancel_recovery_tx(&owner, 1);
    engine.apply_cancel_recovery_tx(&cancel_tx).unwrap();

    // Recovery cleared
    assert!(engine.smart_account(&owner.address()).unwrap().pending_recovery.is_none());
}

#[test]
fn test_mismatched_recovery_params_rejected() {
    let owner = SecretKey::from_bytes([0x05; 32]);
    let g1 = SecretKey::from_bytes([0x60; 32]);
    let g2 = SecretKey::from_bytes([0x61; 32]);
    let new_device1 = SecretKey::from_bytes([0x70; 32]);
    let new_device2 = SecretKey::from_bytes([0x71; 32]);

    let mut engine = setup_engine_with_account(&owner, 1_000_000_000);
    engine.faucet_credit(&g1.address(), 100_000_000).unwrap();
    engine.faucet_credit(&g2.address(), 100_000_000).unwrap();

    let set_tx = make_set_recovery_tx(
        &owner, vec![g1.address(), g2.address()], 2, MIN_RECOVERY_DELAY_ROUNDS, 0,
    );
    engine.apply_set_recovery_tx(&set_tx).unwrap();

    // G1 submits recovery with key1
    let key1 = make_ed25519_authorized_key(&new_device1, "key1");
    let tx1 = make_recover_tx(&g1, owner.address(), key1, true, 0);
    engine.apply_recover_account_tx(&tx1).unwrap();

    // G2 submits recovery with DIFFERENT key — must be rejected
    let key2 = make_ed25519_authorized_key(&new_device2, "key2");
    let tx2 = make_recover_tx(&g2, owner.address(), key2, true, 0);
    let result = engine.apply_recover_account_tx(&tx2);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("don't match"));
}

#[test]
fn test_non_guardian_cannot_recover() {
    let owner = SecretKey::from_bytes([0x06; 32]);
    let g1 = SecretKey::from_bytes([0x80; 32]);
    let attacker = SecretKey::from_bytes([0x90; 32]);
    let new_device = SecretKey::from_bytes([0xA0; 32]);

    let mut engine = setup_engine_with_account(&owner, 1_000_000_000);
    engine.faucet_credit(&attacker.address(), 100_000_000).unwrap();

    let set_tx = make_set_recovery_tx(&owner, vec![g1.address()], 1, MIN_RECOVERY_DELAY_ROUNDS, 0);
    engine.apply_set_recovery_tx(&set_tx).unwrap();

    // Non-guardian tries to submit recovery
    let new_key = make_ed25519_authorized_key(&new_device, "attacker_key");
    let tx = make_recover_tx(&attacker, owner.address(), new_key, true, 0);
    let result = engine.apply_recover_account_tx(&tx);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("not a guardian"));
}

#[test]
fn test_recovery_without_revoke_adds_key() {
    let owner = SecretKey::from_bytes([0x07; 32]);
    let g1 = SecretKey::from_bytes([0xB0; 32]);
    let new_device = SecretKey::from_bytes([0xC0; 32]);

    let mut engine = setup_engine_with_account(&owner, 1_000_000_000);
    engine.faucet_credit(&g1.address(), 100_000_000).unwrap();

    // Setup 1-of-1 recovery (auto-registers owner's key)
    let owner_pubkey = owner.verifying_key().to_bytes();
    let owner_key_id = AuthorizedKey::compute_key_id(KeyType::Ed25519, &owner_pubkey);
    let set_tx = make_set_recovery_tx(&owner, vec![g1.address()], 1, MIN_RECOVERY_DELAY_ROUNDS, 0);
    engine.apply_set_recovery_tx(&set_tx).unwrap();

    // Guardian submits recovery WITHOUT revoke
    let new_key = make_ed25519_authorized_key(&new_device, "new_device");
    let tx = make_recover_tx(&g1, owner.address(), new_key.clone(), false, 0);
    engine.apply_recover_account_tx(&tx).unwrap();

    // Fast forward past time-lock
    let config = engine.smart_account(&owner.address()).unwrap();
    let executes_at = config.pending_recovery.as_ref().unwrap().executes_at_round;
    engine.process_pending_recoveries(executes_at);

    // Both old and new keys should exist
    let config = engine.smart_account(&owner.address()).unwrap();
    assert_eq!(config.authorized_keys.len(), 2); // owner (auto-registered) + new_device
    assert!(config.has_key(&owner_key_id));
    assert!(config.has_key(&new_key.key_id));
}

#[test]
fn test_stale_recovery_auto_expires() {
    let owner = SecretKey::from_bytes([0x08; 32]);
    let g1 = SecretKey::from_bytes([0xD0; 32]);
    let new_device = SecretKey::from_bytes([0xE0; 32]);

    let mut engine = setup_engine_with_account(&owner, 1_000_000_000);
    engine.faucet_credit(&g1.address(), 100_000_000).unwrap();

    // Setup 2-of-2 recovery (need 2 guardians, only have 1 — threshold never reached)
    let g2 = SecretKey::from_bytes([0xD1; 32]);
    let set_tx = make_set_recovery_tx(&owner, vec![g1.address(), g2.address()], 2, MIN_RECOVERY_DELAY_ROUNDS, 0);
    engine.apply_set_recovery_tx(&set_tx).unwrap();

    // Only 1 guardian approves — threshold not reached, executes_at stays MAX
    let new_key = make_ed25519_authorized_key(&new_device, "recovery");
    let tx = make_recover_tx(&g1, owner.address(), new_key.clone(), true, 0);
    engine.apply_recover_account_tx(&tx).unwrap();

    // Fast forward past expiry (RECOVERY_EXPIRY_ROUNDS = 120,960)
    engine.process_pending_recoveries(RECOVERY_EXPIRY_ROUNDS + 1);

    // Recovery should be expired and cleared
    let config = engine.smart_account(&owner.address()).unwrap();
    assert!(config.pending_recovery.is_none());
    // Owner's keys unchanged (recovery was never executed)
}
