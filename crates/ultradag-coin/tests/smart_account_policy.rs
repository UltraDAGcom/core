/// SmartAccount spending policy & vault integration tests.

use ultradag_coin::address::{Address, SecretKey, Signature};
use ultradag_coin::state::StateEngine;
use ultradag_coin::tx::smart_account::*;

fn setup(sk: &SecretKey, balance: u64) -> StateEngine {
    let mut engine = StateEngine::new_with_genesis();
    engine.faucet_credit(&sk.address(), balance).unwrap();
    engine
}

fn make_set_policy_tx(
    sk: &SecretKey, instant_limit: u64, vault_threshold: u64,
    vault_delay: u64, whitelist: Vec<Address>, daily_limit: Option<u64>, nonce: u64,
) -> SetPolicyTx {
    let mut tx = SetPolicyTx {
        from: sk.address(),
        instant_limit,
        vault_threshold,
        vault_delay_rounds: vault_delay,
        whitelisted_recipients: whitelist,
        daily_limit,
        fee: 10_000,
        nonce,
        pub_key: sk.verifying_key().to_bytes(),
        signature: Signature([0u8; 64]),
    };
    tx.signature = sk.sign(&tx.signable_bytes());
    tx
}

fn make_smart_transfer(
    sk: &SecretKey, to: Address, amount: u64, fee: u64, nonce: u64,
) -> SmartTransferTx {
    let pubkey = sk.verifying_key().to_bytes();
    let key_id = AuthorizedKey::compute_key_id(KeyType::Ed25519, &pubkey);
    let mut tx = SmartTransferTx {
        from: sk.address(),
        to,
        amount,
        fee,
        nonce,
        signing_key_id: key_id,
        signature: vec![],
        memo: None,
        webauthn: None,
    };
    let signable = tx.signable_bytes();
    let sig = sk.sign(&signable);
    tx.signature = sig.0.to_vec();
    tx
}

fn make_execute_vault_tx(sk: &SecretKey, transfer_id: [u8; 8], nonce: u64) -> ExecuteVaultTx {
    let mut tx = ExecuteVaultTx {
        from: sk.address(),
        transfer_id,
        nonce,
        pub_key: sk.verifying_key().to_bytes(),
        signature: Signature([0u8; 64]),
    };
    tx.signature = sk.sign(&tx.signable_bytes());
    tx
}

fn make_cancel_vault_tx(sk: &SecretKey, transfer_id: [u8; 8], nonce: u64) -> CancelVaultTx {
    let mut tx = CancelVaultTx {
        from: sk.address(),
        transfer_id,
        nonce,
        pub_key: sk.verifying_key().to_bytes(),
        signature: Signature([0u8; 64]),
    };
    tx.signature = sk.sign(&tx.signable_bytes());
    tx
}

#[test]
fn test_set_policy_time_locked() {
    let owner = SecretKey::from_bytes([0x01; 32]);
    let mut engine = setup(&owner, 10_000_000_000);

    let tx = make_set_policy_tx(&owner, 100_000_000, 1_000_000_000, 720, vec![], Some(500_000_000), 0);
    engine.apply_set_policy_tx(&tx, 100).unwrap();

    // Policy is pending, not yet active
    let config = engine.smart_account(&owner.address()).unwrap();
    assert!(config.policy.is_none());
    assert!(config.pending_policy_change.is_some());

    let executes_at = config.pending_policy_change.as_ref().unwrap().executes_at_round;

    // Process before time-lock — no change
    engine.process_pending_policy_changes(executes_at - 1);
    assert!(engine.smart_account(&owner.address()).unwrap().policy.is_none());

    // Process at time-lock — policy active
    engine.process_pending_policy_changes(executes_at);
    let config = engine.smart_account(&owner.address()).unwrap();
    assert!(config.policy.is_some());
    assert!(config.pending_policy_change.is_none());

    let policy = config.policy.as_ref().unwrap();
    assert_eq!(policy.instant_limit, 100_000_000);
    assert_eq!(policy.vault_threshold, 1_000_000_000);
    assert_eq!(policy.daily_limit, Some(500_000_000));
}

