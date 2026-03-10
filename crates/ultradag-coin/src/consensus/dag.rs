use std::collections::{HashMap, HashSet, VecDeque};

use serde::{Serialize, Deserialize};

use crate::address::Address;
use crate::consensus::vertex::DagVertex;

/// Error returned when a DAG insert is rejected.
#[derive(Debug, PartialEq)]
pub enum DagInsertError {
    /// Validator produced two different vertices for the same round.
    Equivocation { validator: Address, round: u64 },
    /// Vertex references parent hashes that do not exist in the DAG.
    MissingParents(Vec<[u8; 32]>),
    /// Vertex has too many parent references.
    TooManyParents,
}

/// Maximum number of parent references allowed per DagVertex.
pub const MAX_PARENTS: usize = 64;

/// Target number of parents per vertex for partial parent selection.
/// Each validator references K random tips instead of all tips, enabling
/// unlimited validator scaling (removes N=64 ceiling). Follows Narwhal approach.
/// K=32 provides strong DAG connectivity while keeping parent count manageable.
pub const K_PARENTS: usize = 32;

/// Number of rounds to keep in memory before pruning older finalized vertices.
/// Keeps last 1000 rounds = ~7 days at 10-second rounds.
pub const PRUNING_HORIZON: u64 = 1000;

/// Equivocation evidence stored permanently, separate from the prunable DAG.
/// This ensures slashing proofs survive even after the relevant vertices are pruned.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EquivocationEvidence {
    pub validator: Address,
    pub round: u64,
    pub vertex_hash_1: [u8; 32],
    pub vertex_hash_2: [u8; 32],
    pub detected_at_round: u64,
}

/// The DAG data structure for DAG-BFT consensus.
/// Stores vertices (blocks with DAG metadata) and tracks the DAG topology.
/// 
/// # Pruning
/// To prevent unbounded memory growth, the DAG prunes vertices from rounds older than
/// `last_finalized_round - PRUNING_HORIZON`. Pruned vertices are removed from all data structures.
/// New nodes sync from the pruned state via snapshots.
/// 
/// # Equivocation Evidence
/// Evidence of Byzantine behavior is stored permanently in `evidence_store`, separate from
/// the prunable DAG vertices. This ensures slashing proofs remain available indefinitely.
pub struct BlockDag {
    /// All vertices by hash.
    vertices: HashMap<[u8; 32], DagVertex>,
    /// Children of each vertex (reverse edges).
    children: HashMap<[u8; 32], HashSet<[u8; 32]>>,
    /// Current tip hashes (vertices with no children).
    tips: HashSet<[u8; 32]>,
    /// Vertices grouped by round.
    rounds: HashMap<u64, Vec<[u8; 32]>>,
    /// Current round number.
    current_round: u64,
    /// Byzantine validators detected via equivocation.
    byzantine_validators: HashSet<Address>,
    /// Equivocation evidence: (validator, round) -> [vertex1_hash, vertex2_hash]
    /// NOTE: This is the old temporary store. Use evidence_store for permanent retention.
    equivocation_evidence: HashMap<(Address, u64), [[u8; 32]; 2]>,
    /// Permanent equivocation evidence store (survives pruning).
    /// Multiple equivocations per validator are tracked.
    evidence_store: HashMap<Address, Vec<EquivocationEvidence>>,
    /// Rejected equivocation vertices stored for evidence broadcasting.
    /// These vertices were NOT inserted into the DAG but are needed to prove equivocation.
    equivocation_vertices: HashMap<[u8; 32], DagVertex>,
    /// Incremental descendant validator tracking for O(1) finality checks.
    /// Maps vertex hash -> set of distinct validators that have at least one descendant.
    descendant_validators: HashMap<[u8; 32], HashSet<Address>>,
    /// Earliest round still kept in memory (for pruning tracking).
    /// Vertices from rounds < pruning_floor have been pruned.
    pruning_floor: u64,
}

impl BlockDag {
    pub fn new() -> Self {
        Self {
            vertices: HashMap::new(),
            children: HashMap::new(),
            tips: HashSet::new(),
            rounds: HashMap::new(),
            current_round: 0,
            byzantine_validators: HashSet::new(),
            equivocation_evidence: HashMap::new(),
            evidence_store: HashMap::new(),
            equivocation_vertices: HashMap::new(),
            descendant_validators: HashMap::new(),
            pruning_floor: 0,
        }
    }

