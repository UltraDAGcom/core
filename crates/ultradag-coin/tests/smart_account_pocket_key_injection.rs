//! Regression tests for the pocket key-injection attack chain.
//!
//! A pocket's address is derived from `(parent_address, label)` and does NOT
//! derive from any keypair — so `Address::from_pubkey(pub_key) == pocket_addr`
//! is impossible. Before the fix, `auto_register_ed25519_key` only guarded on
//! `authorized_keys.is_empty()`, which meant any attacker could submit a
//! policy/key/recovery tx targeting a pocket and plant their own key on the
//! pocket's SmartAccountConfig. Because `verify_smart_transfer` checks the
//! pocket's own config before falling back to the parent, the planted key
//! then authorized SmartTransferTx → pocket drain.
//!
//! These tests replay each link in the attack chain and assert the
//! authorization check rejects it.

use ultradag_coin::address::{Address, SecretKey, Signature};
use ultradag_coin::state::StateEngine;
use ultradag_coin::tx::name_registry::derive_pocket_address;
use ultradag_coin::tx::smart_account::*;

fn setup_victim_with_pocket(
    alice: &SecretKey, pocket_label: &str, pocket_balance: u64,
) -> (StateEngine, Address) {
    let mut engine = StateEngine::new_with_genesis();
    engine.faucet_credit(&alice.address(), 20_000_000_000).unwrap();
    engine.ensure_smart_account(&alice.address());

    // Seed Alice's key so auto-registration on the parent doesn't mask the test.
    let alice_pub = alice.verifying_key().to_bytes();
    let alice_kid = AuthorizedKey::compute_key_id(KeyType::Ed25519, &alice_pub);
    let cfg = engine.smart_account_mut_for_test(&alice.address()).unwrap();
    if !cfg.authorized_keys.iter().any(|k| k.key_id == alice_kid) {
        cfg.authorized_keys.push(AuthorizedKey {
            key_id: alice_kid,
            key_type: KeyType::Ed25519,
            pubkey: alice_pub.to_vec(),
            label: "owner".to_string(),
            daily_limit: None,
            daily_spent: (0, 0),
        });
    }

    // Alice creates a pocket and funds it.
    let pocket_op = SmartOpTx {
        from: alice.address(),
        operation: SmartOpType::CreatePocket { label: pocket_label.to_string() },
        fee: 0, nonce: 0,
        signing_key_id: [0u8; 8],
        signature: vec![],
        webauthn: None,
        p256_pubkey: None,
    };
    engine.apply_smart_op_tx(&pocket_op, 100).unwrap();
    let pocket = derive_pocket_address(&alice.address(), pocket_label);
    engine.credit(&pocket, pocket_balance).unwrap();

    (engine, pocket)
}

#[test]
fn set_policy_cannot_plant_attacker_key_on_pocket() {
    let alice = SecretKey::from_bytes([0xA1; 32]);
    let mallory = SecretKey::from_bytes([0xEE; 32]);
    let (mut engine, pocket) = setup_victim_with_pocket(&alice, "savings", 5_000_000_000);

    // Mallory forges a SetPolicyTx targeting Alice's pocket with Mallory's own key.
    let mallory_pub = mallory.verifying_key().to_bytes();
    let mut tx = SetPolicyTx {
        from: pocket,
        instant_limit: u64::MAX,
        vault_threshold: 0,
        vault_delay_rounds: 0,
        whitelisted_recipients: vec![],
        daily_limit: None,
        fee: 0, nonce: 0,
        pub_key: mallory_pub,
        signature: Signature([0u8; 64]),
    };
    tx.signature = mallory.sign(&tx.signable_bytes());

    let err = engine.apply_set_policy_tx(&tx, 100).unwrap_err();
    assert!(
        err.to_string().contains("not authorized"),
        "expected authorization failure, got: {err}"
    );

    // Pocket's authorized_keys must remain empty.
    if let Some(cfg) = engine.smart_account(&pocket) {
        assert!(
            cfg.authorized_keys.is_empty(),
            "pocket must never accept an auto-registered key"
        );
    }
}