#[test]
fn test_vault_transfer_lifecycle() {
    let owner = SecretKey::from_bytes([0x02; 32]);
    let recipient = SecretKey::from_bytes([0x03; 32]);
    let mut engine = setup(&owner, 50_000_000_000); // 500 UDAG

    // Register key by doing a transfer first (auto-registers on first outgoing tx)
    let small_tx = make_smart_transfer(&owner, recipient.address(), 10_000, 10_000, 0);
    // Need to register the key first
    engine.ensure_smart_account_for_test(&owner.address(), 0);
    let pubkey = owner.verifying_key().to_bytes();
    let key_id = AuthorizedKey::compute_key_id(KeyType::Ed25519, &pubkey);
    if let Some(config) = engine.smart_account_mut_for_test(&owner.address()) {
        if config.authorized_keys.is_empty() {
            config.authorized_keys.push(AuthorizedKey {
                key_id,
                key_type: KeyType::Ed25519,
                pubkey: pubkey.to_vec(),
                label: "owner".to_string(),
                daily_limit: None,
                daily_spent: (0, 0),
            });
        }
    }

    // Set policy: instant up to 1 UDAG, vault above 10 UDAG, 720 round delay
    let policy_tx = make_set_policy_tx(&owner, 100_000_000, 1_000_000_000, 720, vec![], None, 0);
    engine.apply_set_policy_tx(&policy_tx, 100).unwrap();
    engine.process_pending_policy_changes(100 + POLICY_CHANGE_DELAY_ROUNDS);

    // Transfer below instant_limit — should work instantly
    let balance_before = engine.balance(&recipient.address());
    let tx1 = make_smart_transfer(&owner, recipient.address(), 50_000_000, 10_000, 1);
    engine.apply_smart_transfer_tx(&tx1).unwrap();
    assert_eq!(engine.balance(&recipient.address()), balance_before + 50_000_000);

    // Transfer above vault_threshold — should go to vault
    let tx2 = make_smart_transfer(&owner, recipient.address(), 5_000_000_000, 10_000, 2);
    let recipient_before = engine.balance(&recipient.address());
    engine.apply_smart_transfer_tx(&tx2).unwrap();
    // Recipient should NOT have received the funds yet
    assert_eq!(engine.balance(&recipient.address()), recipient_before);

    // Vault transfer should be pending
    let config = engine.smart_account(&owner.address()).unwrap();
    assert_eq!(config.pending_vault_transfers.len(), 1);
    let vault = &config.pending_vault_transfers[0];
    let transfer_id = vault.transfer_id;
    let executes_at = vault.executes_at_round;

    // Try to execute too early — should fail
    let exec_tx = make_execute_vault_tx(&owner, transfer_id, 3);
    let result = engine.apply_execute_vault_tx(&exec_tx, executes_at - 1);
    assert!(result.is_err());

    // Execute at the right time
    let exec_tx2 = make_execute_vault_tx(&owner, transfer_id, 3);
    engine.apply_execute_vault_tx(&exec_tx2, executes_at).unwrap();

    // Now recipient should have the funds
    assert_eq!(engine.balance(&recipient.address()), recipient_before + 5_000_000_000);

    // Vault should be cleared
    let config = engine.smart_account(&owner.address()).unwrap();
    assert_eq!(config.pending_vault_transfers.len(), 0);
}

#[test]
fn test_cancel_vault_refunds() {
    let owner = SecretKey::from_bytes([0x04; 32]);
    let recipient = SecretKey::from_bytes([0x05; 32]);
    let mut engine = setup(&owner, 50_000_000_000);

    // Setup: register key + set policy with vault
    engine.ensure_smart_account_for_test(&owner.address(), 0);
    let pubkey = owner.verifying_key().to_bytes();
    let key_id = AuthorizedKey::compute_key_id(KeyType::Ed25519, &pubkey);
    if let Some(config) = engine.smart_account_mut_for_test(&owner.address()) {
        config.authorized_keys.push(AuthorizedKey {
            key_id, key_type: KeyType::Ed25519, pubkey: pubkey.to_vec(),
            label: "owner".to_string(), daily_limit: None, daily_spent: (0, 0),
        });
    }

    let policy_tx = make_set_policy_tx(&owner, 100_000_000, 1_000_000_000, 720, vec![], None, 0);
    engine.apply_set_policy_tx(&policy_tx, 100).unwrap();
    engine.process_pending_policy_changes(100 + POLICY_CHANGE_DELAY_ROUNDS);

    // Create vault transfer
    let balance_before_vault = engine.balance(&owner.address());
    let tx = make_smart_transfer(&owner, recipient.address(), 5_000_000_000, 10_000, 1);
    engine.apply_smart_transfer_tx(&tx).unwrap();

    // Balance should be reduced (amount debited for vault)
    assert!(engine.balance(&owner.address()) < balance_before_vault);

    // Cancel — should refund
    let config = engine.smart_account(&owner.address()).unwrap();
    let transfer_id = config.pending_vault_transfers[0].transfer_id;

    let balance_before_cancel = engine.balance(&owner.address());
    let cancel_tx = make_cancel_vault_tx(&owner, transfer_id, 2);
    engine.apply_cancel_vault_tx(&cancel_tx).unwrap();

    // Balance should increase by the refunded amount
    assert_eq!(engine.balance(&owner.address()), balance_before_cancel + 5_000_000_000);

    // No pending vault transfers
    let config = engine.smart_account(&owner.address()).unwrap();
    assert_eq!(config.pending_vault_transfers.len(), 0);
}

