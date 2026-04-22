//! Regression tests for the pocket_to_parent rebuild-on-load contract.
//!
//! `pocket_to_parent` is a derived in-memory reverse index (pocket_addr →
//! parent_addr). It is NOT persisted to redb — only the owning parent's
//! `SmartAccountConfig.pockets` list (of labels) is. So after every node
//! restart, the map must be rebuilt from those labels, or:
//!
//! 1. `verify_smart_transfer` for pocket-originated transfers fails at the
//!    parent-fallback step → every pocket becomes unspendable.
//! 2. `check_spending_policy` cannot resolve pocket→parent → falls through
//!    to the pocket's (empty) config → GHSA-9chc-gjfr-6hrq re-emerges.
//!
//! These tests save a StateEngine with a funded pocket + active policy to a
//! redb file, reload into a fresh engine, and assert both properties hold.

use tempfile::NamedTempFile;
use ultradag_coin::address::{Address, SecretKey, Signature};
use ultradag_coin::state::{db, StateEngine};
use ultradag_coin::tx::name_registry::derive_pocket_address;
use ultradag_coin::tx::smart_account::*;

fn seed_engine_with_pocket() -> (StateEngine, SecretKey, Address) {
    let alice = SecretKey::from_bytes([0xA1; 32]);
    let mut engine = StateEngine::new_with_genesis();
    engine.faucet_credit(&alice.address(), 20_000_000_000).unwrap();
    engine.ensure_smart_account(&alice.address());

    // Seed parent's Ed25519 key so the parent config has a real authorized
    // key that can sign pocket transfers after reload.
    let pubkey = alice.verifying_key().to_bytes();
    let key_id = AuthorizedKey::compute_key_id(KeyType::Ed25519, &pubkey);
    let cfg = engine.smart_account_mut_for_test(&alice.address()).unwrap();
    cfg.authorized_keys.push(AuthorizedKey {
        key_id, key_type: KeyType::Ed25519, pubkey: pubkey.to_vec(),
        label: "owner".to_string(), daily_limit: None, daily_spent: (0, 0),
    });

    // Install a strict parent policy that will be enforced on pocket spends.
    let mut policy_tx = SetPolicyTx {
        from: alice.address(),
        instant_limit: 100_000_000,
        vault_threshold: 0,
        vault_delay_rounds: 0,
        whitelisted_recipients: vec![],
        daily_limit: Some(100_000_000), // 1 UDAG/day on the parent
        fee: 0, nonce: 0, // fee 0 keeps the supply invariant simple in-test
        pub_key: pubkey,
        signature: Signature([0u8; 64]),
    };
    policy_tx.signature = alice.sign(&policy_tx.signable_bytes());
    engine.apply_set_policy_tx(&policy_tx, 100).unwrap();
    engine.process_pending_policy_changes(100 + POLICY_CHANGE_DELAY_ROUNDS);

    // Create + fund the pocket.
    let op = SmartOpTx {
        from: alice.address(),
        operation: SmartOpType::CreatePocket { label: "savings".to_string() },
        fee: 0, nonce: 1, signing_key_id: [0u8; 8],
        signature: vec![], webauthn: None, p256_pubkey: None,
    };
    engine.apply_smart_op_tx(&op, 100).unwrap();
    let pocket = derive_pocket_address(&alice.address(), "savings");
    engine.faucet_credit(&pocket, 5_000_000_000).unwrap();

    (engine, alice, pocket)
}

fn save_and_reload(engine: &StateEngine) -> StateEngine {
    let tmp = NamedTempFile::new().unwrap();
    let path = tmp.path().to_path_buf();
    drop(tmp); // redb wants to own the file
    db::save_to_redb(engine, &path).unwrap();
    db::load_from_redb(&path).unwrap()
}

#[test]
fn pocket_to_parent_map_rebuilt_on_reload() {
    let (engine, alice, pocket) = seed_engine_with_pocket();
    assert_eq!(engine.pocket_parent(&pocket), Some(alice.address()));

    let reloaded = save_and_reload(&engine);
    assert_eq!(
        reloaded.pocket_parent(&pocket),
        Some(alice.address()),
        "pocket_to_parent must be rebuilt from SmartAccountConfig.pockets after load",
    );
}

#[test]
fn parent_policy_still_enforced_on_pocket_after_reload() {
    // Before the fix, pocket_to_parent was empty after load, so
    // check_spending_policy fell through to Ok(None) and the parent's
    // daily_limit was silently skipped for pocket-originated transfers.
    let (engine, alice, pocket) = seed_engine_with_pocket();
    let bob = Address([0xB0; 20]);

    let mut reloaded = save_and_reload(&engine);

    // First 1 UDAG hits the parent's cap exactly.
    let pubkey = alice.verifying_key().to_bytes();
    let key_id = AuthorizedKey::compute_key_id(KeyType::Ed25519, &pubkey);
    let make = |amount: u64, nonce: u64| {
        let mut tx = SmartTransferTx {
            from: pocket, to: bob, amount, fee: 0, nonce,
            signing_key_id: key_id, signature: vec![],
            memo: None, webauthn: None,
        };
        tx.signature = alice.sign(&tx.signable_bytes()).0.to_vec();
        tx
    };

    reloaded.apply_smart_transfer_tx(&make(100_000_000, 0)).unwrap();

    // Second tx must hit the PARENT's daily_spent counter and be rejected.
    let err = reloaded.apply_smart_transfer_tx(&make(1_000_000_000, 1)).unwrap_err();
    assert!(
        err.to_string().contains("daily spending limit exceeded"),
        "parent policy must carry across reload; got: {err}",
    );
}

#[test]
fn pocket_is_spendable_after_reload() {
    // Before the fix, verify_smart_transfer's pocket→parent fallback failed
    // because pocket_to_parent was empty, making every pocket unspendable.
    let (engine, alice, pocket) = seed_engine_with_pocket();
    let bob = Address([0xB1; 20]);
    let reloaded = save_and_reload(&engine);

    let pubkey = alice.verifying_key().to_bytes();
    let key_id = AuthorizedKey::compute_key_id(KeyType::Ed25519, &pubkey);
    let mut tx = SmartTransferTx {
        from: pocket, to: bob, amount: 50_000_000, fee: 0, nonce: 0,
        signing_key_id: key_id, signature: vec![],
        memo: None, webauthn: None,
    };
    tx.signature = alice.sign(&tx.signable_bytes()).0.to_vec();

    assert!(
        reloaded.verify_smart_transfer(&tx),
        "pocket-originated transfer must verify against the parent's keys after reload",
    );
}
