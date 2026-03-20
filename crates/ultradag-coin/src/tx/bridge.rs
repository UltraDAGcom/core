use serde::{Deserialize, Serialize};

use crate::address::{Address, Signature};
use crate::constants::NETWORK_ID;

/// Lock UDAG in bridge reserve for bridging to Arbitrum.
/// The locked amount is held in `StateEngine::bridge_reserve` and is not
/// spendable on the native chain until a corresponding BridgeMint releases it.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BridgeLockTx {
    pub from: Address,
    /// Ethereum/Arbitrum recipient address (20 bytes).
    pub arb_recipient: [u8; 20],
    pub amount: u64,
    pub fee: u64,
    pub nonce: u64,
    pub pub_key: [u8; 32],
    pub signature: Signature,
}

impl BridgeLockTx {
    /// The data that gets signed (everything except the signature).
    /// Includes NETWORK_ID for cross-network replay protection and
    /// type discriminator to prevent cross-type hash collisions.
    pub fn signable_bytes(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(128);
        buf.extend_from_slice(NETWORK_ID);
        buf.extend_from_slice(b"bridge_lock");
        buf.extend_from_slice(&self.from.0);
        buf.extend_from_slice(&self.arb_recipient);
        buf.extend_from_slice(&self.amount.to_le_bytes());
        buf.extend_from_slice(&self.fee.to_le_bytes());
        buf.extend_from_slice(&self.nonce.to_le_bytes());
        buf
    }

    /// Compute the transaction hash (unique identifier).
    pub fn hash(&self) -> [u8; 32] {
        let mut hasher = blake3::Hasher::new();
        hasher.update(b"bridge_lock");
        hasher.update(&self.from.0);
        hasher.update(&self.arb_recipient);
        hasher.update(&self.amount.to_le_bytes());
        hasher.update(&self.fee.to_le_bytes());
        hasher.update(&self.nonce.to_le_bytes());
        *hasher.finalize().as_bytes()
    }

    /// Verify that the Ed25519 signature is valid and the pub_key hashes to `from`.
    pub fn verify_signature(&self) -> bool {
        let expected_addr = Address::from_pubkey(&self.pub_key);
        if expected_addr != self.from {
            return false;
        }
        let Ok(vk) = ed25519_dalek::VerifyingKey::from_bytes(&self.pub_key) else {
            return false;
        };
        self.signature.verify(&vk, &self.signable_bytes())
    }

    /// Total cost to sender: locked amount + fee.
    pub fn total_cost(&self) -> u64 {
        self.amount.saturating_add(self.fee)
    }
}
