use ultradag_coin::{
    BlockDag, DagVertex, FinalityTracker, SecretKey, Signature,
    Block, BlockHeader, CoinbaseTx,
};

fn make_vertex(
    height: u64,
    round: u64,
    parents: Vec<[u8; 32]>,
    sk: &SecretKey,
) -> DagVertex {
    let validator = sk.address();
    let block = Block {
        header: BlockHeader {
            version: 1,
            height,
            timestamp: 1_000_000 + height as i64,
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
fn parent_finality_guarantee_enforced() {
    // Setup: 3 validators for BFT threshold = 3
    let sk1 = SecretKey::generate();
    let sk2 = SecretKey::generate();
    let sk3 = SecretKey::generate();

    let mut dag = BlockDag::new();
    let mut finality = FinalityTracker::new(3);
    finality.register_validator(sk1.address());
    finality.register_validator(sk2.address());
    finality.register_validator(sk3.address());

    // Round 0: Genesis vertices (no parents)
    let v1_r0 = make_vertex(0, 0, vec![], &sk1);
    let v2_r0 = make_vertex(1, 0, vec![], &sk2);
    let v3_r0 = make_vertex(2, 0, vec![], &sk3);

    dag.insert(v1_r0.clone());
    dag.insert(v2_r0.clone());
    dag.insert(v3_r0.clone());

    // Round 1: Each validator references their own round 0 vertex
    let v1_r1 = make_vertex(3, 1, vec![v1_r0.hash()], &sk1);
    let v2_r1 = make_vertex(4, 1, vec![v2_r0.hash()], &sk2);
    let v3_r1 = make_vertex(5, 1, vec![v3_r0.hash()], &sk3);

    dag.insert(v1_r1.clone());
    dag.insert(v2_r1.clone());
    dag.insert(v3_r1.clone());

    // Round 2: Each validator references all round 1 vertices
    let v1_r2 = make_vertex(6, 2, vec![v1_r1.hash(), v2_r1.hash(), v3_r1.hash()], &sk1);
    let v2_r2 = make_vertex(7, 2, vec![v1_r1.hash(), v2_r1.hash(), v3_r1.hash()], &sk2);
    let v3_r2 = make_vertex(8, 2, vec![v1_r1.hash(), v2_r1.hash(), v3_r1.hash()], &sk3);

    dag.insert(v1_r2.clone());
    dag.insert(v2_r2.clone());
    dag.insert(v3_r2.clone());

    // First finalization pass - finalize vertices with sufficient descendants and finalized parents
    let _finalized = finality.find_newly_finalized(&dag);

    // Verify round 0 is finalized (genesis vertices)
    assert!(finality.is_finalized(&v1_r0.hash()), "v1_r0 should be finalized");
    assert!(finality.is_finalized(&v2_r0.hash()), "v2_r0 should be finalized");
    assert!(finality.is_finalized(&v3_r0.hash()), "v3_r0 should be finalized");

    // Continue finalization passes until all vertices are finalized
    loop {
        let newly_finalized = finality.find_newly_finalized(&dag);
        if newly_finalized.is_empty() {
            break;
        }
    }
    
    // Round 1 vertices should be finalized (they have 3 descendants in round 2)
    assert!(finality.is_finalized(&v1_r1.hash()), "v1_r1 should be finalized");
    assert!(finality.is_finalized(&v2_r1.hash()), "v2_r1 should be finalized");
    assert!(finality.is_finalized(&v3_r1.hash()), "v3_r1 should be finalized");
    // Round 2 vertices are tips (no descendants) — they cannot be finalized yet
    assert!(!finality.is_finalized(&v1_r2.hash()), "v1_r2 should NOT be finalized (no descendants)");
    assert!(!finality.is_finalized(&v2_r2.hash()), "v2_r2 should NOT be finalized (no descendants)");
    assert!(!finality.is_finalized(&v3_r2.hash()), "v3_r2 should NOT be finalized (no descendants)");
}

#[test]
fn child_not_finalized_before_parent() {
    // Setup: 3 validators
    let sk1 = SecretKey::generate();
    let sk2 = SecretKey::generate();
    let sk3 = SecretKey::generate();

    let mut dag = BlockDag::new();
    let mut finality = FinalityTracker::new(3);
    finality.register_validator(sk1.address());
    finality.register_validator(sk2.address());
    finality.register_validator(sk3.address());

    // Create a parent vertex
    let parent = make_vertex(0, 0, vec![], &sk1);
    dag.insert(parent.clone());

    // Create child vertices that reference the parent
    let child1 = make_vertex(1, 1, vec![parent.hash()], &sk1);
    let child2 = make_vertex(2, 1, vec![parent.hash()], &sk2);
    let child3 = make_vertex(3, 1, vec![parent.hash()], &sk3);

    dag.insert(child1.clone());
    dag.insert(child2.clone());
    dag.insert(child3.clone());

    // Create grandchild vertices
    let grandchild1 = make_vertex(4, 2, vec![child1.hash(), child2.hash(), child3.hash()], &sk1);
    let grandchild2 = make_vertex(5, 2, vec![child1.hash(), child2.hash(), child3.hash()], &sk2);
    let grandchild3 = make_vertex(6, 2, vec![child1.hash(), child2.hash(), child3.hash()], &sk3);

    dag.insert(grandchild1.clone());
    dag.insert(grandchild2.clone());
    dag.insert(grandchild3.clone());

    // Run finalization passes until all vertices are finalized
    loop {
        let newly_finalized = finality.find_newly_finalized(&dag);
        if newly_finalized.is_empty() {
            break;
        }
    }
    
    // Verify parent finality guarantee
    assert!(finality.is_finalized(&parent.hash()), "Parent should be finalized");
    assert!(finality.is_finalized(&child1.hash()), "Child should be finalized");
    assert!(finality.is_finalized(&child2.hash()), "Child should be finalized");
    assert!(finality.is_finalized(&child3.hash()), "Child should be finalized");
    // Grandchildren are tips (no descendants) — they cannot be finalized yet
    assert!(!finality.is_finalized(&grandchild1.hash()), "Grandchild should NOT be finalized (no descendants)");
    assert!(!finality.is_finalized(&grandchild2.hash()), "Grandchild should NOT be finalized (no descendants)");
    assert!(!finality.is_finalized(&grandchild3.hash()), "Grandchild should NOT be finalized (no descendants)");
}
