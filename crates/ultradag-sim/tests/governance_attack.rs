use ultradag_sim::harness::{SimConfig, SimHarness};
use ultradag_sim::network::DeliveryPolicy;
use ultradag_sim::byzantine::ByzantineStrategy;

#[test]
fn governance_takeover_fails_without_quorum() {
    let config = SimConfig {
        num_honest: 3,
        byzantine: vec![ByzantineStrategy::GovernanceTakeover],
        num_rounds: 300,
        delivery_policy: DeliveryPolicy::Perfect,
        seed: 2001,
        txs_per_round: 0,
        check_every_round: true,
        scenario: None,
        max_finality_lag: 50,
    };
    let mut harness = SimHarness::new(&config);
    let result = harness.run(&config);
    assert!(result.passed, "Violations: {:?}", result.violations);
}

#[test]
fn governance_extreme_values_bounded() {
    let config = SimConfig {
        num_honest: 3,
        byzantine: vec![ByzantineStrategy::GovernanceTakeover],
        num_rounds: 300,
        delivery_policy: DeliveryPolicy::Perfect,
        seed: 2002,
        txs_per_round: 5,
        check_every_round: true,
        scenario: None,
        max_finality_lag: 50,
    };
    let mut harness = SimHarness::new(&config);
    let result = harness.run(&config);
    assert!(result.passed, "Violations: {:?}", result.violations);
}
