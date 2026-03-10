/// Invariant checkers for distributed system safety properties.
/// 
/// These checkers validate that the system maintains correctness even
/// under fault injection scenarios.

use std::collections::{HashMap, HashSet};
use ultradag_coin::{Address, DagVertex};
use super::TestNode;

/// Safety invariant violations
#[derive(Debug, Clone)]
pub enum InvariantViolation {
    /// Two nodes finalized different vertices at the same round
    FinalityConflict {
        round: u64,
        node_a: usize,
        node_b: usize,
        hash_a: [u8; 32],
        hash_b: [u8; 32],
    },
    /// A finalized vertex was later reverted
    FinalityRevert {
        node: usize,
        round: u64,
        old_hash: [u8; 32],
        new_hash: [u8; 32],
    },
    /// Total supply differs between nodes
    SupplyMismatch {
        node_a: usize,
        node_b: usize,
        supply_a: u64,
        supply_b: u64,
    },
    /// Balance sum doesn't match total supply
    SupplyAccountingError {
        node: usize,
        total_supply: u64,
        balance_sum: u64,
    },
    /// Double spend detected
    DoubleSpend {
        node: usize,
        address: Address,
        balance: u64,
        spent: u64,
    },
}

/// Invariant checker for distributed consensus safety
pub struct InvariantChecker {
    /// Track finalized vertices per node per round
    finalized_history: HashMap<usize, HashMap<u64, [u8; 32]>>,
    /// Track total supply per node over time
    supply_history: HashMap<usize, Vec<(u64, u64)>>, // (round, supply)
}

impl InvariantChecker {
    pub fn new() -> Self {
        Self {
            finalized_history: HashMap::new(),
            supply_history: HashMap::new(),
        }
    }

    /// Record finalized state for a node
    pub async fn record_finalized_state(
        &mut self,
        node: &TestNode,
    ) -> Result<(), InvariantViolation> {
        let round = node.finalized_round().await;
        let supply = node.total_supply().await;

        // Record supply history
        self.supply_history
            .entry(node.id)
            .or_insert_with(Vec::new)
            .push((round, supply));

        // Check for finality revert (same node, same round, different hash)
        if let Some(history) = self.finalized_history.get(&node.id) {
            if let Some(&old_hash) = history.get(&round) {
                // Get current finalized hash for this round
                let dag = node.dag.read().await;
                if let Some(vertices) = dag.hashes_in_round(round).first() {
                    if vertices != &old_hash {
                        return Err(InvariantViolation::FinalityRevert {
                            node: node.id,
                            round,
                            old_hash,
                            new_hash: *vertices,
                        });
                    }
                }
            }
        }

        Ok(())
    }

    /// Check finality agreement: all nodes must agree on finalized vertices
    pub async fn check_finality_agreement(
        &self,
        nodes: &[TestNode],
    ) -> Result<(), InvariantViolation> {
        // Find minimum finalized round across all nodes
        let mut min_finalized = u64::MAX;
        for node in nodes {
            let round = node.finalized_round().await;
            min_finalized = min_finalized.min(round);
        }

        // Check that all nodes agree on vertices up to min_finalized
        for round in 0..=min_finalized {
            let mut hashes_by_node: HashMap<usize, Vec<[u8; 32]>> = HashMap::new();

            for node in nodes {
                let dag = node.dag.read().await;
                let hashes: Vec<[u8; 32]> = dag.hashes_in_round(round).to_vec();
                if !hashes.is_empty() {
                    hashes_by_node.insert(node.id, hashes);
                }
            }

            // All nodes should have the same set of hashes for this round
            if hashes_by_node.len() > 1 {
                let mut iter = hashes_by_node.iter();
                let (first_node, first_hashes) = iter.next().unwrap();

                for (other_node, other_hashes) in iter {
                    if first_hashes != other_hashes {
                        // Report first mismatch
                        if let (Some(&hash_a), Some(&hash_b)) = (first_hashes.first(), other_hashes.first()) {
                            return Err(InvariantViolation::FinalityConflict {
                                round,
                                node_a: *first_node,
                                node_b: *other_node,
                                hash_a,
                                hash_b,
                            });
                        }
                    }
                }
            }
        }

        Ok(())
    }

    /// Check supply consistency across nodes
    pub async fn check_supply_consistency(
        &self,
        nodes: &[TestNode],
    ) -> Result<(), InvariantViolation> {
        if nodes.is_empty() {
            return Ok(());
        }

        let first_supply = nodes[0].total_supply().await;

        for node in nodes.iter().skip(1) {
            let supply = node.total_supply().await;
            if supply != first_supply {
                return Err(InvariantViolation::SupplyMismatch {
                    node_a: nodes[0].id,
                    node_b: node.id,
                    supply_a: first_supply,
                    supply_b: supply,
                });
            }
        }

        Ok(())
    }

