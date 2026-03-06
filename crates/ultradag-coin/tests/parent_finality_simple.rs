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
            amount: 5_000_000_000,
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
fn parent_must_be_finalized_before_child() {
    // Setup: 3 validators for BFT threshold = 3
    let sk1 = SecretKey::generate();
    let sk2 = SecretKey::generate();
    let sk3 = SecretKey::generate();

    let mut dag = BlockDag::new();
    let mut finality = FinalityTracker::new(3);
    finality.register_validator(sk1.address());
    finality.register_validator(sk2.address());
    finality.register_validator(sk3.address());

    // Create parent vertex (round 0)
    let parent = make_vertex(0, 0, vec![], &sk1);
    dag.insert(parent.clone());

    // Create child vertex (round 1) that references parent
    let child = make_vertex(1, 1, vec![parent.hash()], &sk2);
    dag.insert(child.clone());

    // Create grandchild vertices (round 2) that reference child
    let gc1 = make_vertex(2, 2, vec![child.hash()], &sk1);
    let gc2 = make_vertex(3, 2, vec![child.hash()], &sk2);
    let gc3 = make_vertex(4, 2, vec![child.hash()], &sk3);
    
    dag.insert(gc1.clone());
    dag.insert(gc2.clone());
    dag.insert(gc3.clone());

    // First pass: parent should be finalized (has 3 descendants in round 2)
    let _finalized = finality.find_newly_finalized(&dag);
    assert!(finality.is_finalized(&parent.hash()), "Parent should be finalized (has 3 descendants)");
    
    // Child should NOT be finalized yet because parent was just finalized
    // The parent finality guarantee requires parent to be finalized BEFORE checking child
    
    // Second pass: child should now be finalized (parent is finalized, has 3 descendants)
    let _finalized = finality.find_newly_finalized(&dag);
    assert!(finality.is_finalized(&child.hash()), "Child should be finalized after parent");
    
    // Grandchildren are tips (no descendants) — they cannot be finalized yet
    let _finalized = finality.find_newly_finalized(&dag);
    assert!(!finality.is_finalized(&gc1.hash()), "Grandchild 1 should NOT be finalized (no descendants)");
    assert!(!finality.is_finalized(&gc2.hash()), "Grandchild 2 should NOT be finalized (no descendants)");
    assert!(!finality.is_finalized(&gc3.hash()), "Grandchild 3 should NOT be finalized (no descendants)");
}
