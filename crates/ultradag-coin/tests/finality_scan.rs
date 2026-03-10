use ultradag_coin::address::SecretKey;
use ultradag_coin::consensus::dag::BlockDag;
use ultradag_coin::consensus::finality::FinalityTracker;
use ultradag_coin::consensus::DagVertex;
use ultradag_coin::block::{Block, BlockHeader, Coinbase};

fn make_vertex(sk: &SecretKey, round: u64, height: u64, parents: Vec<[u8; 32]>) -> DagVertex {
    let coinbase = Coinbase {
        to: sk.address(),
        amount: 5_000_000_000,
        height,
    };
    let header = BlockHeader {
        round,
        coinbase,
        merkle_root: [0u8; 32],
    };
    let block = Block {
        header,
        transactions: vec![],
    };
    DagVertex::new(block, parents, sk)
}

#[test]
fn test_finality_scan_starts_from_correct_round() {
    let validators: Vec<SecretKey> = (0..4).map(|_| SecretKey::generate()).collect();
    let mut dag = BlockDag::new();
    let mut fin = FinalityTracker::new(3);
    
    for sk in &validators {
        fin.register_validator(sk.address());
    }
    
    // Round 0: all validators produce
    for sk in &validators {
        let v = make_vertex(sk, 0, 0, vec![[0u8; 32]]);
        dag.insert(v);
    }
    
    // Find finalized - should finalize round 0
    let newly = fin.find_newly_finalized(&dag);
    assert!(!newly.is_empty());
    
    let last_finalized = fin.last_finalized_round();
    
    // Round 1: all validators produce
    for sk in &validators {
        let parents = dag.tips();
        let v = make_vertex(sk, 1, 1, parents);
        dag.insert(v);
    }
    
    // Find newly finalized - should scan from last_finalized + 1, not re-scan round 0
    let newly = fin.find_newly_finalized(&dag);
    
    // If scan_from was correct, it should find round 1 vertices
    // If scan_from was wrong (starting from last_finalized instead of +1), 
    // it would re-scan round 0 (wasted work)
    assert!(!newly.is_empty());
    assert!(fin.last_finalized_round() > last_finalized);
}

#[test]
fn test_finality_scan_does_not_rescan_finalized_rounds() {
    let validators: Vec<SecretKey> = (0..4).map(|_| SecretKey::generate()).collect();
    let mut dag = BlockDag::new();
    let mut fin = FinalityTracker::new(3);
    
    for sk in &validators {
        fin.register_validator(sk.address());
    }
    
    // Produce vertices for rounds 0-5
    for round in 0..6 {
        let parents = if round == 0 {
            vec![[0u8; 32]]
        } else {
            dag.tips()
        };
        
        for sk in &validators {
            let v = make_vertex(sk, round, round, parents.clone());
            dag.insert(v);
        }
    }
    
    // Finalize in batches
    let mut total_finalized = 0;
    loop {
        let newly = fin.find_newly_finalized(&dag);
        if newly.is_empty() {
            break;
        }
        total_finalized += newly.len();
    }
    
    // Should have finalized multiple rounds
    assert!(total_finalized > 0);
    
    // Call again - should find nothing (no re-scanning)
    let newly = fin.find_newly_finalized(&dag);
    assert_eq!(newly.len(), 0, "Should not re-scan already finalized rounds");
}

#[test]
fn test_finality_scan_handles_round_zero() {
    let validators: Vec<SecretKey> = (0..4).map(|_| SecretKey::generate()).collect();
    let mut dag = BlockDag::new();
    let mut fin = FinalityTracker::new(3);
    
    for sk in &validators {
        fin.register_validator(sk.address());
    }
    
    // Initial state: last_finalized_round = None
    assert_eq!(fin.last_finalized_round(), None);
    
    // Round 0: all validators produce
    for sk in &validators {
        let v = make_vertex(sk, 0, 0, vec![[0u8; 32]]);
        dag.insert(v);
    }
    
    // Should scan from round 0 (not round 0 + 1 = 1)
    let newly = fin.find_newly_finalized(&dag);
    assert!(!newly.is_empty());
    assert_eq!(fin.last_finalized_round(), Some(0));
}

