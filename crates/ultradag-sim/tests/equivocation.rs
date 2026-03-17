use ultradag_sim::harness::{SimConfig, SimHarness};
use ultradag_sim::network::DeliveryPolicy;
use ultradag_sim::byzantine::ByzantineStrategy;

/// 1 equivocator among 4 validators. Equivocation should be detected.
#[test]
fn equivocator_detected() {
    let config = SimConfig {
        num_honest: 3,
        byzantine: vec![ByzantineStrategy::Equivocator],
        num_rounds: 100,
        delivery_policy: DeliveryPolicy::Perfect,
        seed: 777,
        txs_per_round: 0,
        check_every_round: true,
    };
    let mut harness = SimHarness::new(&config);
    let result = harness.run(&config);
    assert!(result.passed, "Violations: {:?}", result.violations);
    assert!(result.equivocations_detected > 0, "Should detect equivocation");
}

/// 1 equivocator among 4 validators with transactions. Supply invariant must hold.
#[test]
fn equivocator_with_transactions_supply_holds() {
    let config = SimConfig {
        num_honest: 3,
        byzantine: vec![ByzantineStrategy::Equivocator],
        num_rounds: 200,
        delivery_policy: DeliveryPolicy::Perfect,
        seed: 888,
        txs_per_round: 10,
        check_every_round: true,
    };
    let mut harness = SimHarness::new(&config);
    let result = harness.run(&config);
    assert!(result.passed, "Violations: {:?}", result.violations);
}