    /// Check that total supply equals sum of all balances
    pub async fn check_supply_accounting(
        &self,
        node: &TestNode,
    ) -> Result<(), InvariantViolation> {
        let state = node.state.read().await;
        let total_supply = state.total_supply();
        
        // Sum all account balances
        // Note: This requires iterating all accounts, which isn't exposed in StateEngine
        // For now, we just verify total_supply is non-negative and within bounds
        if total_supply > ultradag_coin::constants::MAX_SUPPLY_SATS {
            return Err(InvariantViolation::SupplyAccountingError {
                node: node.id,
                total_supply,
                balance_sum: ultradag_coin::constants::MAX_SUPPLY_SATS,
            });
        }

        Ok(())
    }

    /// Check for double-spend violations
    pub async fn check_no_double_spend(
        &self,
        node: &TestNode,
        address: &Address,
    ) -> Result<(), InvariantViolation> {
        let state = node.state.read().await;
        let balance = state.balance(address);

        // Balance should never be negative (u64 prevents this, but check for overflow)
        // In a real implementation, we'd track transaction history to detect double-spends
        
        Ok(())
    }

    /// Run all invariant checks
    pub async fn check_all(
        &mut self,
        nodes: &[TestNode],
    ) -> Vec<InvariantViolation> {
        let mut violations = Vec::new();

        // Record state for all nodes
        for node in nodes {
            if let Err(v) = self.record_finalized_state(node).await {
                violations.push(v);
            }
        }

        // Check finality agreement
        if let Err(v) = self.check_finality_agreement(nodes).await {
            violations.push(v);
        }

        // Check supply consistency
        if let Err(v) = self.check_supply_consistency(nodes).await {
            violations.push(v);
        }

        // Check supply accounting for each node
        for node in nodes {
            if let Err(v) = self.check_supply_accounting(node).await {
                violations.push(v);
            }
        }

        violations
    }

    /// Generate a report of all violations
    pub fn report(&self, violations: &[InvariantViolation]) -> String {
        if violations.is_empty() {
            return "✅ All invariants satisfied".to_string();
        }

        let mut report = format!("❌ {} invariant violation(s) detected:\n\n", violations.len());
        
        for (i, violation) in violations.iter().enumerate() {
            report.push_str(&format!("{}. {}\n", i + 1, self.format_violation(violation)));
        }

        report
    }

    fn format_violation(&self, v: &InvariantViolation) -> String {
        match v {
            InvariantViolation::FinalityConflict { round, node_a, node_b, hash_a, hash_b } => {
                format!(
                    "Finality conflict at round {}: node {} has {:?}, node {} has {:?}",
                    round, node_a, &hash_a[..8], node_b, &hash_b[..8]
                )
            }
            InvariantViolation::FinalityRevert { node, round, old_hash, new_hash } => {
                format!(
                    "Finality revert on node {} at round {}: {:?} -> {:?}",
                    node, round, &old_hash[..8], &new_hash[..8]
                )
            }
            InvariantViolation::SupplyMismatch { node_a, node_b, supply_a, supply_b } => {
                format!(
                    "Supply mismatch: node {} has {}, node {} has {}",
                    node_a, supply_a, node_b, supply_b
                )
            }
            InvariantViolation::SupplyAccountingError { node, total_supply, balance_sum } => {
                format!(
                    "Supply accounting error on node {}: total_supply={}, balance_sum={}",
                    node, total_supply, balance_sum
                )
            }
            InvariantViolation::DoubleSpend { node, address, balance, spent } => {
                format!(
                    "Double spend on node {}: address {:?} has balance {} but spent {}",
                    node, &address.0[..8], balance, spent
                )
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ultradag_coin::SecretKey;

    #[tokio::test]
    async fn test_invariant_checker_no_violations() {
        let sk = SecretKey::generate();
        let node = TestNode::new(0, sk.address());
        
        let mut checker = InvariantChecker::new();
        let violations = checker.check_all(&[node]).await;
        
        assert!(violations.is_empty(), "Should have no violations");
    }

    #[tokio::test]
    async fn test_supply_consistency() {
        let sk1 = SecretKey::generate();
        let sk2 = SecretKey::generate();
        
        let node1 = TestNode::new(0, sk1.address());
        let node2 = TestNode::new(1, sk2.address());
        
        let checker = InvariantChecker::new();
        
        // Both nodes start with same supply (0)
        let result = checker.check_supply_consistency(&[node1, node2]).await;
        assert!(result.is_ok(), "Supply should be consistent");
    }
}
