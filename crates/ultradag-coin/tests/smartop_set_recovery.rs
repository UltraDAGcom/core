//! Regression tests for SmartOpType::SetRecovery and SmartOpType::SetPolicy.
//!
//! These SmartOp variants were added so passkey wallets can configure social
//! recovery and spending policy without needing a standalone Ed25519 sig path.
//! Prior to this, the dashboard had full UIs for both but the submit buttons
//! were "preview only — waiting on Rust variant".

use ultradag_coin::address::{Address, SecretKey};
use ultradag_coin::constants::COIN;
use ultradag_coin::state::StateEngine;
use ultradag_coin::tx::smart_account::*;

fn setup(sk: &SecretKey, balance: u64) -> StateEngine {
    let mut engine = StateEngine::new_with_genesis();
    engine.faucet_credit(&sk.address(), balance).unwrap();
    engine.ensure_smart_account(&sk.address());
    engine
}

fn set_recovery_op(
    from: Address,
    guardians: Vec<Address>,
    threshold: u8,
    delay_rounds: u64,
    nonce: u64,
) -> SmartOpTx {
    SmartOpTx {
        from,
        operation: SmartOpType::SetRecovery { guardians, threshold, delay_rounds },
        fee: 0,
        nonce,
        signing_key_id: [0u8; 8],
        signature: vec![],
        webauthn: None,
        p256_pubkey: None,
    }
}

fn set_policy_op(
    from: Address,
    instant_limit: u64,
    vault_threshold: u64,
    vault_delay_rounds: u64,
    whitelisted_recipients: Vec<Address>,
    daily_limit: Option<u64>,
    nonce: u64,
) -> SmartOpTx {
    SmartOpTx {
        from,
        operation: SmartOpType::SetPolicy {
            instant_limit,
            vault_threshold,
            vault_delay_rounds,
            whitelisted_recipients,
            daily_limit,
        },
        fee: 0,
        nonce,
        signing_key_id: [0u8; 8],
        signature: vec![],
        webauthn: None,
        p256_pubkey: None,
    }
}

// ── SetRecovery ─────────────────────────────────────────────────────────────

#[test]
fn set_recovery_happy_path() {
    let alice = SecretKey::from_bytes([0xa1; 32]);
    let g1 = SecretKey::from_bytes([0xb1; 32]).address();
    let g2 = SecretKey::from_bytes([0xb2; 32]).address();
    let g3 = SecretKey::from_bytes([0xb3; 32]).address();

    let mut engine = setup(&alice, 500 * COIN);

    let op = set_recovery_op(alice.address(), vec![g1, g2, g3], 2, MIN_RECOVERY_DELAY_ROUNDS, 0);
    engine.apply_smart_op_tx(&op, 100).unwrap();

    let config = engine.smart_account(&alice.address()).unwrap();
    let rec = config.recovery.as_ref().expect("recovery set");
    assert_eq!(rec.guardians, vec![g1, g2, g3]);
    assert_eq!(rec.threshold, 2);
    assert_eq!(rec.delay_rounds, MIN_RECOVERY_DELAY_ROUNDS);
}

#[test]
fn set_recovery_rejects_empty_guardians() {
    let alice = SecretKey::from_bytes([0xa2; 32]);
    let mut engine = setup(&alice, 500 * COIN);

    let op = set_recovery_op(alice.address(), vec![], 1, MIN_RECOVERY_DELAY_ROUNDS, 0);
    let err = engine.apply_smart_op_tx(&op, 100).unwrap_err();
    assert!(err.to_string().contains("at least one guardian"));
}

#[test]
fn set_recovery_rejects_threshold_out_of_range() {
    let alice = SecretKey::from_bytes([0xa3; 32]);
    let g1 = SecretKey::from_bytes([0xb1; 32]).address();
    let mut engine = setup(&alice, 500 * COIN);

    // threshold=0
    let op = set_recovery_op(alice.address(), vec![g1], 0, MIN_RECOVERY_DELAY_ROUNDS, 0);
    assert!(engine.apply_smart_op_tx(&op, 100).is_err());

    // threshold > guardians.len()
    let op = set_recovery_op(alice.address(), vec![g1], 2, MIN_RECOVERY_DELAY_ROUNDS, 0);
    let err = engine.apply_smart_op_tx(&op, 100).unwrap_err();
    assert!(err.to_string().contains("threshold"));
}

