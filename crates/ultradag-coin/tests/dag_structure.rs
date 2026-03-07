/// Module 4: DAG Structure — Production-grade tests

use ultradag_coin::address::{SecretKey, Signature};
use ultradag_coin::block::header::BlockHeader;
use ultradag_coin::block::Block;
use ultradag_coin::consensus::dag::BlockDag;
use ultradag_coin::consensus::vertex::DagVertex;
use ultradag_coin::tx::CoinbaseTx;

fn make_vertex(uid: u64, round: u64, parents: Vec<[u8; 32]>, sk: &SecretKey) -> DagVertex {
    let validator = sk.address();
    let block = Block {
        header: BlockHeader {
            version: 1, height: uid,
            timestamp: 1_000_000 + uid as i64,
            prev_hash: parents.first().copied().unwrap_or([0u8; 32]),
            merkle_root: [0u8; 32],
        },
        coinbase: CoinbaseTx { to: validator, amount: 5_000_000_000, height: uid },
        transactions: vec![],
    };
    let mut v = DagVertex::new(
        block, parents, round, validator,
        sk.verifying_key().to_bytes(), Signature([0u8; 64]),
    );
    v.signature = sk.sign(&v.signable_bytes());
    v
}

/// Empty DAG has correct initial state.
/// Mutation: new() setting current_round=1 → assertion fails.
#[test]
fn empty_dag_initial_state() {
    let dag = BlockDag::new();

    // POSITIVE: empty state
    assert_eq!(dag.len(), 0);
    assert!(dag.is_empty());
    assert_eq!(dag.current_round(), 0);
    assert!(dag.tips().is_empty());
    assert!(dag.vertices_in_round(0).is_empty());

    // NEGATIVE: getting nonexistent vertex
    assert!(dag.get(&[0u8; 32]).is_none());
}

/// Adding a valid vertex increases DAG size by exactly one.
/// Mutation: insert not storing vertex → len stays 0.
#[test]
fn insert_increases_size_by_one() {
    let sk = SecretKey::from_bytes([1u8; 32]);
    let mut dag = BlockDag::new();

    let v1 = make_vertex(1, 0, vec![], &sk);
    let h1 = v1.hash();
    assert!(dag.insert(v1));
    assert_eq!(dag.len(), 1);
    assert!(!dag.is_empty());
    assert!(dag.get(&h1).is_some());

    let sk2 = SecretKey::from_bytes([2u8; 32]);
    let v2 = make_vertex(2, 0, vec![], &sk2);
    assert!(dag.insert(v2));
    assert_eq!(dag.len(), 2);

    // NEGATIVE: duplicate returns false, no size increase
    let v1_dup = make_vertex(1, 0, vec![], &sk);
    assert!(!dag.insert(v1_dup));
    assert_eq!(dag.len(), 2);
}

/// Tips are updated correctly when vertices reference parents.
/// Mutation: insert not removing parent from tips → stale tips.
#[test]
fn tips_updated_on_parent_reference() {
    let sk1 = SecretKey::from_bytes([3u8; 32]);
    let sk2 = SecretKey::from_bytes([4u8; 32]);
    let sk3 = SecretKey::from_bytes([5u8; 32]);
    let mut dag = BlockDag::new();

    // Two independent round-0 vertices — both are tips
    let v1 = make_vertex(10, 0, vec![], &sk1);
    let h1 = v1.hash();
    let v2 = make_vertex(11, 0, vec![], &sk2);
    let h2 = v2.hash();
    dag.insert(v1);
    dag.insert(v2);

    let tips = dag.tips();
    assert_eq!(tips.len(), 2);
    assert!(tips.contains(&h1));
    assert!(tips.contains(&h2));

    // v3 references v1 → v1 is no longer a tip, v2 still is, v3 is new tip
    let v3 = make_vertex(12, 1, vec![h1], &sk3);
    let h3 = v3.hash();
    dag.insert(v3);

    let tips = dag.tips();
    assert_eq!(tips.len(), 2);
    assert!(!tips.contains(&h1), "v1 should no longer be a tip");
    assert!(tips.contains(&h2), "v2 should still be a tip");
    assert!(tips.contains(&h3), "v3 should be a new tip");

    // v4 references both v2 and v3 → only v4 is a tip
    let v4 = make_vertex(13, 2, vec![h2, h3], &sk1);
    let h4 = v4.hash();
    dag.insert(v4);

    let tips = dag.tips();
    assert_eq!(tips.len(), 1);
    assert!(tips.contains(&h4));
}

