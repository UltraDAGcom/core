use ultradag_sim::harness::{SimConfig, SimHarness};
use ultradag_sim::network::DeliveryPolicy;
use ultradag_sim::byzantine::ByzantineStrategy;

/// 21 validators, 1000 rounds, heavy tx load with 5% message loss.
#[test]
fn twenty_one_validators_heavy_load() {
    let config = SimConfig {
        num_honest: 21,
        byzantine: vec![],
        num_rounds: 1000,
        delivery_policy: DeliveryPolicy::Lossy { drop_probability: 0.05 },
        seed: 2024,
        txs_per_round: 50,
        check_every_round: false, scenario: None,
    };
    let mut harness = SimHarness::new(&config);
    let result = harness.run(&config);
    assert!(result.passed, "Violations: {:?}", result.violations);
}

/// 7 validators, 500 rounds, mixed Byzantine (1 equivocator + 1 crashed).
#[test]
fn mixed_byzantine_within_tolerance() {
    let config = SimConfig {
        num_honest: 5,
        byzantine: vec![ByzantineStrategy::Equivocator, ByzantineStrategy::Crash],
        num_rounds: 500,
        delivery_policy: DeliveryPolicy::RandomOrder,
        seed: 3030,
        txs_per_round: 10,
        check_every_round: true, scenario: None,
    };
    let mut harness = SimHarness::new(&config);
    let result = harness.run(&config);
    assert!(result.passed, "Violations: {:?}", result.violations);
}