#[test]
fn test_finality_scan_skips_gaps() {
    let validators: Vec<SecretKey> = (0..4).map(|_| SecretKey::generate()).collect();
    let mut dag = BlockDag::new();
    let mut fin = FinalityTracker::new(3);
    
    for sk in &validators {
        fin.register_validator(sk.address());
    }
    
    // Round 0
    for sk in &validators {
        let v = make_vertex(sk, 0, 0, vec![[0u8; 32]]);
        dag.insert(v);
    }
    
    fin.find_newly_finalized(&dag);
    
    // Skip round 1, go to round 2
    let parents = dag.tips();
    for sk in &validators {
        let v = make_vertex(sk, 2, 2, parents.clone());
        dag.insert(v);
    }
    
    // Should scan round 1 (empty) and round 2
    let newly = fin.find_newly_finalized(&dag);
    
    // May or may not finalize round 2 depending on descendants
    // But should not crash or infinite loop
    assert!(newly.len() <= 4 * 2); // At most 4 validators * 2 rounds
}

#[test]
fn test_finality_multi_pass_finalization() {
    let validators: Vec<SecretKey> = (0..4).map(|_| SecretKey::generate()).collect();
    let mut dag = BlockDag::new();
    let mut fin = FinalityTracker::new(3);
    
    for sk in &validators {
        fin.register_validator(sk.address());
    }
    
    // Build a deep DAG (10 rounds)
    for round in 0..10 {
        let parents = if round == 0 {
            vec![[0u8; 32]]
        } else {
            dag.tips()
        };
        
        for sk in &validators {
            let v = make_vertex(sk, round, round, parents.clone());
            dag.insert(v);
        }
    }
    
    // Multi-pass finalization
    let mut all_finalized = Vec::new();
    loop {
        let newly = fin.find_newly_finalized(&dag);
        if newly.is_empty() {
            break;
        }
        all_finalized.extend(newly);
    }
    
    // Should have finalized many vertices
    assert!(all_finalized.len() > 0);
    
    // Each subsequent call should start from last_finalized + 1
    // Verify by checking that we don't finalize the same vertex twice
    let unique_count = all_finalized.iter().collect::<std::collections::HashSet<_>>().len();
    assert_eq!(unique_count, all_finalized.len(), "Should not finalize same vertex twice");
}

#[test]
fn test_finality_scan_performance_no_redundant_work() {
    let validators: Vec<SecretKey> = (0..4).map(|_| SecretKey::generate()).collect();
    let mut dag = BlockDag::new();
    let mut fin = FinalityTracker::new(3);
    
    for sk in &validators {
        fin.register_validator(sk.address());
    }
    
    // Build 100 rounds
    for round in 0..100 {
        let parents = if round == 0 {
            vec![[0u8; 32]]
        } else {
            dag.tips()
        };
        
        for sk in &validators {
            let v = make_vertex(sk, round, round, parents.clone());
            dag.insert(v);
        }
    }
    
    // Finalize all
    let mut iterations = 0;
    loop {
        let newly = fin.find_newly_finalized(&dag);
        if newly.is_empty() {
            break;
        }
        iterations += 1;
        
        // Safety: prevent infinite loop
        if iterations > 200 {
            panic!("Too many iterations - possible infinite loop");
        }
    }
    
    // Should finalize in reasonable number of iterations
    // With proper scan_from, should be ~100 iterations (one per round)
    // With broken scan_from (re-scanning), would be much more
    assert!(iterations < 150, "Too many iterations: {}", iterations);
}

#[test]
fn test_finality_incremental_scanning() {
    let validators: Vec<SecretKey> = (0..4).map(|_| SecretKey::generate()).collect();
    let mut dag = BlockDag::new();
    let mut fin = FinalityTracker::new(3);
    
    for sk in &validators {
        fin.register_validator(sk.address());
    }
    
    // Add rounds incrementally and finalize after each
    for round in 0..10 {
        let parents = if round == 0 {
            vec![[0u8; 32]]
        } else {
            dag.tips()
        };
        
        for sk in &validators {
            let v = make_vertex(sk, round, round, parents.clone());
            dag.insert(v);
        }
        
        // Finalize after each round
        loop {
            let newly = fin.find_newly_finalized(&dag);
            if newly.is_empty() {
                break;
            }
        }
    }
    
    // Should have finalized up to some round
    assert!(fin.last_finalized_round().is_some());
    
    // Verify no more finalization possible
    let newly = fin.find_newly_finalized(&dag);
    assert_eq!(newly.len(), 0);
}
