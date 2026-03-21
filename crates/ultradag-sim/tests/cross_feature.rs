use ultradag_sim::harness::{SimConfig, SimHarness, Scenario};
use ultradag_sim::network::DeliveryPolicy;
use ultradag_sim::byzantine::ByzantineStrategy;

#[test]
fn cross_feature_convergence() {
    let config = SimConfig {
        num_honest: 5,
        byzantine: vec![ByzantineStrategy::Equivocator],
        num_rounds: 500,
        delivery_policy: DeliveryPolicy::Perfect,
        seed: 400,
        txs_per_round: 10,
        check_every_round: true,
        scenario: Some(Scenario::CrossFeature), max_finality_lag: 50,
    };
    let mut harness = SimHarness::new(&config);
    let result = harness.run(&config);
    // Note: Equivocation detection is a known simulator limitation
    // The important thing is that invariants hold even with equivocation
    assert!(result.passed, "Violations: {:?}", result.violations);
}

#[test]
fn cross_feature_random_order() {
    let config = SimConfig {
        num_honest: 5,
        byzantine: vec![],
        num_rounds: 500,
        delivery_policy: DeliveryPolicy::RandomOrder,
        seed: 402,
        txs_per_round: 10,
        check_every_round: true,
        scenario: Some(Scenario::CrossFeature), max_finality_lag: 50,
    };
    let mut harness = SimHarness::new(&config);
    let result = harness.run(&config);
    assert!(result.passed, "Violations: {:?}", result.violations);
}
