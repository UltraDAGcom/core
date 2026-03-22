//! Bridge attestation types and functions for Validator Federation Bridge.
//!
//! This module handles cross-chain bridge operations secured by the DAG validator set.
//! Validators sign attestations as part of normal consensus (2/3 threshold required).
//!
//! ## Signature Scheme
//!
//! Solidity's `ecrecover` only supports secp256k1/ECDSA signatures. UltraDAG validators
//! use Ed25519 for consensus. To bridge this gap, each validator maintains a separate
//! secp256k1 key for bridge signing. The secp256k1 key is derived deterministically
//! from the Ed25519 key via `sha256(ed25519_secret || b"bridge-secp256k1")`.
//!
//! In production, validators SHOULD use a separately generated secp256k1 key rather
//! than the derived one.

use crate::address::Address;
use crate::error::CoinError;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// A bridge attestation signed by DAG validators.
/// Used to prove that funds were locked on the DAG side for withdrawal on Arbitrum.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeAttestation {
    /// Original sender on DAG (native chain)
    pub sender: Address,
    /// Recipient on Arbitrum (20-byte address)
    pub recipient: [u8; 20],
    /// Amount to transfer (in sats)
    pub amount: u64,
    /// Unique nonce for this withdrawal (prevents replay)
    pub nonce: u64,
    /// Chain ID of destination chain (e.g., Arbitrum chain ID)
    pub destination_chain_id: u64,
    /// Bridge contract address on the destination chain (20 bytes).
    /// Included in the Solidity message hash to prevent cross-contract replay.
    #[serde(default)]
    pub bridge_contract_address: [u8; 20],
    /// Round at which this attestation was created (for round-based pruning).
    #[serde(default)]
    pub creation_round: u64,
}

impl BridgeAttestation {
    /// Create a new bridge attestation.
    pub fn new(
        sender: Address,
        recipient: [u8; 20],
        amount: u64,
        nonce: u64,
        destination_chain_id: u64,
    ) -> Self {
        Self {
            sender,
            recipient,
            amount,
            nonce,
            destination_chain_id,
            bridge_contract_address: [0u8; 20],
            creation_round: 0,
        }
    }

    /// Create a new bridge attestation with bridge contract address and creation round.
    pub fn new_with_contract(
        sender: Address,
        recipient: [u8; 20],
        amount: u64,
        nonce: u64,
        destination_chain_id: u64,
        bridge_contract_address: [u8; 20],
        creation_round: u64,
    ) -> Self {
        Self {
            sender,
            recipient,
            amount,
            nonce,
            destination_chain_id,
            bridge_contract_address,
            creation_round,
        }
    }

    /// Compute the hash of this attestation for signing.
    /// This hash is what validators sign to attest to the withdrawal.
    /// Includes `b"bridge-attestation"` domain separator to prevent cross-type hash collisions
    /// (matching the pattern from Bugs #181-183).
    pub fn hash(&self) -> [u8; 32] {
        use blake3::Hasher;
        let mut hasher = Hasher::new();
        hasher.update(b"bridge-attestation");
        hasher.update(&self.sender.0);
        hasher.update(&self.recipient);
        hasher.update(&self.amount.to_le_bytes());
        hasher.update(&self.nonce.to_le_bytes());
        hasher.update(&self.destination_chain_id.to_le_bytes());
        hasher.update(&self.bridge_contract_address);
        *hasher.finalize().as_bytes()
    }

