use ultradag_sim::harness::{SimConfig, SimHarness};
use ultradag_sim::network::DeliveryPolicy;
use ultradag_sim::byzantine::ByzantineStrategy;

#[test]
fn selective_equivocation_state_converges() {
    let config = SimConfig {
        num_honest: 4,
        byzantine: vec![ByzantineStrategy::SelectiveEquivocator],
        num_rounds: 200,
        delivery_policy: DeliveryPolicy::Perfect,
        seed: 5001,
        txs_per_round: 5,
        check_every_round: true,
        scenario: None,
        max_finality_lag: 250, // Generous — equivocator disrupts descendant counts
    };
    let mut harness = SimHarness::new(&config);
    let result = harness.run(&config);
    assert!(result.passed, "Violations: {:?}", result.violations);
    assert!(result.equivocations_detected > 0, "Should detect selective equivocation");
}

#[test]
fn selective_equivocation_with_reorder() {
    let config = SimConfig {
        num_honest: 4,
        byzantine: vec![ByzantineStrategy::SelectiveEquivocator],
        num_rounds: 200,
        delivery_policy: DeliveryPolicy::RandomOrder,
        seed: 5002,
        txs_per_round: 10,
        check_every_round: true,
        scenario: None,
        max_finality_lag: 250,
    };
    let mut harness = SimHarness::new(&config);
    let result = harness.run(&config);
    assert!(result.passed, "Violations: {:?}", result.violations);
}
