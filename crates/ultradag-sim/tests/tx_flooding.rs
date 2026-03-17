use ultradag_sim::harness::{SimConfig, SimHarness};
use ultradag_sim::network::DeliveryPolicy;
use ultradag_sim::byzantine::ByzantineStrategy;

/// Attacker includes stale-nonce transactions in every vertex.
/// Previously this triggered SupplyInvariantBroken (Bug #174), halting all nodes.
/// After the saturating fee clawback fix, the network survives:
/// - Proposer is debited what they can afford
/// - Shortfall is burned from total_supply
/// - Supply invariant holds (liquid + staked + delegated + treasury == total_supply)
/// - State converges across all honest validators
#[test]
fn stale_nonce_flooding_no_longer_halts() {
    let config = SimConfig {
        num_honest: 3,
        byzantine: vec![ByzantineStrategy::DuplicateTxFlooder],
        num_rounds: 300,
        delivery_policy: DeliveryPolicy::Perfect,
        seed: 3001,
        txs_per_round: 0,
        check_every_round: true,
        scenario: None,
        max_finality_lag: 50,
    };
    let mut harness = SimHarness::new(&config);
    let result = harness.run(&config);
    assert!(result.passed, "Violations: {:?}", result.violations);

    // Verify finality actually progressed (not stalled)
    for v in &harness.validators {
        if v.honest {
            assert!(v.last_finalized_round() > 200,
                "Honest validator {} should finalize past round 200 (got {})",
                v.index, v.last_finalized_round());
        }
    }
}

/// Same test with message reordering — the saturating clawback must be
/// deterministic across all validators regardless of message order.
#[test]
fn stale_nonce_flooding_with_reorder_converges() {
    let config = SimConfig {
        num_honest: 3,
        byzantine: vec![ByzantineStrategy::DuplicateTxFlooder],
        num_rounds: 300,
        delivery_policy: DeliveryPolicy::RandomOrder,
        seed: 3002,
        txs_per_round: 10,
        check_every_round: true,
        scenario: None,
        max_finality_lag: 100,
    };
    let mut harness = SimHarness::new(&config);
    let result = harness.run(&config);
    assert!(result.passed, "Violations: {:?}", result.violations);
}