    /// Compute the message hash that matches the Solidity contract.
    /// Uses keccak256 with ABI encoding to match Solidity's
    /// `keccak256(abi.encode("claimWithdrawal", block.chainid, address(this), sender, recipient, amount, depositNonce))`.
    ///
    /// ABI encoding pads each argument to 32 bytes. String is encoded as
    /// (offset, length, data padded to 32-byte boundary). Integers are big-endian
    /// left-padded. Addresses (20 bytes) are left-padded to 32 bytes.
    ///
    /// C1 fix: includes `bridge_contract_address` (address(this)) as the 3rd slot,
    /// matching the Solidity contract's 7-argument encoding.
    pub fn solidity_message_hash(&self) -> [u8; 32] {
        use sha3::{Digest, Keccak256};

        // ABI-encode "claimWithdrawal" as a string:
        // Slot 0: offset to string data (7 * 32 = 224 = 0xE0) — 7 slots: offset + 6 fixed args
        // Then the fixed args:
        // Slot 1: destination_chain_id (uint256, big-endian, padded to 32 bytes)
        // Slot 2: bridge_contract_address (address(this), 20 bytes, left-padded to 32 bytes)
        // Slot 3: sender (bytes20 in Solidity, RIGHT-padded to 32 bytes)
        // Slot 4: recipient (address, 20 bytes, left-padded to 32 bytes)
        // Slot 5: amount (uint256)
        // Slot 6: nonce (uint256)
        // Then the string data:
        // Slot 7: string length (15 = "claimWithdrawal".len())
        // Slot 8: string bytes, right-padded to 32 bytes

        let mut buf = Vec::with_capacity(9 * 32);

        // Slot 0: offset to string data = 7 * 32 = 224 (7 fixed-size args after offset)
        let mut slot = [0u8; 32];
        slot[31] = 224; // 7 * 32
        buf.extend_from_slice(&slot);

        // Slot 1: destination_chain_id as uint256 (big-endian)
        let mut slot = [0u8; 32];
        slot[24..32].copy_from_slice(&self.destination_chain_id.to_be_bytes());
        buf.extend_from_slice(&slot);

        // Slot 2: bridge_contract_address (address(this), 20 bytes, left-padded)
        let mut slot = [0u8; 32];
        slot[12..32].copy_from_slice(&self.bridge_contract_address);
        buf.extend_from_slice(&slot);

        // Slot 3: sender (bytes20 in Solidity, RIGHT-padded to 32 bytes)
        // Solidity abi.encode for bytes20 puts data in bytes 0-19, zeros in bytes 20-31.
        let mut slot = [0u8; 32];
        slot[..20].copy_from_slice(&self.sender.0);
        buf.extend_from_slice(&slot);

        // Slot 4: recipient (20-byte EVM address, left-padded to 32 bytes)
        let mut slot = [0u8; 32];
        slot[12..32].copy_from_slice(&self.recipient);
        buf.extend_from_slice(&slot);

        // Slot 5: amount as uint256
        let mut slot = [0u8; 32];
        slot[24..32].copy_from_slice(&self.amount.to_be_bytes());
        buf.extend_from_slice(&slot);

        // Slot 6: nonce as uint256
        let mut slot = [0u8; 32];
        slot[24..32].copy_from_slice(&self.nonce.to_be_bytes());
        buf.extend_from_slice(&slot);

        // Slot 7: string length = 15
        let mut slot = [0u8; 32];
        slot[31] = 15;
        buf.extend_from_slice(&slot);

        // Slot 8: string data "claimWithdrawal" (15 bytes), right-padded
        let mut slot = [0u8; 32];
        slot[..15].copy_from_slice(b"claimWithdrawal");
        buf.extend_from_slice(&slot);

        let result = Keccak256::digest(&buf);
        let mut hash = [0u8; 32];
        hash.copy_from_slice(&result);
        hash
    }

    /// Verify the attestation is valid (basic checks).
    pub fn verify(&self) -> Result<(), CoinError> {
        if self.recipient == [0u8; 20] {
            return Err(CoinError::ValidationError("Invalid recipient address".into()));
        }
        if self.amount == 0 {
            return Err(CoinError::ValidationError("Amount cannot be zero".into()));
        }
        Ok(())
    }
}

// ─── Secp256k1 Bridge Signing (H1 fix) ───

