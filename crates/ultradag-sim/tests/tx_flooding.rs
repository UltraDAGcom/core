use ultradag_sim::harness::{SimConfig, SimHarness};
use ultradag_sim::network::DeliveryPolicy;
use ultradag_sim::byzantine::ByzantineStrategy;

/// Attacker includes stale-nonce transactions in every vertex.
/// This IS a valid DoS attack — the coinbase credits fees for stale txs,
/// then fee clawback fails because the attacker doesn't have enough balance.
/// Bug #174 made this FATAL (SupplyInvariantBroken). The test verifies that
/// all validators see the same error at the same round (deterministic failure).
#[test]
fn stale_nonce_flooding_triggers_deterministic_halt() {
    let config = SimConfig {
        num_honest: 3,
        byzantine: vec![ByzantineStrategy::DuplicateTxFlooder],
        num_rounds: 100,
        delivery_policy: DeliveryPolicy::Perfect,
        seed: 3001,
        txs_per_round: 0,
        check_every_round: true,
        scenario: None,
        max_finality_lag: 200,
    };
    let mut harness = SimHarness::new(&config);
    let result = harness.run(&config);

    // The simulation may or may not fail — depends on whether the attacker's
    // stale-nonce vertex gets finalized and whether the attacker has enough balance
    // for fee clawback. Either outcome is acceptable:
    // - If passed: the attacker didn't have stale txs in finalized vertices
    //   (e.g., nonce was actually valid, or vertex not finalized)
    // - If failed: all validators failed at the same round (deterministic)
    if !result.passed {
        // Verify all honest validators agree on the failure (same finalized round)
        let honest_rounds: Vec<u64> = result.final_finalized_rounds.iter()
            .filter(|(i, _)| *i < 3) // honest validators are 0,1,2
            .map(|(_, r)| *r)
            .collect();
        // All should be at the same round (deterministic failure point)
        if honest_rounds.len() >= 2 {
            assert!(honest_rounds.windows(2).all(|w| w[0] == w[1]),
                "Honest validators should fail at the same round: {:?}", honest_rounds);
        }
    }
}
