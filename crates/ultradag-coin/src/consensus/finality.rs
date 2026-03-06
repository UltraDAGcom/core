use std::collections::{BTreeSet, HashSet};

use crate::address::Address;
use crate::consensus::dag::BlockDag;
use crate::consensus::validator_set::ValidatorSet;

/// Tracks BFT finality for DAG vertices.
pub struct FinalityTracker {
    /// The validator set (owns threshold logic).
    validators: ValidatorSet,
    /// Vertex hashes that have been finalized.
    finalized: HashSet<[u8; 32]>,
}

impl FinalityTracker {
    pub fn new(min_validators: usize) -> Self {
        Self {
            validators: ValidatorSet::new(min_validators),
            finalized: HashSet::new(),
        }
    }

    /// Register a validator as active.
    pub fn register_validator(&mut self, addr: Address) {
        self.validators.register(addr);
    }

    /// Set the expected validator count for quorum calculations.
    pub fn set_configured_validators(&mut self, count: usize) {
        self.validators.set_configured_validators(count);
    }

    /// Number of known validators.
    pub fn validator_count(&self) -> usize {
        self.validators.len()
    }

    /// Access the underlying validator set.
    pub fn validator_set(&self) -> &ValidatorSet {
        &self.validators
    }

    /// Calculate the minimum number of validators needed for BFT finality.
    pub fn finality_threshold(&self) -> usize {
        self.validators.quorum_threshold()
    }

    /// Check if a vertex is finalized.
    pub fn is_finalized(&self, hash: &[u8; 32]) -> bool {
        self.finalized.contains(hash)
    }

    /// Evaluate finality for a vertex. Returns true if newly finalized.
    pub fn check_finality(&mut self, hash: &[u8; 32], dag: &BlockDag) -> bool {
        if self.finalized.contains(hash) {
            return false;
        }

        let threshold = self.finality_threshold();
        if threshold == usize::MAX {
            return false;
        }

        let descendants = dag.descendants(hash);
        let descendant_validators = dag.distinct_validators(&descendants);

        if descendant_validators.len() >= threshold {
            self.finalized.insert(*hash);
            true
        } else {
            false
        }
    }

    /// Find all newly finalizable vertices in the DAG.
    /// Returns them in ancestor-first order (suitable for committing).
    pub fn find_newly_finalized(&mut self, dag: &BlockDag) -> Vec<[u8; 32]> {
        let threshold = self.finality_threshold();
        if threshold == usize::MAX {
            return vec![];
        }

        // CRITICAL: Use BTreeSet for deterministic iteration order
        // HashSet iteration is randomized per process, causing non-deterministic finality ordering
        let candidates: Vec<[u8; 32]> = dag
            .tips()
            .iter()
            .flat_map(|tip| {
                let mut all = dag.ancestors(tip);
                all.insert(*tip);
                all
            })
            .filter(|h| !self.finalized.contains(h))
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();

        let mut newly_finalized = Vec::new();
        for hash in &candidates {
            // CRITICAL: Parent finality guarantee
            // Only finalize a vertex if all its parents are already finalized
            // Genesis vertices (no parents) can always be finalized
            if let Some(vertex) = dag.get(hash) {
                if !vertex.parent_hashes.is_empty() {
                    let all_parents_finalized = vertex.parent_hashes.iter()
                        .all(|parent| self.finalized.contains(parent));
                    
                    if !all_parents_finalized {
                        // Skip this vertex - will retry when parents are finalized
                        continue;
                    }
                }
            }
            
            let descendants = dag.descendants(hash);
            let validators = dag.distinct_validators(&descendants);
            if validators.len() >= threshold {
                self.finalized.insert(*hash);
                newly_finalized.push(*hash);
            }
        }

        newly_finalized.sort_by(|a, b| {
            if dag.is_ancestor(a, b) {
                std::cmp::Ordering::Less
            } else if dag.is_ancestor(b, a) {
                std::cmp::Ordering::Greater
            } else {
                a.cmp(b)
            }
        });

        newly_finalized
    }

    /// Number of finalized vertices.
    pub fn finalized_count(&self) -> usize {
        self.finalized.len()
    }

    /// Save finality state to disk
    pub fn save(&self, path: &std::path::Path) -> Result<(), crate::persistence::PersistenceError> {
        let snapshot = crate::consensus::persistence::FinalitySnapshot {
            finalized: self.finalized.iter().copied().collect(),
            validators: self.validators.validators(),
            min_validators: self.validators.min_validators(),
        };
        snapshot.save(path)
    }

    /// Load finality state from disk.
    /// NOTE: Validators are NOT restored from snapshot — they must be rebuilt
    /// from the DAG after loading to prevent stale validator registrations
    /// from inflating the quorum threshold.
    pub fn load(path: &std::path::Path, _min_validators: usize) -> Result<Self, crate::persistence::PersistenceError> {
        let snapshot = crate::consensus::persistence::FinalitySnapshot::load(path)?;
        let validators = ValidatorSet::new(snapshot.min_validators);
        Ok(Self {
            validators,
            finalized: snapshot.finalized.into_iter().collect(),
        })
    }

    /// Check if saved state exists
    pub fn exists(path: &std::path::Path) -> bool {
        crate::consensus::persistence::FinalitySnapshot::exists(path)
    }
}

