use ultradag_coin::{BlockDag, DagVertex, FinalityTracker, SecretKey, K_PARENTS};
use ultradag_coin::block::{Block, BlockHeader};
use ultradag_coin::tx::CoinbaseTx;
use ultradag_coin::address::Signature;

fn make_vertex(
    nonce: u64,
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
    let block = Block {
        header: BlockHeader {
            version: 1,
            height: round,
            timestamp: current_timestamp, // Use current time for validation
            prev_hash: parents.first().copied().unwrap_or([0u8; 32]),
            merkle_root: [0u8; 32],
        },
        coinbase: CoinbaseTx {
            to: validator,
            amount: 0,
            height: round,
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
fn select_parents_returns_all_when_below_k() {
    let mut dag = BlockDag::new();
    let proposer = SecretKey::generate().address();
    
    // Create 10 tips (below K_PARENTS=32)
    let validators: Vec<SecretKey> = (0..10).map(|_| SecretKey::generate()).collect();
    for (i, sk) in validators.iter().enumerate() {
        let v = make_vertex(i as u64, 1, vec![], sk);
        dag.insert(v);
    }
    
    let selected = dag.select_parents(&proposer, 1, K_PARENTS);
    assert_eq!(selected.len(), 10, "Should return all tips when below K");
}

#[test]
fn select_parents_returns_k_when_above_k() {
    let mut dag = BlockDag::new();
    let proposer = SecretKey::generate().address();
    
    // Create 100 tips (above K_PARENTS=32)
    let validators: Vec<SecretKey> = (0..100).map(|_| SecretKey::generate()).collect();
    for (i, sk) in validators.iter().enumerate() {
        let v = make_vertex(i as u64, 1, vec![], sk);
        dag.insert(v);
    }
    
    let selected = dag.select_parents(&proposer, 1, K_PARENTS);
    assert_eq!(selected.len(), K_PARENTS, "Should return exactly K parents when above K");
}

#[test]
fn select_parents_is_deterministic() {
    let mut dag = BlockDag::new();
    let proposer = SecretKey::generate().address();
    
    // Create 50 tips
    let validators: Vec<SecretKey> = (0..50).map(|_| SecretKey::generate()).collect();
    for (i, sk) in validators.iter().enumerate() {
        let v = make_vertex(i as u64, 1, vec![], sk);
        dag.insert(v);
    }
    
    let selected1 = dag.select_parents(&proposer, 1, K_PARENTS);
    let selected2 = dag.select_parents(&proposer, 1, K_PARENTS);
    
    assert_eq!(selected1, selected2, "Parent selection should be deterministic");
}

#[test]
fn select_parents_differs_by_proposer() {
    let mut dag = BlockDag::new();
    let proposer1 = SecretKey::generate().address();
    let proposer2 = SecretKey::generate().address();
    
    // Create 50 tips
    let validators: Vec<SecretKey> = (0..50).map(|_| SecretKey::generate()).collect();
    for (i, sk) in validators.iter().enumerate() {
        let v = make_vertex(i as u64, 1, vec![], sk);
        dag.insert(v);
    }
    
    let selected1 = dag.select_parents(&proposer1, 1, K_PARENTS);
    let selected2 = dag.select_parents(&proposer2, 1, K_PARENTS);
    
    assert_ne!(selected1, selected2, "Different proposers should select different parents");
}

#[test]
fn finality_works_with_partial_parents() {
    let mut dag = BlockDag::new();
    let mut finality = FinalityTracker::new(3);
    
    // Create 100 validators (well above K_PARENTS=32)
    let validators: Vec<SecretKey> = (0..100).map(|_| SecretKey::generate()).collect();
    for sk in &validators {
        finality.register_validator(sk.address());
    }
    
    // Round 0: All validators produce vertices
    let mut round0_hashes = Vec::new();
    for (i, sk) in validators.iter().enumerate() {
        let v = make_vertex(i as u64, 0, vec![], sk);
        let h = v.hash();
        dag.insert(v);
        round0_hashes.push(h);
    }
    
    // Round 1: Each validator selects K_PARENTS from round 0
    for (i, sk) in validators.iter().enumerate() {
        let parents = dag.select_parents(&sk.address(), 0, K_PARENTS);
        assert!(parents.len() <= K_PARENTS, "Should not exceed K_PARENTS");

        let v = make_vertex(100 + i as u64, 1, parents, sk);
        dag.insert(v);
    }

    // Round 2: Each validator selects K_PARENTS from round 1
    // More rounds = more descendant propagation = finality can be reached
    for (i, sk) in validators.iter().enumerate() {
        let parents = dag.select_parents(&sk.address(), 1, K_PARENTS);
        let v = make_vertex(200 + i as u64, 2, parents, sk);
        dag.insert(v);
    }

    // Round 3: Another round for full descendant propagation
    for (i, sk) in validators.iter().enumerate() {
        let parents = dag.select_parents(&sk.address(), 2, K_PARENTS);
        let v = make_vertex(300 + i as u64, 3, parents, sk);
        dag.insert(v);
    }

    // Check finality: round 0 vertices should finalize
    // Threshold for 100 validators: ceil(2*100/3) = 67
    let threshold = finality.finality_threshold();
    assert_eq!(threshold, 67, "Threshold should be 67 for 100 validators");

    // Count how many round 0 vertices are finalized
    let newly_finalized = finality.find_newly_finalized(&dag);

    // With K=32 partial parents and 3 rounds of propagation,
    // descendants should reach enough validators for finality
    assert!(!newly_finalized.is_empty(), "Some vertices should finalize with partial parents after 3 rounds");
}

#[test]
fn dag_stays_connected_with_partial_parents() {
    let mut dag = BlockDag::new();
    
    // Create 100 validators
    let validators: Vec<SecretKey> = (0..100).map(|_| SecretKey::generate()).collect();
    
    // Round 0: All validators produce vertices
    for (i, sk) in validators.iter().enumerate() {
        let v = make_vertex(i as u64, 0, vec![], sk);
        dag.insert(v);
    }
    
    // Round 1: Each validator selects K_PARENTS from round 0
    for (i, sk) in validators.iter().enumerate() {
        let parents = dag.select_parents(&sk.address(), 0, K_PARENTS);
        let v = make_vertex(100 + i as u64, 1, parents, sk);
        dag.insert(v);
    }

    // Round 2: Each validator selects K_PARENTS from round 1
    for (i, sk) in validators.iter().enumerate() {
        let parents = dag.select_parents(&sk.address(), 1, K_PARENTS);
        let v = make_vertex(200 + i as u64, 2, parents, sk);
        dag.insert(v);
    }
    
    // Verify DAG connectivity: round 0 vertices should have descendants
    // from many validators through the partial parent graph
    let round0_hashes: Vec<[u8; 32]> = dag.vertices_in_round(0)
        .iter()
        .map(|v| v.hash())
        .collect();
    
    let mut total_descendants = 0;
    for hash in &round0_hashes {
        let desc_count = dag.descendant_validator_count(hash);
        total_descendants += desc_count;
    }
    
    // With 100 validators and K=32 parents, descendants should propagate
    // Average should be well above 0 (connectivity is maintained)
    let avg_descendants = total_descendants / round0_hashes.len();
    assert!(avg_descendants > 10, "Average descendants should be >10, got {}", avg_descendants);
}

#[test]
fn partial_parents_removes_64_validator_ceiling() {
    let mut dag = BlockDag::new();
    
    // Create 200 validators (way above old MAX_PARENTS=64 limit)
    let validators: Vec<SecretKey> = (0..200).map(|_| SecretKey::generate()).collect();
    
    // Round 0: All 200 validators produce vertices
    let mut round0_hashes = Vec::new();
    for (i, sk) in validators.iter().enumerate() {
        let v = make_vertex(i as u64, 0, vec![], sk);
        let h = v.hash();
        dag.insert(v);
        round0_hashes.push(h);
    }
    
    assert_eq!(dag.vertices_in_round(0).len(), 200, "Should have 200 vertices in round 0");
    assert_eq!(dag.tips().len(), 200, "Should have 200 tips after round 0");
    
    // Round 1: Pre-compute all parent selections before inserting any vertices
    // (tips change as we insert, so we need to capture them first)
    let mut round1_vertices = Vec::new();
    for (i, sk) in validators.iter().enumerate() {
        let parents = dag.select_parents(&sk.address(), 0, K_PARENTS);
        assert_eq!(parents.len(), K_PARENTS, "Should select exactly K_PARENTS from 200 vertices");
        
        let v = make_vertex(200 + i as u64, 1, parents, sk);
        round1_vertices.push(v);
    }
    
    // Now insert all round 1 vertices
    for v in round1_vertices {
        let result = dag.try_insert(v);
        assert!(result.is_ok(), "Insertion should succeed with partial parents");
    }
    
    assert_eq!(dag.vertices_in_round(1).len(), 200, "Should have 200 vertices in round 1");
}

#[test]
fn partial_parents_maintains_parent_diversity() {
    let mut dag = BlockDag::new();
    
    // Create 100 validators
    let validators: Vec<SecretKey> = (0..100).map(|_| SecretKey::generate()).collect();
    
    // Round 0: All validators produce vertices
    let mut round0_hashes = Vec::new();
    for (i, sk) in validators.iter().enumerate() {
        let v = make_vertex(i as u64, 0, vec![], sk);
        let h = v.hash();
        dag.insert(v);
        round0_hashes.push(h);
    }
    
    // Track which round 0 vertices are referenced by at least one round 1 vertex
    let mut referenced = std::collections::HashSet::new();
    
    // Round 1: Each validator selects K_PARENTS from round 0
    for (i, sk) in validators.iter().enumerate() {
        let parents = dag.select_parents(&sk.address(), 0, K_PARENTS);
        for parent in &parents {
            referenced.insert(*parent);
        }
        let v = make_vertex(100 + i as u64, 1, parents, sk);
        dag.insert(v);
    }
    
    // With 100 validators each selecting 32 parents from 100 options,
    // we should have good coverage (not all 100, but significant)
    let coverage_percent = (referenced.len() * 100) / round0_hashes.len();
    assert!(coverage_percent > 50, "Should reference >50% of round 0 vertices, got {}%", coverage_percent);
}
