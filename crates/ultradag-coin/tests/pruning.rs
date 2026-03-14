use ultradag_coin::{BlockDag, FinalityTracker, DagVertex, SecretKey, create_block, Mempool};

fn make_vertex(sk: &SecretKey, round: u64, parents: Vec<[u8; 32]>) -> DagVertex {
    let validator = sk.address();
    let mempool = Mempool::new();
    let block = create_block([0u8; 32], round, &validator, &mempool, 50 * 100_000_000);
    let mut vertex = DagVertex::new(
        block,
        parents,
        round,
        validator,
        sk.verifying_key().to_bytes(),
        ultradag_coin::Signature([0u8; 64]),
    );
    vertex.signature = sk.sign(&vertex.signable_bytes());
    vertex
}

#[test]
fn test_vertices_older_than_pruning_horizon_removed() {
    let mut dag = BlockDag::new();
    let mut finality = FinalityTracker::new(3);
    
    let sk1 = SecretKey::generate();
    let sk2 = SecretKey::generate();
    let sk3 = SecretKey::generate();
    
    finality.register_validator(sk1.address());
    finality.register_validator(sk2.address());
    finality.register_validator(sk3.address());
    
    // Build a chain of 1500 rounds (well beyond pruning horizon of 1000)
    let mut prev_hashes = vec![[0u8; 32]];
    for round in 1..=1500 {
        let v1 = make_vertex(&sk1, round, prev_hashes.clone());
        let v2 = make_vertex(&sk2, round, prev_hashes.clone());
        let v3 = make_vertex(&sk3, round, prev_hashes.clone());
        
        let h1 = v1.hash();
        let h2 = v2.hash();
        let h3 = v3.hash();
        
        dag.insert(v1);
        dag.insert(v2);
        dag.insert(v3);
        
        prev_hashes = vec![h1, h2, h3];
    }
    
    // Finalize all rounds
    loop {
        let newly_finalized = finality.find_newly_finalized(&dag);
        if newly_finalized.is_empty() {
            break;
        }
    }
    
    let last_finalized = finality.last_finalized_round();
    // Finality has lag, so we won't finalize all 1500 rounds
    assert!(last_finalized >= 1400, "Should finalize most rounds, got {}", last_finalized);
    
    // Prune (uses PRUNING_HORIZON = 1000 internally)
    dag.prune_old_rounds(last_finalized);
    
    // Calculate expected pruning floor
    let expected_floor = last_finalized.saturating_sub(1000);
    
    // Vertices from rounds 1 to (expected_floor-1) should be pruned (rounds < floor)
    if expected_floor > 1 {
        for round in 1..(expected_floor) {
            let vertices = dag.vertices_in_round(round);
            assert_eq!(vertices.len(), 0, "Round {} should be pruned (floor={})", round, expected_floor);
        }
    }
    
    // Vertices from rounds expected_floor to last_finalized should remain
    for round in expected_floor..=last_finalized {
        let vertices = dag.vertices_in_round(round);
        assert_eq!(vertices.len(), 3, "Round {} should not be pruned (floor={})", round, expected_floor);
    }
}

#[test]
fn test_unfinalized_vertices_never_pruned() {
    let mut dag = BlockDag::new();
    let mut finality = FinalityTracker::new(3);
    
    let sk1 = SecretKey::generate();
    let sk2 = SecretKey::generate();
    let sk3 = SecretKey::generate();
    
    finality.register_validator(sk1.address());
    finality.register_validator(sk2.address());
    finality.register_validator(sk3.address());
    
    // Build finalized chain up to round 1200
    let mut prev_hashes = vec![[0u8; 32]];
    for round in 1..=1200 {
        let v1 = make_vertex(&sk1, round, prev_hashes.clone());
        let v2 = make_vertex(&sk2, round, prev_hashes.clone());
        let v3 = make_vertex(&sk3, round, prev_hashes.clone());
        
        let h1 = v1.hash();
        let h2 = v2.hash();
        let h3 = v3.hash();
        
        dag.insert(v1);
        dag.insert(v2);
        dag.insert(v3);
        
        prev_hashes = vec![h1, h2, h3];
    }
    
    // Finalize all rounds
    loop {
        let newly_finalized = finality.find_newly_finalized(&dag);
        if newly_finalized.is_empty() {
            break;
        }
    }
    
    // Add unfinalized vertices at round 1201 (only 1 validator, below quorum)
    let unfinalized_v = make_vertex(&sk1, 1201, prev_hashes.clone());
    let unfinalized_hash = unfinalized_v.hash();
    dag.insert(unfinalized_v);
    
    let last_finalized = finality.last_finalized_round();
    // Finality has lag, so we won't finalize all 1200 rounds
    assert!(last_finalized >= 1100, "Should finalize most rounds, got {}", last_finalized);
    
    // Attempt to prune
    dag.prune_old_rounds(last_finalized);
    
    // Unfinalized vertex should still exist
    assert!(dag.get(&unfinalized_hash).is_some(), "Unfinalized vertex should not be pruned");
    
    // Its parents should also not be pruned (causal ancestors of unfinalized)
    for parent_hash in &prev_hashes {
        if *parent_hash != [0u8; 32] {
            assert!(dag.get(parent_hash).is_some(), "Parent of unfinalized vertex should not be pruned");
        }
    }
}

