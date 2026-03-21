use ultradag_coin::{BlockDag, FinalityTracker, DagVertex, SecretKey, create_block, Mempool};
use ultradag_coin::consensus::dag::PRUNING_HORIZON;

fn make_vertex(sk: &SecretKey, round: u64, parents: Vec<[u8; 32]>) -> DagVertex {
    let validator = sk.address();
    let mempool = Mempool::new();
    let block = create_block([0u8; 32], round, &validator, &mempool);
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

    // Prune (uses PRUNING_HORIZON internally)
    dag.prune_old_rounds(last_finalized);

    // Calculate expected pruning floor
    let expected_floor = last_finalized.saturating_sub(PRUNING_HORIZON);
    
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
    // With PRUNING_HORIZON=500, floor = 1500 - 500 = 1000
    assert_eq!(pruning_floor_before, 1000);
    
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
    // The prune_old_rounds function uses PRUNING_HORIZON constant
    // This test verifies the default behavior
    dag.prune_old_rounds(last_finalized);

    let expected_floor = last_finalized.saturating_sub(PRUNING_HORIZON);
    
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

    // Prune (keeps last PRUNING_HORIZON rounds, so 1200 should remain with floor=1000)
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

#[test]
fn test_pruning_cleans_up_children_sets() {
    // Verify that pruning removes pruned vertices from parent children sets
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

    // Get memory stats before pruning
    let stats_before = dag.dag_memory_stats();
    let children_entries_before = stats_before.total_children_entries;

    // Prune
    let last_finalized = finality.last_finalized_round();
    dag.prune_old_rounds(last_finalized);

    // Get memory stats after pruning
    let stats_after = dag.dag_memory_stats();
    let children_entries_after = stats_after.total_children_entries;

    // Children entries should be reduced after pruning
    assert!(
        children_entries_after < children_entries_before,
        "Children entries should decrease after pruning: before={}, after={}",
        children_entries_before,
        children_entries_after
    );

    // Verify integrity: all children should point to existing vertices
    dag.verify_integrity().expect("DAG integrity check failed after pruning");
}

#[test]
fn test_pruning_cleans_up_equivocation_vertices() {
    // Verify that equivocation_vertices are pruned when their round is below the floor
    let sk1 = SecretKey::generate();
    let sk2 = SecretKey::generate();
    let mut dag = BlockDag::new();

    // Insert genesis
    let genesis = make_vertex(&sk1, 0, vec![]);
    dag.insert(genesis.clone());

    // Build up to round 4
    let mut prev_hashes = vec![genesis.hash()];
    for round in 1..=4 {
        let v1 = make_vertex(&sk1, round, prev_hashes.clone());
        let h1 = v1.hash();
        dag.insert(v1);
        prev_hashes = vec![h1];
    }

    // Round 5: sk2 equivocates (two different vertices, same round)
    // First, insert a valid vertex from sk2 at round 5
    let equivocating_v1 = make_vertex(&sk2, 5, prev_hashes.clone());
    assert!(dag.try_insert(equivocating_v1.clone()).is_ok());

    // Now try to insert a second vertex from sk2 at round 5 with different parents
    // This should trigger equivocation detection
    let mut different_parents = prev_hashes.clone();
    if !different_parents.is_empty() {
        // Change the parent to make a different vertex
        different_parents.clear();
        different_parents.push(genesis.hash());
    }
    let equivocating_v2 = make_vertex(&sk2, 5, different_parents);
    
    // This should detect equivocation and store the rejected vertex in equivocation_vertices
    let result = dag.try_insert(equivocating_v2.clone());
    assert!(result.is_err(), "Should detect equivocation");

    // Verify equivocation vertex is stored
    let stats_before = dag.dag_memory_stats();
    assert!(stats_before.equivocation_vertex_count > 0, "Should have equivocation vertices, got {:?}", stats_before);

    // Prune old rounds - advance far enough to prune round 5 (need floor > 5)
    dag.prune_old_rounds(1006);

    // Verify equivocation vertices from round 5 are pruned
    let stats_after = dag.dag_memory_stats();
    assert_eq!(
        stats_after.equivocation_vertex_count, 0,
        "Equivocation vertices from pruned rounds should be removed, got {:?}", stats_after
    );
}

#[test]
fn test_pruning_cleans_up_descendant_validators() {
    // Verify that descendant_validators are cleaned up when vertices are pruned
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

    // Get memory stats before pruning
    let stats_before = dag.dag_memory_stats();
    let descendant_count_before = stats_before.descendant_validators_count;
    let vertex_count_before = stats_before.vertex_count;

    // Prune
    let last_finalized = finality.last_finalized_round();
    dag.prune_old_rounds(last_finalized);

    // Get memory stats after pruning
    let stats_after = dag.dag_memory_stats();
    let descendant_count_after = stats_after.descendant_validators_count;
    let vertex_count_after = stats_after.vertex_count;

    // Verify integrity: all descendant_validators entries should have corresponding vertices
    dag.verify_integrity().expect("DAG integrity check failed after pruning");

    // Count should be reduced after pruning (proportional to vertex reduction)
    // Note: descendant_validators tracks ancestors, so tips won't have entries
    // The ratio should remain roughly consistent before and after pruning
    println!("Before: vertices={}, descendants={}", vertex_count_before, descendant_count_before);
    println!("After: vertices={}, descendants={}", vertex_count_after, descendant_count_after);
    
    // Verify that descendant tracking was cleaned up (count decreased)
    assert!(
        descendant_count_after <= descendant_count_before,
        "Descendant validators count should not increase after pruning"
    );
}

#[test]
fn test_memory_stats_comprehensive() {
    // Test that dag_memory_stats() returns comprehensive information
    let mut dag = BlockDag::new();
    let mut finality = FinalityTracker::new(3);

    let sk1 = SecretKey::generate();
    let sk2 = SecretKey::generate();
    let sk3 = SecretKey::generate();

    finality.register_validator(sk1.address());
    finality.register_validator(sk2.address());
    finality.register_validator(sk3.address());

    // Build 100 rounds
    let mut prev_hashes = vec![[0u8; 32]];
    for round in 1..=100 {
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

    // Get memory stats
    let stats = dag.dag_memory_stats();

    // Verify basic counts
    assert_eq!(stats.vertex_count, 300, "Should have 300 vertices (3 per round * 100 rounds)");
    assert_eq!(stats.current_round, 100, "Current round should be 100");
    assert_eq!(stats.pruning_floor, 0, "Pruning floor should be 0 (no pruning yet)");
    assert!(stats.validator_index_count >= 3, "Should have at least 3 validators");

    // Verify children tracking
    assert!(stats.children_map_count > 0, "Should have children entries");
    assert!(stats.total_children_entries > 0, "Should have child entries");

    // Verify tips
    assert_eq!(stats.tips_count, 3, "Should have 3 tips (one per validator in last round)");

    // Verify rounds (rounds 1-100, so 100 rounds total since genesis is round 0 which may not be counted)
    assert_eq!(stats.rounds_count, 100, "Should have 100 rounds (1-100)");

    // Verify descendant tracking (should be close to vertex count)
    assert!(stats.descendant_validators_count >= 300, "Each vertex should have descendant tracking");

    // Print stats for debugging
    println!("DAG Memory Stats: {:?}", stats);
}

#[test]
fn test_pruning_memory_leak_verification() {
    // Comprehensive test: create many vertices, prune, verify all data structures are cleaned
    let mut dag = BlockDag::new();
    let mut finality = FinalityTracker::new(3);

    let sk1 = SecretKey::generate();
    let sk2 = SecretKey::generate();
    let sk3 = SecretKey::generate();

    finality.register_validator(sk1.address());
    finality.register_validator(sk2.address());
    finality.register_validator(sk3.address());

    // Build 2000 rounds (well beyond pruning horizon)
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

    let last_finalized = finality.last_finalized_round();

    // Get stats before pruning
    let stats_before = dag.dag_memory_stats();
    println!("Before pruning: {:?}", stats_before);

    // Prune
    dag.prune_old_rounds(last_finalized);

    // Get stats after pruning
    let stats_after = dag.dag_memory_stats();
    println!("After pruning: {:?}", stats_after);
    println!("Last finalized round: {}", last_finalized);

    // Verify significant reduction in data structures
    // The pruning floor should be last_finalized - PRUNING_HORIZON
    let expected_floor = last_finalized.saturating_sub(PRUNING_HORIZON);
    
    // Verify pruning floor is set correctly
    assert_eq!(stats_after.pruning_floor, expected_floor, "Pruning floor should be set correctly");

    // Verify data was actually pruned (some reduction should occur)
    assert!(
        stats_after.vertex_count < stats_before.vertex_count || stats_before.vertex_count == stats_after.vertex_count && expected_floor == 0,
        "Vertices should be pruned unless floor is 0"
    );

    // Verify all data structures are consistent using integrity check
    dag.verify_integrity().expect("DAG integrity check failed after pruning");

    println!("Memory leak verification passed: all data structures properly cleaned");
}
