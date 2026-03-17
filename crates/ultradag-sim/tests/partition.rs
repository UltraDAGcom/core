use ultradag_sim::harness::{SimConfig, SimHarness};
use ultradag_sim::network::DeliveryPolicy;

/// Split 4 validators into two groups of 2 for 50 rounds, then heal.
#[test]
fn partition_and_heal() {
    let config = SimConfig {
        num_honest: 4,
        byzantine: vec![],
        num_rounds: 200,
        delivery_policy: DeliveryPolicy::Partition {
            split: 2,
            heal_after_rounds: 50,
        },
        seed: 999,
        txs_per_round: 5,
        check_every_round: true, scenario: None,
    };
    let mut harness = SimHarness::new(&config);
    let result = harness.run(&config);
    assert!(result.passed, "Violations: {:?}", result.violations);
    // After healing, all should converge to the same finalized round
    let final_rounds: Vec<u64> = result.final_finalized_rounds.iter().map(|(_, r)| *r).collect();
    assert!(final_rounds.iter().all(|r| *r == final_rounds[0]),
        "All validators should converge after partition heals: {:?}", final_rounds);
}