/// Derive a secp256k1 signing key from an Ed25519 secret key.
/// Uses `sha256(ed25519_secret_bytes || b"bridge-secp256k1")` as the seed.
/// If the SHA-256 output is not a valid secp256k1 scalar (probability ~2^-128),
/// retries with a counter appended to the input.
///
/// WARNING: In production, validators SHOULD use a separately generated secp256k1 key.
/// This deterministic derivation is a convenience for testnet.
pub fn derive_secp_key_from_ed25519(ed25519_secret: &[u8; 32]) -> Option<k256::ecdsa::SigningKey> {
    use sha2::Digest;
    for counter in 0u32..256 {
        let mut hasher = sha2::Sha256::new();
        hasher.update(ed25519_secret);
        hasher.update(b"bridge-secp256k1");
        if counter > 0 {
            hasher.update(&counter.to_le_bytes());
        }
        let seed = hasher.finalize();
        if let Ok(key) = k256::ecdsa::SigningKey::from_slice(&seed) {
            return Some(key);
        }
    }
    None // Astronomically unlikely (256 consecutive invalid scalars)
}

/// Produce an Ethereum-compatible ECDSA signature (r || s || v, 65 bytes).
/// `v` = recovery_id + 27, matching Solidity's `ecrecover` convention.
///
/// C1 fix: applies EIP-191 prefix ("\x19Ethereum Signed Message:\n32") before signing,
/// matching the Solidity contract's `ecrecover(keccak256(abi.encodePacked("\x19Ethereum Signed Message:\n32", messageHash)), ...)`
pub fn sign_for_bridge(message_hash: &[u8; 32], secp_key: &k256::ecdsa::SigningKey) -> Result<[u8; 65], String> {
    use k256::ecdsa::{RecoveryId, Signature, VerifyingKey};
    use sha3::{Digest, Keccak256};

    // EIP-191: prefix the message hash before signing
    let mut prefixed = Vec::with_capacity(60);
    prefixed.extend_from_slice(b"\x19Ethereum Signed Message:\n32");
    prefixed.extend_from_slice(message_hash);
    let eth_signed_hash: [u8; 32] = Keccak256::digest(&prefixed).into();

    // Sign the EIP-191 prefixed hash
    let (sig, recid): (Signature, RecoveryId) = secp_key
        .sign_prehash_recoverable(&eth_signed_hash)
        .map_err(|e| format!("ECDSA signing failed: {}", e))?;

    let sig_bytes = sig.to_bytes();
    let mut result = [0u8; 65];
    result[..64].copy_from_slice(&sig_bytes);
    result[64] = recid.to_byte() + 27; // Ethereum v convention

    // Sanity: verify we can recover the correct key from the prefixed hash
    debug_assert!({
        let recovered = VerifyingKey::recover_from_prehash(&eth_signed_hash, &sig, recid);
        recovered.map(|vk| vk == *secp_key.verifying_key()).unwrap_or(false)
    });

    Ok(result)
}

/// Derive the Ethereum address from a secp256k1 signing key.
/// Address = last 20 bytes of keccak256(uncompressed_pubkey_without_prefix).
pub fn eth_address_from_secp_key(key: &k256::ecdsa::SigningKey) -> [u8; 20] {
    use k256::ecdsa::VerifyingKey;
    use sha3::{Digest, Keccak256};

    let vk = VerifyingKey::from(key);
    let encoded = vk.to_encoded_point(false); // uncompressed: 0x04 || x || y
    let pubkey_bytes = &encoded.as_bytes()[1..]; // skip 0x04 prefix

    let hash = Keccak256::digest(pubkey_bytes);
    let mut addr = [0u8; 20];
    addr.copy_from_slice(&hash[12..32]);
    addr
}

/// A signed bridge attestation from a validator.
/// Uses ECDSA/secp256k1 signatures (65 bytes: r || s || v) for Solidity compatibility.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignedBridgeAttestation {
    /// The attestation being signed
    pub attestation: BridgeAttestation,
    /// Validator's DAG address (Ed25519 public key hash)
    pub validator: Address,
    /// Ethereum address of the signer (derived from secp256k1 key)
    pub eth_address: [u8; 20],
    /// ECDSA signature (65 bytes: r || s || v) — compatible with Solidity's ecrecover.
    /// Stored as Vec<u8> for serde compatibility.
    #[serde(with = "serde_bytes")]
    pub signature: Vec<u8>,
}

