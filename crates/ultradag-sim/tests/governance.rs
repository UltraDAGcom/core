use ultradag_sim::harness::{SimConfig, SimHarness, Scenario};
use ultradag_sim::network::DeliveryPolicy;

#[test]
fn governance_parameter_change() {
    let config = SimConfig {
        num_honest: 4,
        byzantine: vec![],
        num_rounds: 200,
        delivery_policy: DeliveryPolicy::Perfect,
        seed: 300,
        txs_per_round: 0,
        check_every_round: true,
        scenario: Some(Scenario::GovernanceParameterChange),
    };
    let mut harness = SimHarness::new(&config);
    let result = harness.run(&config);
    assert!(result.passed, "Violations: {:?}", result.violations);
}

#[test]
fn governance_with_reorder() {
    let config = SimConfig {
        num_honest: 4,
        byzantine: vec![],
        num_rounds: 200,
        delivery_policy: DeliveryPolicy::RandomOrder,
        seed: 301,
        txs_per_round: 0,
        check_every_round: true,
        scenario: Some(Scenario::GovernanceParameterChange),
    };
    let mut harness = SimHarness::new(&config);
    let result = harness.run(&config);
    assert!(result.passed, "Violations: {:?}", result.violations);
}
