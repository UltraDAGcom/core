use serde::{Deserialize, Serialize};

use crate::address::{Address, Signature};
use crate::block::Block;

/// A vertex in the DAG-BFT consensus layer.
/// Wraps a block with DAG metadata: multiple parent references, round number,
/// and an Ed25519 signature from the proposing validator.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DagVertex {
    /// The underlying block.
    pub block: Block,
    /// Parent vertex hashes from the previous round(s).
    pub parent_hashes: Vec<[u8; 32]>,
    /// DAG round number. Vertices in the same round are created concurrently.
    pub round: u64,
    /// The validator who created this vertex.
    pub validator: Address,
    /// Validator's Ed25519 public key (for signature verification).
    pub pub_key: [u8; 32],
    /// Ed25519 signature over the vertex's signable bytes.
    pub signature: Signature,
    /// Pre-computed topological level: max(parent.topo_level) + 1.
    /// Used for O(N log N) deterministic ordering without ancestor traversal.
    /// Derived data — not included in hash() or signable_bytes().
    /// Skipped in serialization; recomputed on DAG insert.
    #[serde(skip)]
    pub topo_level: u64,
}

impl DagVertex {
    pub fn new(
        block: Block,
        parent_hashes: Vec<[u8; 32]>,
        round: u64,
        validator: Address,
        pub_key: [u8; 32],
        signature: Signature,
    ) -> Self {
        Self {
            block,
            parent_hashes,
            round,
            validator,
            pub_key,
            signature,
            topo_level: 0,
        }
    }

    /// Bytes that are signed by the validator.
    /// Includes network identifier to prevent cross-network replay attacks,
    /// and a type discriminator to prevent cross-type signature reuse
    /// (a vertex signature can never be valid for a transaction or checkpoint).
    pub fn signable_bytes(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(crate::constants::NETWORK_ID);
        buf.extend_from_slice(b"vertex");
        buf.extend_from_slice(&self.block.hash());
        // Parent count prefix added for defense-in-depth (March 2026 audit).
        // Breaking change — requires clean testnet restart.
        buf.extend_from_slice(&(self.parent_hashes.len() as u32).to_le_bytes());
        for parent in &self.parent_hashes {
            buf.extend_from_slice(parent);
        }
        buf.extend_from_slice(&self.round.to_le_bytes());
        buf.extend_from_slice(&self.validator.0);
        buf
    }

    /// Verify the Ed25519 signature on this vertex.
    pub fn verify_signature(&self) -> bool {
        use ed25519_dalek::VerifyingKey;
        let Ok(vk) = VerifyingKey::from_bytes(&self.pub_key) else {
            return false;
        };
        // Check pub_key matches validator address
        let expected_addr = Address::from_pubkey(&self.pub_key);
        if expected_addr != self.validator {
            return false;
        }
        let sig = ed25519_dalek::Signature::from_bytes(&self.signature.0);
        vk.verify_strict(&self.signable_bytes(), &sig).is_ok()
    }

    /// Verify that the vertex timestamp is within acceptable bounds.
    /// Rejects vertices with timestamps too far in the past or future.
    ///
    /// Validation rules:
    /// - Timestamp must not be older than MAX_TIMESTAMP_AGE_SECS (5 minutes)
    /// - Timestamp must not be more than MAX_TIMESTAMP_FUTURE_SECS (1 minute) in future
    ///
    /// This prevents timestamp manipulation attacks while allowing for clock skew.
    ///
    /// Note: When compiled with the `simulator` feature, timestamp validation is disabled
    /// to allow deterministic simulation with fixed genesis timestamps.
    pub fn verify_timestamp(&self, current_time: i64) -> bool {
        // In simulator mode, skip timestamp validation to allow deterministic timestamps
        #[cfg(feature = "simulator")]
        {
            let _ = current_time; // Suppress unused warning
            return true;
        }
        
        let vertex_time = self.block.header.timestamp;

        // Reject timestamps too old (prevents replay attacks with old vertices)
        let min_valid_time = current_time - crate::consensus::dag::MAX_TIMESTAMP_AGE_SECS;

        // Reject timestamps too far in future (prevents timestamp manipulation)
        let max_valid_time = current_time + crate::consensus::dag::MAX_TIMESTAMP_FUTURE_SECS;

        vertex_time >= min_valid_time && vertex_time <= max_valid_time
    }

