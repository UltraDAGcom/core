//! Pocket integration tests — verify the end-to-end lifecycle of creating,
//! resolving, and invalidating labeled sub-addresses under a name.

use ultradag_coin::address::{Address, SecretKey, Signature};
use ultradag_coin::constants::COIN;
use ultradag_coin::state::StateEngine;
use ultradag_coin::tx::name_registry::*;

fn setup(sk: &SecretKey, balance: u64) -> StateEngine {
    let mut engine = StateEngine::new_with_genesis();
    engine.faucet_credit(&sk.address(), balance).unwrap();
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

fn make_pocket(parent: &str, label: &str, target_sk: &SecretKey) -> Pocket {
    let challenge = pocket_claim_bytes(parent, label, &target_sk.address());
    Pocket {
        label: label.to_string(),
        address: target_sk.address(),
        pub_key: target_sk.verifying_key().to_bytes(),
        proof: target_sk.sign(&challenge),
    }
}

fn update_profile_with_pockets(
    engine: &mut StateEngine, owner_sk: &SecretKey, name: &str,
    pockets: Vec<Pocket>, fee: u64, nonce: u64,
) -> Result<(), ultradag_coin::CoinError> {
    let mut tx = UpdateProfileTx {
        from: owner_sk.address(), name: name.to_string(),
        external_addresses: Vec::new(), metadata: Vec::new(),
        pockets, fee, nonce,
        pub_key: owner_sk.verifying_key().to_bytes(),
        signature: Signature([0u8; 64]),
    };
    tx.signature = owner_sk.sign(&tx.signable_bytes());
    engine.apply_update_profile_tx(&tx)
}

// ═══════════════════════════════════════════════════════════════════════

#[test]
fn test_add_pocket_happy_path() {
    let alice = SecretKey::from_bytes([0x01; 32]);
    let wallet2 = SecretKey::from_bytes([0x02; 32]);
    let mut engine = setup(&alice, 500 * COIN);
    engine.faucet_credit(&wallet2.address(), 1 * COIN).unwrap();

    register_name(&mut engine, &alice, "alicewallet", 0, 0);

    let pocket = make_pocket("alicewallet", "savings", &wallet2);
    update_profile_with_pockets(&mut engine, &alice, "alicewallet", vec![pocket], 10_000, 1).unwrap();

    // Resolve via engine.
    let addr = engine.resolve_pocket("alicewallet", "savings");
    assert_eq!(addr, Some(wallet2.address()));

    // Parent name still resolves to alice.
    assert_eq!(engine.resolve_name("alicewallet"), Some(alice.address()));
}

#[test]
fn test_multiple_pockets() {
    let alice = SecretKey::from_bytes([0x10; 32]);
    let w2 = SecretKey::from_bytes([0x11; 32]);
    let w3 = SecretKey::from_bytes([0x12; 32]);
    let mut engine = setup(&alice, 500 * COIN);

    register_name(&mut engine, &alice, "alicemulti", 0, 0);

    let pockets = vec![
        make_pocket("alicemulti", "savings", &w2),
        make_pocket("alicemulti", "business", &w3),
    ];
    update_profile_with_pockets(&mut engine, &alice, "alicemulti", pockets, 10_000, 1).unwrap();

    assert_eq!(engine.resolve_pocket("alicemulti", "savings"), Some(w2.address()));
    assert_eq!(engine.resolve_pocket("alicemulti", "business"), Some(w3.address()));
    assert_eq!(engine.resolve_pocket("alicemulti", "unknown"), None);
}

#[test]
fn test_pocket_wrong_pubkey_mismatch() {
    let alice = SecretKey::from_bytes([0x20; 32]);
    let target = SecretKey::from_bytes([0x21; 32]);
    let imposter = SecretKey::from_bytes([0x22; 32]);
    let mut engine = setup(&alice, 500 * COIN);

    register_name(&mut engine, &alice, "alicebad", 0, 0);

    // Sign with imposter but claim target's address → pubkey won't hash to address.
    let challenge = pocket_claim_bytes("alicebad", "fake", &target.address());
    let bad_pocket = Pocket {
        label: "fake".to_string(),
        address: target.address(),
        pub_key: imposter.verifying_key().to_bytes(), // mismatch
        proof: imposter.sign(&challenge),
    };

    let result = update_profile_with_pockets(&mut engine, &alice, "alicebad", vec![bad_pocket], 10_000, 1);
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("pub_key does not match address"), "got: {err}");
}

#[test]
fn test_pocket_wrong_signature() {
    let alice = SecretKey::from_bytes([0x30; 32]);
    let target = SecretKey::from_bytes([0x31; 32]);
    let mut engine = setup(&alice, 500 * COIN);

    register_name(&mut engine, &alice, "alicesig", 0, 0);

    // Sign a different challenge (wrong label in the claim bytes).
    let wrong_challenge = pocket_claim_bytes("alicesig", "WRONG-LABEL", &target.address());
    let bad_pocket = Pocket {
        label: "real-label".to_string(),
        address: target.address(),
        pub_key: target.verifying_key().to_bytes(),
        proof: target.sign(&wrong_challenge), // signed wrong label
    };

    let result = update_profile_with_pockets(&mut engine, &alice, "alicesig", vec![bad_pocket], 10_000, 1);
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("signature invalid"), "got: {err}");
}

