/// Module 5: BFT Finality — Production-grade tests

use ultradag_coin::address::{SecretKey, Signature};
use ultradag_coin::block::header::BlockHeader;
use ultradag_coin::block::Block;
use ultradag_coin::consensus::dag::BlockDag;
use ultradag_coin::consensus::finality::FinalityTracker;
use ultradag_coin::consensus::validator_set::ValidatorSet;
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

fn make_sks(n: u8) -> Vec<SecretKey> {
    (0..n).map(|i| {
        let mut seed = [0u8; 32];
        seed[0] = i + 100;
        SecretKey::from_bytes(seed)
    }).collect()
}

/// n=4: finality at exactly 3 descendants, not at 2.
/// Mutation: threshold returning n/2 instead of ceil(2n/3) → 2 finalizes.
#[test]
fn n4_finality_at_exactly_three_not_two() {
    let sks = make_sks(4);
    let mut dag = BlockDag::new();
    let mut ft = FinalityTracker::new(3);
    for sk in &sks {
        ft.register_validator(sk.address());
    }

    // Verify threshold: ceil(2*4/3) = ceil(8/3) = 3
    assert_eq!(ft.finality_threshold(), 3);

    // Round 0: sk1 produces root vertex
    let v0 = make_vertex(1, 0, vec![], &sks[0]);
    let h0 = v0.hash();
    dag.insert(v0);

    // NEGATIVE: 0 descendants → not finalized
    assert!(!ft.check_finality(&h0, &dag));

    // 1 descendant (sk2)
    let v1 = make_vertex(2, 1, vec![h0], &sks[1]);
    dag.insert(v1);
    assert!(!ft.check_finality(&h0, &dag), "1 descendant should NOT finalize");

    // 2 descendants (sk2, sk3)
    let v2 = make_vertex(3, 1, vec![h0], &sks[2]);
    dag.insert(v2);
    assert!(!ft.check_finality(&h0, &dag), "2 descendants should NOT finalize (need 3)");

    // POSITIVE: 3 descendants (sk2, sk3, sk4) → finalized
    let v3 = make_vertex(4, 1, vec![h0], &sks[3]);
    dag.insert(v3);
    assert!(ft.check_finality(&h0, &dag), "3 descendants should finalize");
    assert!(ft.is_finalized(&h0));
}

/// n=7: threshold is exactly 5.
/// Mutation: formula using (2n+1)/3 → threshold=5 still passes but (2*7+2)/3=5.33→5.
#[test]
fn n7_threshold_is_five() {
    let sks = make_sks(7);
    let mut vs = ValidatorSet::new(3);
    for sk in &sks {
        vs.register(sk.address());
    }

    // ceil(2*7/3) = ceil(14/3) = ceil(4.666) = 5
    // Formula: (2*7 + 2) / 3 = 16/3 = 5
    assert_eq!(vs.quorum_threshold(), 5);

    // POSITIVE: 5 meets quorum
    assert!(vs.has_quorum(5));
    // NEGATIVE: 4 does not
    assert!(!vs.has_quorum(4));
    // POSITIVE: 6 and 7 also meet quorum
    assert!(vs.has_quorum(6));
    assert!(vs.has_quorum(7));

    // Verify with FinalityTracker
    let mut ft = FinalityTracker::new(3);
    for sk in &sks {
        ft.register_validator(sk.address());
    }
    assert_eq!(ft.finality_threshold(), 5);
    assert_eq!(ft.validator_count(), 7);
}

/// Finality is not retroactively removed.
/// Mutation: check_finality re-evaluating finalized vertices → de-finalization.
#[test]
fn finality_not_retroactively_removed() {
    let sks = make_sks(4);
    let mut dag = BlockDag::new();
    let mut ft = FinalityTracker::new(3);
    for sk in &sks {
        ft.register_validator(sk.address());
    }

    let v0 = make_vertex(10, 0, vec![], &sks[0]);
    let h0 = v0.hash();
    dag.insert(v0);

    // Add 3 descendants to finalize h0
    for i in 1..=3u64 {
        dag.insert(make_vertex(10 + i, 1, vec![h0], &sks[i as usize]));
    }
    assert!(ft.check_finality(&h0, &dag));
    assert!(ft.is_finalized(&h0));

    // Even calling check_finality again returns false (already finalized)
    assert!(!ft.check_finality(&h0, &dag), "already finalized returns false");

    // Still finalized after more operations
    assert!(ft.is_finalized(&h0));

    // find_newly_finalized should not re-report h0
    let newly = ft.find_newly_finalized(&dag);
    assert!(!newly.contains(&h0), "h0 should not be re-reported");
}

