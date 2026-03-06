use serde::{Deserialize, Serialize};

/// Block header — the hashable part of a block.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockHeader {
    pub version: u32,
    pub height: u64,
    pub timestamp: i64,
    pub prev_hash: [u8; 32],
    pub merkle_root: [u8; 32],
}

impl BlockHeader {
    /// Compute the block hash (Blake3 of the serialized header).
    pub fn hash(&self) -> [u8; 32] {
        let mut hasher = blake3::Hasher::new();
        hasher.update(&self.version.to_le_bytes());
        hasher.update(&self.height.to_le_bytes());
        hasher.update(&self.timestamp.to_le_bytes());
        hasher.update(&self.prev_hash);
        hasher.update(&self.merkle_root);
        *hasher.finalize().as_bytes()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_header(height: u64) -> BlockHeader {
        BlockHeader {
            version: 1,
            height,
            timestamp: 1_000_000,
            prev_hash: [0u8; 32],
            merkle_root: [0u8; 32],
        }
    }

    #[test]
    fn hash_is_deterministic() {
        let h = make_header(0);
        assert_eq!(h.hash(), h.hash());
    }

    #[test]
    fn different_heights_give_different_hashes() {
        let h1 = make_header(0);
        let h2 = make_header(1);
        assert_ne!(h1.hash(), h2.hash());
    }

    #[test]
    fn different_prev_hashes_give_different_hashes() {
        let mut h1 = make_header(0);
        let mut h2 = make_header(0);
        h1.prev_hash = [0u8; 32];
        h2.prev_hash = [1u8; 32];
        assert_ne!(h1.hash(), h2.hash());
    }
}
