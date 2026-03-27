use ultradag_coin::{
    BlockDag, DagVertex, SecretKey, Signature,
    Block, BlockHeader, CoinbaseTx,
};

fn make_vertex(
    height: u64,
    round: u64,
    parents: Vec<[u8; 32]>,
    sk: &SecretKey,
) -> DagVertex {
    // Use current time for timestamp to pass validation (within 5 min past, 1 min future)
    let current_timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;
    
    let validator = sk.address();
    let mut block = Block {
        header: BlockHeader {
            version: 1,
            height,
            timestamp: current_timestamp, // Use current time for validation
            prev_hash: parents.first().copied().unwrap_or([0u8; 32]),
            merkle_root: [0u8; 32],
        },
        coinbase: CoinbaseTx {
            to: validator,
            amount: 0,
            height,
        },
        transactions: vec![],
    };
    block.header.merkle_root = block.compute_merkle_root();
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
fn equivocation_gossip_marks_byzantine() {
    // Setup: 4 validators
    let sk1 = SecretKey::generate();
    let sk2 = SecretKey::generate();
    let sk3 = SecretKey::generate(); // This one will equivocate
    let sk4 = SecretKey::generate();

    // Create 4 independent DAGs (one per validator/node)
    let mut dag1 = BlockDag::new();
    let mut dag2 = BlockDag::new();
    let mut dag3 = BlockDag::new();
    let mut dag4 = BlockDag::new();

    // Round 0: All validators produce vertices
    let v1_r0 = make_vertex(0, 0, vec![], &sk1);
    let v2_r0 = make_vertex(1, 0, vec![], &sk2);
    let v3_r0 = make_vertex(2, 0, vec![], &sk3);
    let v4_r0 = make_vertex(3, 0, vec![], &sk4);

    // All nodes insert round 0 vertices
    for dag in [&mut dag1, &mut dag2, &mut dag3, &mut dag4] {
        assert!(dag.insert(v1_r0.clone()));
        assert!(dag.insert(v2_r0.clone()));
        assert!(dag.insert(v3_r0.clone()));
        assert!(dag.insert(v4_r0.clone()));
    }

    // Round 1: Validator 3 equivocates
    // Create two different vertices from validator 3 in round 1
    let v3_r1_a = make_vertex(4, 1, vec![v3_r0.hash()], &sk3);
    let v3_r1_b = make_vertex(5, 1, vec![v3_r0.hash()], &sk3); // Different height = different hash

    // Verify they are genuinely different
    assert_ne!(v3_r1_a.hash(), v3_r1_b.hash());
    assert_eq!(v3_r1_a.validator, v3_r1_b.validator);
    assert_eq!(v3_r1_a.round, v3_r1_b.round);

    // Validator 3 sends vertex A to nodes 1 and 2
    let result1 = dag1.try_insert(v3_r1_a.clone());
    assert!(result1.is_ok() && result1.unwrap(), "Node 1 should accept first vertex from validator 3");
    
    let result2 = dag2.try_insert(v3_r1_a.clone());
    assert!(result2.is_ok() && result2.unwrap(), "Node 2 should accept first vertex from validator 3");

    // Validator 3 sends vertex B to nodes 3 and 4
    let result3 = dag3.try_insert(v3_r1_b.clone());
    assert!(result3.is_ok() && result3.unwrap(), "Node 3 should accept first vertex from validator 3");
    
    let result4 = dag4.try_insert(v3_r1_b.clone());
    assert!(result4.is_ok() && result4.unwrap(), "Node 4 should accept first vertex from validator 3");

    // At this point, nodes 1&2 have vertex A, nodes 3&4 have vertex B
    // Network is split - this is the dangerous state

    // Node 1 receives vertex B from the network (gossip)
    let result = dag1.try_insert(v3_r1_b.clone());
    assert!(result.is_err(), "Node 1 should detect equivocation when receiving vertex B");

    // Verify equivocation evidence was stored
    let evidence = dag1.get_equivocation_evidence(&sk3.address(), 1);
    assert!(evidence.is_some(), "Equivocation evidence should be stored");
    let [hash1, hash2] = evidence.unwrap();
    assert!(
        (hash1 == v3_r1_a.hash() && hash2 == v3_r1_b.hash()) ||
        (hash1 == v3_r1_b.hash() && hash2 == v3_r1_a.hash()),
        "Evidence should contain both vertex hashes"
    );

    // Simulate gossip: Node 1 broadcasts EquivocationEvidence to all peers
    // Node 2 processes the evidence
    let newly_marked = dag2.process_equivocation_evidence(&v3_r1_a, &v3_r1_b);
    assert!(newly_marked, "Node 2 should newly mark validator 3 as Byzantine");
    assert!(dag2.is_byzantine(&sk3.address()), "Node 2 should mark validator 3 as Byzantine");

    // Node 3 processes the evidence
    let newly_marked = dag3.process_equivocation_evidence(&v3_r1_a, &v3_r1_b);
    assert!(newly_marked, "Node 3 should newly mark validator 3 as Byzantine");
    assert!(dag3.is_byzantine(&sk3.address()), "Node 3 should mark validator 3 as Byzantine");

    // Node 4 processes the evidence
    let newly_marked = dag4.process_equivocation_evidence(&v3_r1_a, &v3_r1_b);
    assert!(newly_marked, "Node 4 should newly mark validator 3 as Byzantine");
    assert!(dag4.is_byzantine(&sk3.address()), "Node 4 should mark validator 3 as Byzantine");

    // All nodes now have validator 3 marked as Byzantine
    assert!(dag1.is_byzantine(&sk3.address()));
    assert!(dag2.is_byzantine(&sk3.address()));
    assert!(dag3.is_byzantine(&sk3.address()));
    assert!(dag4.is_byzantine(&sk3.address()));

    // Round 2: Honest validators continue, Byzantine validator 3 is ignored
    let v1_r2 = make_vertex(6, 2, vec![v1_r0.hash()], &sk1);
    let v2_r2 = make_vertex(7, 2, vec![v2_r0.hash()], &sk2);
    let v3_r2 = make_vertex(8, 2, vec![v3_r1_a.hash()], &sk3); // Validator 3 tries to continue
    let v4_r2 = make_vertex(9, 2, vec![v4_r0.hash()], &sk4);

    // All nodes accept vertices from honest validators
    for dag in [&mut dag1, &mut dag2, &mut dag3, &mut dag4] {
        assert!(dag.try_insert(v1_r2.clone()).unwrap(), "Should accept vertex from honest validator 1");
        assert!(dag.try_insert(v2_r2.clone()).unwrap(), "Should accept vertex from honest validator 2");
        assert!(dag.try_insert(v4_r2.clone()).unwrap(), "Should accept vertex from honest validator 4");
        
        // Vertex from Byzantine validator 3 should be rejected
        let result = dag.try_insert(v3_r2.clone());
        assert!(result.is_ok() && !result.unwrap(), "Should reject vertex from Byzantine validator 3");
    }

    // Verify network can continue with 3 honest validators
    // All 4 DAGs have the same honest vertices in round 2
    assert_eq!(dag1.vertices_in_round(2).len(), 3, "Node 1 should have 3 honest vertices in round 2");
    assert_eq!(dag2.vertices_in_round(2).len(), 3, "Node 2 should have 3 honest vertices in round 2");
    assert_eq!(dag3.vertices_in_round(2).len(), 3, "Node 3 should have 3 honest vertices in round 2");
    assert_eq!(dag4.vertices_in_round(2).len(), 3, "Node 4 should have 3 honest vertices in round 2");

    // All nodes have consistent view of round 2 (Byzantine validator excluded)
    let dag1_validators: Vec<_> = dag1.vertices_in_round(2).iter().map(|v| v.validator).collect();
    let dag2_validators: Vec<_> = dag2.vertices_in_round(2).iter().map(|v| v.validator).collect();
    assert_eq!(dag1_validators, dag2_validators, "All nodes should have consistent view of round 2");
}

#[test]
fn equivocation_evidence_validation() {
    let mut dag = BlockDag::new();
    let sk1 = SecretKey::generate();
    let sk2 = SecretKey::generate();

    let v1 = make_vertex(0, 0, vec![], &sk1);
    let v2 = make_vertex(1, 0, vec![], &sk2);
    let v3 = make_vertex(2, 1, vec![v1.hash()], &sk1);

    // Invalid evidence: different validators
    let result = dag.process_equivocation_evidence(&v1, &v2);
    assert!(!result, "Should reject evidence from different validators");

    // Invalid evidence: different rounds
    let result = dag.process_equivocation_evidence(&v1, &v3);
    assert!(!result, "Should reject evidence from different rounds");

    // Invalid evidence: same vertex
    let result = dag.process_equivocation_evidence(&v1, &v1);
    assert!(!result, "Should reject evidence with same vertex");

    // Valid evidence: same validator, same round, different hash
    let v1_alt = make_vertex(10, 0, vec![], &sk1); // Different height = different hash
    assert_ne!(v1.hash(), v1_alt.hash());
    
    let result = dag.process_equivocation_evidence(&v1, &v1_alt);
    assert!(result, "Should accept valid equivocation evidence");
    assert!(dag.is_byzantine(&sk1.address()), "Should mark validator as Byzantine");
}