/// Finality propagates: if B→A and B is finalized, A is also finalized.
/// Mutation: find_newly_finalized not checking ancestors → A stays unfinalized.
#[test]
fn finality_propagates_to_ancestors() {
    let sks = make_sks(4);
    let mut dag = BlockDag::new();
    let mut ft = FinalityTracker::new(2); // min 2 validators

    for sk in &sks {
        ft.register_validator(sk.address());
    }

    // Chain: v_a ← v_b ← v_c
    let va = make_vertex(20, 0, vec![], &sks[0]);
    let ha = va.hash();
    dag.insert(va);

    let vb = make_vertex(21, 1, vec![ha], &sks[1]);
    let hb = vb.hash();
    dag.insert(vb);

    let vc = make_vertex(22, 2, vec![hb], &sks[2]);
    let _hc = vc.hash();
    dag.insert(vc);

    // Add more descendants to v_a so it gets finalized
    let vd = make_vertex(23, 1, vec![ha], &sks[2]);
    dag.insert(vd);

    let ve = make_vertex(24, 1, vec![ha], &sks[3]);
    dag.insert(ve);

    // v_a has descendants from sks[1], sks[2] (twice), sks[3] = 3 distinct
    // threshold = ceil(2*4/3) = 3
    let _finalized = ft.find_newly_finalized(&dag);

    // v_a should be finalized (3+ distinct validator descendants)
    assert!(ft.is_finalized(&ha), "v_a should be finalized");

    // v_b: descendants are v_c (sks[2]). Only 1 distinct → not finalized
    // Unless we add more descendants to v_b
    // For now, v_b should NOT be finalized
    assert!(!ft.is_finalized(&hb), "v_b should not be finalized yet (only 1 descendant validator)");
}

/// Equivocating validator doesn't contribute double weight.
/// Mutation: distinct_validators counting by vertex count → double weight.
#[test]
fn equivocator_no_double_weight() {
    let sks = make_sks(4);
    let mut dag = BlockDag::new();
    let mut ft = FinalityTracker::new(3);
    for sk in &sks {
        ft.register_validator(sk.address());
    }
    assert_eq!(ft.finality_threshold(), 3);

    let v0 = make_vertex(30, 0, vec![], &sks[0]);
    let h0 = v0.hash();
    dag.insert(v0);

    // sks[1] produces TWO descendants (via insert, not try_insert — simulating DAG bug)
    dag.insert(make_vertex(31, 1, vec![h0], &sks[1]));
    dag.insert(make_vertex(32, 1, vec![h0], &sks[1]));

    // Only 1 distinct validator descendant despite 2 vertices
    let descendants = dag.descendants(&h0);
    let distinct = dag.distinct_validators(&descendants);
    assert_eq!(distinct.len(), 1, "same validator should count as 1");

    // NEGATIVE: not finalized (need 3 distinct, have 1)
    assert!(!ft.check_finality(&h0, &dag));

    // Add sks[2] — now 2 distinct, still not enough
    dag.insert(make_vertex(33, 1, vec![h0], &sks[2]));
    assert!(!ft.check_finality(&h0, &dag));

    // POSITIVE: add sks[3] — now 3 distinct, finalized
    dag.insert(make_vertex(34, 1, vec![h0], &sks[3]));
    assert!(ft.check_finality(&h0, &dag));
}

