use ultradag_sim::harness::{SimConfig, SimHarness, Scenario};
use ultradag_sim::network::DeliveryPolicy;

#[test]
fn staking_lifecycle_supply_holds() {
    let config = SimConfig {
        num_honest: 4,
        byzantine: vec![],
        num_rounds: 500,
        delivery_policy: DeliveryPolicy::Perfect,
        seed: 100,
        txs_per_round: 0,
        check_every_round: true,
        scenario: Some(Scenario::StakingLifecycle), max_finality_lag: 50,
    };
    let mut harness = SimHarness::new(&config);
    let result = harness.run(&config);
    assert!(result.passed, "Violations: {:?}", result.violations);
}

#[test]
fn staking_with_reorder() {
    let config = SimConfig {
        num_honest: 4,
        byzantine: vec![],
        num_rounds: 500,
        delivery_policy: DeliveryPolicy::RandomOrder,
        seed: 101,
        txs_per_round: 5,
        check_every_round: true,
        scenario: Some(Scenario::StakingLifecycle), max_finality_lag: 50,
    };
    let mut harness = SimHarness::new(&config);
    let result = harness.run(&config);
    assert!(result.passed, "Violations: {:?}", result.violations);
}
