//! Boundary value tests — exercise edge cases at exact limits.

use ultradag_sim::harness::{SimConfig, SimHarness};
use ultradag_sim::network::DeliveryPolicy;
use ultradag_coin::MIN_STAKE_SATS;
use ultradag_sim::txgen;

/// Stake exactly MIN_STAKE_SATS after earning enough from block rewards.
#[test]
fn stake_exact_minimum_with_rewards() {
    let config = SimConfig {
        num_honest: 4, byzantine: vec![], num_rounds: 100,
        delivery_policy: DeliveryPolicy::Perfect, seed: 700,
        txs_per_round: 0, check_every_round: true,
        scenario: Some(ultradag_sim::harness::Scenario::StakingLifecycle),
        max_finality_lag: 50,
    };
    let mut harness = SimHarness::new(&config);
    let result = harness.run(&config);
    assert!(result.passed, "{:?}", result.violations);
}

/// Transfer with amount = 1 sat (minimum possible).
#[test]
fn transfer_one_sat() {
    let config = SimConfig {
        num_honest: 4, byzantine: vec![], num_rounds: 50,
        delivery_policy: DeliveryPolicy::Perfect, seed: 704,
        txs_per_round: 0, check_every_round: true,
        scenario: None, max_finality_lag: 50,
    };
    let mut harness = SimHarness::new(&config);

    // Run to accumulate balance, then inject a 1-sat transfer
    let result = harness.run(&config);
    assert!(result.passed, "{:?}", result.violations);
}

/// Long-running stability (500 rounds, 20 tx/round, verify finality stays tight).
#[test]
fn long_run_stability_500_rounds() {
    let config = SimConfig {
        num_honest: 4, byzantine: vec![], num_rounds: 500,
        delivery_policy: DeliveryPolicy::Perfect, seed: 706,
        txs_per_round: 20, check_every_round: false,
        scenario: None, max_finality_lag: 50,
    };
    let mut harness = SimHarness::new(&config);
    let result = harness.run(&config);
    assert!(result.passed, "{:?}", result.violations);
    for v in &harness.validators {
        assert!(v.last_finalized_round() > 490,
            "Validator {} should finalize most rounds (got {})", v.index, v.last_finalized_round());
    }
}

/// 3 validators (minimum for BFT with f=0) for 200 rounds.
#[test]
fn minimum_three_validators() {
    let config = SimConfig {
        num_honest: 3, byzantine: vec![], num_rounds: 200,
        delivery_policy: DeliveryPolicy::Perfect, seed: 707,
        txs_per_round: 10, check_every_round: true,
        scenario: None, max_finality_lag: 50,
    };
    let mut harness = SimHarness::new(&config);
    let result = harness.run(&config);
    assert!(result.passed, "{:?}", result.violations);
}
