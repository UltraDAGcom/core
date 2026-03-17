use ultradag_sim::harness::{SimConfig, SimHarness, Scenario};
use ultradag_sim::network::DeliveryPolicy;

#[test]
fn epoch_transition_active_set_converges() {
    let config = SimConfig {
        num_honest: 6,
        byzantine: vec![],
        num_rounds: 300,
        delivery_policy: DeliveryPolicy::Perfect,
        seed: 500,
        txs_per_round: 0,
        check_every_round: true,
        scenario: Some(Scenario::EpochTransition),
    };
    let mut harness = SimHarness::new(&config);
    let result = harness.run(&config);
    assert!(result.passed, "Violations: {:?}", result.violations);
}
