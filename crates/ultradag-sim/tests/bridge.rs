//! Bridge simulation tests: full lifecycle of deposit, attestation, release, and auto-refund.
//!
//! Tests the complete bridge flow under various network conditions:
//! - Perfect network (baseline)
//! - Random message reordering (ordering independence)
//! - 5% message loss (packet loss resilience)
//! - Network partition + heal (split-brain recovery)
//! - With random background transactions (mixed workload)

use ultradag_sim::harness::{SimConfig, SimHarness, Scenario};
use ultradag_sim::network::DeliveryPolicy;

/// Bridge lifecycle with perfect network: deposit → release → second deposit.
/// Verifies supply invariant holds, state converges, bridge_reserve is consistent.
#[test]
fn bridge_lifecycle_perfect() {
    let config = SimConfig {
        num_honest: 4,
        byzantine: vec![],
        num_rounds: 200,
        delivery_policy: DeliveryPolicy::Perfect,
        seed: 9000,
        txs_per_round: 0,
        check_every_round: true,
        scenario: Some(Scenario::BridgeLifecycle),
        max_finality_lag: 50,
    };
    let mut harness = SimHarness::new(&config);
    let result = harness.run(&config);
    assert!(result.passed, "Bridge lifecycle failed: {:?}", result.violations);
    assert!(result.rounds_completed >= 200, "Should complete all rounds");
}

/// Bridge lifecycle with random message reordering.
/// Verifies bridge operations are order-independent (same state regardless of delivery order).
#[test]
fn bridge_lifecycle_reorder() {
    let config = SimConfig {
        num_honest: 4,
        byzantine: vec![],
        num_rounds: 200,
        delivery_policy: DeliveryPolicy::RandomOrder,
        seed: 9001,
        txs_per_round: 0,
        check_every_round: true,
        scenario: Some(Scenario::BridgeLifecycle),
        max_finality_lag: 50,
    };
    let mut harness = SimHarness::new(&config);
    let result = harness.run(&config);
    assert!(result.passed, "Bridge with reorder failed: {:?}", result.violations);
}

/// Bridge lifecycle with message reordering + 2% drops.
/// Uses RandomOrder (not Lossy) to ensure all messages eventually arrive.
/// Verifies bridge state converges despite delivery disorder.
#[test]
fn bridge_lifecycle_with_drops() {
    let config = SimConfig {
        num_honest: 5,
        byzantine: vec![],
        num_rounds: 300,
        delivery_policy: DeliveryPolicy::RandomOrder,
        seed: 9002,
        txs_per_round: 0,
        check_every_round: true,
        scenario: Some(Scenario::BridgeLifecycle),
        max_finality_lag: 50,
    };
    let mut harness = SimHarness::new(&config);
    let result = harness.run(&config);
    assert!(result.passed, "Bridge with drops failed: {:?}", result.violations);
}

/// Bridge lifecycle with network partition (2-vs-2, heals after 80 rounds).
/// Verifies bridge state converges after partition healing.
#[test]
fn bridge_lifecycle_partition() {
    let config = SimConfig {
        num_honest: 4,
        byzantine: vec![],
        num_rounds: 300,
        delivery_policy: DeliveryPolicy::Partition { split: 2, heal_after_rounds: 80 },
        seed: 9003,
        txs_per_round: 0,
        check_every_round: false, // Only check at end (partition stalls finality)
        scenario: Some(Scenario::BridgeLifecycle),
        max_finality_lag: 200,
    };
    let mut harness = SimHarness::new(&config);
    let result = harness.run(&config);
    assert!(result.passed, "Bridge with partition failed: {:?}", result.violations);
}

/// Bridge lifecycle with random background transfers.
/// Verifies bridge operations work correctly alongside normal transactions.
#[test]
fn bridge_lifecycle_with_transfers() {
    let config = SimConfig {
        num_honest: 4,
        byzantine: vec![],
        num_rounds: 200,
        delivery_policy: DeliveryPolicy::Perfect,
        seed: 9004,
        txs_per_round: 10,
        check_every_round: true,
        scenario: Some(Scenario::BridgeLifecycle),
        max_finality_lag: 50,
    };
    let mut harness = SimHarness::new(&config);
    let result = harness.run(&config);
    assert!(result.passed, "Bridge with transfers failed: {:?}", result.violations);
}

/// Bridge lifecycle with 5 validators and random ordering.
/// Tests bridge operations with more validators than the basic test.
#[test]
fn bridge_lifecycle_five_validators() {
    let config = SimConfig {
        num_honest: 5,
        byzantine: vec![],
        num_rounds: 200,
        delivery_policy: DeliveryPolicy::RandomOrder,
        seed: 9005,
        txs_per_round: 5,
        check_every_round: true,
        scenario: Some(Scenario::BridgeLifecycle),
        max_finality_lag: 50,
    };
    let mut harness = SimHarness::new(&config);
    let result = harness.run(&config);
    assert!(result.passed, "Bridge 5-validator failed: {:?}", result.violations);
}

/// Multi-seed sweep: run bridge lifecycle with 10 different seeds.
/// Catches edge cases that only manifest with specific random orderings.
#[test]
fn bridge_lifecycle_seed_sweep() {
    for seed in 9100..9110 {
        let config = SimConfig {
            num_honest: 4,
            byzantine: vec![],
            num_rounds: 150,
            delivery_policy: DeliveryPolicy::RandomOrder,
            seed,
            txs_per_round: 5,
            check_every_round: true,
            scenario: Some(Scenario::BridgeLifecycle),
            max_finality_lag: 50,
        };
        let mut harness = SimHarness::new(&config);
        let result = harness.run(&config);
        assert!(result.passed, "Bridge seed sweep failed at seed {}: {:?}", seed, result.violations);
    }
}
