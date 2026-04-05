//! Tests for SmartOpType::AddKey — the passkey-friendly way to register
//! a new authorized key on an existing SmartAccount.
//!
//! These exercise the engine dispatch logic (apply_smart_op_tx). Signature
//! verification is skipped here because it lives in the higher-level DAG
//! validation path; the dispatch itself only sees already-verified ops.

use ultradag_coin::address::{Address, SecretKey};
use ultradag_coin::state::StateEngine;
use ultradag_coin::tx::smart_account::*;

fn setup(sk: &SecretKey) -> StateEngine {
    let mut engine = StateEngine::new_with_genesis();
    engine.faucet_credit(&sk.address(), 100_000_000).unwrap();
    engine.ensure_smart_account(&sk.address());
    engine
}

/// Build a SmartOpTx::AddKey without a signature — apply_smart_op_tx skips
/// verification because the caller is the DAG validation layer.
fn add_key_op(
    from: Address,
    key_type: KeyType,
    pubkey: Vec<u8>,
    label: &str,
    nonce: u64,
) -> SmartOpTx {
    SmartOpTx {
        from,
        operation: SmartOpType::AddKey {
            key_type,
            pubkey,
            label: label.to_string(),
        },
        fee: 0,
        nonce,
        signing_key_id: [0u8; 8],
        signature: vec![],
        webauthn: None,
        p256_pubkey: None,
    }
}

fn new_ed25519_pubkey(seed: u8) -> Vec<u8> {
    let sk = SecretKey::from_bytes([seed; 32]);
    sk.verifying_key().to_bytes().to_vec()
}

fn new_p256_compressed_pubkey(seed: u8) -> Vec<u8> {
    // Construct a structurally-valid 33-byte compressed P256 pubkey.
    // Real format is [0x02|0x03, 32 bytes x-coord]. AddKey validation only
    // checks length and duplicate-pubkey, not curve membership, so a
    // deterministic filler is fine for these unit tests.
    let mut pk = vec![0x02];
    pk.extend_from_slice(&[seed; 32]);
    pk
}

#[test]
fn test_add_key_happy_path() {
    let alice = SecretKey::from_bytes([0x01; 32]);
    let mut engine = setup(&alice);

    // Account starts with zero authorized keys (ensure_smart_account creates
    // an empty SmartAccountConfig).
    let keys_before = engine
        .smart_account(&alice.address())
        .unwrap()
        .authorized_keys
        .len();
    assert_eq!(keys_before, 0);

    let new_pubkey = new_p256_compressed_pubkey(0xAB);
    let tx = add_key_op(
        alice.address(),
        KeyType::P256,
        new_pubkey.clone(),
        "YubiKey",
        0,
    );
    engine.apply_smart_op_tx(&tx, 100).unwrap();

    let cfg = engine.smart_account(&alice.address()).unwrap();
    assert_eq!(cfg.authorized_keys.len(), 1);
    assert_eq!(cfg.authorized_keys[0].key_type, KeyType::P256);
    assert_eq!(cfg.authorized_keys[0].pubkey, new_pubkey);
    assert_eq!(cfg.authorized_keys[0].label, "YubiKey");
    // key_id must be the derived blake3(key_type || pubkey)[..8]
    let expected_id = AuthorizedKey::compute_key_id(KeyType::P256, &new_pubkey);
    assert_eq!(cfg.authorized_keys[0].key_id, expected_id);
}

#[test]
fn test_add_key_duplicate_pubkey_rejected() {
    let alice = SecretKey::from_bytes([0x02; 32]);
    let mut engine = setup(&alice);

    let pk = new_p256_compressed_pubkey(0xCD);
    engine
        .apply_smart_op_tx(&add_key_op(alice.address(), KeyType::P256, pk.clone(), "A", 0), 100)
        .unwrap();

    let result = engine.apply_smart_op_tx(
        &add_key_op(alice.address(), KeyType::P256, pk, "B", 1),
        101,
    );
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(
        err.contains("already authorized"),
        "expected duplicate-key error, got: {err}"
    );

    // Still only one key registered.
    assert_eq!(
        engine.smart_account(&alice.address()).unwrap().authorized_keys.len(),
        1
    );
}

#[test]
fn test_add_key_max_keys_rejected() {
    let alice = SecretKey::from_bytes([0x03; 32]);
    let mut engine = setup(&alice);

    // Fill to MAX_AUTHORIZED_KEYS.
    for i in 0..MAX_AUTHORIZED_KEYS {
        let pk = new_p256_compressed_pubkey(i as u8);
        engine
            .apply_smart_op_tx(
                &add_key_op(alice.address(), KeyType::P256, pk, &format!("k{i}"), i as u64),
                100,
            )
            .unwrap();
    }

    // The (N+1)-th key must be rejected.
    let extra = new_p256_compressed_pubkey(0xFF);
    let result = engine.apply_smart_op_tx(
        &add_key_op(alice.address(), KeyType::P256, extra, "overflow", MAX_AUTHORIZED_KEYS as u64),
        200,
    );
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(
        err.contains("maximum") && err.contains("authorized"),
        "expected max-keys error, got: {err}"
    );

    assert_eq!(
        engine.smart_account(&alice.address()).unwrap().authorized_keys.len(),
        MAX_AUTHORIZED_KEYS
    );
}