impl Default for FinalityTracker {
    fn default() -> Self {
        Self::new(3)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::address::{SecretKey, Signature};
    use crate::block::header::BlockHeader;
    use crate::block::Block;
    use crate::consensus::vertex::DagVertex;
    use crate::tx::CoinbaseTx;

    fn make_vertex(
        nonce: u64,
        round: u64,
        parents: Vec<[u8; 32]>,
        sk: &SecretKey,
    ) -> DagVertex {
        let validator = sk.address();
        let block = Block {
            header: BlockHeader {
                version: 1,
                height: 0,
                timestamp: 1_000_000 + nonce as i64,
                prev_hash: parents.first().copied().unwrap_or([0u8; 32]),
                merkle_root: [0u8; 32],
            },
            coinbase: CoinbaseTx {
                to: validator,
                amount: 5_000_000_000,
                height: 0,
            },
            transactions: vec![],
        };
        let mut v = DagVertex::new(block, parents, round, validator, sk.verifying_key().to_bytes(), Signature([0u8; 64]));
        v.signature = sk.sign(&v.signable_bytes());
        v
    }

    #[test]
    fn threshold_calculation() {
        let mut ft = FinalityTracker::new(3);
        assert_eq!(ft.finality_threshold(), usize::MAX);

        ft.register_validator(SecretKey::generate().address());
        ft.register_validator(SecretKey::generate().address());
        assert_eq!(ft.finality_threshold(), usize::MAX);

        ft.register_validator(SecretKey::generate().address());
        assert_eq!(ft.finality_threshold(), 2);

        ft.register_validator(SecretKey::generate().address());
        assert_eq!(ft.finality_threshold(), 3);
    }

    #[test]
    fn vertex_not_finalized_initially() {
        let ft = FinalityTracker::new(1);
        assert!(!ft.is_finalized(&[0u8; 32]));
    }

    #[test]
    fn finality_with_three_validators() {
        let mut dag = BlockDag::new();
        let mut ft = FinalityTracker::new(2);

        let sk1 = SecretKey::generate();
        let sk2 = SecretKey::generate();
        let sk3 = SecretKey::generate();

        ft.register_validator(sk1.address());
        ft.register_validator(sk2.address());
        ft.register_validator(sk3.address());

        let v1 = make_vertex(1, 0, vec![], &sk1);
        let h1 = v1.hash();
        dag.insert(v1);
        assert!(!ft.check_finality(&h1, &dag));

        let v2 = make_vertex(2, 1, vec![h1], &sk2);
        let _h2 = v2.hash();
        dag.insert(v2);
        assert!(!ft.check_finality(&h1, &dag));

        let v3 = make_vertex(3, 1, vec![h1], &sk3);
        dag.insert(v3);

        assert!(ft.check_finality(&h1, &dag));
        assert!(ft.is_finalized(&h1));
        assert!(!ft.check_finality(&h1, &dag));
    }

    #[test]
    fn transitive_descendant_counts() {
        let mut dag = BlockDag::new();
        let mut ft = FinalityTracker::new(2);

        let sk1 = SecretKey::generate();
        let sk2 = SecretKey::generate();
        let sk3 = SecretKey::generate();

        ft.register_validator(sk1.address());
        ft.register_validator(sk2.address());
        ft.register_validator(sk3.address());

        let v1 = make_vertex(1, 0, vec![], &sk1);
        let h1 = v1.hash();
        dag.insert(v1);

        let v2 = make_vertex(2, 1, vec![h1], &sk2);
        let h2 = v2.hash();
        dag.insert(v2);

        let v3 = make_vertex(3, 2, vec![h2], &sk3);
        dag.insert(v3);

        assert!(ft.check_finality(&h1, &dag));
        assert!(!ft.check_finality(&h2, &dag));
    }

    #[test]
    fn find_newly_finalized_batch() {
        let mut dag = BlockDag::new();
        let mut ft = FinalityTracker::new(2);

        let sk1 = SecretKey::generate();
        let sk2 = SecretKey::generate();
        let sk3 = SecretKey::generate();

        ft.register_validator(sk1.address());
        ft.register_validator(sk2.address());
        ft.register_validator(sk3.address());

        let v1 = make_vertex(1, 0, vec![], &sk1);
        let h1 = v1.hash();
        dag.insert(v1);

        let v2 = make_vertex(2, 1, vec![h1], &sk2);
        let h2 = v2.hash();
        dag.insert(v2);

        let v3 = make_vertex(3, 2, vec![h2], &sk3);
        let h3 = v3.hash();
        dag.insert(v3);

        let v4 = make_vertex(4, 3, vec![h3], &sk1);
        let h4 = v4.hash();
        dag.insert(v4);

        let v5 = make_vertex(5, 4, vec![h4], &sk2);
        dag.insert(v5);

        // Run finalization passes until all vertices are finalized
        // Parent finality guarantee requires multiple passes
        loop {
            let newly_finalized = ft.find_newly_finalized(&dag);
            if newly_finalized.is_empty() {
                break;
            }
        }
        
        assert!(ft.is_finalized(&h1));
        assert!(ft.is_finalized(&h2));
        assert!(ft.is_finalized(&h3));
    }
}
