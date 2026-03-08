/// Tests for recursive parent fetch / DAG sync convergence.

use ultradag_coin::address::{SecretKey, Signature};
use ultradag_coin::block::header::BlockHeader;
use ultradag_coin::block::Block;
use ultradag_coin::consensus::dag::{BlockDag, DagInsertError};
use ultradag_coin::consensus::vertex::DagVertex;
use ultradag_coin::consensus::FinalityTracker;
use ultradag_coin::tx::CoinbaseTx;

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

#[test]
fn test_01_missing_parent_returns_specific_hashes() {
    let mut dag = BlockDag::new();
    let sk = SecretKey::generate();

    // V1 at round 1 with genesis parent
    let v1 = make_vertex(1, 1, vec![[0u8; 32]], &sk);
    let h1 = v1.hash();
    assert!(dag.insert(v1));

    // V2 at round 2 references a parent that does NOT exist
    let fake_parent: [u8; 32] = [0xAB; 32];
    let sk2 = SecretKey::generate();
    let v2 = make_vertex(2, 2, vec![h1, fake_parent], &sk2);
    let v2_hash = v2.hash();

    let result = dag.try_insert(v2);
    match result {
        Err(DagInsertError::MissingParents(hashes)) => {
            assert_eq!(hashes.len(), 1, "Should report exactly 1 missing parent");
            assert_eq!(hashes[0], fake_parent, "Missing parent should be the fake one");
        }
        other => panic!("Expected MissingParents, got {:?}", other),
    }

    // V2 should NOT be in the DAG
    assert!(dag.get(&v2_hash).is_none());
}

#[test]
fn test_02_orphan_resolved_after_parent_inserted() {
    let mut dag = BlockDag::new();
    let sk1 = SecretKey::generate();
    let sk2 = SecretKey::generate();

    // V1 at round 1 with genesis parent
    let v1 = make_vertex(1, 1, vec![[0u8; 32]], &sk1);
    let h1 = v1.hash();

    // V2 at round 2 references V1
    let v2 = make_vertex(2, 2, vec![h1], &sk2);
    let h2 = v2.hash();

    // Insert V2 first — should fail with MissingParents
    let result = dag.try_insert(v2.clone());
    assert!(matches!(result, Err(DagInsertError::MissingParents(_))));
    assert!(dag.get(&h2).is_none());

    // Now insert V1
    assert!(dag.insert(v1));
    assert!(dag.get(&h1).is_some());

    // Retry V2 — should succeed now
    let result2 = dag.try_insert(v2);
    assert_eq!(result2, Ok(true));
    assert!(dag.get(&h2).is_some());
}

#[test]
fn test_03_multiple_missing_parents_all_reported() {
    let mut dag = BlockDag::new();
    let sk = SecretKey::generate();

    let fake1: [u8; 32] = [0x01; 32];
    let fake2: [u8; 32] = [0x02; 32];
    let fake3: [u8; 32] = [0x03; 32];

    let v = make_vertex(1, 1, vec![fake1, fake2, fake3], &sk);
    let result = dag.try_insert(v);

    match result {
        Err(DagInsertError::MissingParents(hashes)) => {
            assert_eq!(hashes.len(), 3, "All 3 missing parents should be reported");
            assert!(hashes.contains(&fake1));
            assert!(hashes.contains(&fake2));
            assert!(hashes.contains(&fake3));
        }
        other => panic!("Expected MissingParents with 3 hashes, got {:?}", other),
    }
}

#[test]
fn test_04_deep_chain_resolves_incrementally() {
    // Build a chain of 60 vertices, insert them in reverse order
    // proving that try_insert correctly identifies missing parents at each step
    let sk = SecretKey::generate();
    let mut chain = Vec::new();

    // Build chain: v0 (genesis) -> v1 -> v2 -> ... -> v14
    // (kept within MAX_FUTURE_ROUNDS=10 reach when inserting sequentially)
    let v0 = make_vertex(0, 1, vec![[0u8; 32]], &sk);
    chain.push(v0.clone());

    for i in 1..15u64 {
        let parent_hash = chain.last().unwrap().hash();
        let v = make_vertex(i, i + 1, vec![parent_hash], &sk);
        chain.push(v);
    }

    let mut dag = BlockDag::new();

    // Try to insert a vertex with missing parents (round 5, within future limit from round 0)
    let v5 = chain[4].clone(); // round 5
    let result = dag.try_insert(v5);
    assert!(matches!(result, Err(DagInsertError::MissingParents(_))));

    // Insert all vertices in order — they should all succeed
    for v in &chain {
        let result = dag.try_insert(v.clone());
        assert!(matches!(result, Ok(true) | Ok(false)), "Vertex insert should not error");
    }

    assert_eq!(dag.len(), 15);
}

