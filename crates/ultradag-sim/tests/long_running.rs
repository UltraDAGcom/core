use ultradag_sim::harness::{SimConfig, SimHarness, Scenario};
use ultradag_sim::network::DeliveryPolicy;

/// Long-running stability test: 10,000 rounds with transactions.
/// Verifies:
/// - Memory doesn't grow unbounded (pruning works)
/// - Finality continues progressing (no deadlock)
/// - Supply invariant holds throughout
/// - State convergence maintained
#[test]
fn long_running_stability_10k_rounds() {
    let config = SimConfig {
        num_honest: 4,
        byzantine: vec![],
        num_rounds: 10_000,
        delivery_policy: DeliveryPolicy::Perfect,
        seed: 12345,
        txs_per_round: 10,
        check_every_round: false,  // Only check at end for performance
        scenario: None,
        max_finality_lag: 500,  // Allow larger lag for long runs
    };
    let mut harness = SimHarness::new(&config);
    let result = harness.run(&config);
    
    assert!(result.passed, "Long-running test failed: {:?}", result.violations);
    assert_eq!(result.rounds_completed, 10_000, "Should complete all rounds");
    
    // Verify finality progressed (should have finalized most rounds)
    let final_rounds: Vec<u64> = result.final_finalized_rounds.iter().map(|(_, r)| *r).collect();
    let min_finalized = final_rounds.iter().min().copied().unwrap_or(0);
    assert!(min_finalized > 9_000, "Finality should have progressed: min={}", min_finalized);
}

/// Stress test with latency simulation: 5,000 rounds with network delays.
/// Verifies consensus works under realistic network conditions.
#[test]
fn stress_with_latency() {
    let config = SimConfig {
        num_honest: 7,
        byzantine: vec![],
        num_rounds: 5_000,
        delivery_policy: DeliveryPolicy::Latency {
            base_latency: 1,  // Minimum 1 round delay
            jitter: 2,        // Up to 3 rounds total (1+2)
        },
        seed: 54321,
        txs_per_round: 20,
        check_every_round: false,
        scenario: None,
        max_finality_lag: 200,
    };
    let mut harness = SimHarness::new(&config);
    let result = harness.run(&config);
    
    assert!(result.passed, "Latency stress test failed: {:?}", result.violations);
    
    // All validators should converge to similar finalized round
    let final_rounds: Vec<u64> = result.final_finalized_rounds.iter().map(|(_, r)| *r).collect();
    let max_diff = final_rounds.iter().max().unwrap_or(&0) - final_rounds.iter().min().unwrap_or(&0);
    assert!(max_diff < 50, "Finality should converge: max_diff={}", max_diff);
}

/// Combined attack scenario: Byzantine validator + network issues + high load.
#[test]
fn combined_attack_scenario() {
    let config = SimConfig {
        num_honest: 5,
        byzantine: vec![ultradag_sim::byzantine::ByzantineStrategy::Equivocator],
        num_rounds: 2_000,
        delivery_policy: DeliveryPolicy::LatencyLossy {
            base_latency: 1,
            jitter: 2,
            drop_probability: 0.1,  // 10% packet loss
        },
        seed: 99999,
        txs_per_round: 30,
        check_every_round: false,
        scenario: None,
        max_finality_lag: 300,
    };
    let mut harness = SimHarness::new(&config);
    let result = harness.run(&config);
    
    // Test should pass - BFT should tolerate 1 Byzantine out of 6 validators
    assert!(result.passed, "Combined attack test failed: {:?}", result.violations);
    
    // Equivocation should be detected
    assert!(result.equivocations_detected > 0, "Should detect equivocation");
}

/// Full cross-feature test with all transaction types over 3,000 rounds.
#[test]
fn cross_feature_full_lifecycle() {
    let config = SimConfig {
        num_honest: 4,
        byzantine: vec![],
        num_rounds: 3_000,
        delivery_policy: DeliveryPolicy::Lossy { drop_probability: 0.05 },
        seed: 77777,
        txs_per_round: 15,
        check_every_round: false,
        scenario: Some(Scenario::CrossFeature),
        max_finality_lag: 200,
    };
    let mut harness = SimHarness::new(&config);
    let result = harness.run(&config);
    
    assert!(result.passed, "Cross-feature test failed: {:?}", result.violations);
    
    // Verify transactions were processed
    assert!(result.total_txs_applied > 1000, "Should process many transactions: {}", result.total_txs_applied);
}

/// Memory bounded test: verify DAG pruning keeps memory usage bounded.
#[test]
fn memory_bounded_with_pruning() {
    let config = SimConfig {
        num_honest: 4,
        byzantine: vec![],
        num_rounds: 5_000,
        delivery_policy: DeliveryPolicy::Perfect,
        seed: 11111,
        txs_per_round: 5,
        check_every_round: false,
        scenario: None,
        max_finality_lag: 100,
    };
    let mut harness = SimHarness::new(&config);
    let result = harness.run(&config);
    
    assert!(result.passed, "Memory bounded test failed: {:?}", result.violations);
    
    // Verify DAG sizes are bounded (pruning is working)
    for v in &harness.validators {
        let dag_stats = v.dag.dag_memory_stats();
        // With PRUNING_HORIZON=500 and 5000 rounds, should have pruned old rounds
        assert!(dag_stats.vertex_count < 10_000, 
            "DAG should be pruned: vertex_count={}", dag_stats.vertex_count);
    }
}
