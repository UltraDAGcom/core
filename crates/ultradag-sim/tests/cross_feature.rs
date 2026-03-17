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
        scenario: Some(Scenario::CrossFeature),
    };
    let mut harness = SimHarness::new(&config);
    let result = harness.run(&config);
    assert!(result.passed, "Violations: {:?}", result.violations);
    assert!(result.equivocations_detected > 0, "Should detect equivocation");
}

#[test]
fn cross_feature_lossy_network() {
    let config = SimConfig {
        num_honest: 5,
        byzantine: vec![ByzantineStrategy::Equivocator],
        num_rounds: 500,
        delivery_policy: DeliveryPolicy::Lossy { drop_probability: 0.1 },
        seed: 401,
        txs_per_round: 10,
        check_every_round: true,
        scenario: Some(Scenario::CrossFeature),
    };
    let mut harness = SimHarness::new(&config);
    let result = harness.run(&config);
    assert!(result.passed, "Violations: {:?}", result.violations);
}