#[test]
fn test_pocket_wrong_parent_binding() {
    // Signature for parent "bob" replayed against parent "alice" — must be rejected.
    let alice = SecretKey::from_bytes([0x40; 32]);
    let bob = SecretKey::from_bytes([0x41; 32]);
    let target = SecretKey::from_bytes([0x42; 32]);
    let mut engine = setup(&alice, 500 * COIN);
    engine.faucet_credit(&bob.address(), 500 * COIN).unwrap();

    register_name(&mut engine, &alice, "aliceparent", 0, 0);
    register_name(&mut engine, &bob, "bobparent", 0, 0);

    // Sign the pocket for "bobparent" — then try to attach it to "aliceparent".
    let challenge_for_bob = pocket_claim_bytes("bobparent", "shared", &target.address());
    let misbound = Pocket {
        label: "shared".to_string(),
        address: target.address(),
        pub_key: target.verifying_key().to_bytes(),
        proof: target.sign(&challenge_for_bob),
    };

    let result = update_profile_with_pockets(&mut engine, &alice, "aliceparent", vec![misbound], 10_000, 1);
    assert!(result.is_err(), "pocket proof bound to wrong parent must be rejected");
}

#[test]
fn test_pocket_duplicate_label_rejected() {
    let alice = SecretKey::from_bytes([0x50; 32]);
    let w1 = SecretKey::from_bytes([0x51; 32]);
    let w2 = SecretKey::from_bytes([0x52; 32]);
    let mut engine = setup(&alice, 500 * COIN);

    register_name(&mut engine, &alice, "alicedup", 0, 0);

    let pockets = vec![
        make_pocket("alicedup", "same", &w1),
        make_pocket("alicedup", "same", &w2),
    ];
    let result = update_profile_with_pockets(&mut engine, &alice, "alicedup", pockets, 10_000, 1);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("duplicate"));
}

#[test]
fn test_pocket_max_exceeded() {
    let alice = SecretKey::from_bytes([0x60; 32]);
    let mut engine = setup(&alice, 500 * COIN);

    register_name(&mut engine, &alice, "alicemax", 0, 0);

    let pockets: Vec<Pocket> = (0..=MAX_POCKETS).map(|i| {
        let sk = SecretKey::from_bytes([(0x70 + i as u8); 32]);
        make_pocket("alicemax", &format!("p{}", i), &sk)
    }).collect();

    let result = update_profile_with_pockets(&mut engine, &alice, "alicemax", pockets, 10_000, 1);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("too many pockets"));
}

#[test]
fn test_pocket_invalid_label_chars() {
    let alice = SecretKey::from_bytes([0x80; 32]);
    let target = SecretKey::from_bytes([0x81; 32]);
    let mut engine = setup(&alice, 500 * COIN);

    register_name(&mut engine, &alice, "alicechar", 0, 0);

    // Uppercase in label is rejected.
    let bad = Pocket {
        label: "UPPER".to_string(),
        address: target.address(),
        pub_key: target.verifying_key().to_bytes(),
        proof: target.sign(&pocket_claim_bytes("alicechar", "UPPER", &target.address())),
    };
    let result = update_profile_with_pockets(&mut engine, &alice, "alicechar", vec![bad], 10_000, 1);
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("lowercase"), "got: {err}");
}

#[test]
fn test_pocket_cleared_on_name_expiry() {
    // Premium (5-char) name expires → pockets die with it.
    let alice = SecretKey::from_bytes([0x90; 32]);
    let target = SecretKey::from_bytes([0x91; 32]);
    let mut engine = setup(&alice, 500 * COIN);

    register_name(&mut engine, &alice, "abcde", 100 * COIN, 0);

    let pocket = make_pocket("abcde", "target", &target);
    update_profile_with_pockets(&mut engine, &alice, "abcde", vec![pocket], 10_000, 1).unwrap();

    // Verify pocket exists.
    assert_eq!(engine.resolve_pocket("abcde", "target"), Some(target.address()));

    // Fast-forward past expiry + grace.
    let expiry = engine.name_expiry("abcde").unwrap();
    let grace_end = expiry + NAME_GRACE_PERIOD_ROUNDS;
    engine.process_name_expiry(grace_end + 1);

    // Parent name gone → pocket gone.
    assert_eq!(engine.resolve_name("abcde"), None);
    assert_eq!(engine.resolve_pocket("abcde", "target"), None);
}

#[test]
fn test_perpetual_name_pocket_survives() {
    // Free-tier (7-char) name is perpetual → pocket survives forever.
    let alice = SecretKey::from_bytes([0xA0; 32]);
    let target = SecretKey::from_bytes([0xA1; 32]);
    let mut engine = setup(&alice, 500 * COIN);

    register_name(&mut engine, &alice, "alicelong", 0, 0);

    let pocket = make_pocket("alicelong", "cold", &target);
    update_profile_with_pockets(&mut engine, &alice, "alicelong", vec![pocket], 10_000, 1).unwrap();

    // Push far into the future — perpetual name survives.
    engine.process_name_expiry(u64::MAX / 2);

    assert_eq!(engine.resolve_name("alicelong"), Some(alice.address()));
    assert_eq!(engine.resolve_pocket("alicelong", "cold"), Some(target.address()));
}
