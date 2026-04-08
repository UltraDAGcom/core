//! Pocket integration tests — verify the end-to-end lifecycle of derived
//! sub-addresses under a SmartAccount.
//!
//! Pockets are now derived deterministically from the parent address + label.
//! No separate keys, no counter-signatures. The parent's passkey signs for all.

use ultradag_coin::address::{Address, SecretKey, Signature};
use ultradag_coin::constants::COIN;
use ultradag_coin::state::StateEngine;
use ultradag_coin::tx::name_registry::*;
use ultradag_coin::tx::smart_account::*;

fn setup(sk: &SecretKey, balance: u64) -> StateEngine {
    let mut engine = StateEngine::new_with_genesis();
    engine.faucet_credit(&sk.address(), balance).unwrap();
    engine.ensure_smart_account(&sk.address());
    engine
}

fn register_name(engine: &mut StateEngine, sk: &SecretKey, name: &str, fee: u64, nonce: u64) {
    let mut tx = RegisterNameTx {
        from: sk.address(), name: name.to_string(), duration_years: 1,
        fee, nonce, pub_key: sk.verifying_key().to_bytes(),
        signature: Signature([0u8; 64]), fee_payer: None,
    };
    tx.signature = sk.sign(&tx.signable_bytes());
    engine.apply_register_name_tx(&tx).unwrap();
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

fn remove_pocket_op(from: Address, label: &str, nonce: u64) -> SmartOpTx {
    SmartOpTx {
        from,
        operation: SmartOpType::RemovePocket { label: label.to_string() },
        fee: 0, nonce,
        signing_key_id: [0u8; 8],
        signature: vec![],
        webauthn: None,
        p256_pubkey: None,
    }
}

#[test]
fn test_create_pocket_happy_path() {
    let alice = SecretKey::from_bytes([0x01; 32]);
    let mut engine = setup(&alice, 500 * COIN);
    register_name(&mut engine, &alice, "alicelong", 0, 0);

    engine.apply_smart_op_tx(&create_pocket_op(alice.address(), "savings", 1), 100).unwrap();

    // Resolve via engine.
    let addr = engine.resolve_pocket("alicelong", "savings");
    assert!(addr.is_some());
    // The pocket address must equal the deterministic derivation.
    let expected = derive_pocket_address(&alice.address(), "savings");
    assert_eq!(addr.unwrap(), expected);
    // Parent name still resolves to alice.
    assert_eq!(engine.resolve_name("alicelong"), Some(alice.address()));
}

#[test]
fn test_multiple_pockets() {
    let alice = SecretKey::from_bytes([0x10; 32]);
    let mut engine = setup(&alice, 500 * COIN);
    register_name(&mut engine, &alice, "alicemulti", 0, 0);

    engine.apply_smart_op_tx(&create_pocket_op(alice.address(), "savings", 1), 100).unwrap();
    engine.apply_smart_op_tx(&create_pocket_op(alice.address(), "business", 2), 100).unwrap();

    assert!(engine.resolve_pocket("alicemulti", "savings").is_some());
    assert!(engine.resolve_pocket("alicemulti", "business").is_some());
    assert!(engine.resolve_pocket("alicemulti", "unknown").is_none());
    // Each pocket has a unique derived address.
    assert_ne!(
        engine.resolve_pocket("alicemulti", "savings"),
        engine.resolve_pocket("alicemulti", "business"),
    );
}

#[test]
fn test_pocket_duplicate_label_rejected() {
    let alice = SecretKey::from_bytes([0x20; 32]);
    let mut engine = setup(&alice, 500 * COIN);

    engine.apply_smart_op_tx(&create_pocket_op(alice.address(), "same", 0), 100).unwrap();
    let result = engine.apply_smart_op_tx(&create_pocket_op(alice.address(), "same", 1), 100);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("already exists"));
}

#[test]
fn test_pocket_max_exceeded() {
    let alice = SecretKey::from_bytes([0x30; 32]);
    let mut engine = setup(&alice, 500 * COIN);

    for i in 0..MAX_POCKETS {
        engine.apply_smart_op_tx(
            &create_pocket_op(alice.address(), &format!("p{}", i), i as u64),
            100,
        ).unwrap();
    }
    let result = engine.apply_smart_op_tx(
        &create_pocket_op(alice.address(), "overflow", MAX_POCKETS as u64),
        100,
    );
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("maximum"));
}

#[test]
fn test_pocket_invalid_label_chars() {
    let alice = SecretKey::from_bytes([0x40; 32]);
    let mut engine = setup(&alice, 500 * COIN);

    let result = engine.apply_smart_op_tx(
        &create_pocket_op(alice.address(), "UPPER", 0),
        100,
    );
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("lowercase"));
}

#[test]
fn test_remove_pocket() {
    let alice = SecretKey::from_bytes([0x50; 32]);
    let mut engine = setup(&alice, 500 * COIN);
    register_name(&mut engine, &alice, "aliceremove", 0, 0);

    engine.apply_smart_op_tx(&create_pocket_op(alice.address(), "temp", 1), 100).unwrap();
    assert!(engine.resolve_pocket("aliceremove", "temp").is_some());

    engine.apply_smart_op_tx(&remove_pocket_op(alice.address(), "temp", 2), 100).unwrap();
    assert!(engine.resolve_pocket("aliceremove", "temp").is_none());
}

#[test]
fn test_remove_nonexistent_pocket_rejected() {
    let alice = SecretKey::from_bytes([0x55; 32]);
    let mut engine = setup(&alice, 500 * COIN);

    let result = engine.apply_smart_op_tx(
        &remove_pocket_op(alice.address(), "ghost", 0),
        100,
    );
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("not found"));
}

#[test]
fn test_pocket_parent_delegation() {
    // The pocket_to_parent reverse map enables the parent's passkey to sign
    // for the derived pocket address.
    let alice = SecretKey::from_bytes([0x60; 32]);
    let mut engine = setup(&alice, 500 * COIN);

    engine.apply_smart_op_tx(&create_pocket_op(alice.address(), "vault", 0), 100).unwrap();

    let pocket_addr = derive_pocket_address(&alice.address(), "vault");
    // pocket_parent() should return alice's address.
    assert_eq!(engine.pocket_parent(&pocket_addr), Some(alice.address()));

    // After removal, reverse lookup must return None.
    engine.apply_smart_op_tx(&remove_pocket_op(alice.address(), "vault", 1), 100).unwrap();
    assert_eq!(engine.pocket_parent(&pocket_addr), None);
}

#[test]
fn test_derive_pocket_address_deterministic() {
    let addr = Address([0xAA; 20]);
    let a1 = derive_pocket_address(&addr, "savings");
    let a2 = derive_pocket_address(&addr, "savings");
    assert_eq!(a1, a2, "same parent + label must produce the same address");

    let a3 = derive_pocket_address(&addr, "business");
    assert_ne!(a1, a3, "different labels must produce different addresses");

    let other = Address([0xBB; 20]);
    let a4 = derive_pocket_address(&other, "savings");
    assert_ne!(a1, a4, "different parents must produce different addresses");
}

#[test]
fn test_pocket_no_account_rejected() {
    // CreatePocket requires a SmartAccount to exist.
    let ghost = SecretKey::from_bytes([0x70; 32]);
    let mut engine = StateEngine::new_with_genesis();
    // NO ensure_smart_account.
    let result = engine.apply_smart_op_tx(
        &create_pocket_op(ghost.address(), "fail", 0),
        100,
    );
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("smart account not found"));
}