#[test]
fn set_recovery_rejects_self_as_guardian() {
    let alice = SecretKey::from_bytes([0xa4; 32]);
    let mut engine = setup(&alice, 500 * COIN);

    let op = set_recovery_op(
        alice.address(),
        vec![alice.address()],
        1,
        MIN_RECOVERY_DELAY_ROUNDS,
        0,
    );
    let err = engine.apply_smart_op_tx(&op, 100).unwrap_err();
    assert!(err.to_string().contains("self as guardian"));
}

#[test]
fn set_recovery_rejects_duplicate_guardians() {
    let alice = SecretKey::from_bytes([0xa5; 32]);
    let g1 = SecretKey::from_bytes([0xb1; 32]).address();
    let mut engine = setup(&alice, 500 * COIN);

    let op = set_recovery_op(alice.address(), vec![g1, g1], 1, MIN_RECOVERY_DELAY_ROUNDS, 0);
    let err = engine.apply_smart_op_tx(&op, 100).unwrap_err();
    assert!(err.to_string().contains("duplicate"));
}

#[test]
fn set_recovery_rejects_delay_out_of_range() {
    let alice = SecretKey::from_bytes([0xa6; 32]);
    let g1 = SecretKey::from_bytes([0xb1; 32]).address();
    let mut engine = setup(&alice, 500 * COIN);

    // Below minimum
    let op = set_recovery_op(alice.address(), vec![g1], 1, MIN_RECOVERY_DELAY_ROUNDS - 1, 0);
    assert!(engine.apply_smart_op_tx(&op, 100).is_err());

    // Above maximum
    let op = set_recovery_op(alice.address(), vec![g1], 1, MAX_RECOVERY_DELAY_ROUNDS + 1, 0);
    assert!(engine.apply_smart_op_tx(&op, 100).is_err());
}

#[test]
fn set_recovery_cancels_pending_recovery() {
    // Calling SetRecovery again should wipe any pending_recovery state.
    let alice = SecretKey::from_bytes([0xa7; 32]);
    let g1 = SecretKey::from_bytes([0xb1; 32]).address();
    let g2 = SecretKey::from_bytes([0xb2; 32]).address();
    let mut engine = setup(&alice, 500 * COIN);

    // First, set recovery.
    let op1 = set_recovery_op(alice.address(), vec![g1, g2], 1, MIN_RECOVERY_DELAY_ROUNDS, 0);
    engine.apply_smart_op_tx(&op1, 100).unwrap();

    // Reconfigure — new guardian set.
    let g3 = SecretKey::from_bytes([0xb3; 32]).address();
    let op2 = set_recovery_op(alice.address(), vec![g1, g3], 2, MIN_RECOVERY_DELAY_ROUNDS, 1);
    engine.apply_smart_op_tx(&op2, 101).unwrap();

    let config = engine.smart_account(&alice.address()).unwrap();
    let rec = config.recovery.as_ref().unwrap();
    assert_eq!(rec.guardians, vec![g1, g3]);
    assert_eq!(rec.threshold, 2);
    assert!(config.pending_recovery.is_none());
}

// ── SetPolicy ───────────────────────────────────────────────────────────────

#[test]
fn set_policy_happy_path() {
    let alice = SecretKey::from_bytes([0xc1; 32]);
    let mut engine = setup(&alice, 500 * COIN);

    let op = set_policy_op(
        alice.address(),
        10 * COIN,   // instant_limit
        100 * COIN,  // vault_threshold
        20,          // vault_delay_rounds
        vec![],
        Some(50 * COIN),
        0,
    );
    engine.apply_smart_op_tx(&op, 100).unwrap();

    let config = engine.smart_account(&alice.address()).unwrap();
    let pending = config.pending_policy_change.as_ref().expect("pending policy change");
    assert_eq!(pending.new_policy.instant_limit, 10 * COIN);
    assert_eq!(pending.new_policy.vault_threshold, 100 * COIN);
    assert_eq!(pending.new_policy.daily_limit, Some(50 * COIN));
    assert_eq!(pending.initiated_at_round, 100);
    assert_eq!(
        pending.executes_at_round,
        100u64.saturating_add(POLICY_CHANGE_DELAY_ROUNDS)
    );
}