    /// The hash of this vertex covers all semantic fields:
    /// block hash + round + validator + parent hashes.
    pub fn hash(&self) -> [u8; 32] {
        let mut buf = Vec::new();
        buf.extend_from_slice(&self.block.hash());
        buf.extend_from_slice(&self.round.to_le_bytes());
        buf.extend_from_slice(&self.validator.0);
        // Parent count prefix added for defense-in-depth (March 2026 audit).
        // Breaking change — requires clean testnet restart.
        buf.extend_from_slice(&(self.parent_hashes.len() as u32).to_le_bytes());
        for parent in &self.parent_hashes {
            buf.extend_from_slice(parent);
        }
        *blake3::hash(&buf).as_bytes()
    }

    pub fn height(&self) -> u64 {
        self.block.height()
    }
}

#[cfg(test)]
fn make_signed_vertex_with_parents(round: u64, parents: Vec<[u8; 32]>, sk: &crate::address::SecretKey) -> DagVertex {
    use crate::block::{Block, BlockHeader};
    use crate::tx::CoinbaseTx;
    
    let validator = sk.address();
    let block = Block {
        header: BlockHeader {
            version: 1,
            height: round,
            timestamp: 1_000_000 + round as i64,
            prev_hash: parents.first().copied().unwrap_or([0u8; 32]),
            merkle_root: [0u8; 32],
        },
        coinbase: CoinbaseTx {
            to: validator,
            amount: 5_000_000_000,
            height: round,
        },
        transactions: vec![],
    };
    let mut v = DagVertex::new(
        block,
        parents,
        round,
        validator,
        sk.verifying_key().to_bytes(),
        Signature([0u8; 64]),
    );
    v.signature = sk.sign(&v.signable_bytes());
    v
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::address::SecretKey;
    use crate::block::header::BlockHeader;
    use crate::tx::CoinbaseTx;

    fn make_signed_vertex(round: u64, parents: Vec<[u8; 32]>) -> DagVertex {
        let sk = SecretKey::generate();
        let addr = sk.address();
        let coinbase = CoinbaseTx {
            to: addr,
            amount: 5_000_000_000,
            height: 0,
        };
        let block = Block {
            header: BlockHeader {
                version: 1,
                height: 0,
                timestamp: 1_000_000,
                prev_hash: parents.first().copied().unwrap_or([0u8; 32]),
                merkle_root: [0u8; 32],
            },
            coinbase,
            transactions: vec![],
        };
        let mut v = DagVertex {
            block,
            parent_hashes: parents,
            round,
            validator: addr,
            pub_key: sk.verifying_key().to_bytes(),
            signature: Signature([0u8; 64]),
            topo_level: 0,
        };
        v.signature = sk.sign(&v.signable_bytes());
        v
    }

    #[test]
    fn vertex_hash_covers_all_fields() {
        let v = make_signed_vertex(0, vec![[0u8; 32]]);
        // Hash includes block hash + round + validator + parents, not just block hash
        assert_ne!(v.hash(), v.block.hash());
        // Hash should be deterministic
        assert_eq!(v.hash(), v.hash());
    }

    #[test]
    fn vertex_stores_parents() {
        let p1 = [1u8; 32];
        let p2 = [2u8; 32];
        let v = make_signed_vertex(1, vec![p1, p2]);
        assert_eq!(v.parent_hashes.len(), 2);
        assert_eq!(v.parent_hashes[0], p1);
        assert_eq!(v.parent_hashes[1], p2);
    }

    #[test]
    fn vertex_round() {
        let v = make_signed_vertex(5, vec![[0u8; 32]]);
        assert_eq!(v.round, 5);
    }

    #[test]
    fn vertex_signature_valid() {
        let v = make_signed_vertex(0, vec![[0u8; 32]]);
        assert!(v.verify_signature());
    }

    #[test]
    fn vertex_signature_rejects_tampered_round() {
        let mut v = make_signed_vertex(0, vec![[0u8; 32]]);
        v.round = 999;
        assert!(!v.verify_signature());
    }

    #[test]
    fn vertex_signature_rejects_wrong_key() {
        let mut v = make_signed_vertex(0, vec![[0u8; 32]]);
        let other_sk = SecretKey::generate();
        v.pub_key = other_sk.verifying_key().to_bytes();
        assert!(!v.verify_signature());
    }

    #[test]
    fn parent_hash_order_affects_signable_bytes() {
        // Parent hash order DOES affect signable_bytes (they're concatenated in order)
        // This is CORRECT - different parent orderings should produce different signatures
        let sk = SecretKey::generate();
        let p1 = [1u8; 32];
        let p2 = [2u8; 32];
        
        // Create vertices with same block content but different parent orders
        let mut v1 = make_signed_vertex_with_parents(0, vec![p1, p2], &sk);
        let mut v2 = make_signed_vertex_with_parents(0, vec![p2, p1], &sk);
        
        // Make blocks identical by using same prev_hash (otherwise block hash differs)
        v1.block.header.prev_hash = [0u8; 32];
        v2.block.header.prev_hash = [0u8; 32];
        
        // Different parent order = different signable bytes
        assert_ne!(v1.signable_bytes(), v2.signable_bytes(), 
            "Parent order affects signable bytes (correct - prevents signature reuse)");
        
        // Vertex hash now covers parents too, so different parent order = different hash
        assert_ne!(v1.hash(), v2.hash(),
            "Different parent order produces different vertex hash");
    }

    #[test]
    fn timestamp_validation_accepts_past_timestamps() {
        let v = make_signed_vertex(0, vec![]);
        // Vertex timestamp is 1_000_000
        // Allow timestamps up to 5 minutes (300 seconds) in the past
        let current_time = 1_000_200; // 200 seconds after vertex timestamp (within 5 min window)
        assert!(v.verify_timestamp(current_time), "Recent past timestamps should be accepted");
        
        // Old timestamps should be rejected
        let old_current_time = 1_500_000; // 500_000 seconds after vertex timestamp (way too old)
        assert!(!v.verify_timestamp(old_current_time), "Very old timestamps should be rejected");
    }

    #[test]
    fn timestamp_validation_accepts_near_future() {
        let sk = SecretKey::generate();
        let mut v = make_signed_vertex_with_parents(0, vec![], &sk);
        let current_time = 1_000_000;
        // Set timestamp 30 seconds in future (within 60 second tolerance)
        v.block.header.timestamp = current_time + 30;
        assert!(v.verify_timestamp(current_time), "Near-future timestamps within tolerance should be accepted");
    }

    #[test]
    fn timestamp_validation_rejects_far_future() {
        let sk = SecretKey::generate();
        let mut v = make_signed_vertex_with_parents(0, vec![], &sk);
        let current_time = 1_000_000;
        // Set timestamp 120 seconds in future (exceeds 60 second tolerance)
        v.block.header.timestamp = current_time + 120;
        assert!(!v.verify_timestamp(current_time), "Far-future timestamps should be rejected");
    }

    #[test]
    fn timestamp_validation_boundary_exact() {
        let sk = SecretKey::generate();
        let mut v = make_signed_vertex_with_parents(0, vec![], &sk);
        let current_time = 1_000_000;
        // Set timestamp exactly at MAX_TIMESTAMP_FUTURE_SECS boundary (60 seconds)
        v.block.header.timestamp = current_time + crate::consensus::dag::MAX_TIMESTAMP_FUTURE_SECS;
        assert!(v.verify_timestamp(current_time), "Timestamp exactly at boundary should be accepted");
    }

    #[test]
    fn timestamp_validation_boundary_plus_one() {
        let sk = SecretKey::generate();
        let mut v = make_signed_vertex_with_parents(0, vec![], &sk);
        let current_time = 1_000_000;
        // Set timestamp one second beyond MAX_TIMESTAMP_FUTURE_SECS boundary
        v.block.header.timestamp = current_time + crate::consensus::dag::MAX_TIMESTAMP_FUTURE_SECS + 1;
        assert!(!v.verify_timestamp(current_time), "Timestamp beyond boundary should be rejected");
    }

    #[test]
    fn timestamp_validation_rejects_old_timestamps() {
        let sk = SecretKey::generate();
        let mut v = make_signed_vertex_with_parents(0, vec![], &sk);
        let current_time = 1_000_000;
        // Set timestamp 400 seconds in the past (exceeds 300 second tolerance)
        v.block.header.timestamp = current_time - 400;
        assert!(!v.verify_timestamp(current_time), "Old timestamps beyond tolerance should be rejected");
    }

    #[test]
    fn timestamp_validation_accepts_recent_past() {
        let sk = SecretKey::generate();
        let mut v = make_signed_vertex_with_parents(0, vec![], &sk);
        let current_time = 1_000_000;
        // Set timestamp 200 seconds in the past (within 300 second tolerance)
        v.block.header.timestamp = current_time - 200;
        assert!(v.verify_timestamp(current_time), "Recent past timestamps should be accepted");
    }
}
