use ultradag_sim::harness::{SimConfig, SimHarness};
use ultradag_sim::network::DeliveryPolicy;
use ultradag_sim::validator::SimValidator;
use ultradag_coin::SecretKey;
use ultradag_sim::invariants;

/// One validator starts 50 rounds late. It receives all past vertices on join
/// and must converge to the same state as validators present from round 1.
#[test]
fn late_joiner_converges() {
    // Phase 1: Run 4 validators for 50 rounds
    let config = SimConfig {
        num_honest: 4,
        byzantine: vec![],
        num_rounds: 50,
        delivery_policy: DeliveryPolicy::Perfect,
        seed: 42,
        txs_per_round: 0,
        check_every_round: false,
    };
    let mut harness = SimHarness::new(&config);
    let result = harness.run(&config);
    assert!(result.passed, "Phase 1 failed: {:?}", result.violations);

    // Phase 2: Create a 5th validator and feed it all vertices from validator 0's DAG
    // IMPORTANT: The new validator must start with the SAME configured_validator_count (4)
    // as the existing validators, since that's part of the state root hash. It will
    // produce vertices but not affect the configured count — matching how a real new
    // node joins (it syncs state from checkpoint, which includes configured_validator_count).
    let new_sk = SecretKey::from_bytes([5u8; 32]);
    let mut new_validator = SimValidator::new(4, new_sk, 3, 4);
    new_validator.honest = true;

    // Register all existing validators in the new validator's finality tracker
    for v in &harness.validators {
        new_validator.finality.register_validator(v.address);
    }
    // Register self
    new_validator.finality.register_validator(new_validator.address);

    // Register new validator in existing validators' finality trackers
    // Keep configured_validators at 4 (matching genesis state root)
    for v in &mut harness.validators {
        v.finality.register_validator(new_validator.address);
    }

    // Feed all vertices from validator 0's DAG to the new validator
    // Collect vertices by round for ordered replay
    let existing_dag = &harness.validators[0].dag;
    let max_round = existing_dag.current_round();
    for round in 1..=max_round {
        for vertex in existing_dag.vertices_in_round(round) {
            let _ = new_validator.receive_vertex(vertex.clone());
        }
    }

    // Run finality on the new validator
    new_validator.run_finality();

    // Verify convergence: new validator should have same state root as existing validators
    let new_root = new_validator.state_root();
    let new_round = new_validator.last_finalized_round();

    // Find an existing validator at the same round
    let mut found_match = false;
    for v in &harness.validators {
        if v.last_finalized_round() == new_round && new_round > 0 {
            assert_eq!(new_root, v.state_root(),
                "Late joiner state root differs at round {}:\n  existing: {:?}\n  new: {:?}",
                new_round, &v.state_root()[..4], &new_root[..4]);
            found_match = true;
            break;
        }
    }

    if new_round > 0 {
        assert!(found_match, "No existing validator at round {} to compare with", new_round);
    }

    // Phase 3: Continue the simulation with 5 validators for 50 more rounds
    // Add new validator to harness
    harness.validators.push(new_validator);
    harness.byzantine_strategies.push(None);
    harness.network = ultradag_sim::network::VirtualNetwork::new(5, DeliveryPolicy::Perfect, 100);

    let config2 = SimConfig {
        num_honest: 5,
        byzantine: vec![],
        num_rounds: 50,
        delivery_policy: DeliveryPolicy::Perfect,
        seed: 100,
        txs_per_round: 0,
        check_every_round: true,
    };

    // Manually run rounds 51-100
    for round in 51..=100 {
        harness.network.deliver(round);

        for i in 0..harness.validators.len() {
            let messages = harness.network.drain_inbox(i);
            for vertex in messages {
                let _ = harness.validators[i].receive_vertex(vertex);
            }
        }

        for i in 0..harness.validators.len() {
            let vertex = harness.validators[i].produce_vertex(round);
            harness.network.broadcast(i, vertex);
        }

        for v in &mut harness.validators {
            v.run_finality();
        }

        // Check invariants
        let result = invariants::check_all(&harness.validators, &[]);
        assert!(result.is_ok(), "Post-join round {} failed: {:?}", round, result.err());
    }
}
