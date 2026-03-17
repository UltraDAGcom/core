use ultradag_sim::harness::{SimConfig, SimHarness};
use ultradag_sim::network::DeliveryPolicy;

/// Messages arrive in random order. State must still converge.
#[test]
fn random_message_order_converges() {
    let config = SimConfig {
        num_honest: 4,
        byzantine: vec![],
        num_rounds: 200,
        delivery_policy: DeliveryPolicy::RandomOrder,
        seed: 123,
        txs_per_round: 10,
        check_every_round: true,
    };
    let mut harness = SimHarness::new(&config);
    let result = harness.run(&config);
    assert!(result.passed, "Violations: {:?}", result.violations);
}

/// Run the same scenario with 100 different seeds.
#[test]
fn random_order_multi_seed() {
    for seed in 0..100u64 {
        let config = SimConfig {
            num_honest: 4,
            byzantine: vec![],
            num_rounds: 100,
            delivery_policy: DeliveryPolicy::RandomOrder,
            seed,
            txs_per_round: 5,
            check_every_round: false,
        };
        let mut harness = SimHarness::new(&config);
        let result = harness.run(&config);
        assert!(result.passed, "Failed at seed {}: {:?}", seed, result.violations);
    }
}