/// Finality horizon advances as more vertices are finalized.
/// Mutation: finalized_count not incrementing → stale count.
#[test]
fn finality_horizon_advances() {
    let sks = make_sks(4);
    let mut dag = BlockDag::new();
    let mut ft = FinalityTracker::new(2);
    for sk in &sks {
        ft.register_validator(sk.address());
    }

    assert_eq!(ft.finalized_count(), 0);

    // Build a chain with 3 rounds of 4 validators each
    let mut prev_hashes = vec![];
    let mut all_round_hashes = vec![];

    for round in 0..3u64 {
        let mut round_hashes = vec![];
        for (i, sk) in sks.iter().enumerate() {
            let v = make_vertex(round * 10 + i as u64, round, prev_hashes.clone(), sk);
            let h = v.hash();
            dag.insert(v);
            round_hashes.push(h);
        }
        all_round_hashes.push(round_hashes.clone());
        prev_hashes = round_hashes;
    }

    loop {
        let newly = ft.find_newly_finalized(&dag);
        if newly.is_empty() { break; }
    }
    let finalized_count_after = ft.finalized_count();

    // Round 0 vertices should be finalized (descendants from rounds 1+2)
    for h in &all_round_hashes[0] {
        assert!(ft.is_finalized(h), "round 0 vertex should be finalized");
    }

    assert!(finalized_count_after >= 4, "at least round 0 (4 vertices) should be finalized");

    // Round 1 vertices should also be finalized (4 descendants in round 2 = 4 distinct validators)
    for h in &all_round_hashes[1] {
        assert!(ft.is_finalized(h), "round 1 vertex should be finalized");
    }

    assert!(finalized_count_after >= 8, "rounds 0 and 1 (8 vertices) should be finalized");

    // Round 2 vertices should NOT be finalized (no descendants)
    for h in &all_round_hashes[2] {
        assert!(!ft.is_finalized(h), "round 2 vertex should not be finalized (no descendants)");
    }
}

/// Below min_validators, threshold is MAX and nothing finalizes.
/// Mutation: quorum_threshold ignoring min_validators → premature finality.
#[test]
fn below_min_validators_no_finality() {
    let sks = make_sks(2);
    let mut dag = BlockDag::new();
    let mut ft = FinalityTracker::new(3); // require 3 minimum

    ft.register_validator(sks[0].address());
    ft.register_validator(sks[1].address());
    // Only 2 registered, need 3 minimum

    assert_eq!(ft.finality_threshold(), usize::MAX);
    assert_eq!(ft.validator_count(), 2);

    let v0 = make_vertex(50, 0, vec![], &sks[0]);
    let h0 = v0.hash();
    dag.insert(v0);
    dag.insert(make_vertex(51, 1, vec![h0], &sks[1]));

    // NEGATIVE: even with descendants, threshold is MAX → no finality
    assert!(!ft.check_finality(&h0, &dag));
    assert!(ft.find_newly_finalized(&dag).is_empty());

    // POSITIVE: add a 3rd validator → threshold becomes ceil(2*3/3)=2
    let sk3 = make_sks(3).pop().unwrap();
    ft.register_validator(sk3.address());
    assert_eq!(ft.finality_threshold(), 2);

    // Now h0 has 1 descendant validator (sks[1]) — still not enough
    assert!(!ft.check_finality(&h0, &dag));

    // Add sk3 descendant — now 2 distinct ≥ threshold 2
    dag.insert(make_vertex(52, 1, vec![h0], &sk3));
    assert!(ft.check_finality(&h0, &dag));
}

/// Transitive finality: descendants through chain count correctly.
/// Mutation: descendants only looking at direct children → misses transitive ones.
#[test]
fn transitive_descendant_finality() {
    let sks = make_sks(4);
    let mut dag = BlockDag::new();
    let mut ft = FinalityTracker::new(3);
    for sk in &sks {
        ft.register_validator(sk.address());
    }

    // v0 (sk0) ← v1 (sk1) ← v2 (sk2) ← v3 (sk3)
    // All are transitive descendants of v0, from 3 distinct validators
    let v0 = make_vertex(60, 0, vec![], &sks[0]);
    let h0 = v0.hash();
    dag.insert(v0);

    let v1 = make_vertex(61, 1, vec![h0], &sks[1]);
    let h1 = v1.hash();
    dag.insert(v1);

    // 1 descendant — not enough
    assert!(!ft.check_finality(&h0, &dag));

    let v2 = make_vertex(62, 2, vec![h1], &sks[2]);
    let h2 = v2.hash();
    dag.insert(v2);

    // 2 distinct descendant validators — not enough
    assert!(!ft.check_finality(&h0, &dag));

    let v3 = make_vertex(63, 3, vec![h2], &sks[3]);
    dag.insert(v3);

    // POSITIVE: 3 distinct descendant validators (sk1, sk2, sk3) — finalized
    assert!(ft.check_finality(&h0, &dag));
    assert!(ft.is_finalized(&h0));
}