    /// Insert a vertex into the DAG. Returns false if already present.
    /// Does NOT check equivocation — use `try_insert` for untrusted vertices.
    pub fn insert(&mut self, vertex: DagVertex) -> bool {
        let hash = vertex.hash();

        if self.vertices.contains_key(&hash) {
            return false;
        }

        // CRITICAL: Verify all parents exist before inserting.
        // The zero hash [0u8; 32] is the sentinel genesis parent for round-1 vertices.
        let genesis_parent: [u8; 32] = [0u8; 32];
        for parent in &vertex.parent_hashes {
            if *parent != genesis_parent && !self.vertices.contains_key(parent) {
                // Reject vertex with non-existent parent to prevent DAG corruption
                return false;
            }
        }

        // CRITICAL: Reject vertices claiming rounds too far in the future
        // Prevents memory exhaustion from future-round flooding
        const MAX_FUTURE_ROUNDS: u64 = 10;
        if vertex.round > self.current_round + MAX_FUTURE_ROUNDS {
            return false;
        }

        // Compute topo_level: max(parent.topo_level) + 1
        let mut max_parent_level: u64 = 0;
        let genesis_topo: [u8; 32] = [0u8; 32];
        for parent in &vertex.parent_hashes {
            if *parent != genesis_topo {
                if let Some(pv) = self.vertices.get(parent) {
                    max_parent_level = max_parent_level.max(pv.topo_level);
                }
            }
        }
        let mut vertex = vertex;
        vertex.topo_level = if vertex.parent_hashes.is_empty() || (vertex.parent_hashes.len() == 1 && vertex.parent_hashes[0] == genesis_topo) {
            0
        } else {
            max_parent_level + 1
        };

        // Update parent -> child edges
        for parent in &vertex.parent_hashes {
            self.children
                .entry(*parent)
                .or_default()
                .insert(hash);
            // Parent is no longer a tip
            self.tips.remove(parent);
        }

        // This vertex is a new tip
        self.tips.insert(hash);

        // Track by round
        let round = vertex.round;
        self.rounds.entry(round).or_default().push(hash);

        if round > self.current_round {
            self.current_round = round;
        }

        // Update incremental descendant validator counts.
        // Walk upward through ancestors; stop early when the validator is already tracked.
        let validator = vertex.validator;
        self.descendant_validators.entry(hash).or_default();
        let mut queue = VecDeque::new();
        for parent in &vertex.parent_hashes {
            queue.push_back(*parent);
        }
        let mut visited = HashSet::new();
        let genesis: [u8; 32] = [0u8; 32];
        while let Some(ancestor) = queue.pop_front() {
            if ancestor == genesis || !visited.insert(ancestor) {
                continue;
            }
            let set = self.descendant_validators.entry(ancestor).or_default();
            if !set.insert(validator) {
                // Already tracked — all further ancestors already have this validator
                continue;
            }
            if let Some(v) = self.vertices.get(&ancestor) {
                for p in &v.parent_hashes {
                    if !visited.contains(p) {
                        queue.push_back(*p);
                    }
                }
            }
        }

        self.vertices.insert(hash, vertex);
        true
    }

