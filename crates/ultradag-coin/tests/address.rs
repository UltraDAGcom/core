/// Module 1: Ed25519 Keys and Addresses — Production-grade tests

use ultradag_coin::address::{Address, SecretKey, Signature};

/// Fixed seed produces deterministic pubkey and address.
/// Mutation: changing from_bytes seed → different address.
#[test]
fn fixed_seed_deterministic_keypair() {
    let seed = [42u8; 32];
    let sk1 = SecretKey::from_bytes(seed);
    let sk2 = SecretKey::from_bytes(seed);

    // POSITIVE: same seed → same public key bytes
    assert_eq!(
        sk1.verifying_key().to_bytes(),
        sk2.verifying_key().to_bytes(),
    );
    // POSITIVE: same seed → same address
    assert_eq!(sk1.address(), sk2.address());

    // NEGATIVE: different seed → different address
    let sk3 = SecretKey::from_bytes([43u8; 32]);
    assert_ne!(sk1.address(), sk3.address());
    assert_ne!(
        sk1.verifying_key().to_bytes(),
        sk3.verifying_key().to_bytes(),
    );
}

/// Sign+verify with correct key succeeds; wrong key/message/signature fails.
/// Mutation: removing blake3 address check → forged key passes.
#[test]
fn sign_verify_correct_key_and_negative_cases() {
    let sk = SecretKey::from_bytes([7u8; 32]);
    let data = b"hello ultradag";
    let sig = sk.sign(data);

    // POSITIVE: correct key + data verifies
    assert!(sig.verify(&sk.verifying_key(), data));

    // NEGATIVE: different key fails
    let other_sk = SecretKey::from_bytes([8u8; 32]);
    assert!(!sig.verify(&other_sk.verifying_key(), data));

    // NEGATIVE: tampered message fails
    assert!(!sig.verify(&sk.verifying_key(), b"hello tinydaG"));

    // NEGATIVE: tampered signature fails
    let mut bad_sig = sig;
    bad_sig.0[0] ^= 0xff;
    assert!(!bad_sig.verify(&sk.verifying_key(), data));
}

/// Two different seeds produce two different addresses.
/// Mutation: address() returning constant → addresses collide.
#[test]
fn different_seeds_different_addresses() {
    let seeds: Vec<[u8; 32]> = (0..10u8).map(|i| {
        let mut s = [0u8; 32];
        s[0] = i;
        s
    }).collect();

    let addresses: Vec<Address> = seeds.iter()
        .map(|s| SecretKey::from_bytes(*s).address())
        .collect();

    // Every pair must be distinct
    for i in 0..addresses.len() {
        for j in (i + 1)..addresses.len() {
            assert_ne!(addresses[i], addresses[j],
                "seeds {i} and {j} should produce different addresses");
        }
    }
}

/// Address is exactly blake3(pubkey) — byte-for-byte verification.
/// Mutation: using sha256 instead of blake3 → bytes mismatch.
#[test]
fn address_is_blake3_of_pubkey_bytes() {
    let sk = SecretKey::from_bytes([99u8; 32]);
    let pubkey_bytes = sk.verifying_key().to_bytes();
    let expected = *blake3::hash(&pubkey_bytes).as_bytes();
    let addr = sk.address();

    assert_eq!(addr.0, expected,
        "address must be exactly blake3(pubkey_bytes)");

    // NEGATIVE: wrong pubkey → different hash
    let other_sk = SecretKey::from_bytes([100u8; 32]);
    let other_expected = *blake3::hash(&other_sk.verifying_key().to_bytes()).as_bytes();
    assert_ne!(addr.0, other_expected);
}

/// SecretKey serializes and deserializes with identical signing.
/// Mutation: from_bytes ignoring input → different signatures.
#[test]
fn secret_key_roundtrip_identical_signing() {
    let original = SecretKey::from_bytes([55u8; 32]);
    let bytes = original.to_bytes();
    assert_eq!(bytes, [55u8; 32]);

    let restored = SecretKey::from_bytes(bytes);

    // Same address
    assert_eq!(original.address(), restored.address());

    // Same signature on same data
    let data = b"roundtrip test data";
    let sig1 = original.sign(data);
    let sig2 = restored.sign(data);
    assert_eq!(sig1.0, sig2.0, "signatures must be byte-identical");

    // And both verify
    assert!(sig1.verify(&original.verifying_key(), data));
    assert!(sig2.verify(&restored.verifying_key(), data));
}

/// Address hex roundtrip preserves exact bytes.
/// Mutation: to_hex dropping last byte → from_hex returns None.
#[test]
fn address_hex_roundtrip_exact() {
    let sk = SecretKey::from_bytes([77u8; 32]);
    let addr = sk.address();
    let hex = addr.to_hex();

    assert_eq!(hex.len(), 64, "hex string must be exactly 64 chars");

    let recovered = Address::from_hex(&hex).expect("valid hex must parse");
    assert_eq!(recovered.0, addr.0, "roundtrip must preserve exact bytes");

    // NEGATIVE: truncated hex fails
    assert!(Address::from_hex(&hex[..62]).is_none());
    // NEGATIVE: invalid hex fails
    let bad_hex = "zz".repeat(32);
    assert!(Address::from_hex(&bad_hex).is_none());
}

/// Signature serde roundtrip through JSON.
/// Mutation: serialize changing byte order → deserialized bytes differ.
#[test]
fn signature_serde_roundtrip() {
    let sk = SecretKey::from_bytes([33u8; 32]);
    let sig = sk.sign(b"serde test");

    let json = serde_json::to_string(&sig).expect("serialize");
    let deserialized: Signature = serde_json::from_str(&json).expect("deserialize");

    assert_eq!(sig.0, deserialized.0, "serde roundtrip must preserve exact bytes");

    // NEGATIVE: wrong-length hex fails deserialization
    let bad_json = "\"aabb\"";
    assert!(serde_json::from_str::<Signature>(bad_json).is_err());
}