impl SignedBridgeAttestation {
    /// Create a new signed attestation with ECDSA signature.
    pub fn new(
        attestation: BridgeAttestation,
        validator: Address,
        eth_address: [u8; 20],
        signature: [u8; 65],
    ) -> Self {
        Self {
            attestation,
            validator,
            eth_address,
            signature: signature.to_vec(),
        }
    }

    /// Get signature as fixed-size 65-byte array. Returns None if the signature
    /// is not exactly 65 bytes.
    pub fn signature_array(&self) -> Option<[u8; 65]> {
        if self.signature.len() != 65 {
            return None;
        }
        let mut arr = [0u8; 65];
        arr.copy_from_slice(&self.signature);
        Some(arr)
    }

    /// Verify the ECDSA signature over the attestation's Solidity message hash.
    /// Recovers the signer's Ethereum address from the EIP-191-prefixed hash and checks it matches.
    ///
    /// C1 fix: applies EIP-191 prefix before ECDSA recovery, matching sign_for_bridge().
    pub fn verify_signature(&self) -> bool {
        use k256::ecdsa::{RecoveryId, Signature, VerifyingKey};
        use sha3::{Digest, Keccak256};

        if self.signature.len() != 65 {
            return false;
        }

        let sig = match Signature::from_slice(&self.signature[..64]) {
            Ok(s) => s,
            Err(_) => return false,
        };

        let v = self.signature[64];
        let recovery_id = match RecoveryId::from_byte(v.wrapping_sub(27)) {
            Some(id) => id,
            None => return false,
        };

        let message_hash = self.attestation.solidity_message_hash();

        // EIP-191: apply the same prefix used in sign_for_bridge()
        let mut prefixed = Vec::with_capacity(60);
        prefixed.extend_from_slice(b"\x19Ethereum Signed Message:\n32");
        prefixed.extend_from_slice(&message_hash);
        let eth_signed_hash: [u8; 32] = Keccak256::digest(&prefixed).into();

        let recovered_vk = match VerifyingKey::recover_from_prehash(&eth_signed_hash, &sig, recovery_id) {
            Ok(vk) => vk,
            Err(_) => return false,
        };

        // Derive Ethereum address from recovered public key
        let encoded = recovered_vk.to_encoded_point(false);
        let pubkey_bytes = &encoded.as_bytes()[1..];
        let hash = Keccak256::digest(pubkey_bytes);
        let mut recovered_addr = [0u8; 20];
        recovered_addr.copy_from_slice(&hash[12..32]);

        recovered_addr == self.eth_address
    }
}

/// Complete bridge proof with threshold signatures.
/// This is what users submit to the Arbitrum contract.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeProof {
    /// The original attestation
    pub attestation: BridgeAttestation,
    /// Threshold signatures from validators (2/3+)
    pub signatures: Vec<SignedBridgeAttestation>,
    /// Message hash for verification
    pub message_hash: [u8; 32],
}

impl BridgeProof {
    /// Create a new bridge proof.
    pub fn new(
        attestation: BridgeAttestation,
        signatures: Vec<SignedBridgeAttestation>,
    ) -> Self {
        let message_hash = attestation.solidity_message_hash();
        Self {
            attestation,
            signatures,
            message_hash,
        }
    }

    /// Verify the proof has enough valid signatures.
    ///
    /// C2 fix: checks for duplicate signers (by validator address).
    /// C3 fix: only counts signatures from validators in `active_validators`.
    /// H3 fix: verifies each signed attestation's hash matches the top-level attestation.
    pub fn verify(&self, threshold: usize, active_validators: &HashSet<Address>) -> Result<(), CoinError> {
        let expected_hash = self.attestation.solidity_message_hash();
        let mut seen_validators: HashSet<Address> = HashSet::new();
        let mut valid_count = 0usize;

        for signed in &self.signatures {
            // H3: verify each signed attestation matches the top-level attestation
            if signed.attestation.solidity_message_hash() != expected_hash {
                return Err(CoinError::ValidationError("attestation mismatch in proof".into()));
            }

            // C2: check for duplicate signers BEFORE counting
            if seen_validators.contains(&signed.validator) {
                return Err(CoinError::ValidationError("duplicate signer".into()));
            }
            seen_validators.insert(signed.validator);

            // C3: check validator set membership
            if !active_validators.contains(&signed.validator) {
                continue; // skip non-active validators, don't count toward threshold
            }

            // Verify ECDSA signature
            if !signed.verify_signature() {
                return Err(CoinError::InvalidSignature);
            }

            valid_count += 1;
        }

        if valid_count < threshold {
            return Err(CoinError::ValidationError(
                format!("Insufficient valid signatures from active validators: {} < {}", valid_count, threshold)
            ));
        }

        Ok(())
    }