#[test]
fn test_add_key_invalid_p256_length_rejected() {
    let alice = SecretKey::from_bytes([0x04; 32]);
    let mut engine = setup(&alice);

    // 32-byte pubkey with key_type=P256 is illegal (P256 expects 33 or 65).
    let bad = vec![0x42; 32];
    let result = engine.apply_smart_op_tx(
        &add_key_op(alice.address(), KeyType::P256, bad, "bad", 0),
        100,
    );
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("P256"), "expected P256 length error, got: {err}");
}

#[test]
fn test_add_key_invalid_ed25519_length_rejected() {
    let alice = SecretKey::from_bytes([0x05; 32]);
    let mut engine = setup(&alice);

    // 33-byte pubkey with key_type=Ed25519 is illegal (Ed25519 expects 32).
    let bad = vec![0x42; 33];
    let result = engine.apply_smart_op_tx(
        &add_key_op(alice.address(), KeyType::Ed25519, bad, "bad", 0),
        100,
    );
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("Ed25519"), "expected Ed25519 length error, got: {err}");
}

#[test]
fn test_add_key_label_too_long_rejected() {
    let alice = SecretKey::from_bytes([0x06; 32]);
    let mut engine = setup(&alice);

    let huge_label = "x".repeat(MAX_KEY_LABEL_BYTES + 1);
    let pk = new_p256_compressed_pubkey(0x11);
    let result = engine.apply_smart_op_tx(
        &SmartOpTx {
            from: alice.address(),
            operation: SmartOpType::AddKey {
                key_type: KeyType::P256,
                pubkey: pk,
                label: huge_label,
            },
            fee: 0,
            nonce: 0,
            signing_key_id: [0u8; 8],
            signature: vec![],
            webauthn: None,
            p256_pubkey: None,
        },
        100,
    );
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("label"), "expected label length error, got: {err}");
}

#[test]
fn test_add_key_account_not_found_rejected() {
    let ghost = SecretKey::from_bytes([0x07; 32]);
    let mut engine = StateEngine::new_with_genesis();
    // NO ensure_smart_account — the account does not exist.

    let pk = new_ed25519_pubkey(0x22);
    let result = engine.apply_smart_op_tx(
        &add_key_op(ghost.address(), KeyType::Ed25519, pk, "ghost", 0),
        100,
    );
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(
        err.contains("smart account not found"),
        "expected account-not-found error, got: {err}"
    );
}

#[test]
fn test_add_key_signable_bytes_stable_encoding() {
    // Guard against accidental wire-format changes. If this hash drifts the
    // TypeScript side in dashboard/src/lib/webauthn-sign.ts must be updated
    // to match.
    let addr = Address([0xAA; 20]);
    let pk = vec![0x02; 33];
    let tx = SmartOpTx {
        from: addr,
        operation: SmartOpType::AddKey {
            key_type: KeyType::P256,
            pubkey: pk.clone(),
            label: "Backup".to_string(),
        },
        fee: 0,
        nonce: 0,
        signing_key_id: [0u8; 8],
        signature: vec![],
        webauthn: None,
        p256_pubkey: None,
    };
    let bytes = tx.signable_bytes();

    // Layout sanity: NETWORK_ID || "smart_op" || from(20) || 13 || key_type(1)
    //   || pubkey_len_le(4) || pubkey(33) || label_len_le(4) || label(6)
    //   || fee_le(8) || nonce_le(8) || signing_key_id(8)
    // We can't hardcode exact bytes without depending on NETWORK_ID, but we
    // can check stable structural offsets.
    let network = crate::NETWORK_ID_SLICE;
    assert!(bytes.starts_with(network), "signable_bytes must start with NETWORK_ID");
    let tail = &bytes[network.len()..];
    assert!(tail.starts_with(b"smart_op"));
    let after_tag = &tail[b"smart_op".len()..];
    assert_eq!(&after_tag[..20], &[0xAA; 20]); // from
    assert_eq!(after_tag[20], 13); // AddKey discriminant
    assert_eq!(after_tag[21], KeyType::P256 as u8);
    // pubkey len u32 LE = 33
    assert_eq!(&after_tag[22..26], &(33u32).to_le_bytes());
    // pubkey
    assert_eq!(&after_tag[26..59], pk.as_slice());
    // label len u32 LE = 6
    assert_eq!(&after_tag[59..63], &(6u32).to_le_bytes());
    // label
    assert_eq!(&after_tag[63..69], b"Backup");
}

// Re-export NETWORK_ID for the stability test above.
mod crate_alias {
    pub use ultradag_coin::constants::NETWORK_ID;
}
#[allow(non_upper_case_globals)]
const NETWORK_ID_SLICE: &[u8] = crate_alias::NETWORK_ID;