/// Causal history (ancestors) is complete and correct.
/// Mutation: ancestors not following transitive closure → missing ancestors.
#[test]
fn causal_history_complete() {
    let sk1 = SecretKey::from_bytes([6u8; 32]);
    let sk2 = SecretKey::from_bytes([7u8; 32]);
    let sk3 = SecretKey::from_bytes([8u8; 32]);
    let mut dag = BlockDag::new();

    // Chain: v1 ← v2 ← v3
    let v1 = make_vertex(20, 0, vec![], &sk1);
    let h1 = v1.hash();
    dag.insert(v1);

    let v2 = make_vertex(21, 1, vec![h1], &sk2);
    let h2 = v2.hash();
    dag.insert(v2);

    let v3 = make_vertex(22, 2, vec![h2], &sk3);
    let h3 = v3.hash();
    dag.insert(v3);

    // POSITIVE: v3 ancestors include v1 and v2
    let ancestors = dag.ancestors(&h3);
    assert_eq!(ancestors.len(), 2);
    assert!(ancestors.contains(&h1));
    assert!(ancestors.contains(&h2));

    // POSITIVE: v2 ancestors include only v1
    let ancestors = dag.ancestors(&h2);
    assert_eq!(ancestors.len(), 1);
    assert!(ancestors.contains(&h1));

    // POSITIVE: v1 has no ancestors
    assert!(dag.ancestors(&h1).is_empty());

    // POSITIVE: is_ancestor check
    assert!(dag.is_ancestor(&h1, &h3));
    assert!(dag.is_ancestor(&h1, &h2));
    assert!(dag.is_ancestor(&h2, &h3));

    // NEGATIVE: reverse direction
    assert!(!dag.is_ancestor(&h3, &h1));
    assert!(!dag.is_ancestor(&h2, &h1));
}

/// Descendants are complete and correct.
/// Mutation: descendants not following children map → missing descendants.
#[test]
fn descendants_complete() {
    let sk1 = SecretKey::from_bytes([9u8; 32]);
    let sk2 = SecretKey::from_bytes([10u8; 32]);
    let sk3 = SecretKey::from_bytes([11u8; 32]);
    let mut dag = BlockDag::new();

    let v1 = make_vertex(30, 0, vec![], &sk1);
    let h1 = v1.hash();
    dag.insert(v1);

    let v2 = make_vertex(31, 1, vec![h1], &sk2);
    let h2 = v2.hash();
    dag.insert(v2);

    let v3 = make_vertex(32, 2, vec![h2], &sk3);
    let h3 = v3.hash();
    dag.insert(v3);

    // POSITIVE: v1 descendants include v2 and v3
    let desc = dag.descendants(&h1);
    assert_eq!(desc.len(), 2);
    assert!(desc.contains(&h2));
    assert!(desc.contains(&h3));

    // POSITIVE: v2 descendants include only v3
    let desc = dag.descendants(&h2);
    assert_eq!(desc.len(), 1);
    assert!(desc.contains(&h3));

    // POSITIVE: v3 has no descendants
    assert!(dag.descendants(&h3).is_empty());
}

/// Same vertices in different insertion orders produce same structure.
/// Mutation: insert order affecting hash → structure changes.
#[test]
fn insertion_order_independent() {
    let sk1 = SecretKey::from_bytes([12u8; 32]);
    let sk2 = SecretKey::from_bytes([13u8; 32]);
    let sk3 = SecretKey::from_bytes([14u8; 32]);

    let v1 = make_vertex(40, 0, vec![], &sk1);
    let v2 = make_vertex(41, 0, vec![], &sk2);
    let h1 = v1.hash();
    let h2 = v2.hash();

    let v3 = make_vertex(42, 1, vec![h1, h2], &sk3);
    let h3 = v3.hash();

    // DAG A: insert v1, v2, v3
    let mut dag_a = BlockDag::new();
    dag_a.insert(v1.clone());
    dag_a.insert(v2.clone());
    dag_a.insert(v3.clone());

    // DAG B: insert v2, v1, v3
    let mut dag_b = BlockDag::new();
    dag_b.insert(v2.clone());
    dag_b.insert(v1.clone());
    dag_b.insert(v3.clone());

    // Both should have same structure
    assert_eq!(dag_a.len(), dag_b.len());
    assert_eq!(dag_a.len(), 3);
    assert_eq!(dag_a.current_round(), dag_b.current_round());

    // Same tips
    let mut tips_a = dag_a.tips();
    let mut tips_b = dag_b.tips();
    tips_a.sort();
    tips_b.sort();
    assert_eq!(tips_a, tips_b);
    assert_eq!(tips_a, vec![h3]);

    // Same ancestors
    assert_eq!(dag_a.ancestors(&h3), dag_b.ancestors(&h3));

    // Same children
    let mut ca = dag_a.children_of(&h1);
    let mut cb = dag_b.children_of(&h1);
    ca.sort();
    cb.sort();
    assert_eq!(ca, cb);
}