    /// Try to insert a vertex. Checks for equivocation and Byzantine validators.
    /// Returns:
    /// - `Ok(true)` if inserted successfully
    /// - `Ok(false)` if duplicate hash (already present)
    /// - `Err(reason)` if equivocation detected (same validator + round, different hash)
    pub fn try_insert(&mut self, vertex: DagVertex) -> Result<bool, DagInsertError> {
        let hash = vertex.hash();

        if self.vertices.contains_key(&hash) {
            return Ok(false);
        }

        // Reject vertices with too many parents
        if vertex.parent_hashes.len() > MAX_PARENTS {
            return Err(DagInsertError::TooManyParents);
        }

        // Reject vertices from Byzantine validators
        if self.is_byzantine(&vertex.validator) {
            return Ok(false);
        }

        // Reject vertices claiming rounds too far in the future
        const MAX_FUTURE_ROUNDS: u64 = 10;
        if vertex.round > self.current_round + MAX_FUTURE_ROUNDS {
            return Ok(false);
        }

        // Reject vertices with timestamps too far in the future
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        if !vertex.verify_timestamp(now) {
            return Ok(false); // Timestamp too far in future
        }

        // Equivocation: same validator, same round, different vertex
        if let Some(existing_hash) = self.rounds
            .get(&vertex.round)
            .and_then(|hashes| {
                hashes.iter()
                    .find(|&&h| {
                        self.vertices.get(&h)
                            .map(|v| v.validator == vertex.validator)
                            .unwrap_or(false)
                    })
                    .copied()
            })
        {
            // Store equivocation evidence and mark as Byzantine.
            // Also store the rejected vertex so evidence can be broadcast
            // (the vertex is NOT inserted into the DAG, so dag.get(hash) would fail).
            self.equivocation_vertices.insert(hash, vertex.clone());
            self.store_equivocation_evidence(
                vertex.validator,
                vertex.round,
                existing_hash,
                hash,
            );
            self.mark_byzantine(vertex.validator);

            return Err(DagInsertError::Equivocation {
                validator: vertex.validator,
                round: vertex.round,
            });
        }

        // Check for missing parents before inserting
        let genesis_parent: [u8; 32] = [0u8; 32];
        let missing: Vec<[u8; 32]> = vertex.parent_hashes
            .iter()
            .filter(|h| **h != genesis_parent && !self.vertices.contains_key(*h))
            .copied()
            .collect();

        if !missing.is_empty() {
            return Err(DagInsertError::MissingParents(missing));
        }

        // All checks passed — insert
        Ok(self.insert(vertex))
    }

    /// Get a vertex by hash.
    pub fn get(&self, hash: &[u8; 32]) -> Option<&DagVertex> {
        self.vertices.get(hash)
    }

    /// Get a vertex by hash, also checking rejected equivocation vertices.
    /// Use this when looking up vertices for equivocation evidence broadcasting.
    pub fn get_including_equivocations(&self, hash: &[u8; 32]) -> Option<&DagVertex> {
        self.vertices.get(hash).or_else(|| self.equivocation_vertices.get(hash))
    }

    /// Get all current tip hashes.
    pub fn tips(&self) -> Vec<[u8; 32]> {
        self.tips.iter().copied().collect()
    }

    /// Select K parents from available tips using deterministic sampling.
    /// If tips.len() <= K, returns all tips. Otherwise, selects K tips deterministically
    /// based on the proposer's address (for reproducibility in tests/verification).
    /// 
    /// This enables unlimited validator scaling by keeping parent count bounded at K
    /// regardless of the number of validators N. Follows Narwhal's approach.
    pub fn select_parents(&self, proposer: &Address, k: usize) -> Vec<[u8; 32]> {
        let tips = self.tips();
        
        // If we have K or fewer tips, use all of them
        if tips.len() <= k {
            return tips;
        }
        
        // Deterministic sampling: use proposer address as seed for reproducibility
        // XOR proposer address with each tip hash to create a score, then take top K
        let mut scored_tips: Vec<([u8; 32], u64)> = tips
            .iter()
            .map(|tip| {
                // Simple deterministic score: XOR first 8 bytes of proposer with tip
                let mut score = 0u64;
                for i in 0..8 {
                    score ^= (proposer.0[i] as u64) << (i * 8);
                    score ^= (tip[i] as u64) << (i * 8);
                }
                (*tip, score)
            })
            .collect();
        
        // Sort by score (deterministic) and take top K
        scored_tips.sort_by_key(|(_, score)| *score);
        scored_tips.truncate(k);
        
        scored_tips.into_iter().map(|(tip, _)| tip).collect()
    }

    /// Get all vertices in a given round.
    /// Get vertex hashes in a given round (lightweight — no vertex cloning).
    pub fn hashes_in_round(&self, round: u64) -> &[[u8; 32]] {
        self.rounds.get(&round).map(|v| v.as_slice()).unwrap_or(&[])
    }

