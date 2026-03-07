use std::collections::HashSet;

use crate::consensus::dag::BlockDag;
use crate::consensus::vertex::DagVertex;

/// Produces a deterministic total ordering of DAG vertices.
/// Uses round number as primary key, then topological order within a round,
/// then hash as final tiebreaker for determinism.
///
/// # Performance
/// O(N log N) for sorting, but O(N²) worst case due to ancestor traversal during comparison.
/// Each `count_ancestors_in_set()` call traverses the full DAG via `dag.ancestors()`.
/// For N finalized vertices, this can result in N² ancestor traversals.
///
/// **Impact:** Acceptable for small/medium DAGs (<5-10K vertices) and low finalization rates.
/// Not the primary bottleneck (finality check optimization was P2, now complete).
///
/// **Future optimization (P3 - non-urgent):**
/// - Memoize ancestor counts during sort (HashMap cache)
/// - Pre-compute topological levels during finality collection
/// - Incremental tracking similar to descendant validator counts
///
/// Estimated effort: 1-2 days when needed for high-throughput deployments.
pub fn order_vertices<'a>(
    hashes: &[[u8; 32]],
    dag: &'a BlockDag,
) -> Vec<&'a DagVertex> {
    let hash_set: HashSet<[u8; 32]> = hashes.iter().copied().collect();

    let mut vertices: Vec<&DagVertex> = hashes
        .iter()
        .filter_map(|h| dag.get(h))
        .collect();

    // Sort by: (round, topological depth, hash)
    vertices.sort_by(|a, b| {
        // Primary: round number
        let round_cmp = a.round.cmp(&b.round);
        if round_cmp != std::cmp::Ordering::Equal {
            return round_cmp;
        }

        // Secondary: number of ancestors in the set (topological depth)
        let depth_a = count_ancestors_in_set(&a.hash(), dag, &hash_set);
        let depth_b = count_ancestors_in_set(&b.hash(), dag, &hash_set);
        let depth_cmp = depth_a.cmp(&depth_b);
        if depth_cmp != std::cmp::Ordering::Equal {
            return depth_cmp;
        }

        // Tertiary: deterministic hash tiebreak
        a.hash().cmp(&b.hash())
    });

    vertices
}

/// Count how many of a vertex's ancestors are in the given set.
fn count_ancestors_in_set(
    hash: &[u8; 32],
    dag: &BlockDag,
    set: &HashSet<[u8; 32]>,
) -> usize {
    dag.ancestors(hash)
        .iter()
        .filter(|h| set.contains(*h))
        .count()
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
    fn ordering_by_round() {
        let mut dag = BlockDag::new();

        let v1 = make_vertex(1, 0, vec![], &SecretKey::generate());
        let h1 = v1.hash();
        dag.insert(v1);

        let v2 = make_vertex(2, 1, vec![h1], &SecretKey::generate());
        let h2 = v2.hash();
        dag.insert(v2);

        let v3 = make_vertex(3, 2, vec![h2], &SecretKey::generate());
        let h3 = v3.hash();
        dag.insert(v3);

        let ordered = order_vertices(&[h3, h1, h2], &dag);
        assert_eq!(ordered[0].hash(), h1);
        assert_eq!(ordered[1].hash(), h2);
        assert_eq!(ordered[2].hash(), h3);
    }

    #[test]
    fn ordering_same_round_by_depth() {
        let mut dag = BlockDag::new();

        let v1 = make_vertex(1, 0, vec![], &SecretKey::generate());
        let h1 = v1.hash();
        dag.insert(v1);

        let v2 = make_vertex(2, 0, vec![h1], &SecretKey::generate());
        let h2 = v2.hash();
        dag.insert(v2);

        let ordered = order_vertices(&[h2, h1], &dag);
        assert_eq!(ordered[0].hash(), h1);
        assert_eq!(ordered[1].hash(), h2);
    }

    #[test]
    fn ordering_is_deterministic() {
        let mut dag = BlockDag::new();
        let v1 = make_vertex(1, 0, vec![], &SecretKey::generate());
        let h1 = v1.hash();
        let v2 = make_vertex(2, 0, vec![], &SecretKey::generate());
        let h2 = v2.hash();
        dag.insert(v1);
        dag.insert(v2);

        let o1 = order_vertices(&[h1, h2], &dag);
        let o2 = order_vertices(&[h2, h1], &dag);
        assert_eq!(o1[0].hash(), o2[0].hash());
        assert_eq!(o1[1].hash(), o2[1].hash());
    }

    #[test]
    fn ordering_empty() {
        let dag = BlockDag::new();
        let ordered = order_vertices(&[], &dag);
        assert!(ordered.is_empty());
    }
}
