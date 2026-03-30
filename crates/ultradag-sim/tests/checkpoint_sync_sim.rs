use ultradag_sim::harness::{SimConfig, SimHarness};
use ultradag_sim::network::{DeliveryPolicy, VirtualNetwork};
use ultradag_sim::validator::SimValidator;
use ultradag_sim::invariants;
use ultradag_coin::{SecretKey, DagVertex};
use ultradag_coin::consensus::compute_state_root;

/// Helper: hex-encode first 8 bytes of a 32-byte hash for display.
fn hex_short(bytes: &[u8; 32]) -> String {
    bytes.iter().take(8).map(|b| format!("{:02x}", b)).collect()
}

/// Comprehensive checkpoint fast-sync simulation.
///
/// Tests the entire checkpoint sync lifecycle:
/// 1. 5 validators run for 200 rounds with Perfect delivery
/// 2. State snapshot captured from validator 0 at the finalized round
/// 3. A 6th validator is created with empty state
/// 4. The 6th validator fast-syncs: load snapshot, insert suffix vertices,
///    mark pre-checkpoint vertices as finalized
/// 5. All 6 validators run 50 more rounds together
/// 6. Verify: finality advances, state roots converge, supply invariant holds,
///    new validator produces multi-parent vertices
#[test]
fn test_checkpoint_fast_sync() {
    // =========================================================================
    // Phase 1: Run 5 validators for 200 rounds
    // =========================================================================
    let config = SimConfig {
        num_honest: 5,
        byzantine: vec![],
        num_rounds: 200,
        delivery_policy: DeliveryPolicy::Perfect,
        seed: 7777,
        txs_per_round: 0,
        check_every_round: true,
        scenario: None,
        max_finality_lag: 50,
    };
    let mut harness = SimHarness::new(&config);
    let result = harness.run(&config);

    assert!(result.passed, "Phase 1 violations: {:?}", result.violations);

    // Flush remaining messages from phase 1. The harness.run() loop's last
    // round produces vertices that get broadcast but never delivered.
    harness.network.deliver(config.num_rounds + 1);
    for i in 0..harness.validators.len() {
        let messages = harness.network.drain_inbox(i);
        for vertex in messages {
            let _ = harness.validators[i].receive_vertex(vertex);
        }
    }
    for v in &mut harness.validators {
        v.run_finality();
    }

    // All validators should have finalized well past round 100
    let finalized_rounds: Vec<u64> = harness.validators.iter()
        .map(|v| v.last_finalized_round())
        .collect();
    for (i, &r) in finalized_rounds.iter().enumerate() {
        assert!(r > 100, "Validator {} only finalized to round {}, need > 100", i, r);
    }

    // All honest validators should agree on state root
    let roots: Vec<[u8; 32]> = harness.validators.iter()
        .map(|v| v.state_root())
        .collect();
    for i in 1..roots.len() {
        if finalized_rounds[i] == finalized_rounds[0] {
            assert_eq!(roots[i], roots[0],
                "Validators 0 and {} disagree on state root at round {}",
                i, finalized_rounds[0]);
        }
    }

    // =========================================================================
    // Phase 2: Capture checkpoint state from validator 0
    // =========================================================================
    let checkpoint_round = finalized_rounds[0];
    let checkpoint_snapshot = harness.validators[0].state.snapshot();
    let checkpoint_state_root = compute_state_root(&checkpoint_snapshot);
    let dag_current_round = harness.validators[0].dag.current_round();

    // Collect suffix vertices from validator 0's DAG. Include vertices from
    // a few rounds before checkpoint_round so that parent references can be
    // resolved within the suffix (vertices reference parents from the prior round).
    let mut suffix_vertices: Vec<DagVertex> = Vec::new();
    let source_dag = &harness.validators[0].dag;
    let suffix_start = if checkpoint_round > 10 { checkpoint_round - 10 } else { 1 };
    for round in suffix_start..=dag_current_round {
        for vertex in source_dag.vertices_in_round(round) {
            suffix_vertices.push(vertex.clone());
        }
    }
    suffix_vertices.sort_by_key(|v| v.round);

    // =========================================================================
    // Phase 3: Create a 6th validator and fast-sync it
    // =========================================================================
    let new_sk = SecretKey::from_bytes([6u8; 32]);
    // Use configured_validator_count = 5 (matching existing validators) since
    // that field is part of the state root hash and is preserved across
    // load_snapshot (set from CLI --validators, not from snapshot).
    let mut new_validator = SimValidator::new(5, new_sk, 3, 5);
    new_validator.honest = true;

    // Step 3a: Load state from checkpoint snapshot
    new_validator.state.load_snapshot(checkpoint_snapshot);

    // Verify state root matches after loading snapshot
    let new_state_root = new_validator.state_root();
    assert_eq!(new_state_root, checkpoint_state_root,
        "State root mismatch after load_snapshot:\n  expected: {}\n  got: {}",
        hex_short(&checkpoint_state_root), hex_short(&new_state_root));

    // Step 3b: Reset finality tracker to checkpoint round
    new_validator.finality.reset_to_checkpoint(checkpoint_round);
    assert_eq!(new_validator.finality.last_finalized_round(), checkpoint_round);

    // Step 3c: Set DAG pruning floor and current round
    let suffix_floor = if checkpoint_round > 10 { checkpoint_round - 10 } else { 0 };
    new_validator.dag.set_pruning_floor(suffix_floor);
    new_validator.dag.set_current_round(checkpoint_round);

    // Step 3d: Register all validators (existing + self) in finality tracker
    for v in &harness.validators {
        new_validator.finality.register_validator(v.address);
    }
    new_validator.finality.register_validator(new_validator.address);

    // Step 3e: Insert suffix vertices into the new validator's DAG.
    // Use dag.insert() (bypasses parent check) because the earliest suffix
    // vertices reference parents from before our DAG window. The state
    // snapshot provides the trust anchor, not the DAG parent chain.
    //
    // Vertices at or before checkpoint_round are marked as already finalized
    // to prevent double-application (snapshot already includes their effects).
    for vertex in &suffix_vertices {
        let hash = vertex.hash();
        new_validator.finality.register_validator(vertex.validator);
        new_validator.dag.insert(vertex.clone());
        if vertex.round <= checkpoint_round {
            new_validator.finality.mark_as_finalized(hash);
        }
    }

    // Verify the new node's DAG round is at or near the current round
    let new_dag_round = new_validator.dag.current_round();
    assert!(new_dag_round >= checkpoint_round,
        "New validator DAG round {} should be >= checkpoint round {}",
        new_dag_round, checkpoint_round);

    // Step 3f: Run finality on the new validator to process post-checkpoint vertices
    new_validator.run_finality();
    let new_fin_round = new_validator.last_finalized_round();
    assert!(new_fin_round >= checkpoint_round,
        "New validator finalized round {} should be >= checkpoint round {}",
        new_fin_round, checkpoint_round);

    // =========================================================================
    // Phase 4: Register the new validator on existing validators and continue
    // =========================================================================
    for v in &mut harness.validators {
        v.finality.register_validator(new_validator.address);
    }

    harness.validators.push(new_validator);
    harness.byzantine_strategies.push(None);
    harness.network = VirtualNetwork::new(6, DeliveryPolicy::Perfect, 9999);

    // =========================================================================
    // Phase 5: Run 50 more rounds with all 6 validators
    // =========================================================================
    let start_round = dag_current_round + 1;
    let end_round = start_round + 50;

    for round in start_round..=end_round {
        // Deliver pending messages
        harness.network.deliver(round);

        // Receive phase
        for i in 0..harness.validators.len() {
            let messages = harness.network.drain_inbox(i);
            for vertex in messages {
                let _ = harness.validators[i].receive_vertex(vertex);
            }
        }

        // Produce phase
        for i in 0..harness.validators.len() {
            let vertex = harness.validators[i].produce_vertex(round);
            harness.network.broadcast(i, vertex);
        }

        // Finality phase
        for v in &mut harness.validators {
            v.run_finality();
        }

        // Check invariants every 10 rounds
        if round % 10 == 0 {
            let check = invariants::check_all(&harness.validators, &[], round, 60);
            assert!(check.is_ok(),
                "Invariant violation at round {}: {:?}", round, check.err());
        }
    }

    // =========================================================================
    // Phase 6: Final verification
    // =========================================================================

    // 6a: All validators should have advanced finality beyond checkpoint
    let final_rounds: Vec<u64> = harness.validators.iter()
        .map(|v| v.last_finalized_round())
        .collect();
    for (i, &r) in final_rounds.iter().enumerate() {
        assert!(r > checkpoint_round,
            "Validator {} finalized round {} should be > checkpoint round {}",
            i, r, checkpoint_round);
    }

    // 6b: The new validator (index 5) should have finalized well past checkpoint
    let new_val_round = final_rounds[5];
    assert!(new_val_round > checkpoint_round + 20,
        "New validator should have finalized well past checkpoint: got {}",
        new_val_round);

    // 6c: State convergence -- all validators at the same finalized round
    // should have the same state root
    let final_check = invariants::check_all(&harness.validators, &[], end_round, 60);
    assert!(final_check.is_ok(),
        "Final invariant check failed: {:?}", final_check.err());

    // 6d: Specifically verify state roots match between new and original validators
    let new_root = harness.validators[5].state_root();
    let new_round = harness.validators[5].last_finalized_round();
    let mut found_match = false;
    for v in &harness.validators[..5] {
        if v.last_finalized_round() == new_round && new_round > 0 {
            assert_eq!(new_root, v.state_root(),
                "New validator state root differs from validator {} at round {}:\n  original: {}\n  new:      {}",
                v.index, new_round,
                hex_short(&v.state_root()), hex_short(&new_root));
            found_match = true;
            break;
        }
    }
    if new_round > 0 {
        assert!(found_match,
            "No original validator at finalized round {} to compare with new validator",
            new_round);
    }

    // 6e: Verify the new validator produced vertices with >1 parent
    // (meaning it referenced peers' vertices, not just a genesis stub)
    let new_addr = harness.validators[5].address;
    let new_dag = &harness.validators[5].dag;
    let mut multi_parent_count = 0;
    let mut total_new_vertices = 0;
    for round in start_round..=end_round {
        for vertex in new_dag.vertices_in_round(round) {
            if vertex.validator == new_addr {
                total_new_vertices += 1;
                if vertex.parent_hashes.len() > 1 {
                    multi_parent_count += 1;
                }
            }
        }
    }
    assert!(total_new_vertices > 0,
        "New validator should have produced vertices");
    assert!(multi_parent_count > 0,
        "New validator should reference peers' vertices as parents (got {} with >1 parent out of {})",
        multi_parent_count, total_new_vertices);

    // 6f: Supply invariant on all validators
    let supply_check = invariants::check_supply_invariant(&harness.validators);
    assert!(supply_check.is_ok(),
        "Supply invariant violated: {:?}", supply_check.err());

    eprintln!("Checkpoint fast-sync simulation PASSED:");
    eprintln!("  Checkpoint round: {}, suffix vertices: {}", checkpoint_round, suffix_vertices.len());
    eprintln!("  Final finalized (all 6): {:?}", final_rounds);
    eprintln!("  New validator: {} vertices ({} multi-parent)", total_new_vertices, multi_parent_count);
}
