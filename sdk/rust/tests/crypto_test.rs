use ultradag_sdk::crypto::{derive_address, Keypair};

#[test]
fn generate_unique_keypairs() {
    let a = Keypair::generate();
    let b = Keypair::generate();
    assert_ne!(a.secret_key, b.secret_key);
    assert_ne!(a.public_key, b.public_key);
    assert_ne!(a.address, b.address);
}

#[test]
fn from_secret_bytes_deterministic() {
    let seed = [0x42; 32];
    let a = Keypair::from_secret_bytes(seed);
    let b = Keypair::from_secret_bytes(seed);
    assert_eq!(a.secret_key, b.secret_key);
    assert_eq!(a.public_key, b.public_key);
    assert_eq!(a.address, b.address);
}

#[test]
fn address_matches_blake3_spec() {
    let kp = Keypair::from_secret_bytes([0x01; 32]);
    let expected = blake3::hash(&kp.public_key);
    assert_eq!(kp.address, *expected.as_bytes());
}

#[test]
fn derive_address_standalone() {
    let kp = Keypair::generate();
    let addr = derive_address(&kp.public_key);
    assert_eq!(addr, kp.address);
}

#[test]
fn hex_roundtrip() {
    let kp = Keypair::generate();
    let hex = kp.secret_key_hex();
    let restored = Keypair::from_hex(&hex).unwrap();
    assert_eq!(kp.public_key, restored.public_key);
    assert_eq!(kp.address, restored.address);
}

#[test]
fn from_hex_bad_input() {
    assert!(Keypair::from_hex("xyz").is_err());
    assert!(Keypair::from_hex("aabb").is_err());
    assert!(Keypair::from_hex("").is_err());
}

#[test]
fn sign_verify_roundtrip() {
    let kp = Keypair::generate();
    let msg = b"ultradag transaction data";
    let sig = kp.sign(msg);
    assert!(kp.verify(msg, &sig));
}

#[test]
fn verify_wrong_message_fails() {
    let kp = Keypair::generate();
    let sig = kp.sign(b"correct");
    assert!(!kp.verify(b"wrong", &sig));
}

#[test]
fn verify_wrong_key_fails() {
    let kp1 = Keypair::generate();
    let kp2 = Keypair::generate();
    let sig = kp1.sign(b"hello");
    assert!(!kp2.verify(b"hello", &sig));
}

#[test]
fn verify_tampered_signature_fails() {
    let kp = Keypair::generate();
    let mut sig = kp.sign(b"data");
    sig[0] ^= 0xFF;
    assert!(!kp.verify(b"data", &sig));
}

#[test]
fn faucet_keypair_matches_spec() {
    // CLAUDE.md: faucet key = SecretKey::from_bytes([0xFA; 32])
    let kp = Keypair::from_secret_bytes([0xFA; 32]);
    let expected_addr = blake3::hash(&kp.public_key);
    assert_eq!(kp.address, *expected_addr.as_bytes());
    // Should be deterministic across runs
    let kp2 = Keypair::from_secret_bytes([0xFA; 32]);
    assert_eq!(kp.address, kp2.address);
}

#[test]
fn dev_keypair_matches_spec() {
    // CLAUDE.md: dev key = SecretKey::from_bytes([0xDE; 32])
    let kp = Keypair::from_secret_bytes([0xDE; 32]);
    let expected_addr = blake3::hash(&kp.public_key);
    assert_eq!(kp.address, *expected_addr.as_bytes());
}

#[test]
fn hex_lengths_correct() {
    let kp = Keypair::generate();
    assert_eq!(kp.secret_key_hex().len(), 64);
    assert_eq!(kp.public_key_hex().len(), 64);
    assert_eq!(kp.address_hex().len(), 64);
}

#[test]
fn sign_empty_message() {
    let kp = Keypair::generate();
    let sig = kp.sign(b"");
    assert!(kp.verify(b"", &sig));
}

#[test]
fn sign_large_message() {
    let kp = Keypair::generate();
    let msg = vec![0xAB; 10_000];
    let sig = kp.sign(&msg);
    assert!(kp.verify(&msg, &sig));
}

#[test]
fn sats_to_udag_conversion() {
    assert!((ultradag_sdk::sats_to_udag(100_000_000) - 1.0).abs() < f64::EPSILON);
    assert!((ultradag_sdk::sats_to_udag(50_000_000) - 0.5).abs() < f64::EPSILON);
    assert!((ultradag_sdk::sats_to_udag(0) - 0.0).abs() < f64::EPSILON);
}

#[test]
fn udag_to_sats_conversion() {
    assert_eq!(ultradag_sdk::udag_to_sats(1.0), 100_000_000);
    assert_eq!(ultradag_sdk::udag_to_sats(0.5), 50_000_000);
    assert_eq!(ultradag_sdk::udag_to_sats(0.0), 0);
}
