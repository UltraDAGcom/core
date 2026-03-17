use ultradag_sim::harness::{SimConfig, SimHarness};
use ultradag_sim::network::DeliveryPolicy;
use ultradag_sim::byzantine::ByzantineStrategy;

#[test]
fn finality_staller_cannot_halt_with_supermajority() {
    let config = SimConfig {
        num_honest: 3,
        byzantine: vec![ByzantineStrategy::FinalityStaller],
        num_rounds: 200,
        delivery_policy: DeliveryPolicy::Perfect,
        seed: 4001,
        txs_per_round: 0,
        check_every_round: true,
        scenario: None,
        max_finality_lag: 100,
    };
    let mut harness = SimHarness::new(&config);
    let result = harness.run(&config);
    assert!(result.passed, "Violations: {:?}", result.violations);

    for v in &harness.validators {
        if v.honest {
            assert!(v.last_finalized_round() > 100,
                "Honest validator {} should finalize past round 100 (got {})",
                v.index, v.last_finalized_round());
        }
    }
}

#[test]
fn two_stallers_within_bft_tolerance() {
    let config = SimConfig {
        num_honest: 5,
        byzantine: vec![ByzantineStrategy::FinalityStaller, ByzantineStrategy::FinalityStaller],
        num_rounds: 300,
        delivery_policy: DeliveryPolicy::Perfect,
        seed: 4002,
        txs_per_round: 5,
        check_every_round: true,
        scenario: None,
        max_finality_lag: 100,
    };
    let mut harness = SimHarness::new(&config);
    let result = harness.run(&config);
    assert!(result.passed, "Violations: {:?}", result.violations);
}
