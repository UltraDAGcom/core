//! Bridge deposit transaction for locking funds on DAG for withdrawal on Arbitrum.

use serde::{Deserialize, Serialize};
use crate::address::{Address, Signature};
use crate::error::CoinError;

/// Bridge deposit transaction: lock UDAG on DAG for withdrawal on Arbitrum.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeDepositTx {
    /// Sender on DAG
    pub from: Address,
    /// Recipient on Arbitrum (20-byte address)
    pub recipient: [u8; 20],
    /// Amount to bridge (in sats)
    pub amount: u64,
    /// Destination chain ID (e.g., 42161 for Arbitrum)
    pub destination_chain_id: u64,
    /// Transaction nonce
    pub nonce: u64,
    /// Transaction fee (in sats)
    pub fee: u64,
    /// Sender's public key (32 bytes)
    pub pub_key: [u8; 32],
    /// Sender's signature
    pub signature: Signature,
}

impl BridgeDepositTx {
    /// Create signable bytes for this transaction.
    pub fn signable_bytes(&self) -> Vec<u8> {
        use crate::constants::NETWORK_ID;
        let mut buf = Vec::new();
        buf.extend_from_slice(NETWORK_ID);
        buf.extend_from_slice(b"bridge_deposit");
        buf.extend_from_slice(&self.from.0);
        buf.extend_from_slice(&self.recipient);
        buf.extend_from_slice(&self.amount.to_le_bytes());
        buf.extend_from_slice(&self.destination_chain_id.to_le_bytes());
        buf.extend_from_slice(&self.nonce.to_le_bytes());
        buf.extend_from_slice(&self.fee.to_le_bytes());
        buf.extend_from_slice(&self.pub_key);
        buf
    }

    /// Get total cost (amount + fee).
    pub fn total_cost(&self) -> u64 {
        self.amount.saturating_add(self.fee)
    }

    /// Verify the transaction signature.
    pub fn verify_signature(&self) -> bool {
        use ed25519_dalek::{Signature, Verifier, VerifyingKey};
        
        let message = self.signable_bytes();
        let sig = match Signature::from_slice(&self.signature.0) {
            Ok(s) => s,
            Err(_) => return false,
        };
        
        let vk = match VerifyingKey::from_bytes(&self.pub_key) {
            Ok(k) => k,
            Err(_) => return false,
        };
        
        vk.verify(&message, &sig).is_ok()
    }

    /// Verify the transaction is valid.
    pub fn verify(&self) -> Result<(), CoinError> {
        if self.recipient == [0u8; 20] {
            return Err(CoinError::ValidationError("Invalid recipient".into()));
        }
        if self.amount == 0 {
            return Err(CoinError::ValidationError("Amount cannot be zero".into()));
        }
        if !self.verify_signature() {
            return Err(CoinError::InvalidSignature);
        }
        Ok(())
    }

    /// Get the transaction hash.
    pub fn hash(&self) -> [u8; 32] {
        blake3::hash(&self.signable_bytes()).into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::address::SecretKey;

    #[test]
    fn test_bridge_deposit_tx() {
        let sk = SecretKey::generate();
        let mut tx = BridgeDepositTx {
            from: sk.address(),
            recipient: [1u8; 20],
            amount: 1000,
            destination_chain_id: 42161,
            fee: 10_000,
            nonce: 0,
            pub_key: sk.verifying_key().to_bytes(),
            signature: Signature([0u8; 64]),
        };
        
        // Sign
        tx.signature = sk.sign(&tx.signable_bytes());
        
        // Verify
        assert!(tx.verify_signature());
        assert!(tx.verify().is_ok());
        
        // Hash is deterministic
        let hash1 = tx.hash();
        let hash2 = tx.hash();
        assert_eq!(hash1, hash2);
    }

    #[test]
    fn test_invalid_recipient() {
        let sk = SecretKey::generate();
        let mut tx = BridgeDepositTx {
            from: sk.address(),
            recipient: [0u8; 20], // Invalid
            amount: 1000,
            destination_chain_id: 42161,
            fee: 10_000,
            nonce: 0,
            pub_key: sk.verifying_key().to_bytes(),
            signature: Signature([0u8; 64]),
        };
        tx.signature = sk.sign(&tx.signable_bytes());
        
        assert!(tx.verify().is_err());
    }

    #[test]
    fn test_zero_amount() {
        let sk = SecretKey::generate();
        let mut tx = BridgeDepositTx {
            from: sk.address(),
            recipient: [1u8; 20],
            amount: 0, // Invalid
            destination_chain_id: 42161,
            fee: 10_000,
            nonce: 0,
            pub_key: sk.verifying_key().to_bytes(),
            signature: Signature([0u8; 64]),
        };
        tx.signature = sk.sign(&tx.signable_bytes());
        
        assert!(tx.verify().is_err());
    }
}
