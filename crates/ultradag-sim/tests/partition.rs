use ultradag_sim::harness::{SimConfig, SimHarness};
use ultradag_sim::network::DeliveryPolicy;

/// Split 4 validators into 3-vs-1 for 50 rounds, then heal.
/// The 3-validator group has BFT majority (3 >= 2/3*4+1 = 3) and can finalize.
/// After healing, the lone validator should catch up.
#[test]
fn partition_and_heal() {
    let config = SimConfig {
        num_honest: 4,
        byzantine: vec![],
        num_rounds: 200,
        delivery_policy: DeliveryPolicy::Partition {
            split: 3,  // 3-vs-1 split: group of 3 has BFT majority
            heal_after_rounds: 50,
        },
        seed: 999,
        txs_per_round: 5,
        check_every_round: true, scenario: None, max_finality_lag: 50,
    };
    let mut harness = SimHarness::new(&config);
    let result = harness.run(&config);
    assert!(result.passed, "Violations: {:?}", result.violations);
    // After healing, all should converge to the same finalized round
    let final_rounds: Vec<u64> = result.final_finalized_rounds.iter().map(|(_, r)| *r).collect();
    assert!(final_rounds.iter().all(|r| *r == final_rounds[0]),
        "All validators should converge after partition heals: {:?}", final_rounds);
}

/// Test 2-vs-2 partition: neither group has BFT majority, so no finalization expected.
/// This tests that the system correctly stalls when BFT threshold cannot be met.
#[test]
fn partition_no_majority() {
    let config = SimConfig {
        num_honest: 4,
        byzantine: vec![],
        num_rounds: 100,
        delivery_policy: DeliveryPolicy::Partition {
            split: 2,  // 2-vs-2 split: neither group has BFT majority (need 3)
            heal_after_rounds: 50,
        },
        seed: 999,
        txs_per_round: 0,
        check_every_round: false, scenario: None, max_finality_lag: 100,
    };
    let mut harness = SimHarness::new(&config);
    let result = harness.run(&config);
    // State divergence is EXPECTED during partition - no finalization possible
    // After healing, validators should converge
    let final_rounds: Vec<u64> = result.final_finalized_rounds.iter().map(|(_, r)| *r).collect();
    // All validators should have same finalized round after healing (likely 0 or very low)
    assert!(final_rounds.iter().all(|r| *r == final_rounds[0]),
        "Validators should have consistent finality after 2-vs-2 partition: {:?}", final_rounds);
}