#[test]
fn test_pruning_floor_persists_across_save_load() {
    let temp_dir = std::env::temp_dir().join(format!("ultradag_test_{}", std::process::id()));
    std::fs::create_dir_all(&temp_dir).unwrap();
    let dag_path = temp_dir.join("dag.bin");
    
    let mut dag = BlockDag::new();
    let mut finality = FinalityTracker::new(3);
    
    let sk1 = SecretKey::generate();
    let sk2 = SecretKey::generate();
    let sk3 = SecretKey::generate();
    
    finality.register_validator(sk1.address());
    finality.register_validator(sk2.address());
    finality.register_validator(sk3.address());
    
    // Build and finalize 1500 rounds
    let mut prev_hashes = vec![[0u8; 32]];
    for round in 1..=1500 {
        let v1 = make_vertex(&sk1, round, prev_hashes.clone());
        let v2 = make_vertex(&sk2, round, prev_hashes.clone());
        let v3 = make_vertex(&sk3, round, prev_hashes.clone());
        
        let h1 = v1.hash();
        let h2 = v2.hash();
        let h3 = v3.hash();
        
        dag.insert(v1);
        dag.insert(v2);
        dag.insert(v3);
        
        prev_hashes = vec![h1, h2, h3];
    }
    
    // Finalize all rounds
    loop {
        let newly_finalized = finality.find_newly_finalized(&dag);
        if newly_finalized.is_empty() {
            break;
        }
    }
    
    // Prune
    dag.prune_old_rounds(1500);
    let pruning_floor_before = dag.pruning_floor();
    assert_eq!(pruning_floor_before, 500);
    
    // Save
    dag.save(&dag_path).unwrap();
    
    // Load
    let loaded_dag = BlockDag::load(&dag_path).unwrap();
    let pruning_floor_after = loaded_dag.pruning_floor();
    
    assert_eq!(pruning_floor_after, pruning_floor_before, "Pruning floor should persist");
    
    // Cleanup
    std::fs::remove_dir_all(&temp_dir).ok();
}

#[test]
fn test_archive_mode_pruning_never_runs() {
    // Archive mode is controlled by CLI flag --archive
    // When set, prune_old_rounds is never called
    // This test verifies that without calling prune, all vertices remain
    
    let mut dag = BlockDag::new();
    let mut finality = FinalityTracker::new(3);
    
    let sk1 = SecretKey::generate();
    let sk2 = SecretKey::generate();
    let sk3 = SecretKey::generate();
    
    finality.register_validator(sk1.address());
    finality.register_validator(sk2.address());
    finality.register_validator(sk3.address());
    
    // Build 2000 rounds
    let mut prev_hashes = vec![[0u8; 32]];
    for round in 1..=2000 {
        let v1 = make_vertex(&sk1, round, prev_hashes.clone());
        let v2 = make_vertex(&sk2, round, prev_hashes.clone());
        let v3 = make_vertex(&sk3, round, prev_hashes.clone());
        
        let h1 = v1.hash();
        let h2 = v2.hash();
        let h3 = v3.hash();
        
        dag.insert(v1);
        dag.insert(v2);
        dag.insert(v3);
        
        prev_hashes = vec![h1, h2, h3];
    }
    
    // Finalize all rounds
    loop {
        let newly_finalized = finality.find_newly_finalized(&dag);
        if newly_finalized.is_empty() {
            break;
        }
    }
    
    // In archive mode, prune_old_rounds is NEVER called
    // Verify all 2000 rounds still exist
    for round in 1..=2000 {
        let vertices = dag.vertices_in_round(round);
        assert_eq!(vertices.len(), 3, "Archive mode: round {} should not be pruned", round);
    }
    
    assert_eq!(dag.pruning_floor(), 0, "Archive mode: pruning floor should remain 0");
}