#[test]
fn add_key_cannot_plant_attacker_key_on_pocket() {
    let alice = SecretKey::from_bytes([0xA2; 32]);
    let mallory = SecretKey::from_bytes([0xED; 32]);
    let (mut engine, pocket) = setup_victim_with_pocket(&alice, "savings", 5_000_000_000);

    let mallory_pub = mallory.verifying_key().to_bytes();
    let mallory_kid = AuthorizedKey::compute_key_id(KeyType::Ed25519, &mallory_pub);
    let mut tx = AddKeyTx {
        from: pocket,
        new_key: AuthorizedKey {
            key_id: mallory_kid,
            key_type: KeyType::Ed25519,
            pubkey: mallory_pub.to_vec(),
            label: "attacker".to_string(),
            daily_limit: None,
            daily_spent: (0, 0),
        },
        fee: 0, nonce: 0,
        pub_key: mallory_pub,
        signature: Signature([0u8; 64]),
    };
    tx.signature = mallory.sign(&tx.signable_bytes());

    let err = engine.apply_add_key_tx(&tx).unwrap_err();
    assert!(
        err.to_string().contains("not authorized"),
        "expected authorization failure, got: {err}"
    );

    if let Some(cfg) = engine.smart_account(&pocket) {
        assert!(cfg.authorized_keys.is_empty());
    }
}

#[test]
fn set_recovery_cannot_plant_attacker_key_on_pocket() {
    let alice = SecretKey::from_bytes([0xA3; 32]);
    let mallory = SecretKey::from_bytes([0xEC; 32]);
    let (mut engine, pocket) = setup_victim_with_pocket(&alice, "savings", 5_000_000_000);

    let mallory_pub = mallory.verifying_key().to_bytes();
    let mut tx = SetRecoveryTx {
        from: pocket,
        guardians: vec![mallory.address()],
        threshold: 1,
        delay_rounds: 100,
        fee: 0, nonce: 0,
        pub_key: mallory_pub,
        signature: Signature([0u8; 64]),
    };
    tx.signature = mallory.sign(&tx.signable_bytes());

    let err = engine.apply_set_recovery_tx(&tx).unwrap_err();
    assert!(
        err.to_string().contains("not authorized"),
        "expected authorization failure, got: {err}"
    );

    if let Some(cfg) = engine.smart_account(&pocket) {
        assert!(cfg.authorized_keys.is_empty());
    }
}

#[test]
fn full_drain_attack_chain_fails() {
    let alice = SecretKey::from_bytes([0xA4; 32]);
    let mallory = SecretKey::from_bytes([0xEB; 32]);
    let (mut engine, pocket) = setup_victim_with_pocket(&alice, "savings", 5_000_000_000);
    let initial_pocket_balance = engine.balance(&pocket);

    // Step 1: Mallory tries to plant her key on the pocket via SetPolicyTx.
    // (Before the fix, this would succeed and register mallory_pub on the pocket.)
    let mallory_pub = mallory.verifying_key().to_bytes();
    let mut set_policy = SetPolicyTx {
        from: pocket,
        instant_limit: u64::MAX,
        vault_threshold: 0,
        vault_delay_rounds: 0,
        whitelisted_recipients: vec![],
        daily_limit: None,
        fee: 0, nonce: 0,
        pub_key: mallory_pub,
        signature: Signature([0u8; 64]),
    };
    set_policy.signature = mallory.sign(&set_policy.signable_bytes());
    let _ = engine.apply_set_policy_tx(&set_policy, 100);

    // Step 2: Mallory attempts the drain. Even if step 1 had silently no-op'd
    // on the authorization side, a subsequent SmartTransferTx must fail at
    // verify_smart_transfer because Mallory's key is neither on the pocket
    // (the fix) nor on Alice's parent config.
    let mallory_kid = AuthorizedKey::compute_key_id(KeyType::Ed25519, &mallory_pub);
    let mut transfer = SmartTransferTx {
        from: pocket,
        to: mallory.address(),
        amount: initial_pocket_balance,
        fee: 0, nonce: 0,
        signing_key_id: mallory_kid,
        signature: vec![],
        memo: None,
        webauthn: None,
    };
    transfer.signature = mallory.sign(&transfer.signable_bytes()).0.to_vec();

    assert!(
        !engine.verify_smart_transfer(&transfer),
        "Mallory's signature must not verify against the pocket or its parent"
    );

    // Pocket balance must be untouched.
    assert_eq!(engine.balance(&pocket), initial_pocket_balance);
}

#[test]
fn auto_register_key_rejects_pocket_address() {
    let alice = SecretKey::from_bytes([0xA5; 32]);
    let mallory = SecretKey::from_bytes([0xEA; 32]);
    let (mut engine, pocket) = setup_victim_with_pocket(&alice, "savings", 1_000_000_000);

    // The RPC relay-endpoint path. Even though this path is currently
    // called with a p256-derived address in production, the function must
    // refuse to register a key directly on a pocket as defense in depth.
    let mallory_pub = mallory.verifying_key().to_bytes();
    let err = engine
        .auto_register_key(&pocket, KeyType::Ed25519, mallory_pub.to_vec())
        .unwrap_err();
    assert!(
        err.to_string().contains("pocket"),
        "expected pocket-specific rejection, got: {err}"
    );
}
