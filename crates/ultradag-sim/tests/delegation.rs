use ultradag_sim::harness::{SimConfig, SimHarness, Scenario};
use ultradag_sim::network::DeliveryPolicy;

#[test]
fn delegation_rewards_converge() {
    let config = SimConfig {
        num_honest: 4,
        byzantine: vec![],
        num_rounds: 300,
        delivery_policy: DeliveryPolicy::Perfect,
        seed: 200,
        txs_per_round: 0,
        check_every_round: true,
        scenario: Some(Scenario::DelegationRewards),
    };
    let mut harness = SimHarness::new(&config);
    let result = harness.run(&config);
    assert!(result.passed, "Violations: {:?}", result.violations);
}

#[test]
fn delegation_with_reorder() {
    let config = SimConfig {
        num_honest: 4,
        byzantine: vec![],
        num_rounds: 300,
        delivery_policy: DeliveryPolicy::RandomOrder,
        seed: 201,
        txs_per_round: 5,
        check_every_round: true,
        scenario: Some(Scenario::DelegationRewards),
    };
    let mut harness = SimHarness::new(&config);
    let result = harness.run(&config);
    assert!(result.passed, "Violations: {:?}", result.violations);
}
