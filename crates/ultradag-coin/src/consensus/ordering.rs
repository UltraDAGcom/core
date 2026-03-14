use crate::consensus::dag::BlockDag;
use crate::consensus::vertex::DagVertex;

/// Produces a deterministic total ordering of DAG vertices.
/// Uses `(round, hash)` as the sort key — round for causal ordering,
/// hash as tiebreaker for determinism.
///
/// IMPORTANT: We intentionally do NOT use `topo_level` in ordering because
/// `topo_level` is derived locally during `insert()` and is `#[serde(skip)]`.
/// If two nodes have different DAG states when inserting (e.g., missing a parent),
/// they could compute different `topo_level` for the same vertex, causing a
/// consensus split. `(round, hash)` is fully deterministic from signed data.
///
/// # Performance
/// O(N log N) — single sort pass with O(1) per-comparison cost.
pub fn order_vertices<'a>(
    hashes: &[[u8; 32]],
    dag: &'a BlockDag,
) -> Vec<&'a DagVertex> {
    // Precompute hashes to avoid recomputing blake3 in sort comparator
    let mut vertices: Vec<([u8; 32], &DagVertex)> = hashes
        .iter()
        .filter_map(|h| dag.get(h).map(|v| (v.hash(), v)))
        .collect();

    // Sort by: (round, hash) — both are deterministic from signed vertex data.
    vertices.sort_by(|(ha, a), (hb, b)| {
        a.round.cmp(&b.round)
            .then_with(|| ha.cmp(hb))
    });

    vertices.into_iter().map(|(_, v)| v).collect()
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
    fn ordering_same_round_by_hash() {
        let mut dag = BlockDag::new();

        let v1 = make_vertex(1, 0, vec![], &SecretKey::generate());
        let h1 = v1.hash();
        dag.insert(v1);

        let v2 = make_vertex(2, 0, vec![], &SecretKey::generate());
        let h2 = v2.hash();
        dag.insert(v2);

        // Same round: ordered by hash (deterministic from signed data)
        let ordered = order_vertices(&[h2, h1], &dag);
        let first = ordered[0].hash();
        let second = ordered[1].hash();
        assert!(first < second, "Same-round vertices should be ordered by hash");
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

    #[test]
    fn topo_level_computed_on_insert() {
        let mut dag = BlockDag::new();

        // Genesis vertex (no real parents) -> topo_level 0
        let v1 = make_vertex(1, 0, vec![], &SecretKey::generate());
        let h1 = v1.hash();
        dag.insert(v1);
        assert_eq!(dag.get(&h1).unwrap().topo_level, 0);

        // Child of v1 -> topo_level 1
        let v2 = make_vertex(2, 1, vec![h1], &SecretKey::generate());
        let h2 = v2.hash();
        dag.insert(v2);
        assert_eq!(dag.get(&h2).unwrap().topo_level, 1);

        // Child of v2 -> topo_level 2
        let v3 = make_vertex(3, 2, vec![h2], &SecretKey::generate());
        let h3 = v3.hash();
        dag.insert(v3);
        assert_eq!(dag.get(&h3).unwrap().topo_level, 2);
    }

    #[test]
    fn topo_level_takes_max_parent() {
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

        // v4 has parents v1 (level 0) and v3 (level 2) -> max + 1 = 3
        let v4 = make_vertex(4, 3, vec![h1, h3], &SecretKey::generate());
        let h4 = v4.hash();
        dag.insert(v4);
        assert_eq!(dag.get(&h4).unwrap().topo_level, 3);
    }
}