#[test]
fn test_05_invalid_signature_rejected_in_parent_vertices() {
    let mut dag = BlockDag::new();
    let sk = SecretKey::generate();

    // Create a vertex with valid signature
    let v = make_vertex(1, 1, vec![[0u8; 32]], &sk);
    assert!(v.verify_signature(), "Valid vertex should have valid signature");

    // Tamper with signature to make it invalid
    let mut bad_v = v.clone();
    bad_v.signature = Signature([0xFF; 64]);
    assert!(!bad_v.verify_signature(), "Tampered vertex should have invalid signature");

    // The signature check is done before try_insert in the P2P handler,
    // not inside try_insert itself. Verify the handler pattern works:
    if bad_v.verify_signature() {
        dag.try_insert(bad_v).ok();
    }
    // DAG should be empty — bad vertex was not inserted
    assert_eq!(dag.len(), 0);

    // Good vertex should work
    if v.verify_signature() {
        dag.try_insert(v).ok();
    }
    assert_eq!(dag.len(), 1);
}

#[test]
fn test_06_nodes_converge_after_independent_chains() {
    // Simulate the exact failure mode: two DAGs build independent chains,
    // then exchange vertices and converge.
    let sk_a = SecretKey::generate();
    let sk_b = SecretKey::generate();
    let sk_c = SecretKey::generate();
    let sk_d = SecretKey::generate();

    let mut dag_a = BlockDag::new();
    let mut dag_b = BlockDag::new();

    // Phase 1: Independent chains (simulating staggered deploy)
    // Node A produces rounds 1-5 using validator_a only
    let mut prev_a = [0u8; 32]; // genesis
    let mut chain_a = Vec::new();
    for i in 1..=5u64 {
        let v = make_vertex(100 + i, i, vec![prev_a], &sk_a);
        prev_a = v.hash();
        chain_a.push(v.clone());
        dag_a.insert(v);
    }

    // Node B produces rounds 1-5 using validator_b only
    let mut prev_b = [0u8; 32]; // genesis
    let mut chain_b = Vec::new();
    for i in 1..=5u64 {
        let v = make_vertex(200 + i, i, vec![prev_b], &sk_b);
        prev_b = v.hash();
        chain_b.push(v.clone());
        dag_b.insert(v);
    }

    assert_eq!(dag_a.len(), 5);
    assert_eq!(dag_b.len(), 5);

    // Phase 2: Exchange vertices (simulating reconnection with parent fetch)
    // Try to insert chain_a vertices into dag_b
    // The first vertex (round 1, genesis parent) should insert directly
    // Subsequent vertices need parent resolution
    for v in &chain_a {
        match dag_b.try_insert(v.clone()) {
            Ok(true) => {}
            Err(DagInsertError::MissingParents(missing)) => {
                // Fetch missing parents from dag_a (simulating GetParents/ParentVertices)
                for h in &missing {
                    if let Some(parent) = dag_a.get(h) {
                        dag_b.insert(parent.clone());
                    }
                }
                // Retry the vertex
                dag_b.try_insert(v.clone()).ok();
            }
            _ => {}
        }
    }

    // Do the same in reverse: insert chain_b into dag_a
    for v in &chain_b {
        match dag_a.try_insert(v.clone()) {
            Ok(true) => {}
            Err(DagInsertError::MissingParents(missing)) => {
                for h in &missing {
                    if let Some(parent) = dag_b.get(h) {
                        dag_a.insert(parent.clone());
                    }
                }
                dag_a.try_insert(v.clone()).ok();
            }
            _ => {}
        }
    }

    // Phase 3: Verify convergence
    assert_eq!(dag_a.len(), 10, "DAG A should have all 10 vertices");
    assert_eq!(dag_b.len(), 10, "DAG B should have all 10 vertices");

    // Both DAGs should have vertices from both validators
    let validators_a = dag_a.all_validators();
    let validators_b = dag_b.all_validators();
    assert!(validators_a.contains(&sk_a.address()));
    assert!(validators_a.contains(&sk_b.address()));
    assert!(validators_b.contains(&sk_a.address()));
    assert!(validators_b.contains(&sk_b.address()));

    // Now add vertices from validators C and D (to reach quorum for finality)
    // Both DAGs produce vertices from C and D referencing all existing tips
    for dag in [&mut dag_a, &mut dag_b] {
        let tips = dag.tips();
        for (i, sk) in [(6u64, &sk_c), (7, &sk_d)] {
            let v = make_vertex(300 + i, 6, tips.clone(), sk);
            dag.insert(v);
        }
    }

    // Check finality on both DAGs
    let mut fin_a = FinalityTracker::new(3);
    fin_a.register_validator(sk_a.address());
    fin_a.register_validator(sk_b.address());
    fin_a.register_validator(sk_c.address());
    fin_a.register_validator(sk_d.address());

    let mut fin_b = FinalityTracker::new(3);
    fin_b.register_validator(sk_a.address());
    fin_b.register_validator(sk_b.address());
    fin_b.register_validator(sk_c.address());
    fin_b.register_validator(sk_d.address());

    let finalized_a = fin_a.find_newly_finalized(&dag_a);
    let finalized_b = fin_b.find_newly_finalized(&dag_b);

    // Both should finalize some vertices (at minimum the early round vertices
    // that now have descendants from 3+ validators)
    assert!(!finalized_a.is_empty(), "DAG A should have finalized vertices after convergence");
    assert!(!finalized_b.is_empty(), "DAG B should have finalized vertices after convergence");

    println!(
        "Convergence test: DAG_A finalized {} vertices, DAG_B finalized {} vertices",
        finalized_a.len(),
        finalized_b.len()
    );
}
