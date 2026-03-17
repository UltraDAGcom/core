//! Extended proptest fuzzing: more cases, longer runs, cross-feature.
//! Run with PROPTEST_CASES=5000 for overnight exhaustive testing.

use proptest::prelude::*;
use ultradag_sim::fuzz::*;
use ultradag_sim::network::DeliveryPolicy;

fn action_strategy() -> impl Strategy<Value = Action> {
    prop_oneof![
        40 => Just(Action::ProduceNormal),
        5 => Just(Action::Equivocate),
        5 => (1u8..=5).prop_map(|count| Action::IncludeStaleTxs { count }),
        5 => Just(Action::StallFinality),
        2 => Just(Action::Skip),
        8 => Just(Action::Stake),
        4 => Just(Action::Unstake),
        8 => (0u8..10).prop_map(|offset| Action::Delegate { target_offset: offset }),
        4 => Just(Action::Undelegate),
        4 => (0u8..=100).prop_map(|p| Action::SetCommission { percent: p }),
        8 => (1u8..=50).prop_map(|f| Action::Transfer { amount_fraction: f }),
        2 => prop_oneof![
            Just(Action::BadTimestamp { offset: 600 }),
            Just(Action::BadTimestamp { offset: -1000 }),
        ],
    ]
}

fn tx_injection_strategy(max_idx: u8) -> impl Strategy<Value = TxInjection> {
    prop_oneof![
        30 => (0..=max_idx, 0..=max_idx, 1u8..=30).prop_map(|(f, t, a)| TxInjection::Transfer { from_idx: f, to_idx: t, amount_fraction: a }),
        20 => (0..=max_idx).prop_map(|f| TxInjection::Stake { from_idx: f }),
        10 => (0..=max_idx).prop_map(|f| TxInjection::Unstake { from_idx: f }),
        20 => (0..=max_idx, 0..=max_idx).prop_map(|(f, t)| TxInjection::Delegate { from_idx: f, to_idx: t }),
        10 => (0..=max_idx).prop_map(|f| TxInjection::Undelegate { from_idx: f }),
    ]
}

fn cross_feature_plan(num_byz: usize, num_total: usize) -> impl Strategy<Value = RoundPlan> {
    let byz = proptest::collection::vec(action_strategy(), num_byz);
    let max_idx = (num_total as u8).max(1);
    // More injections per round for cross-feature coverage
    let inj = proptest::collection::vec(tx_injection_strategy(max_idx), 0..=5);
    (byz, inj).prop_map(|(byzantine_actions, inject_txs)| {
        RoundPlan { byzantine_actions, inject_txs }
    })
}

fn cross_feature_plans(num_byz: usize, num_total: usize, rounds: usize) -> impl Strategy<Value = Vec<RoundPlan>> {
    proptest::collection::vec(cross_feature_plan(num_byz, num_total), rounds)
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 500, timeout: 120_000, ..ProptestConfig::default() })]

    /// Cross-feature fuzz: staking + delegation + transfers + Byzantine all in one run.
    /// 500 cases × 80 rounds × all action types = maximum feature interaction coverage.
    #[test]
    fn fuzz_cross_feature_500_cases(plans in cross_feature_plans(1, 4, 80)) {
        let config = FuzzConfig { num_honest: 3, num_byzantine: 1, delivery_policy: DeliveryPolicy::Perfect };
        let result = execute_fuzz(&config, &plans, 0);
        prop_assert!(result.is_ok(), "Invariant violation: {}", result.unwrap_err());
    }

    /// 7 validators, 2 Byzantine, cross-feature. Tests BFT with 2 adversaries.
    #[test]
    fn fuzz_two_byz_cross_feature(plans in cross_feature_plans(2, 7, 50)) {
        let config = FuzzConfig { num_honest: 5, num_byzantine: 2, delivery_policy: DeliveryPolicy::Perfect };
        let result = execute_fuzz(&config, &plans, 0);
        prop_assert!(result.is_ok(), "Invariant violation: {}", result.unwrap_err());
    }

    /// Reorder delivery with cross-feature actions.
    #[test]
    fn fuzz_cross_feature_reorder(plans in cross_feature_plans(1, 4, 60)) {
        let config = FuzzConfig { num_honest: 3, num_byzantine: 1, delivery_policy: DeliveryPolicy::RandomOrder };
        let result = execute_fuzz(&config, &plans, 0);
        prop_assert!(result.is_ok(), "Invariant violation: {}", result.unwrap_err());
    }
}
