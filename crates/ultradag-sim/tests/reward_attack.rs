use ultradag_sim::harness::{SimConfig, SimHarness};
use ultradag_sim::network::DeliveryPolicy;
use ultradag_sim::byzantine::ByzantineStrategy;
use ultradag_coin::SecretKey;

#[test]
fn reward_gambler_cannot_inflate_supply() {
    let puppet_sk = SecretKey::from_bytes([200u8; 32]);
    let puppet_address = puppet_sk.address();
    let config = SimConfig {
        num_honest: 3,
        byzantine: vec![ByzantineStrategy::RewardGambler { puppet_sk, puppet_address }],
        num_rounds: 500,
        delivery_policy: DeliveryPolicy::Perfect,
        seed: 1001,
        txs_per_round: 0,
        check_every_round: true,
        scenario: None,
        max_finality_lag: 50,
    };
    let mut harness = SimHarness::new(&config);
    let result = harness.run(&config);
    assert!(result.passed, "Violations: {:?}", result.violations);
}

#[test]
fn reward_gambler_with_reorder() {
    let puppet_sk = SecretKey::from_bytes([201u8; 32]);
    let puppet_address = puppet_sk.address();
    let config = SimConfig {
        num_honest: 3,
        byzantine: vec![ByzantineStrategy::RewardGambler { puppet_sk, puppet_address }],
        num_rounds: 500,
        delivery_policy: DeliveryPolicy::RandomOrder,
        seed: 1002,
        txs_per_round: 5,
        check_every_round: true,
        scenario: None,
        max_finality_lag: 100,
    };
    let mut harness = SimHarness::new(&config);
    let result = harness.run(&config);
    assert!(result.passed, "Violations: {:?}", result.violations);
}
