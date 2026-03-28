//! End-to-end test: simulate WebAuthn SmartOp signing + verification.
//! This replicates exactly what the browser JS does and what the server verifies.

use ultradag_coin::state::engine::StateEngine;
use ultradag_coin::tx::smart_account::*;
use ultradag_coin::Address;
use sha2::{Sha256, Digest};
use p256::ecdsa::{SigningKey, signature::Signer};

/// Simulate the full WebAuthn SmartOp flow:
/// 1. Generate P256 keypair (what the browser's secure enclave does)
/// 2. Derive address (what the relay does)
/// 3. Build SmartOpTx signable_bytes (what the JS does)
/// 4. Create WebAuthn challenge + sign (what the browser does)
/// 5. Verify via StateEngine (what the server does)
#[test]
fn test_webauthn_smartop_stream_create() {
    // 1. Generate P256 keypair
    let signing_key = SigningKey::random(&mut rand::thread_rng());
    let verifying_key = signing_key.verifying_key();
    
    // Get compressed public key (33 bytes)
    // Compressed P256 pubkey (33 bytes)
    let pubkey_bytes = verifying_key.to_encoded_point(true).as_bytes().to_vec();
    assert_eq!(pubkey_bytes.len(), 33, "Compressed P256 pubkey should be 33 bytes");
    
    // 2. Derive address (same as relay: blake3("smart_account_p256" || pubkey)[:20])
    let address = {
        let mut hasher = blake3::Hasher::new();
        hasher.update(b"smart_account_p256");
        hasher.update(&pubkey_bytes);
        let hash = hasher.finalize();
        let mut addr = [0u8; 20];
        addr.copy_from_slice(&hash.as_bytes()[..20]);
        Address(addr)
    };
    
    // 3. Compute key_id
    let key_id = AuthorizedKey::compute_key_id(KeyType::P256, &pubkey_bytes);
    
    // 4. Create a StreamCreate SmartOpTx
    let recipient = Address([1u8; 20]);
    let rate: u64 = 100;
    let deposit: u64 = 10000;
    let cliff: u64 = 0;
    let fee: u64 = 10000;
    let nonce: u64 = 0;
    
    let operation = SmartOpType::StreamCreate {
        recipient,
        rate_sats_per_round: rate,
        deposit,
        cliff_rounds: cliff,
    };
    
    let tx = SmartOpTx {
        from: address,
        operation,
        fee,
        nonce,
        signing_key_id: key_id,
        signature: vec![],
        webauthn: None,
        p256_pubkey: Some(pubkey_bytes.clone()),
    };
    
    // 5. Build signable_bytes (same as Rust and JS)
    let signable = tx.signable_bytes();
    println!("signable_bytes len: {}", signable.len());
    println!("signable_bytes hex: {}", signable.iter().map(|b| format!("{b:02x}")).collect::<String>());
    
    // 6. Simulate WebAuthn signing
    // Challenge = SHA-256(signable_bytes)
    let challenge = Sha256::digest(&signable);
    let challenge_b64 = base64url_encode(&challenge);
    
    // Simulate authenticatorData (37 bytes minimum: rpIdHash(32) + flags(1) + signCount(4))
    let mut authenticator_data = vec![0u8; 37];
    authenticator_data[32] = 0x05; // flags: UP + UV
    
    // Simulate clientDataJSON
    let origin = "https://ultradag.com";
    let client_data_json = format!(
        r#"{{"type":"webauthn.get","challenge":"{}","origin":"{}","crossOrigin":false}}"#,
        challenge_b64, origin
    );
    let client_data_bytes = client_data_json.as_bytes().to_vec();
    
    // WebAuthn signature is over: authenticatorData || SHA-256(clientDataJSON)
    let client_data_hash = Sha256::digest(&client_data_bytes);
    let mut signed_data = Vec::new();
    signed_data.extend_from_slice(&authenticator_data);
    signed_data.extend_from_slice(&client_data_hash);
    
    // P256 ECDSA sign (the p256 crate's sign() applies SHA-256 internally)
    let (sig, _) = signing_key.sign(&signed_data);
    let sig_bytes = sig.to_bytes();
    assert_eq!(sig_bytes.len(), 64, "Raw P256 signature should be 64 bytes");
    
    // 7. Build the final SmartOpTx with WebAuthn envelope
    let final_tx = SmartOpTx {
        from: address,
        operation: SmartOpType::StreamCreate {
            recipient,
            rate_sats_per_round: rate,
            deposit,
            cliff_rounds: cliff,
        },
        fee,
        nonce,
        signing_key_id: key_id,
        signature: vec![],
        webauthn: Some(WebAuthnSignature {
            authenticator_data: authenticator_data.clone(),
            client_data_json: client_data_bytes.clone(),
            signature: sig_bytes.to_vec(),
        }),
        p256_pubkey: Some(pubkey_bytes.clone()),
    };
    
    // 8. Verify with StateEngine (no SmartAccount exists yet — auto-registration path)
    let mut engine = StateEngine::new_with_genesis();
    
    // Test verify_smart_op — should auto-register and succeed
    let result = engine.verify_smart_op(&final_tx);
    
    if !result {
        // Debug: test individual steps
        println!("\n=== DEBUG ===");
        
        // Check address derivation
        let mut hasher = blake3::Hasher::new();
        hasher.update(b"smart_account_p256");
        hasher.update(&pubkey_bytes);
        let hash = hasher.finalize();
        let mut derived = [0u8; 20];
        derived.copy_from_slice(&hash.as_bytes()[..20]);
        println!("Address derivation match: {}", derived == address.0);
        
        // Check key_id
        let computed_key_id = AuthorizedKey::compute_key_id(KeyType::P256, &pubkey_bytes);
        println!("Key ID match: {}", computed_key_id == key_id);
        
        // Check challenge
        let expected_challenge = Sha256::digest(&final_tx.signable_bytes());
        let expected_b64 = base64url_encode(&expected_challenge);
        let client_str = std::str::from_utf8(&client_data_bytes).unwrap();
        println!("Challenge in clientData: {}", client_str);
        println!("Expected challenge b64: {}", expected_b64);
        
        // Check raw P256 verification
        let verify_result = verify_p256(&pubkey_bytes, &sig_bytes, &signed_data);
        println!("Direct P256 verify (signed_data): {}", verify_result);
        
        // Check prehashed verification
        let prehash = Sha256::digest(&signed_data);
        let verify_prehash = verify_p256_prehashed(&pubkey_bytes, &sig_bytes, &prehash);
        println!("Prehashed P256 verify: {}", verify_prehash);
        
        // Check verify_webauthn directly
        let webauthn_result = verify_webauthn(
            &pubkey_bytes,
            final_tx.webauthn.as_ref().unwrap(),
            &final_tx.signable_bytes(),
        );
        println!("verify_webauthn result: {}", webauthn_result);
    }
    
    assert!(result, "WebAuthn SmartOp verification should succeed");
    
    // 9. Verify SmartAccount was auto-created
    assert!(engine.smart_account(&address).is_some(), "SmartAccount should be auto-created");
    let config = engine.smart_account(&address).unwrap();
    assert_eq!(config.authorized_keys.len(), 1, "Should have 1 key registered");
    assert_eq!(config.authorized_keys[0].key_type, KeyType::P256);
}

fn base64url_encode(data: &[u8]) -> String {
    use base64::Engine;
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(data)
}