/// Round tracking is correct after multiple inserts.
/// Mutation: rounds map not storing round → vertices_in_round returns empty.
#[test]
fn round_tracking() {
    let sks: Vec<SecretKey> = (0..4u8).map(|i| SecretKey::from_bytes({
        let mut s = [0u8; 32]; s[0] = i + 20; s
    })).collect();
    let mut dag = BlockDag::new();

    // Round 0: 2 vertices
    let v0a = make_vertex(50, 0, vec![], &sks[0]);
    let v0b = make_vertex(51, 0, vec![], &sks[1]);
    let h0a = v0a.hash();
    let h0b = v0b.hash();
    dag.insert(v0a);
    dag.insert(v0b);

    assert_eq!(dag.vertices_in_round(0).len(), 2);
    assert_eq!(dag.current_round(), 0);

    // Round 1: 3 vertices
    let v1a = make_vertex(52, 1, vec![h0a, h0b], &sks[0]);
    let v1b = make_vertex(53, 1, vec![h0a], &sks[1]);
    let v1c = make_vertex(54, 1, vec![h0b], &sks[2]);
    dag.insert(v1a);
    dag.insert(v1b);
    dag.insert(v1c);

    assert_eq!(dag.vertices_in_round(1).len(), 3);
    assert_eq!(dag.current_round(), 1);

    // Round 2: 1 vertex
    let v2a = make_vertex(55, 2, vec![], &sks[3]);
    dag.insert(v2a);

    assert_eq!(dag.vertices_in_round(2).len(), 1);
    assert_eq!(dag.current_round(), 2);

    // NEGATIVE: round 3 has no vertices
    assert_eq!(dag.vertices_in_round(3).len(), 0);

    // Total vertices: 2 + 3 + 1 = 6
    assert_eq!(dag.len(), 6);
}

/// Distinct validators per round works correctly.
/// Mutation: distinct_validators_in_round counting duplicates → wrong count.
#[test]
fn distinct_validators_per_round() {
    let sk1 = SecretKey::from_bytes([30u8; 32]);
    let sk2 = SecretKey::from_bytes([31u8; 32]);
    let mut dag = BlockDag::new();

    // sk1 produces two vertices in round 0 (via insert, not try_insert)
    dag.insert(make_vertex(60, 0, vec![], &sk1));
    dag.insert(make_vertex(61, 0, vec![], &sk1));
    dag.insert(make_vertex(62, 0, vec![], &sk2));

    let validators = dag.distinct_validators_in_round(0);
    // Only 2 distinct validators despite 3 vertices
    assert_eq!(validators.len(), 2);
    assert!(validators.contains(&sk1.address()));
    assert!(validators.contains(&sk2.address()));

    // NEGATIVE: round 1 has no validators
    assert_eq!(dag.distinct_validators_in_round(1).len(), 0);
}

/// Descendant validator counts update incrementally on insert.
#[test]
fn descendant_count_updates_on_insert() {
    let sk0 = SecretKey::from_bytes([50u8; 32]);
    let sk1 = SecretKey::from_bytes([51u8; 32]);
    let sk2 = SecretKey::from_bytes([52u8; 32]);
    let mut dag = BlockDag::new();

    let v1 = make_vertex(80, 0, vec![], &sk0);
    let h1 = v1.hash();
    dag.insert(v1);
    assert_eq!(dag.descendant_validator_count(&h1), 0);

    let v2 = make_vertex(81, 1, vec![h1], &sk1);
    let h2 = v2.hash();
    dag.insert(v2);
    assert_eq!(dag.descendant_validator_count(&h1), 1, "v1 should have 1 descendant validator");
    assert_eq!(dag.descendant_validator_count(&h2), 0);

    let v3 = make_vertex(82, 2, vec![h2], &sk2);
    dag.insert(v3);
    assert_eq!(dag.descendant_validator_count(&h1), 2, "v1 should have 2 descendant validators");
    assert_eq!(dag.descendant_validator_count(&h2), 1, "v2 should have 1 descendant validator");
}

/// Same validator appearing multiple times only counts once.
#[test]
fn descendant_count_deduplicates_validators() {
    let sk0 = SecretKey::from_bytes([55u8; 32]);
    let mut dag = BlockDag::new();

    let v1 = make_vertex(90, 0, vec![], &sk0);
    let h1 = v1.hash();
    dag.insert(v1);

    // Same validator produces descendant
    let v2 = make_vertex(91, 1, vec![h1], &sk0);
    dag.insert(v2);

    assert_eq!(dag.descendant_validator_count(&h1), 1,
        "Same validator should only count once in descendant set");
}