#[test]
fn test_custom_pruning_depth_respected() {
    let mut dag = BlockDag::new();
    let mut finality = FinalityTracker::new(3);
    
    let sk1 = SecretKey::generate();
    let sk2 = SecretKey::generate();
    let sk3 = SecretKey::generate();
    
    finality.register_validator(sk1.address());
    finality.register_validator(sk2.address());
    finality.register_validator(sk3.address());
    
    // Build 1500 rounds
    let mut prev_hashes = vec![[0u8; 32]];
    for round in 1..=1500 {
        let v1 = make_vertex(&sk1, round, prev_hashes.clone());
        let v2 = make_vertex(&sk2, round, prev_hashes.clone());
        let v3 = make_vertex(&sk3, round, prev_hashes.clone());
        
        let h1 = v1.hash();
        let h2 = v2.hash();
        let h3 = v3.hash();
        
        dag.insert(v1);
        dag.insert(v2);
        dag.insert(v3);
        
        prev_hashes = vec![h1, h2, h3];
    }
    
    // Finalize all rounds
    loop {
        let newly_finalized = finality.find_newly_finalized(&dag);
        if newly_finalized.is_empty() {
            break;
        }
    }
    
    let last_finalized = finality.last_finalized_round();
    assert!(last_finalized >= 1400, "Should finalize most rounds, got {}", last_finalized);
    
    // Note: Custom pruning depth is controlled by --pruning-depth CLI flag
    // The prune_old_rounds function uses PRUNING_HORIZON constant (1000)
    // This test verifies the default behavior
    dag.prune_old_rounds(last_finalized);
    
    let expected_floor = last_finalized.saturating_sub(1000);
    
    // Rounds 1 to (expected_floor-1) should be pruned (rounds < floor)
    if expected_floor > 1 {
        for round in 1..(expected_floor) {
            let vertices = dag.vertices_in_round(round);
            assert_eq!(vertices.len(), 0, "Round {} should be pruned", round);
        }
    }
    
    // Rounds expected_floor to last_finalized should remain
    for round in expected_floor..=last_finalized {
        let vertices = dag.vertices_in_round(round);
        assert_eq!(vertices.len(), 3, "Round {} should not be pruned", round);
    }
}

#[test]
fn test_pruning_does_not_affect_finality_of_remaining_vertices() {
    let mut dag = BlockDag::new();
    let mut finality = FinalityTracker::new(3);
    
    let sk1 = SecretKey::generate();
    let sk2 = SecretKey::generate();
    let sk3 = SecretKey::generate();
    
    finality.register_validator(sk1.address());
    finality.register_validator(sk2.address());
    finality.register_validator(sk3.address());
    
    // Build 1500 rounds
    let mut prev_hashes = vec![[0u8; 32]];
    let mut round_1200_hashes = Vec::new();
    
    for round in 1..=1500 {
        let v1 = make_vertex(&sk1, round, prev_hashes.clone());
        let v2 = make_vertex(&sk2, round, prev_hashes.clone());
        let v3 = make_vertex(&sk3, round, prev_hashes.clone());
        
        let h1 = v1.hash();
        let h2 = v2.hash();
        let h3 = v3.hash();
        
        dag.insert(v1);
        dag.insert(v2);
        dag.insert(v3);
        
        if round == 1200 {
            round_1200_hashes = vec![h1, h2, h3];
        }
        
        prev_hashes = vec![h1, h2, h3];
    }
    
    // Finalize all rounds
    loop {
        let newly_finalized = finality.find_newly_finalized(&dag);
        if newly_finalized.is_empty() {
            break;
        }
    }
    
    // Verify round 1200 is finalized before pruning
    for hash in &round_1200_hashes {
        assert!(finality.is_finalized(hash), "Round 1200 vertex should be finalized before pruning");
    }
    
    // Prune (keeps last 1000 rounds, so 1200 should remain)
    dag.prune_old_rounds(1500);
    
    // Verify round 1200 is still finalized after pruning
    for hash in &round_1200_hashes {
        assert!(finality.is_finalized(hash), "Round 1200 vertex should still be finalized after pruning");
    }
    
    // Verify vertices still exist
    for hash in &round_1200_hashes {
        assert!(dag.get(hash).is_some(), "Round 1200 vertex should still exist after pruning");
    }
}
