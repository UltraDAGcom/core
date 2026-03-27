/// Name Registry integration tests.

use ultradag_coin::address::{Address, SecretKey, Signature};
use ultradag_coin::constants::COIN;
use ultradag_coin::state::StateEngine;
use ultradag_coin::tx::name_registry::*;

fn setup(sk: &SecretKey, balance: u64) -> StateEngine {
    let mut engine = StateEngine::new_with_genesis();
    engine.faucet_credit(&sk.address(), balance).unwrap();
    engine
}

fn make_register(sk: &SecretKey, name: &str, years: u8, fee: u64, nonce: u64) -> RegisterNameTx {
    let mut tx = RegisterNameTx {
        from: sk.address(), name: name.to_string(), duration_years: years,
        fee, nonce, pub_key: sk.verifying_key().to_bytes(), signature: Signature([0u8; 64]),
        fee_payer: None,
    };
    tx.signature = sk.sign(&tx.signable_bytes());
    tx
}

fn make_renew(sk: &SecretKey, name: &str, years: u8, fee: u64, nonce: u64) -> RenewNameTx {
    let mut tx = RenewNameTx {
        from: sk.address(), name: name.to_string(), additional_years: years,
        fee, nonce, pub_key: sk.verifying_key().to_bytes(), signature: Signature([0u8; 64]),
    };
    tx.signature = sk.sign(&tx.signable_bytes());
    tx
}

fn make_transfer_name(sk: &SecretKey, name: &str, new_owner: Address, fee: u64, nonce: u64) -> TransferNameTx {
    let mut tx = TransferNameTx {
        from: sk.address(), name: name.to_string(), new_owner,
        fee, nonce, pub_key: sk.verifying_key().to_bytes(), signature: Signature([0u8; 64]),
    };
    tx.signature = sk.sign(&tx.signable_bytes());
    tx
}

fn make_update_profile(sk: &SecretKey, name: &str, ext: Vec<(String, String)>, meta: Vec<(String, String)>, fee: u64, nonce: u64) -> UpdateProfileTx {
    let mut tx = UpdateProfileTx {
        from: sk.address(), name: name.to_string(),
        external_addresses: ext, metadata: meta,
        fee, nonce, pub_key: sk.verifying_key().to_bytes(), signature: Signature([0u8; 64]),
    };
    tx.signature = sk.sign(&tx.signable_bytes());
    tx
}

#[test]
fn test_register_name() {
    let alice = SecretKey::from_bytes([0x01; 32]);
    let mut engine = setup(&alice, 1000 * COIN);

    let tx = make_register(&alice, "alice", 1, 100 * COIN, 0);
    engine.apply_register_name_tx(&tx).unwrap();

    assert_eq!(engine.resolve_name("alice"), Some(alice.address()));
    assert_eq!(engine.reverse_name(&alice.address()), Some("alice"));
    assert!(engine.name_expiry("alice").is_some());
}

#[test]
fn test_register_duplicate_rejected() {
    let alice = SecretKey::from_bytes([0x02; 32]);
    let bob = SecretKey::from_bytes([0x03; 32]);
    let mut engine = setup(&alice, 1000 * COIN);
    engine.faucet_credit(&bob.address(), 1000 * COIN).unwrap();

    let tx1 = make_register(&alice, "coolname", 1, 10 * COIN, 0);
    engine.apply_register_name_tx(&tx1).unwrap();

    let tx2 = make_register(&bob, "coolname", 1, 10 * COIN, 0);
    let result = engine.apply_register_name_tx(&tx2);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("taken"));
}

#[test]
fn test_invalid_name_rejected() {
    let alice = SecretKey::from_bytes([0x04; 32]);
    let mut engine = setup(&alice, 1000 * COIN);

    // Too short
    let tx = make_register(&alice, "ab", 1, 1000 * COIN, 0);
    assert!(engine.apply_register_name_tx(&tx).is_err());

    // Reserved word
    let tx = make_register(&alice, "admin", 1, 1000 * COIN, 0);
    assert!(engine.apply_register_name_tx(&tx).is_err());

    // Uppercase
    let tx = make_register(&alice, "Alice", 1, 100 * COIN, 0);
    assert!(engine.apply_register_name_tx(&tx).is_err());
}

#[test]
fn test_tiered_pricing() {
    let alice = SecretKey::from_bytes([0x05; 32]);
    let mut engine = setup(&alice, 2000 * COIN);

    // 3-char name costs 1000 UDAG — fee too low
    let tx = make_register(&alice, "abc", 1, 500 * COIN, 0);
    let result = engine.apply_register_name_tx(&tx);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("too low"));

    // Correct fee
    let tx = make_register(&alice, "abc", 1, 1000 * COIN, 0);
    engine.apply_register_name_tx(&tx).unwrap();
}

