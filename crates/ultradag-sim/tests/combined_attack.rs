use ultradag_sim::harness::{SimConfig, SimHarness, Scenario};
use ultradag_sim::network::DeliveryPolicy;
use ultradag_sim::byzantine::ByzantineStrategy;

/// Multiple attack strategies simultaneously against honest majority.
#[test]
fn combined_attack_all_invariants_hold() {
    let config = SimConfig {
        num_honest: 6,
        byzantine: vec![
            ByzantineStrategy::Equivocator,
            ByzantineStrategy::Crash,
        ],
        num_rounds: 500,
        delivery_policy: DeliveryPolicy::Perfect,
        seed: 9001,
        txs_per_round: 20,
        check_every_round: false,
        scenario: None,
        max_finality_lag: 200,
    };
    let mut harness = SimHarness::new(&config);
    let result = harness.run(&config);
    assert!(result.passed, "Violations: {:?}", result.violations);
    assert!(result.equivocations_detected > 0, "Should detect equivocation");
}

/// Combined attack with staking scenario active.
#[test]
fn combined_attack_with_staking() {
    let config = SimConfig {
        num_honest: 6,
        byzantine: vec![
            ByzantineStrategy::Equivocator,
            ByzantineStrategy::Crash,
        ],
        num_rounds: 500,
        delivery_policy: DeliveryPolicy::Perfect,
        seed: 9002,
        txs_per_round: 15,
        check_every_round: true,
        scenario: Some(Scenario::StakingLifecycle),
        max_finality_lag: 150,
    };
    let mut harness = SimHarness::new(&config);
    let result = harness.run(&config);
    assert!(result.passed, "Violations: {:?}", result.violations);
}

/// Multi-seed sweep with combined attacks.
#[test]
fn combined_attack_multi_seed() {
    for seed in 0..20u64 {
        let config = SimConfig {
            num_honest: 5,
            byzantine: vec![
                ByzantineStrategy::Equivocator,
                ByzantineStrategy::Crash,
            ],
            num_rounds: 200,
            delivery_policy: DeliveryPolicy::Perfect,
            seed,
            txs_per_round: 10,
            check_every_round: false,
            scenario: None,
            max_finality_lag: 100,
        };
        let mut harness = SimHarness::new(&config);
        let result = harness.run(&config);
        assert!(result.passed, "Combined attack failed at seed {}: {:?}", seed, result.violations);
    }
}
