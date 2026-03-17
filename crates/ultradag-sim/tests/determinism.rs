//! Determinism verification tests.
//! Run the same scenario with different message orderings and verify
//! bit-identical state roots. Proves no non-determinism in the engine.

use ultradag_sim::oracle;
use ultradag_sim::harness::{SimConfig, SimHarness, Scenario};
use ultradag_sim::network::DeliveryPolicy;

/// Run 4 validators for 200 rounds with two different random orderings.
/// Both must produce identical state roots at the same finalized round.
#[test]
fn determinism_different_message_order() {
    let result = oracle::verify_ordering_independence(4, 200, 111, 222);
    assert!(result.is_ok(), "Determinism failed: {}", result.unwrap_err());
}

/// Run with 10 different seed pairs to stress-test ordering independence.
#[test]
fn determinism_multi_seed() {
    for i in 0..10u64 {
        let result = oracle::verify_ordering_independence(4, 100, i * 100, i * 100 + 50);
        assert!(result.is_ok(), "Determinism failed at seed pair ({}, {}): {}",
            i * 100, i * 100 + 50, result.unwrap_err());
    }
}

/// Run a staking scenario twice with different orderings.
/// Reward distribution must be deterministic despite message order.
#[test]
fn determinism_staking_rewards() {
    // Run A
    let config_a = SimConfig {
        num_honest: 4, byzantine: vec![], num_rounds: 500,
        delivery_policy: DeliveryPolicy::RandomOrder, seed: 333,
        txs_per_round: 0, check_every_round: false,
        scenario: Some(Scenario::StakingLifecycle), max_finality_lag: 50,
    };
    let mut harness_a = SimHarness::new(&config_a);
    let result_a = harness_a.run(&config_a);
    assert!(result_a.passed, "Run A failed: {:?}", result_a.violations);

    // Run B with different seed (different message order)
    let config_b = SimConfig {
        num_honest: 4, byzantine: vec![], num_rounds: 500,
        delivery_policy: DeliveryPolicy::RandomOrder, seed: 444,
        txs_per_round: 0, check_every_round: false,
        scenario: Some(Scenario::StakingLifecycle), max_finality_lag: 50,
    };
    let mut harness_b = SimHarness::new(&config_b);
    let result_b = harness_b.run(&config_b);
    assert!(result_b.passed, "Run B failed: {:?}", result_b.violations);

    // Both should have finalized to the same round and state root
    // (same transactions, different delivery order)
    let roots_a: Vec<[u8; 32]> = harness_a.validators.iter()
        .filter(|v| v.honest).map(|v| v.state_root()).collect();
    let roots_b: Vec<[u8; 32]> = harness_b.validators.iter()
        .filter(|v| v.honest).map(|v| v.state_root()).collect();

    // All honest validators within each run should agree
    assert!(roots_a.windows(2).all(|w| w[0] == w[1]),
        "Run A validators disagree on state root");
    assert!(roots_b.windows(2).all(|w| w[0] == w[1]),
        "Run B validators disagree on state root");

    // Both runs should produce the same state root
    // (Note: this may differ if different rounds were finalized due to ordering.
    // Only compare if same finalized round.)
    let round_a = harness_a.validators[0].last_finalized_round();
    let round_b = harness_b.validators[0].last_finalized_round();
    if round_a == round_b && round_a > 0 {
        assert_eq!(roots_a[0], roots_b[0],
            "Same finalized round {} but different state roots across runs", round_a);
    }
}