#[test]
fn test_whitelisted_recipient_bypasses_vault() {
    let owner = SecretKey::from_bytes([0x06; 32]);
    let cold_storage = SecretKey::from_bytes([0x07; 32]);
    let mut engine = setup(&owner, 50_000_000_000);

    // Setup key
    engine.ensure_smart_account_for_test(&owner.address(), 0);
    let pubkey = owner.verifying_key().to_bytes();
    let key_id = AuthorizedKey::compute_key_id(KeyType::Ed25519, &pubkey);
    if let Some(config) = engine.smart_account_mut_for_test(&owner.address()) {
        config.authorized_keys.push(AuthorizedKey {
            key_id, key_type: KeyType::Ed25519, pubkey: pubkey.to_vec(),
            label: "owner".to_string(), daily_limit: None, daily_spent: (0, 0),
        });
    }

    // Set policy with cold_storage whitelisted
    let policy_tx = make_set_policy_tx(
        &owner, 100_000_000, 1_000_000_000, 720,
        vec![cold_storage.address()], Some(200_000_000), 0,
    );
    engine.apply_set_policy_tx(&policy_tx, 100).unwrap();
    engine.process_pending_policy_changes(100 + POLICY_CHANGE_DELAY_ROUNDS);

    // Large transfer to whitelisted address — should be instant (no vault)
    let balance_before = engine.balance(&cold_storage.address());
    let tx = make_smart_transfer(&owner, cold_storage.address(), 20_000_000_000, 10_000, 1);
    engine.apply_smart_transfer_tx(&tx).unwrap();

    // Recipient got funds instantly
    assert_eq!(engine.balance(&cold_storage.address()), balance_before + 20_000_000_000);

    // No vault transfers pending
    let config = engine.smart_account(&owner.address()).unwrap();
    assert_eq!(config.pending_vault_transfers.len(), 0);
}

#[test]
fn test_daily_limit_enforced() {
    let owner = SecretKey::from_bytes([0x08; 32]);
    let recipient = SecretKey::from_bytes([0x09; 32]);
    let mut engine = setup(&owner, 50_000_000_000);

    // Setup key + policy with 1 UDAG daily limit
    engine.ensure_smart_account_for_test(&owner.address(), 0);
    let pubkey = owner.verifying_key().to_bytes();
    let key_id = AuthorizedKey::compute_key_id(KeyType::Ed25519, &pubkey);
    if let Some(config) = engine.smart_account_mut_for_test(&owner.address()) {
        config.authorized_keys.push(AuthorizedKey {
            key_id, key_type: KeyType::Ed25519, pubkey: pubkey.to_vec(),
            label: "owner".to_string(), daily_limit: None, daily_spent: (0, 0),
        });
    }

    let policy_tx = make_set_policy_tx(&owner, u64::MAX, 0, 0, vec![], Some(100_000_000), 0); // 1 UDAG daily
    engine.apply_set_policy_tx(&policy_tx, 100).unwrap();
    engine.process_pending_policy_changes(100 + POLICY_CHANGE_DELAY_ROUNDS);

    // First transfer: 0.5 UDAG — should succeed
    let tx1 = make_smart_transfer(&owner, recipient.address(), 50_000_000, 10_000, 1);
    engine.apply_smart_transfer_tx(&tx1).unwrap();

    // Second transfer: 0.6 UDAG — should fail (total would be 1.1 UDAG > 1 UDAG limit)
    let tx2 = make_smart_transfer(&owner, recipient.address(), 60_000_000, 10_000, 2);
    let result = engine.apply_smart_transfer_tx(&tx2);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("daily spending limit"));
}