    /// Encode signatures for Solidity contract (packed format: r || s || v for each, 65 bytes each).
    /// Produces the format expected by Solidity's ecrecover.
    ///
    /// H4 fix: sorts signatures by ascending Ethereum address before encoding,
    /// matching the Solidity contract's requirement (`signer <= lastSigner` check).
    pub fn encode_signatures(&self) -> Vec<u8> {
        let mut sorted = self.signatures.clone();
        sorted.sort_by_key(|s| s.eth_address);
        let mut encoded = Vec::with_capacity(sorted.len() * 65);
        for signed in &sorted {
            // Each signature is already in r || s || v format (65 bytes)
            encoded.extend_from_slice(&signed.signature);
        }
        encoded
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::address::SecretKey;

    #[test]
    fn test_attestation_hash() {
        let sk = SecretKey::generate();
        let attestation = BridgeAttestation::new(
            sk.address(),
            [1u8; 20],
            1000,
            0,
            42161, // Arbitrum chain ID
        );

        let hash = attestation.hash();
        assert_ne!(hash, [0u8; 32]);

        // Hash should be deterministic
        let hash2 = attestation.hash();
        assert_eq!(hash, hash2);
    }

    #[test]
    fn test_attestation_hash_includes_contract_address() {
        let sk = SecretKey::generate();
        let mut att1 = BridgeAttestation::new(sk.address(), [1u8; 20], 1000, 0, 42161);
        att1.bridge_contract_address = [0xAA; 20];

        let mut att2 = BridgeAttestation::new(sk.address(), [1u8; 20], 1000, 0, 42161);
        att2.bridge_contract_address = [0xBB; 20];

        // Different contract addresses must produce different hashes
        assert_ne!(att1.hash(), att2.hash());
    }

    #[test]
    fn test_attestation_verify() {
        let sk = SecretKey::generate();
        let attestation = BridgeAttestation::new(
            sk.address(),
            [1u8; 20],
            1000,
            0,
            42161,
        );

        assert!(attestation.verify().is_ok());

        // Invalid recipient
        let invalid = BridgeAttestation::new(
            sk.address(),
            [0u8; 20],
            1000,
            0,
            42161,
        );
        assert!(invalid.verify().is_err());

        // Zero amount
        let invalid = BridgeAttestation::new(
            sk.address(),
            [1u8; 20],
            0,
            0,
            42161,
        );
        assert!(invalid.verify().is_err());
    }

    #[test]
    fn test_secp256k1_signing_and_verification() {
        let ed_sk = SecretKey::generate();
        let secp_key = derive_secp_key_from_ed25519(&ed_sk.to_bytes()).unwrap();
        let eth_addr = eth_address_from_secp_key(&secp_key);

        let attestation = BridgeAttestation::new(
            ed_sk.address(),
            [1u8; 20],
            1000,
            0,
            42161,
        );

        let msg_hash = attestation.solidity_message_hash();
        let sig = sign_for_bridge(&msg_hash, &secp_key).unwrap();
        assert_eq!(sig.len(), 65);
        assert!(sig[64] == 27 || sig[64] == 28); // v must be 27 or 28

        let signed = SignedBridgeAttestation::new(
            attestation,
            ed_sk.address(),
            eth_addr,
            sig,
        );

        assert!(signed.verify_signature());
    }

    #[test]
    fn test_bridge_proof_duplicate_signer_rejected() {
        let sk = SecretKey::generate();
        let secp_key = derive_secp_key_from_ed25519(&sk.to_bytes()).unwrap();
        let eth_addr = eth_address_from_secp_key(&secp_key);

        let attestation = BridgeAttestation::new(sk.address(), [1u8; 20], 1000, 0, 42161);
        let msg_hash = attestation.solidity_message_hash();
        let sig = sign_for_bridge(&msg_hash, &secp_key).unwrap();

        let signed = SignedBridgeAttestation::new(attestation.clone(), sk.address(), eth_addr, sig);

        // Duplicate signer
        let proof = BridgeProof::new(attestation, vec![signed.clone(), signed]);
        let mut active = HashSet::new();
        active.insert(sk.address());

        let result = proof.verify(1, &active);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("duplicate signer"));
    }

