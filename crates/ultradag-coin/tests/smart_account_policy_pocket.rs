//! Regression tests for GHSA-9chc-gjfr-6hrq — pockets must inherit their
//! parent's spending policy. A transfer originating from a pocket is
//! authorized by the parent's keys; enforcement has to resolve to the parent
//! config too, or every limit (daily, vault, per-key) is bypassed for the
//! pocket balance.

use ultradag_coin::address::{Address, SecretKey, Signature};
use ultradag_coin::state::StateEngine;
use ultradag_coin::tx::name_registry::derive_pocket_address;
use ultradag_coin::tx::smart_account::*;

fn setup(sk: &SecretKey, balance: u64) -> StateEngine {
    let mut engine = StateEngine::new_with_genesis();
    engine.faucet_credit(&sk.address(), balance).unwrap();
    engine.ensure_smart_account(&sk.address());
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

fn create_pocket_op(from: Address, label: &str, nonce: u64) -> SmartOpTx {
    SmartOpTx {
        from,
        operation: SmartOpType::CreatePocket { label: label.to_string() },
        fee: 0, nonce,
        signing_key_id: [0u8; 8],
        signature: vec![],
        webauthn: None,
        p256_pubkey: None,
    }
}

fn make_smart_transfer(
    signer: &SecretKey, from: Address, to: Address, amount: u64, fee: u64, nonce: u64,
) -> SmartTransferTx {
    let pubkey = signer.verifying_key().to_bytes();
    let key_id = AuthorizedKey::compute_key_id(KeyType::Ed25519, &pubkey);
    let mut tx = SmartTransferTx {
        from, to, amount, fee, nonce,
        signing_key_id: key_id,
        signature: vec![],
        memo: None,
        webauthn: None,
    };
    tx.signature = signer.sign(&tx.signable_bytes()).0.to_vec();
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

/// Register the parent's Ed25519 key on its SmartAccount so SmartTransfer
/// signatures verify against it. Auto-registration normally handles this on
/// the first outgoing tx, but we want the policy in place before the first
/// pocket transfer — so we seed it manually.
fn seed_parent_key(engine: &mut StateEngine, parent: &SecretKey) {
    let pubkey = parent.verifying_key().to_bytes();
    let key_id = AuthorizedKey::compute_key_id(KeyType::Ed25519, &pubkey);
    let config = engine.smart_account_mut_for_test(&parent.address()).unwrap();
    if !config.authorized_keys.iter().any(|k| k.key_id == key_id) {
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

fn activate_policy(engine: &mut StateEngine, tx: SetPolicyTx) {
    engine.apply_set_policy_tx(&tx, 100).unwrap();
    engine.process_pending_policy_changes(100 + POLICY_CHANGE_DELAY_ROUNDS);
}

/// Create a pocket on the parent and fund it.
fn create_and_fund_pocket(
    engine: &mut StateEngine, parent: &SecretKey, label: &str, nonce: u64, amount: u64,
) -> Address {
    engine.apply_smart_op_tx(&create_pocket_op(parent.address(), label, nonce), 100).unwrap();
    let pocket = derive_pocket_address(&parent.address(), label);
    engine.credit(&pocket, amount).unwrap();
    pocket
}

#[test]
fn pocket_transfer_respects_parent_daily_limit() {
    let alice = SecretKey::from_bytes([0xA1; 32]);
    let bob = Address([0xB0; 20]);
    let mut engine = setup(&alice, 20_000_000_000);
    seed_parent_key(&mut engine, &alice);

    // 1 UDAG daily cap (100_000_000 sats).
    let policy = make_set_policy_tx(&alice, 100_000_000, 0, 0, vec![], Some(100_000_000), 0);
    activate_policy(&mut engine, policy);

    let pocket = create_and_fund_pocket(&mut engine, &alice, "savings", 1, 10_000_000_000);

    // First 1 UDAG transfer from pocket exactly fills the parent's daily cap.
    let ok = make_smart_transfer(&alice, pocket, bob, 100_000_000, 0, 0);
    engine.apply_smart_transfer_tx(&ok).unwrap();

    // A second pocket transfer must hit the *parent's* daily_spent counter
    // and be rejected — otherwise pockets bypass the cap entirely.
    let bypass = make_smart_transfer(&alice, pocket, bob, 1_000_000_000, 0, 1);
    let err = engine.apply_smart_transfer_tx(&bypass).unwrap_err();
    assert!(
        err.to_string().contains("daily spending limit exceeded"),
        "expected daily limit error, got: {err}"
    );
}

#[test]
fn pocket_transfer_routes_large_spend_to_parent_vault() {
    let alice = SecretKey::from_bytes([0xA2; 32]);
    let bob = Address([0xB1; 20]);
    let mut engine = setup(&alice, 20_000_000_000);
    seed_parent_key(&mut engine, &alice);

    // vault_threshold = 5 UDAG, 100-round delay.
    let policy = make_set_policy_tx(&alice, 100_000_000, 500_000_000, 100, vec![], None, 0);
    activate_policy(&mut engine, policy);

    let pocket = create_and_fund_pocket(&mut engine, &alice, "savings", 1, 10_000_000_000);

    // 10 UDAG from pocket — above threshold, must be held on the parent.
    let big = make_smart_transfer(&alice, pocket, bob, 1_000_000_000, 0, 0);
    engine.apply_smart_transfer_tx(&big).unwrap();

    // Recipient not credited yet.
    assert_eq!(engine.balance(&bob), 0);
    // Pocket already debited.
    assert_eq!(engine.balance(&pocket), 10_000_000_000 - 1_000_000_000);

    // Pending vault lives on the *parent's* config, and remembers the pocket
    // as origin so cancel refunds the right surface.
    let parent_cfg = engine.smart_account(&alice.address()).unwrap();
    assert_eq!(parent_cfg.pending_vault_transfers.len(), 1);
    let vault = &parent_cfg.pending_vault_transfers[0];
    assert_eq!(vault.amount, 1_000_000_000);
    assert_eq!(vault.to, bob);
    assert_eq!(vault.from, pocket);

    // Pocket must NOT have accumulated a pending vault of its own — every
    // vault for a pocket transfer must live on the parent.
    if let Some(pocket_cfg) = engine.smart_account(&pocket) {
        assert!(pocket_cfg.pending_vault_transfers.is_empty());
    }
}

#[test]
fn pocket_vault_cancel_refunds_pocket_not_parent() {
    let alice = SecretKey::from_bytes([0xA3; 32]);
    let bob = Address([0xB2; 20]);
    let mut engine = setup(&alice, 20_000_000_000);
    seed_parent_key(&mut engine, &alice);

    let policy = make_set_policy_tx(&alice, 100_000_000, 500_000_000, 100, vec![], None, 0);
    activate_policy(&mut engine, policy);

    let pocket_balance = 10_000_000_000;
    let pocket = create_and_fund_pocket(&mut engine, &alice, "savings", 1, pocket_balance);
    let parent_balance_before = engine.balance(&alice.address());

    // Send a vault-sized transfer from the pocket.
    let big = make_smart_transfer(&alice, pocket, bob, 1_000_000_000, 0, 0);
    engine.apply_smart_transfer_tx(&big).unwrap();

    let vault_id = engine.smart_account(&alice.address()).unwrap()
        .pending_vault_transfers[0].transfer_id;

    // Parent cancels. Funds must return to the pocket, not the parent — a
    // cancel that silently rehomes the balance across security surfaces is
    // itself a policy bypass.
    let cancel = make_cancel_vault_tx(&alice, vault_id, 0);
    engine.apply_cancel_vault_tx(&cancel).unwrap();

    assert_eq!(engine.balance(&pocket), pocket_balance);
    assert_eq!(engine.balance(&alice.address()), parent_balance_before);
    let parent_cfg = engine.smart_account(&alice.address()).unwrap();
    assert!(parent_cfg.pending_vault_transfers.is_empty());
}

#[test]
fn pocket_transfer_respects_per_key_daily_limit() {
    let alice = SecretKey::from_bytes([0xA4; 32]);
    let bob = Address([0xB3; 20]);
    let mut engine = setup(&alice, 20_000_000_000);
    seed_parent_key(&mut engine, &alice);

    // No account-level policy — test that the per-key cap on the parent's
    // signing key applies to pocket-originated transfers.
    let pubkey = alice.verifying_key().to_bytes();
    let key_id = AuthorizedKey::compute_key_id(KeyType::Ed25519, &pubkey);
    {
        let cfg = engine.smart_account_mut_for_test(&alice.address()).unwrap();
        let key = cfg.authorized_keys.iter_mut().find(|k| k.key_id == key_id).unwrap();
        key.daily_limit = Some(200_000_000); // 2 UDAG/day for this key
    }

    let pocket = create_and_fund_pocket(&mut engine, &alice, "savings", 0, 10_000_000_000);

    // First spend fills the per-key cap.
    let ok = make_smart_transfer(&alice, pocket, bob, 200_000_000, 0, 0);
    engine.apply_smart_transfer_tx(&ok).unwrap();

    // Second spend with the same key must be rejected by the per-key limit.
    let bypass = make_smart_transfer(&alice, pocket, bob, 1_000_000, 0, 1);
    let err = engine.apply_smart_transfer_tx(&bypass).unwrap_err();
    assert!(
        err.to_string().contains("per-key daily spending limit exceeded"),
        "expected per-key limit error, got: {err}"
    );
}

#[test]
fn pocket_transfer_inherits_parent_whitelist() {
    let alice = SecretKey::from_bytes([0xA5; 32]);
    let friend = Address([0xF0; 20]);
    let mut engine = setup(&alice, 20_000_000_000);
    seed_parent_key(&mut engine, &alice);

    // Strict 1 UDAG daily cap, but `friend` is whitelisted on the parent —
    // the whitelist must apply to pocket transfers too.
    let policy = make_set_policy_tx(
        &alice, 100_000_000, 0, 0, vec![friend], Some(100_000_000), 0,
    );
    activate_policy(&mut engine, policy);

    let pocket = create_and_fund_pocket(&mut engine, &alice, "savings", 1, 10_000_000_000);

    // 50 UDAG to a whitelisted address from the pocket — bypasses the 1 UDAG
    // cap because the whitelist is inherited.
    let tx = make_smart_transfer(&alice, pocket, friend, 5_000_000_000, 0, 0);
    engine.apply_smart_transfer_tx(&tx).unwrap();
    assert_eq!(engine.balance(&friend), 5_000_000_000);
}
