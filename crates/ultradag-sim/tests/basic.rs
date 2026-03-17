use ultradag_sim::harness::{SimConfig, SimHarness};
use ultradag_sim::network::DeliveryPolicy;

/// 4 honest validators, 100 rounds, perfect network, no transactions.
#[test]
fn four_validators_perfect_network() {
    let config = SimConfig {
        num_honest: 4,
        byzantine: vec![],
        num_rounds: 100,
        delivery_policy: DeliveryPolicy::Perfect,
        seed: 1,
        txs_per_round: 0,
        check_every_round: true,
    };
    let mut harness = SimHarness::new(&config);
    let result = harness.run(&config);
    assert!(result.passed, "Violations: {:?}", result.violations);
    // All validators should have finalized to the same round
    let rounds: Vec<u64> = result.final_finalized_rounds.iter().map(|(_, r)| *r).collect();
    assert!(rounds.iter().all(|r| *r == rounds[0]),
        "All validators should finalize same round, got {:?}", rounds);
    assert!(rounds[0] > 90, "Should finalize most rounds: got {}", rounds[0]);
}

/// 4 honest validators, 200 rounds, perfect network, 20 transactions per round.
#[test]
fn four_validators_with_transactions() {
    let config = SimConfig {
        num_honest: 4,
        byzantine: vec![],
        num_rounds: 200,
        delivery_policy: DeliveryPolicy::Perfect,
        seed: 42,
        txs_per_round: 20,
        check_every_round: true,
    };
    let mut harness = SimHarness::new(&config);
    let result = harness.run(&config);
    assert!(result.passed, "Violations: {:?}", result.violations);
}

/// Single validator (minimum viable network with min_validators=1).
#[test]
fn single_validator() {
    let config = SimConfig {
        num_honest: 1,
        byzantine: vec![],
        num_rounds: 50,
        delivery_policy: DeliveryPolicy::Perfect,
        seed: 7,
        txs_per_round: 0,
        check_every_round: true,
    };
    let mut harness = SimHarness::new(&config);
    let result = harness.run(&config);
    assert!(result.passed, "Violations: {:?}", result.violations);
}