    pub fn vertices_in_round(&self, round: u64) -> Vec<&DagVertex> {
        self.rounds
            .get(&round)
            .map(|hashes| {
                hashes
                    .iter()
                    .filter_map(|h| self.vertices.get(h))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Get the current round number.
    pub fn current_round(&self) -> u64 {
        self.current_round
    }

    /// Get all unique validator addresses from vertices in the DAG.
    pub fn all_validators(&self) -> HashSet<Address> {
        self.vertices.values().map(|v| v.validator).collect()
    }

    /// Total number of vertices in the DAG.
    pub fn len(&self) -> usize {
        self.vertices.len()
    }

    pub fn is_empty(&self) -> bool {
        self.vertices.is_empty()
    }

    /// Iterate over all vertex hashes in the DAG.
    pub fn all_hashes(&self) -> impl Iterator<Item = &[u8; 32]> {
        self.vertices.keys()
    }

    /// Iterate over all vertices in the DAG as (hash, vertex) pairs.
    pub fn all_vertices(&self) -> impl Iterator<Item = (&[u8; 32], &DagVertex)> {
        self.vertices.iter()
    }

    /// Get the number of distinct validators that have at least one descendant of this vertex.
    /// O(1) lookup using incrementally maintained counts.
    pub fn descendant_validator_count(&self, hash: &[u8; 32]) -> usize {
        self.descendant_validators
            .get(hash)
            .map(|s| s.len())
            .unwrap_or(0)
    }

    /// Get the set of distinct validators that have at least one descendant of this vertex.
    pub fn descendant_validators_of(&self, hash: &[u8; 32]) -> Option<&HashSet<Address>> {
        self.descendant_validators.get(hash)
    }

    /// Get direct children of a vertex.
    pub fn children_of(&self, hash: &[u8; 32]) -> Vec<[u8; 32]> {
        self.children
            .get(hash)
            .map(|c| c.iter().copied().collect())
            .unwrap_or_default()
    }

    /// Get all ancestors of a vertex (transitive closure of parents).
    pub fn ancestors(&self, hash: &[u8; 32]) -> HashSet<[u8; 32]> {
        let mut result = HashSet::new();
        let mut stack = vec![*hash];

        while let Some(current) = stack.pop() {
            if let Some(vertex) = self.vertices.get(&current) {
                for parent in &vertex.parent_hashes {
                    if result.insert(*parent) {
                        stack.push(*parent);
                    }
                }
            }
        }

        result
    }

    /// Get all descendants of a vertex (transitive closure of children).
    pub fn descendants(&self, hash: &[u8; 32]) -> HashSet<[u8; 32]> {
        let mut result = HashSet::new();
        let mut stack = vec![*hash];

        while let Some(current) = stack.pop() {
            for child in self.children_of(&current) {
                if result.insert(child) {
                    stack.push(child);
                }
            }
        }

        result
    }

    /// Get the set of distinct validators among a set of vertex hashes.
    pub fn distinct_validators(&self, hashes: &HashSet<[u8; 32]>) -> HashSet<crate::address::Address> {
        hashes
            .iter()
            .filter_map(|h| self.vertices.get(h))
            .map(|v| v.validator)
            .collect()
    }

    /// Check if vertex `ancestor` is an ancestor of vertex `descendant`.
    pub fn is_ancestor(&self, ancestor: &[u8; 32], descendant: &[u8; 32]) -> bool {
        self.ancestors(descendant).contains(ancestor)
    }

    /// Check if a validator already produced a vertex in the given round (equivocation).
    pub fn has_vertex_from_validator_in_round(
        &self,
        validator: &crate::address::Address,
        round: u64,
    ) -> bool {
        self.vertices_in_round(round)
            .iter()
            .any(|v| &v.validator == validator)
    }

    /// Count distinct validators that produced vertices in the given round.
    pub fn distinct_validators_in_round(&self, round: u64) -> HashSet<crate::address::Address> {
        self.vertices_in_round(round)
            .iter()
            .map(|v| v.validator)
            .collect()
    }

    /// Mark a validator as Byzantine (detected via equivocation).
    /// All future vertices from this validator will be rejected.
    pub fn mark_byzantine(&mut self, validator: Address) {
        self.byzantine_validators.insert(validator);
    }

    /// Check if a validator is marked as Byzantine.
    pub fn is_byzantine(&self, validator: &Address) -> bool {
        self.byzantine_validators.contains(validator)
    }

    /// Store equivocation evidence (two vertices from same validator, same round).
    /// Returns true if this is new evidence.
    /// Evidence is stored both in the temporary map and the permanent evidence_store.
    pub fn store_equivocation_evidence(
        &mut self,
        validator: Address,
        round: u64,
        vertex1_hash: [u8; 32],
        vertex2_hash: [u8; 32],
    ) -> bool {
        let key = (validator, round);
        if self.equivocation_evidence.contains_key(&key) {
            return false;
        }
        self.equivocation_evidence.insert(key, [vertex1_hash, vertex2_hash]);
        
        // Also store in permanent evidence_store (survives pruning)
        let entries = self.evidence_store.entry(validator).or_default();
        // Avoid duplicate evidence for the same round
        if !entries.iter().any(|e| e.round == round) {
            entries.push(EquivocationEvidence {
                validator,
                round,
                vertex_hash_1: vertex1_hash,
                vertex_hash_2: vertex2_hash,
                detected_at_round: self.current_round,
            });
        }
        
        true
    }

    /// Get equivocation evidence for a validator in a specific round.
    pub fn get_equivocation_evidence(&self, validator: &Address, round: u64) -> Option<[[u8; 32]; 2]> {
        self.equivocation_evidence.get(&(*validator, round)).copied()
    }

    /// Get permanent equivocation evidence for a validator (survives pruning).
    pub fn get_permanent_evidence(&self, validator: &Address) -> Option<&[EquivocationEvidence]> {
        self.evidence_store.get(validator).map(|v| v.as_slice())
    }

    /// Get all permanent equivocation evidence.
    pub fn all_evidence(&self) -> Vec<&EquivocationEvidence> {
        self.evidence_store.values().flat_map(|v| v.iter()).collect()
    }

    /// Verify and process equivocation evidence from a peer.
    /// Returns true if evidence is valid and validator is newly marked as Byzantine.
    pub fn process_equivocation_evidence(
        &mut self,
        vertex1: &DagVertex,
        vertex2: &DagVertex,
    ) -> bool {
        // Verify evidence is valid
        if vertex1.validator != vertex2.validator {
            return false; // Not from same validator
        }
        if vertex1.round != vertex2.round {
            return false; // Not from same round
        }
        if vertex1.hash() == vertex2.hash() {
            return false; // Same vertex (not equivocation)
        }
        // Verify Ed25519 signatures to prevent framing honest validators
        if !vertex1.verify_signature() || !vertex2.verify_signature() {
            return false; // Forged evidence
        }

        // Valid equivocation - store evidence and mark Byzantine
        let validator = vertex1.validator;
        let round = vertex1.round;
        let newly_stored = self.store_equivocation_evidence(
            validator,
            round,
            vertex1.hash(),
            vertex2.hash(),
        );

        // Mark as Byzantine (returns true if newly marked)
        let was_not_byzantine = !self.is_byzantine(&validator);
        self.mark_byzantine(validator);

        newly_stored && was_not_byzantine
    }

    /// Prune vertices from rounds older than the pruning horizon.
    /// Called after finality advances to prevent unbounded memory growth.
    /// 
    /// # Arguments
    /// * `last_finalized_round` - The most recent finalized round
    /// 
    /// # Returns
    /// Number of vertices pruned
    pub fn prune_old_rounds(&mut self, last_finalized_round: u64) -> usize {
        self.prune_old_rounds_with_depth(last_finalized_round, PRUNING_HORIZON)
    }

    /// Prune old rounds with a custom pruning depth.
    /// depth=0 means no pruning (archive mode).
    pub fn prune_old_rounds_with_depth(&mut self, last_finalized_round: u64, depth: u64) -> usize {
        if depth == 0 {
            return 0; // Archive mode: never prune
        }
        // Calculate the new pruning floor
        let new_floor = last_finalized_round.saturating_sub(depth);
        
        // Only prune if we've advanced beyond the current floor
        if new_floor <= self.pruning_floor {
            return 0;
        }
        
        let mut pruned_count = 0;
        
        // Collect rounds to prune (all rounds < new_floor)
        let rounds_to_prune: Vec<u64> = self.rounds.keys()
            .copied()
            .filter(|&r| r < new_floor)
            .collect();
        
        for round in rounds_to_prune {
            if let Some(hashes) = self.rounds.remove(&round) {
                for hash in hashes {
                    // Remove from vertices
                    self.vertices.remove(&hash);
                    
                    // Remove from children map
                    self.children.remove(&hash);
                    
                    // Remove from tips (if present)
                    self.tips.remove(&hash);
                    
                    // Remove from descendant_validators
                    self.descendant_validators.remove(&hash);
                    
                    // Remove from children of other vertices
                    for children_set in self.children.values_mut() {
                        children_set.remove(&hash);
                    }
                    
                    pruned_count += 1;
                }
            }
        }
        
        // Update pruning floor
        self.pruning_floor = new_floor;
        
        pruned_count
    }

    /// Get the current pruning floor (earliest round still in memory).
    pub fn pruning_floor(&self) -> u64 {
        self.pruning_floor
    }

    /// Set the pruning floor directly (used after checkpoint fast-sync).
    pub fn set_pruning_floor(&mut self, floor: u64) {
        self.pruning_floor = floor;
    }

    /// Save DAG state to disk
    pub fn save(&self, path: &std::path::Path) -> Result<(), crate::persistence::PersistenceError> {
        let snapshot = crate::consensus::persistence::DagSnapshot {
            vertices: self.vertices.iter().map(|(k, v)| (*k, v.clone())).collect(),
            children: self.children.iter().map(|(k, v)| (*k, v.iter().copied().collect())).collect(),
            tips: self.tips.iter().copied().collect(),
            rounds: self.rounds.iter().map(|(k, v)| (*k, v.clone())).collect(),
            current_round: self.current_round,
            byzantine_validators: self.byzantine_validators.iter().copied().collect(),
            equivocation_evidence: self.equivocation_evidence.iter().map(|(k, v)| (*k, *v)).collect(),
            pruning_floor: self.pruning_floor,
            evidence_store: self.evidence_store.iter().map(|(k, v)| (*k, v.clone())).collect(),
            equivocation_vertices: self.equivocation_vertices.iter().map(|(k, v)| (*k, v.clone())).collect(),
        };
        snapshot.save(path)
    }

    /// Load DAG state from disk
    pub fn load(path: &std::path::Path) -> Result<Self, crate::persistence::PersistenceError> {
        let snapshot = crate::consensus::persistence::DagSnapshot::load(path)?;
        let vertices: HashMap<[u8; 32], DagVertex> = snapshot.vertices.into_iter().collect();

        // Rebuild descendant_validators from all vertices
        let mut descendant_validators: HashMap<[u8; 32], HashSet<Address>> = HashMap::new();
        for hash in vertices.keys() {
            descendant_validators.entry(*hash).or_default();
        }
        // Sort vertices by round so we process parents before children
        let mut sorted: Vec<_> = vertices.iter().collect();
        sorted.sort_by_key(|(_, v)| v.round);
        let genesis: [u8; 32] = [0u8; 32];
        for (hash, vertex) in &sorted {
            let validator = vertex.validator;
            let mut queue = VecDeque::new();
            for parent in &vertex.parent_hashes {
                queue.push_back(*parent);
            }
            let mut visited = HashSet::new();
            while let Some(ancestor) = queue.pop_front() {
                if ancestor == genesis || !visited.insert(ancestor) {
                    continue;
                }
                let set = descendant_validators.entry(ancestor).or_default();
                if !set.insert(validator) {
                    continue;
                }
                if let Some(v) = vertices.get(&ancestor) {
                    for p in &v.parent_hashes {
                        if !visited.contains(p) {
                            queue.push_back(*p);
                        }
                    }
                }
            }
            let _ = hash; // used above via sorted iteration
        }

        Ok(Self {
            vertices,
            children: snapshot.children.into_iter().map(|(k, v)| (k, v.into_iter().collect())).collect(),
            tips: snapshot.tips.into_iter().collect(),
            rounds: snapshot.rounds.into_iter().collect(),
            current_round: snapshot.current_round,
            byzantine_validators: snapshot.byzantine_validators.into_iter().collect(),
            equivocation_evidence: snapshot.equivocation_evidence.into_iter().collect(),
            evidence_store: snapshot.evidence_store.into_iter().collect(),
            equivocation_vertices: snapshot.equivocation_vertices.into_iter().collect(),
            descendant_validators,
            pruning_floor: snapshot.pruning_floor,
        })
    }

    /// Check if saved state exists
    pub fn exists(path: &std::path::Path) -> bool {
        crate::consensus::persistence::DagSnapshot::exists(path)
    }
}

impl Default for BlockDag {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::address::SecretKey;
    use crate::block::header::BlockHeader;
    use crate::block::Block;
    use crate::address::Signature;
    use crate::tx::CoinbaseTx;

    fn make_vertex_with(
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

    fn random_sk() -> SecretKey {
        SecretKey::generate()
    }

    #[test]
    fn insert_and_get() {
        let mut dag = BlockDag::new();
        let v = make_vertex_with(0, 0, vec![], &random_sk());
        let hash = v.hash();
        assert!(dag.insert(v));
        assert!(dag.get(&hash).is_some());
        assert_eq!(dag.len(), 1);
    }

    #[test]
    fn duplicate_insert_returns_false() {
        let mut dag = BlockDag::new();
        let v = make_vertex_with(0, 0, vec![], &random_sk());
        assert!(dag.insert(v.clone()));
        assert!(!dag.insert(v));
    }

    #[test]
    fn tips_updated_on_insert() {
        let mut dag = BlockDag::new();
        let v1 = make_vertex_with(1, 0, vec![], &random_sk());
        let h1 = v1.hash();
        dag.insert(v1);
        assert!(dag.tips().contains(&h1));

        let v2 = make_vertex_with(2, 1, vec![h1], &random_sk());
        let h2 = v2.hash();
        dag.insert(v2);

        assert!(!dag.tips().contains(&h1));
        assert!(dag.tips().contains(&h2));
    }

    #[test]
    fn children_tracked() {
        let mut dag = BlockDag::new();
        let v1 = make_vertex_with(1, 0, vec![], &random_sk());
        let h1 = v1.hash();
        dag.insert(v1);

        let v2 = make_vertex_with(2, 1, vec![h1], &random_sk());
        let h2 = v2.hash();
        dag.insert(v2);

        let children = dag.children_of(&h1);
        assert_eq!(children.len(), 1);
        assert!(children.contains(&h2));
    }

    #[test]
    fn ancestors_transitive() {
        let mut dag = BlockDag::new();
        let v1 = make_vertex_with(1, 0, vec![], &random_sk());
        let h1 = v1.hash();
        dag.insert(v1);

        let v2 = make_vertex_with(2, 1, vec![h1], &random_sk());
        let h2 = v2.hash();
        dag.insert(v2);

        let v3 = make_vertex_with(3, 2, vec![h2], &random_sk());
        let h3 = v3.hash();
        dag.insert(v3);

        let anc = dag.ancestors(&h3);
        assert!(anc.contains(&h1));
        assert!(anc.contains(&h2));
    }

    #[test]
    fn descendants_transitive() {
        let mut dag = BlockDag::new();
        let v1 = make_vertex_with(1, 0, vec![], &random_sk());
        let h1 = v1.hash();
        dag.insert(v1);

        let v2 = make_vertex_with(2, 1, vec![h1], &random_sk());
        let h2 = v2.hash();
        dag.insert(v2);

        let v3 = make_vertex_with(3, 2, vec![h2], &random_sk());
        let h3 = v3.hash();
        dag.insert(v3);

        let desc = dag.descendants(&h1);
        assert!(desc.contains(&h2));
        assert!(desc.contains(&h3));
    }

    #[test]
    fn vertices_in_round() {
        let mut dag = BlockDag::new();
        let sk = random_sk();
        let v1 = make_vertex_with(1, 0, vec![], &sk);
        let v2 = make_vertex_with(2, 0, vec![], &sk);
        let v3 = make_vertex_with(3, 1, vec![], &sk);
        dag.insert(v1);
        dag.insert(v2);
        dag.insert(v3);

        assert_eq!(dag.vertices_in_round(0).len(), 2);
        assert_eq!(dag.vertices_in_round(1).len(), 1);
        assert_eq!(dag.vertices_in_round(2).len(), 0);
    }

    #[test]
    fn distinct_validators() {
        let mut dag = BlockDag::new();
        let sk1 = random_sk();
        let sk2 = random_sk();

        let v1 = make_vertex_with(1, 0, vec![], &sk1);
        let h1 = v1.hash();
        let v2 = make_vertex_with(2, 0, vec![], &sk2);
        let h2 = v2.hash();
        let v3 = make_vertex_with(3, 0, vec![], &sk1); // same validator as v1
        let h3 = v3.hash();

        dag.insert(v1);
        dag.insert(v2);
        dag.insert(v3);

        let set: std::collections::HashSet<[u8; 32]> = [h1, h2, h3].into();
        let validators = dag.distinct_validators(&set);
        assert_eq!(validators.len(), 2);
    }

    #[test]
    fn multiple_parents() {
        let mut dag = BlockDag::new();
        let v1 = make_vertex_with(1, 0, vec![], &random_sk());
        let h1 = v1.hash();
        let v2 = make_vertex_with(2, 0, vec![], &random_sk());
        let h2 = v2.hash();
        dag.insert(v1);
        dag.insert(v2);

        // v3 references both v1 and v2
        let v3 = make_vertex_with(3, 1, vec![h1, h2], &random_sk());
        let h3 = v3.hash();
        dag.insert(v3);

        // Both parents should no longer be tips
        assert!(!dag.tips().contains(&h1));
        assert!(!dag.tips().contains(&h2));
        assert!(dag.tips().contains(&h3));

        // v3's ancestors include both
        let anc = dag.ancestors(&h3);
        assert!(anc.contains(&h1));
        assert!(anc.contains(&h2));
    }

    #[test]
    fn equivocation_detection() {
        let mut dag = BlockDag::new();
        let sk = random_sk();

        let v1 = make_vertex_with(1, 5, vec![], &sk);
        dag.insert(v1);

        assert!(dag.has_vertex_from_validator_in_round(&sk.address(), 5));
        assert!(!dag.has_vertex_from_validator_in_round(&sk.address(), 6));
    }

    #[test]
    fn distinct_validators_in_round() {
        let mut dag = BlockDag::new();
        let sk1 = random_sk();
        let sk2 = random_sk();
        let sk3 = random_sk();

        dag.insert(make_vertex_with(1, 1, vec![], &sk1));
        dag.insert(make_vertex_with(2, 1, vec![], &sk2));
        dag.insert(make_vertex_with(3, 2, vec![], &sk3));

        let round1 = dag.distinct_validators_in_round(1);
        assert_eq!(round1.len(), 2);
        assert!(round1.contains(&sk1.address()));
        assert!(round1.contains(&sk2.address()));

        let round2 = dag.distinct_validators_in_round(2);
        assert_eq!(round2.len(), 1);
    }

    #[test]
    fn reject_future_round_vertex() {
        let mut dag = BlockDag::new();
        let sk = random_sk();

        // Insert vertex at round 1
        let v1 = make_vertex_with(1, 1, vec![], &sk);
        assert!(dag.insert(v1), "Round 1 vertex should be accepted");
        assert_eq!(dag.current_round(), 1);

        // Try to insert vertex at round 1000 (way beyond MAX_FUTURE_ROUNDS=10)
        let sk2 = random_sk();
        let v_future = make_vertex_with(2, 1000, vec![], &sk2);
        assert!(!dag.insert(v_future), "Round 1000 vertex should be rejected when current round is 1");

        // Vertex at round 11 (current_round=1 + 10) should be accepted
        let sk3 = random_sk();
        let v_boundary = make_vertex_with(3, 11, vec![], &sk3);
        assert!(dag.insert(v_boundary), "Round 11 vertex should be accepted (within MAX_FUTURE_ROUNDS)");
        assert_eq!(dag.current_round(), 11, "Current round should be updated to 11");

        // Vertex at round 22 (current_round=11 + 11) should be rejected (exceeds MAX_FUTURE_ROUNDS=10)
        let sk4 = random_sk();
        let v_over = make_vertex_with(4, 22, vec![], &sk4);
        assert!(!dag.insert(v_over), "Round 22 vertex should be rejected (exceeds MAX_FUTURE_ROUNDS)");
    }

    #[test]
    fn reject_future_timestamp_vertex() {
        let mut dag = BlockDag::new();
        let sk = random_sk();

        // Create vertex with timestamp 600 seconds (10 minutes) in the future
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        
        let mut vertex = make_vertex_with(1, 1, vec![], &sk);
        vertex.block.header.timestamp = now + 600; // 10 minutes in future
        
        // Re-sign with new timestamp
        let signable = vertex.signable_bytes();
        vertex.signature = sk.sign(&signable);

        // Should be rejected (>5 minutes in future)
        let result = dag.try_insert(vertex);
        assert_eq!(result, Ok(false), "Vertex with timestamp 10 minutes in future should be rejected");

        // Create vertex with timestamp 200 seconds (3.3 minutes) in the future
        let mut vertex2 = make_vertex_with(2, 1, vec![], &sk);
        vertex2.block.header.timestamp = now + 200; // 3.3 minutes in future
        
        // Re-sign
        let signable2 = vertex2.signable_bytes();
        vertex2.signature = sk.sign(&signable2);

        // Should be accepted (<5 minutes in future)
        let result2 = dag.try_insert(vertex2);
        assert_eq!(result2, Ok(true), "Vertex with timestamp 3.3 minutes in future should be accepted");
    }
}
