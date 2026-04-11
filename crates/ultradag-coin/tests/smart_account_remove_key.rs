//! Tests for SmartOpType::RemoveKey — the passkey-friendly counterpart to
//! RemoveKeyTx (which is Ed25519-only). Initiates a time-locked removal of
//! an authorized key, executed after KEY_REMOVAL_DELAY_ROUNDS.

use ultradag_coin::address::{Address, SecretKey};
use ultradag_coin::state::StateEngine;
use ultradag_coin::tx::smart_account::*;

fn setup(sk: &SecretKey) -> StateEngine {
    let mut engine = StateEngine::new_with_genesis();
    engine.faucet_credit(&sk.address(), 100_000_000).unwrap();
    engine.ensure_smart_account(&sk.address());
    engine
}

fn p256_pubkey(seed: u8) -> Vec<u8> {
    let mut pk = vec![0x02];
    pk.extend_from_slice(&[seed; 32]);
    pk
}

fn add_key(engine: &mut StateEngine, from: Address, pubkey: Vec<u8>, label: &str, nonce: u64) -> [u8; 8] {
    let key_id = AuthorizedKey::compute_key_id(KeyType::P256, &pubkey);
    engine
        .apply_smart_op_tx(
            &SmartOpTx {
                from,
                operation: SmartOpType::AddKey {
                    key_type: KeyType::P256,
                    pubkey,
                    label: label.to_string(),
                },
                fee: 0,
                nonce,
                signing_key_id: [0u8; 8],
                signature: vec![],
                webauthn: None,
                p256_pubkey: None,
            },
            100,
        )
        .unwrap();
    key_id
}

fn remove_key_op(from: Address, key_id_to_remove: [u8; 8], nonce: u64) -> SmartOpTx {
    SmartOpTx {
        from,
        operation: SmartOpType::RemoveKey { key_id_to_remove },
        fee: 0,
        nonce,
        signing_key_id: [0u8; 8],
        signature: vec![],
        webauthn: None,
        p256_pubkey: None,
    }
}

#[test]
fn test_remove_key_happy_path() {
    let alice = SecretKey::from_bytes([0x01; 32]);
    let mut engine = setup(&alice);

    // Add two keys so we can remove one without hitting the last-key guard.
    let _primary = add_key(&mut engine, alice.address(), p256_pubkey(0xAA), "primary", 0);
    let backup = add_key(&mut engine, alice.address(), p256_pubkey(0xBB), "backup", 1);

    // Initiate removal of the backup key.
    engine
        .apply_smart_op_tx(&remove_key_op(alice.address(), backup, 2), 100)
        .unwrap();

    let cfg = engine.smart_account(&alice.address()).unwrap();
    assert!(cfg.pending_key_removal.is_some(), "pending_key_removal should be set");
    let pending = cfg.pending_key_removal.as_ref().unwrap();
    assert_eq!(pending.key_id, backup);
    assert_eq!(pending.initiated_at_round, 100);
    assert_eq!(pending.executes_at_round, 100 + KEY_REMOVAL_DELAY_ROUNDS);

    // Key is still present until the time-lock elapses.
    assert_eq!(cfg.authorized_keys.len(), 2);

    // Advance past the delay and process pending removals.
    engine.process_pending_key_removals(100 + KEY_REMOVAL_DELAY_ROUNDS);
    let cfg = engine.smart_account(&alice.address()).unwrap();
    assert_eq!(cfg.authorized_keys.len(), 1);
    assert!(cfg.pending_key_removal.is_none());
    // The primary key remains.
    assert!(cfg.authorized_keys.iter().any(|k| k.label == "primary"));
}

#[test]
fn test_remove_key_cannot_remove_last_key() {
    let alice = SecretKey::from_bytes([0x02; 32]);
    let mut engine = setup(&alice);

    let only = add_key(&mut engine, alice.address(), p256_pubkey(0x01), "only", 0);

    let result = engine.apply_smart_op_tx(&remove_key_op(alice.address(), only, 1), 100);
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(
        err.contains("last authorized key"),
        "expected last-key error, got: {err}"
    );
}

#[test]
fn test_remove_key_not_found_rejected() {
    let alice = SecretKey::from_bytes([0x03; 32]);
    let mut engine = setup(&alice);

    add_key(&mut engine, alice.address(), p256_pubkey(0x10), "a", 0);
    add_key(&mut engine, alice.address(), p256_pubkey(0x11), "b", 1);

    // Random key_id that isn't on the account.
    let result = engine.apply_smart_op_tx(&remove_key_op(alice.address(), [0xFF; 8], 2), 100);
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("not found"), "expected not-found error, got: {err}");
}

#[test]
fn test_remove_key_double_pending_rejected() {
    let alice = SecretKey::from_bytes([0x04; 32]);
    let mut engine = setup(&alice);

    let _a = add_key(&mut engine, alice.address(), p256_pubkey(0x20), "a", 0);
    let b = add_key(&mut engine, alice.address(), p256_pubkey(0x21), "b", 1);
    let c = add_key(&mut engine, alice.address(), p256_pubkey(0x22), "c", 2);

    engine
        .apply_smart_op_tx(&remove_key_op(alice.address(), b, 3), 100)
        .unwrap();

    let result = engine.apply_smart_op_tx(&remove_key_op(alice.address(), c, 4), 101);
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(
        err.contains("already pending"),
        "expected already-pending error, got: {err}"
    );
}

#[test]
fn test_remove_key_account_not_found_rejected() {
    let ghost = SecretKey::from_bytes([0x05; 32]);
    let mut engine = StateEngine::new_with_genesis();
    // No ensure_smart_account — account does not exist.

    let result = engine.apply_smart_op_tx(&remove_key_op(ghost.address(), [0xAA; 8], 0), 100);
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(
        err.contains("smart account not found"),
        "expected account-not-found error, got: {err}"
    );
}

#[test]
fn test_remove_key_signable_bytes_stable_encoding() {
    // Wire-format guard. If this layout drifts the TypeScript encoder in
    // dashboard/src/lib/webauthn-sign.ts must be updated to match.
    let addr = Address([0xCC; 20]);
    let key_id: [u8; 8] = [0xDE, 0xAD, 0xBE, 0xEF, 0xCA, 0xFE, 0x42, 0x01];
    let tx = SmartOpTx {
        from: addr,
        operation: SmartOpType::RemoveKey { key_id_to_remove: key_id },
        fee: 0,
        nonce: 7,
        signing_key_id: [0u8; 8],
        signature: vec![],
        webauthn: None,
        p256_pubkey: None,
    };
    let bytes = tx.signable_bytes();

    // Layout: NETWORK_ID || "smart_op" || from(20) || 17 || key_id(8)
    //   || fee_le(8) || nonce_le(8) || signing_key_id(8)
    let network: &[u8] = ultradag_coin::constants::NETWORK_ID;
    assert!(bytes.starts_with(network));
    let tail = &bytes[network.len()..];
    assert!(tail.starts_with(b"smart_op"));
    let after_tag = &tail[b"smart_op".len()..];
    assert_eq!(&after_tag[..20], &[0xCC; 20]); // from
    assert_eq!(after_tag[20], 17); // RemoveKey discriminant
    assert_eq!(&after_tag[21..29], &key_id); // key_id_to_remove
    // fee (LE u64 = 0)
    assert_eq!(&after_tag[29..37], &0u64.to_le_bytes());
    // nonce (LE u64 = 7)
    assert_eq!(&after_tag[37..45], &7u64.to_le_bytes());
}
