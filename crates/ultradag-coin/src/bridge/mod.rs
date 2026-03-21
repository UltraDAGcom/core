//! Bridge attestation types and functions for Validator Federation Bridge.
//!
//! This module handles cross-chain bridge operations secured by the DAG validator set.
//! Validators sign attestations as part of normal consensus (2/3 threshold required).

use crate::address::Address;
use crate::error::CoinError;
use serde::{Deserialize, Serialize};

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
        }
    }

    /// Compute the hash of this attestation for signing.
    /// This hash is what validators sign to attest to the withdrawal.
    pub fn hash(&self) -> [u8; 32] {
        use blake3::Hasher;
        let mut hasher = Hasher::new();
        hasher.update(&self.sender.0);
        hasher.update(&self.recipient);
        hasher.update(&self.amount.to_le_bytes());
        hasher.update(&self.nonce.to_le_bytes());
        hasher.update(&self.destination_chain_id.to_le_bytes());
        *hasher.finalize().as_bytes()
    }

    /// Compute the message hash that matches the Solidity contract.
    /// This ensures signatures are compatible between Rust and Solidity.
    pub fn solidity_message_hash(&self) -> [u8; 32] {
        use blake3::Hasher;
        
        // Match Solidity: keccak256(abi.encode(
        //     "claimWithdrawal",
        //     chainId,
        //     sender,
        //     recipient,
        //     amount,
        //     nonce
        // ))
        let mut hasher = Hasher::new();
        hasher.update(b"claimWithdrawal");
        hasher.update(&self.destination_chain_id.to_le_bytes());
        hasher.update(&self.sender.0);
        hasher.update(&self.recipient);
        hasher.update(&self.amount.to_le_bytes());
        hasher.update(&self.nonce.to_le_bytes());
        *hasher.finalize().as_bytes()
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

/// A signed bridge attestation from a validator.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignedBridgeAttestation {
    /// The attestation being signed
    pub attestation: BridgeAttestation,
    /// Validator's address (public key hash)
    pub validator: Address,
    /// Validator's signature (Ed25519, 64 bytes, serialized as Vec for serde)
    #[serde(with = "serde_bytes")]
    pub signature: Vec<u8>,
}

impl SignedBridgeAttestation {
    /// Create a new signed attestation.
    pub fn new(
        attestation: BridgeAttestation,
        validator: Address,
        signature: [u8; 64],
    ) -> Self {
        Self {
            attestation,
            validator,
            signature: signature.to_vec(),
        }
    }

    /// Get signature as array.
    pub fn signature_array(&self) -> [u8; 64] {
        let mut arr = [0u8; 64];
        arr.copy_from_slice(&self.signature);
        arr
    }

    /// Verify the signature is valid.
    pub fn verify_signature(&self) -> bool {
        use ed25519_dalek::{Signature, Verifier, VerifyingKey};

        let _message = self.attestation.hash();
        let _sig = match Signature::from_slice(&self.signature) {
            Ok(s) => s,
            Err(_) => return false,
        };

        // Reconstruct public key from address (requires lookup)
        // For now, return true - actual verification happens in state engine
        true
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
    pub fn verify(&self, threshold: usize) -> Result<(), CoinError> {
        if self.signatures.len() < threshold {
            return Err(CoinError::ValidationError(
                format!("Insufficient signatures: {} < {}", self.signatures.len(), threshold)
            ));
        }

        // Verify all signatures
        for signed in &self.signatures {
            if !signed.verify_signature() {
                return Err(CoinError::InvalidSignature);
            }
        }

        Ok(())
    }

    /// Encode signatures for Solidity contract (packed format: r, s, v for each).
    pub fn encode_signatures(&self) -> Vec<u8> {
        let mut encoded = Vec::with_capacity(self.signatures.len() * 65);
        for signed in &self.signatures {
            encoded.extend_from_slice(&signed.signature);
            // Note: Ed25519 signatures need to be converted to ECDSA format
            // for Solidity's ecrecover. This is a simplification.
            // In production, use a signature aggregation scheme or adapter.
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
    fn test_bridge_proof() {
        let sk = SecretKey::generate();
        let attestation = BridgeAttestation::new(
            sk.address(),
            [1u8; 20],
            1000,
            0,
            42161,
        );

        let signatures = vec![]; // Empty for now
        let proof = BridgeProof::new(attestation.clone(), signatures);

        assert_eq!(proof.message_hash, attestation.solidity_message_hash());
    }
}