/// Incremental counts match full BFS traversal result.
#[test]
fn incremental_count_matches_full_traversal() {
    let sks: Vec<SecretKey> = (0..4u8).map(|i| {
        let mut s = [0u8; 32]; s[0] = i + 60; SecretKey::from_bytes(s)
    }).collect();
    let mut dag = BlockDag::new();

    // Build 25 rounds × 4 validators = 100 vertices
    for round in 0u64..25 {
        let tips = dag.tips();
        let parents = if tips.is_empty() { vec![] } else { tips };
        for (i, sk) in sks.iter().enumerate() {
            let v = make_vertex(round * 4 + i as u64 + 100, round, parents.clone(), sk);
            dag.insert(v);
        }
    }
    assert_eq!(dag.len(), 100);

    // For each vertex, verify incremental count matches full BFS
    for round in 0u64..25 {
        for v in dag.vertices_in_round(round) {
            let h = v.hash();
            let full_descendants = dag.descendants(&h);
            let full_validators = dag.distinct_validators(&full_descendants);
            let incremental_count = dag.descendant_validator_count(&h);
            assert_eq!(
                incremental_count, full_validators.len(),
                "Mismatch at round {}: incremental={} vs full={}",
                round, incremental_count, full_validators.len()
            );
        }
    }
}

/// Finality check completes in < 10ms with precomputed counts.
#[test]
fn finality_check_uses_precomputed_counts() {
    use ultradag_coin::consensus::dag::BlockDag;
    use ultradag_coin::FinalityTracker;

    let sks: Vec<SecretKey> = (0..4u8).map(|i| {
        let mut s = [0u8; 32]; s[0] = i + 70; SecretKey::from_bytes(s)
    }).collect();
    let mut dag = BlockDag::new();
    let mut ft = FinalityTracker::new(3);
    for sk in &sks { ft.register_validator(sk.address()); }

    // Build 50 rounds × 4 validators = 200 vertices
    for round in 0u64..50 {
        let tips = dag.tips();
        let parents = if tips.is_empty() { vec![] } else { tips };
        for (i, sk) in sks.iter().enumerate() {
            let v = make_vertex(round * 4 + i as u64 + 200, round, parents.clone(), sk);
            dag.insert(v);
        }
    }

    let start = std::time::Instant::now();
    loop {
        let newly = ft.find_newly_finalized(&dag);
        if newly.is_empty() { break; }
    }
    let elapsed = start.elapsed();
    assert!(elapsed.as_millis() < 10,
        "Finality check with 200 vertices should complete in < 10ms, took {}ms", elapsed.as_millis());
}

/// Diamond DAG topology: v1, v2 → v3 (references both), v4 (references both).
/// Mutation: ancestors missing one branch in diamond → incomplete set.
#[test]
fn diamond_topology() {
    let sk1 = SecretKey::from_bytes([40u8; 32]);
    let sk2 = SecretKey::from_bytes([41u8; 32]);
    let sk3 = SecretKey::from_bytes([42u8; 32]);
    let sk4 = SecretKey::from_bytes([43u8; 32]);
    let mut dag = BlockDag::new();

    let v1 = make_vertex(70, 0, vec![], &sk1);
    let v2 = make_vertex(71, 0, vec![], &sk2);
    let h1 = v1.hash();
    let h2 = v2.hash();
    dag.insert(v1);
    dag.insert(v2);

    // Both v3 and v4 reference both v1 and v2
    let v3 = make_vertex(72, 1, vec![h1, h2], &sk3);
    let v4 = make_vertex(73, 1, vec![h1, h2], &sk4);
    let h3 = v3.hash();
    let h4 = v4.hash();
    dag.insert(v3);
    dag.insert(v4);

    // v1 has children v3 and v4
    let children = dag.children_of(&h1);
    assert_eq!(children.len(), 2);
    assert!(children.contains(&h3));
    assert!(children.contains(&h4));

    // v1 descendants = {v3, v4}
    let desc = dag.descendants(&h1);
    assert_eq!(desc.len(), 2);

    // v3 and v4 share same ancestors {v1, v2}
    assert_eq!(dag.ancestors(&h3), dag.ancestors(&h4));
    assert_eq!(dag.ancestors(&h3).len(), 2);

    // Tips should be v3 and v4
    let tips = dag.tips();
    assert_eq!(tips.len(), 2);
    assert!(tips.contains(&h3));
    assert!(tips.contains(&h4));
}