#[test]
fn test_renew_extends_expiry() {
    let alice = SecretKey::from_bytes([0x06; 32]);
    let mut engine = setup(&alice, 500 * COIN);

    let tx = make_register(&alice, "myname", 1, 10 * COIN, 0);
    engine.apply_register_name_tx(&tx).unwrap();

    let expiry1 = engine.name_expiry("myname").unwrap();

    let renew_tx = make_renew(&alice, "myname", 1, 10 * COIN, 1);
    engine.apply_renew_name_tx(&renew_tx).unwrap();

    let expiry2 = engine.name_expiry("myname").unwrap();
    assert!(expiry2 > expiry1);
    assert_eq!(expiry2 - expiry1, ROUNDS_PER_YEAR);
}

#[test]
fn test_transfer_name() {
    let alice = SecretKey::from_bytes([0x07; 32]);
    let bob = SecretKey::from_bytes([0x08; 32]);
    let mut engine = setup(&alice, 500 * COIN);

    let tx = make_register(&alice, "myname", 1, 10 * COIN, 0);
    engine.apply_register_name_tx(&tx).unwrap();
    assert_eq!(engine.resolve_name("myname"), Some(alice.address()));

    let transfer_tx = make_transfer_name(&alice, "myname", bob.address(), 10_000, 1);
    engine.apply_transfer_name_tx(&transfer_tx).unwrap();

    assert_eq!(engine.resolve_name("myname"), Some(bob.address()));
    assert_eq!(engine.reverse_name(&bob.address()), Some("myname"));
    assert_eq!(engine.reverse_name(&alice.address()), None);
}

#[test]
fn test_non_owner_cannot_transfer() {
    let alice = SecretKey::from_bytes([0x09; 32]);
    let bob = SecretKey::from_bytes([0x0A; 32]);
    let charlie = SecretKey::from_bytes([0x0B; 32]);
    let mut engine = setup(&alice, 500 * COIN);
    engine.faucet_credit(&bob.address(), 500 * COIN).unwrap();

    let tx = make_register(&alice, "myname", 1, 10 * COIN, 0);
    engine.apply_register_name_tx(&tx).unwrap();

    // Bob tries to transfer Alice's name
    let transfer_tx = make_transfer_name(&bob, "myname", charlie.address(), 10_000, 0);
    let result = engine.apply_transfer_name_tx(&transfer_tx);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("only the name owner"));
}

#[test]
fn test_name_expiry_releases_name() {
    let alice = SecretKey::from_bytes([0x0C; 32]);
    let bob = SecretKey::from_bytes([0x0D; 32]);
    let mut engine = setup(&alice, 500 * COIN);
    engine.faucet_credit(&bob.address(), 500 * COIN).unwrap();

    let tx = make_register(&alice, "expiring", 1, 10 * COIN, 0);
    engine.apply_register_name_tx(&tx).unwrap();

    let expiry = engine.name_expiry("expiring").unwrap();

    // Still valid during grace period
    let grace_end = expiry + NAME_GRACE_PERIOD_ROUNDS;
    assert!(engine.resolve_name("expiring").is_some());

    // Process expiry past grace
    engine.process_name_expiry(grace_end + 1);

    // Name should be released
    assert!(engine.resolve_name("expiring").is_none());
    assert!(engine.reverse_name(&alice.address()).is_none());

    // Bob can now register the expired name
    let tx2 = make_register(&bob, "expiring", 1, 10 * COIN, 0);
    engine.apply_register_name_tx(&tx2).unwrap();
    assert_eq!(engine.resolve_name("expiring"), Some(bob.address()));
}

#[test]
fn test_update_profile() {
    let alice = SecretKey::from_bytes([0x0E; 32]);
    let mut engine = setup(&alice, 500 * COIN);

    let tx = make_register(&alice, "myname", 1, 10 * COIN, 0);
    engine.apply_register_name_tx(&tx).unwrap();

    let profile_tx = make_update_profile(
        &alice, "myname",
        vec![("eth".to_string(), "0xabc123".to_string())],
        vec![("website".to_string(), "https://example.com".to_string())],
        10_000, 1,
    );
    engine.apply_update_profile_tx(&profile_tx).unwrap();

    let profile = engine.name_profile("myname").unwrap();
    assert_eq!(profile.external_addresses.len(), 1);
    assert_eq!(profile.external_addresses[0].0, "eth");
    assert_eq!(profile.metadata.len(), 1);
    assert_eq!(profile.metadata[0].0, "website");
}

#[test]
fn test_one_name_per_address() {
    let alice = SecretKey::from_bytes([0x0F; 32]);
    let mut engine = setup(&alice, 500 * COIN);

    let tx1 = make_register(&alice, "myname", 1, 10 * COIN, 0);
    engine.apply_register_name_tx(&tx1).unwrap();

    // Try to register a second name — should fail
    let tx2 = make_register(&alice, "myname2", 1, 10 * COIN, 1);
    let result = engine.apply_register_name_tx(&tx2);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("already has name"));
}

#[test]
fn test_fee_goes_to_treasury() {
    let alice = SecretKey::from_bytes([0x10; 32]);
    let mut engine = setup(&alice, 500 * COIN);

    let treasury_before = engine.treasury_balance();
    let tx = make_register(&alice, "myname", 1, 10 * COIN, 0);
    engine.apply_register_name_tx(&tx).unwrap();

    assert_eq!(engine.treasury_balance(), treasury_before + 10 * COIN);
}