    #[test]
    fn test_bridge_proof_non_active_validator_skipped() {
        let sk = SecretKey::generate();
        let secp_key = derive_secp_key_from_ed25519(&sk.to_bytes()).unwrap();
        let eth_addr = eth_address_from_secp_key(&secp_key);

        let attestation = BridgeAttestation::new(sk.address(), [1u8; 20], 1000, 0, 42161);
        let msg_hash = attestation.solidity_message_hash();
        let sig = sign_for_bridge(&msg_hash, &secp_key).unwrap();

        let signed = SignedBridgeAttestation::new(attestation.clone(), sk.address(), eth_addr, sig);

        let proof = BridgeProof::new(attestation, vec![signed]);

        // Empty active set: signature should not count
        let active = HashSet::new();
        let result = proof.verify(1, &active);
        assert!(result.is_err());
    }

    #[test]
    fn test_solidity_message_hash_includes_contract_address() {
        let sk = SecretKey::generate();
        let mut att1 = BridgeAttestation::new(sk.address(), [1u8; 20], 1000, 0, 42161);
        att1.bridge_contract_address = [0xAA; 20];

        let mut att2 = BridgeAttestation::new(sk.address(), [1u8; 20], 1000, 0, 42161);
        att2.bridge_contract_address = [0xBB; 20];

        // Different contract addresses must produce different Solidity hashes
        assert_ne!(att1.solidity_message_hash(), att2.solidity_message_hash());
    }

    #[test]
    fn test_bridge_proof_valid() {
        let sk = SecretKey::generate();
        let secp_key = derive_secp_key_from_ed25519(&sk.to_bytes()).unwrap();
        let eth_addr = eth_address_from_secp_key(&secp_key);

        let attestation = BridgeAttestation::new(sk.address(), [1u8; 20], 1000, 0, 42161);
        let msg_hash = attestation.solidity_message_hash();
        let sig = sign_for_bridge(&msg_hash, &secp_key).unwrap();

        let signed = SignedBridgeAttestation::new(attestation.clone(), sk.address(), eth_addr, sig);

        let proof = BridgeProof::new(attestation.clone(), vec![signed]);
        let mut active = HashSet::new();
        active.insert(sk.address());

        assert!(proof.verify(1, &active).is_ok());
        assert_eq!(proof.message_hash, attestation.solidity_message_hash());
    }

    #[test]
    fn test_encode_signatures_65_bytes_each() {
        let sk = SecretKey::generate();
        let secp_key = derive_secp_key_from_ed25519(&sk.to_bytes()).unwrap();
        let eth_addr = eth_address_from_secp_key(&secp_key);

        let attestation = BridgeAttestation::new(sk.address(), [1u8; 20], 1000, 0, 42161);
        let msg_hash = attestation.solidity_message_hash();
        let sig = sign_for_bridge(&msg_hash, &secp_key).unwrap();

        let signed = SignedBridgeAttestation::new(attestation.clone(), sk.address(), eth_addr, sig);
        let proof = BridgeProof::new(attestation, vec![signed]);

        let encoded = proof.encode_signatures();
        assert_eq!(encoded.len(), 65); // one signature, 65 bytes
    }
}
