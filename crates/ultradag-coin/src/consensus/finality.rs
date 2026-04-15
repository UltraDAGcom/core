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
    /// Highest round number that has been finalized (for pruning).
    last_finalized_round: u64,
}

impl FinalityTracker {
    pub fn new(min_validators: usize) -> Self {
        Self {
            validators: ValidatorSet::new(min_validators),
            finalized: HashSet::new(),
            last_finalized_round: 0,
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

    /// Set the permissioned validator allowlist.
    /// When set, only addresses in this set can register as validators.
    pub fn set_allowed_validators(&mut self, addrs: std::collections::HashSet<crate::address::Address>) {
        self.validators.set_allowed_validators(addrs);
    }

    /// Number of known validators.
    pub fn validator_count(&self) -> usize {
        self.validators.len()
    }

    /// Access the underlying validator set.
    pub fn validator_set(&self) -> &ValidatorSet {
        &self.validators
    }

    /// Remove a validator from the set (e.g., after equivocation/slashing).
    /// This prevents slashed validators from inflating the quorum threshold.
    pub fn remove_validator(&mut self, addr: &Address) -> bool {
        self.validators.remove(addr)
    }

    /// Calculate the minimum number of validators needed for BFT finality.
    ///
    /// Static quorum based on configured validator count. Use this when
    /// `current_round` is unavailable (tests, standalone calls).
    pub fn finality_threshold(&self) -> usize {
        self.validators.quorum_threshold()
    }

    /// Adaptive finality threshold that uses the min of configured and
    /// active-producer count — lets the network heal when validators go
    /// offline (by proof of recent vertex production).
    pub fn finality_threshold_at(&self, current_round: u64) -> usize {
        self.validators.adaptive_quorum_threshold(current_round)
    }

    /// Record that `addr` produced a vertex at `round`. Feeds the adaptive
    /// quorum's liveness map.
    pub fn record_production(&mut self, addr: Address, round: u64) {
        self.validators.record_production(addr, round);
    }

    /// Check if a vertex is finalized.
    pub fn is_finalized(&self, hash: &[u8; 32]) -> bool {
        self.finalized.contains(hash)
    }

    /// Mark a vertex as finalized (used during reconciliation on startup).
    pub fn mark_as_finalized(&mut self, hash: [u8; 32]) {
        self.finalized.insert(hash);
    }

    /// Set the last finalized round (used during WAL replay).
    pub fn set_last_finalized_round(&mut self, round: u64) {
        if round > self.last_finalized_round {
            self.last_finalized_round = round;
        }
    }

    /// Evaluate finality for a vertex. Returns true if newly finalized.
    /// Uses O(1) precomputed descendant validator counts, filtered to active set.
    pub fn check_finality(&mut self, hash: &[u8; 32], dag: &BlockDag) -> bool {
        if self.finalized.contains(hash) {
            return false;
        }

        let threshold = self.finality_threshold_at(dag.current_round());
        if threshold == usize::MAX {
            return false;
        }

        let active = self.validators.active_addresses();
        if dag.descendant_validator_count_filtered(hash, active) >= threshold {
            self.finalized.insert(*hash);
            true
        } else {
            false
        }
    }

    /// Find all newly finalizable vertices in the DAG.
    /// Returns them in ancestor-first order (suitable for committing).
    ///
    /// Scans from `pruning_floor` to `current_round` to catch late-arriving
    /// vertices recovered via reconciliation. Vertices in already-finalized
    /// rounds are auto-finalized (they have no descendants but are safe since
    /// they passed signature + equivocation checks at insertion).
    /// Then forward-propagates through children for cascading finality.
    pub fn find_newly_finalized(&mut self, dag: &BlockDag) -> Vec<[u8; 32]> {
        // Populate the liveness map from the DAG tips so the adaptive quorum
        // reflects which validators are actually producing. This is cheap —
        // we only scan from the pruning floor to current round.
        let scan_from_liveness = dag.pruning_floor();
        let scan_to_liveness = dag.current_round();
        for round in scan_from_liveness..=scan_to_liveness {
            for hash in dag.hashes_in_round(round) {
                if let Some(v) = dag.get(hash) {
                    self.validators.record_production(v.validator, v.round);
                }
            }
        }

        // Use the adaptive threshold based on actually-producing validators.
        // Falls back to the static threshold if no liveness data is available.
        let threshold = self.finality_threshold_at(dag.current_round());
        if threshold == usize::MAX {
            return vec![];
        }

        let genesis: [u8; 32] = [0u8; 32];
        // Stuck parent threshold: parents >100 rounds behind last finalized round
        // are treated as finalized for liveness. See comment in parents_ok check.
        let stuck_threshold = self.last_finalized_round.saturating_sub(100);
        // Byzantine parents get a longer timeout (200 rounds) before auto-finalization.
        // This prevents a single equivocation from delaying finality for an entire
        // subgraph until pruning (~500 rounds). The 200-round delay is long enough to
        // ensure the equivocation is detected and slashed, but short enough to restore
        // liveness without waiting for pruning.
        let byzantine_stuck_threshold = self.last_finalized_round.saturating_sub(200);

        // Scan from pruning_floor (not last_finalized_round) so that late-arriving
        // vertices recovered via reconciliation are picked up. Most vertices between
        // pruning_floor and last_finalized_round are already in the `finalized` HashSet,
        // so the `continue` below makes this cheap (O(1) per already-finalized vertex).
        let scan_from = dag.pruning_floor();
        let scan_to = dag.current_round();

        let active = self.validators.active_addresses();
        let mut ready: BTreeSet<[u8; 32]> = BTreeSet::new();
        for round in scan_from..=scan_to {
            for hash in dag.hashes_in_round(round) {
                if self.finalized.contains(hash) {
                    continue;
                }
                if let Some(vertex) = dag.get(hash) {
                    // Parent is considered "ok" if any of:
                    // 1. It's the genesis sentinel hash, OR
                    // 2. It's in the finalized set, OR
                    // 3. It's been pruned from the DAG (pruned == deeply finalized), OR
                    // 4. It's a stuck parent: still in the DAG but >100 rounds behind
                    //    last_finalized_round. This prevents a finality liveness hole
                    //    where one bad parent (slashed/offline validator whose vertex
                    //    never gets 2f+1 descendants) blocks an entire subgraph from
                    //    finalizing indefinitely. 100 rounds (~8 min at 5s) is enough
                    //    time for descendants to propagate under normal conditions.
                    let parents_ok = vertex.parent_hashes.is_empty()
                        || vertex.parent_hashes.iter()
                            .all(|p| {
                                *p == genesis
                                    || self.finalized.contains(p)
                                    || dag.get(p).is_none()
                                    // Stuck parent escape: treat as finalized if >100 rounds behind,
                                    // BUT only if the parent's validator is NOT known-Byzantine.
                                    // A Byzantine validator's stuck vertex should never be auto-finalized.
                                    || dag.get(p).is_some_and(|pv| {
                                        let is_byz = dag.is_byzantine(&pv.validator);
                                        // Non-Byzantine: 100-round escape. Byzantine: 200-round escape.
                                        let round_threshold = if is_byz { byzantine_stuck_threshold } else { stuck_threshold };
                                        pv.round < round_threshold
                                    })
                            });

                    if !parents_ok {
                        continue;
                    }

                    // Only finalize vertices that have reached the proper BFT threshold.
                    // Late-arriving vertices (recovered via reconciliation) must still
                    // meet the 2f+1 descendant requirement — auto-finalizing without
                    // threshold would allow injection of malicious vertices.
                    // Filter by active set to avoid counting stale validators.
                    if dag.descendant_validator_count_filtered(hash, active) >= threshold {
                        ready.insert(*hash);
                    }
                }
            }
        }

        let mut newly_finalized = Vec::new();

        // Forward propagation: finalize ready vertices, then check their children
        while !ready.is_empty() {
            let batch: Vec<[u8; 32]> = ready.iter().copied().collect();
            ready.clear();

            for hash in &batch {
                if self.finalized.contains(hash) {
                    continue;
                }
                self.finalized.insert(*hash);
                newly_finalized.push(*hash);

                // Check children of this newly finalized vertex
                for child in dag.children_of(hash) {
                    if self.finalized.contains(&child) {
                        continue;
                    }
                    if dag.descendant_validator_count_filtered(&child, active) < threshold {
                        continue;
                    }
                    if let Some(vertex) = dag.get(&child) {
                        let parents_ok = vertex.parent_hashes.iter()
                            .all(|p| {
                                *p == genesis
                                    || self.finalized.contains(p)
                                    || dag.get(p).is_none()
                                    || dag.get(p).is_some_and(|pv| {
                                        let is_byz = dag.is_byzantine(&pv.validator);
                                        // Non-Byzantine: 100-round escape. Byzantine: 200-round escape.
                                        let round_threshold = if is_byz { byzantine_stuck_threshold } else { stuck_threshold };
                                        pv.round < round_threshold
                                    })
                            });
                        if parents_ok {
                            ready.insert(child);
                        }
                    }
                }
            }
        }

        // Sort by (round, hash) for deterministic ancestor-first ordering.
        newly_finalized.sort_by(|a, b| {
            let ra = dag.get(a).map(|v| v.round).unwrap_or(0);
            let rb = dag.get(b).map(|v| v.round).unwrap_or(0);
            ra.cmp(&rb).then_with(|| a.cmp(b))
        });

        // Update last_finalized_round to highest round in newly finalized vertices
        if let Some(last_hash) = newly_finalized.last() {
            if let Some(vertex) = dag.get(last_hash) {
                self.last_finalized_round = self.last_finalized_round.max(vertex.round);
            }
        }

        newly_finalized
    }

    /// Reset the tracker after a checkpoint fast-sync.
    /// Sets `last_finalized_round` and clears the finalized set (suffix vertices
    /// will be finalized normally going forward from the checkpoint round).
    pub fn reset_to_checkpoint(&mut self, checkpoint_round: u64) {
        self.last_finalized_round = checkpoint_round;
        self.finalized.clear();
    }

    /// Get the highest round number that has been finalized.
    pub fn last_finalized_round(&self) -> u64 {
        self.last_finalized_round
    }

    /// Prune old vertices from the DAG based on finalized rounds.
    /// Returns the number of vertices pruned.
    pub fn prune_dag(&self, dag: &mut BlockDag) -> usize {
        dag.prune_old_rounds(self.last_finalized_round)
    }

    /// Remove finalized hashes for vertices that have been pruned from the DAG.
    /// Call after `prune_dag()` to keep the finalized set bounded.
    pub fn prune_finalized(&mut self, dag: &BlockDag) {
        self.finalized.retain(|hash| dag.get(hash).is_some());
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
            last_finalized_round: self.last_finalized_round,
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
            last_finalized_round: snapshot.last_finalized_round,
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

        // Permissionless mode stays fail-closed even after registrations
        // (GHSA-rprp-wjrh-hx7g).
        ft.register_validator(SecretKey::generate().address());
        ft.register_validator(SecretKey::generate().address());
        assert_eq!(ft.finality_threshold(), usize::MAX);

        ft.set_configured_validators(3);
        assert_eq!(ft.finality_threshold(), 2);

        ft.set_configured_validators(4);
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
        ft.set_configured_validators(3);

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
        ft.set_configured_validators(3);

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
        ft.set_configured_validators(3);

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