#[test]
fn set_policy_rejects_instant_exceeding_vault_threshold() {
    let alice = SecretKey::from_bytes([0xc2; 32]);
    let mut engine = setup(&alice, 500 * COIN);

    let op = set_policy_op(
        alice.address(),
        100 * COIN,  // instant_limit
        10 * COIN,   // vault_threshold < instant
        20,
        vec![],
        None,
        0,
    );
    let err = engine.apply_smart_op_tx(&op, 100).unwrap_err();
    assert!(err.to_string().contains("instant_limit cannot exceed vault_threshold"));
}

#[test]
fn set_policy_rejects_too_many_whitelisted_recipients() {
    let alice = SecretKey::from_bytes([0xc3; 32]);
    let mut engine = setup(&alice, 500 * COIN);

    let many: Vec<Address> = (0..(MAX_WHITELISTED_RECIPIENTS + 1))
        .map(|i| SecretKey::from_bytes([i as u8 | 0x80; 32]).address())
        .collect();

    let op = set_policy_op(alice.address(), 0, 0, 20, many, None, 0);
    let err = engine.apply_smart_op_tx(&op, 100).unwrap_err();
    assert!(err.to_string().contains("too many whitelisted recipients"));
}

#[test]
fn set_policy_time_lock_applied() {
    // Sanity: the SmartOp path uses the same POLICY_CHANGE_DELAY_ROUNDS time
    // lock as the standalone SetPolicyTx path. The new policy does NOT take
    // effect immediately — it goes into pending_policy_change.
    let alice = SecretKey::from_bytes([0xc4; 32]);
    let mut engine = setup(&alice, 500 * COIN);

    let op = set_policy_op(alice.address(), 5 * COIN, 0, 0, vec![], None, 0);
    engine.apply_smart_op_tx(&op, 42).unwrap();

    let config = engine.smart_account(&alice.address()).unwrap();
    // Live policy is still default — the change is pending.
    assert!(config.pending_policy_change.is_some());
    let pending = config.pending_policy_change.as_ref().unwrap();
    assert_eq!(pending.initiated_at_round, 42);
    assert_eq!(pending.executes_at_round, 42 + POLICY_CHANGE_DELAY_ROUNDS);
}

// ── Serialization sanity ────────────────────────────────────────────────────

#[test]
fn discriminants_are_stable() {
    // Genesis-critical: if discriminants shift, every persisted tx becomes
    // unparseable. Lock them down by byte-checking signable_bytes prefix.
    let addr = SecretKey::from_bytes([0xd0; 32]).address();

    let rec = SmartOpTx {
        from: addr,
        operation: SmartOpType::SetRecovery {
            guardians: vec![],
            threshold: 0,
            delay_rounds: 0,
        },
        fee: 0, nonce: 0, signing_key_id: [0; 8],
        signature: vec![], webauthn: None, p256_pubkey: None,
    };
    let bytes = rec.signable_bytes();
    // Skip NETWORK_ID + "smart_op" + from (20) — discriminant byte comes next.
    let prefix_len = ultradag_coin::constants::NETWORK_ID.len() + b"smart_op".len() + 20;
    assert_eq!(bytes[prefix_len], 18, "SetRecovery must stay at discriminant 18");

    let pol = SmartOpTx {
        from: addr,
        operation: SmartOpType::SetPolicy {
            instant_limit: 0,
            vault_threshold: 0,
            vault_delay_rounds: 0,
            whitelisted_recipients: vec![],
            daily_limit: None,
        },
        fee: 0, nonce: 0, signing_key_id: [0; 8],
        signature: vec![], webauthn: None, p256_pubkey: None,
    };
    let bytes = pol.signable_bytes();
    assert_eq!(bytes[prefix_len], 19, "SetPolicy must stay at discriminant 19");
}
