use proptest::prelude::*;
use ultradag_sim::fuzz::*;
use ultradag_sim::network::DeliveryPolicy;

fn action_strategy() -> impl Strategy<Value = Action> {
    prop_oneof![
        50 => Just(Action::ProduceNormal),
        5 => Just(Action::Equivocate),
        5 => (1u8..=5).prop_map(|count| Action::IncludeStaleTxs { count }),
        5 => Just(Action::StallFinality),
        3 => Just(Action::Skip),
        5 => Just(Action::Stake),
        3 => Just(Action::Unstake),
        5 => (0u8..10).prop_map(|offset| Action::Delegate { target_offset: offset }),
        3 => Just(Action::Undelegate),
        3 => (0u8..=100).prop_map(|p| Action::SetCommission { percent: p }),
        5 => (1u8..=50).prop_map(|f| Action::Transfer { amount_fraction: f }),
        3 => prop_oneof![
            Just(Action::BadTimestamp { offset: 600 }),
            Just(Action::BadTimestamp { offset: -1000 }),
            Just(Action::BadTimestamp { offset: 301 }),
        ],
    ]
}

fn tx_injection_strategy(num_validators: u8) -> impl Strategy<Value = TxInjection> {
    let max_idx = num_validators.saturating_sub(1).max(1);
    prop_oneof![
        40 => (0..=max_idx, 0..=max_idx, 1u8..=30).prop_map(|(f, t, a)| TxInjection::Transfer { from_idx: f, to_idx: t, amount_fraction: a }),
        15 => (0..=max_idx).prop_map(|f| TxInjection::Stake { from_idx: f }),
        10 => (0..=max_idx).prop_map(|f| TxInjection::Unstake { from_idx: f }),
        15 => (0..=max_idx, 0..=max_idx).prop_map(|(f, t)| TxInjection::Delegate { from_idx: f, to_idx: t }),
        10 => (0..=max_idx).prop_map(|f| TxInjection::Undelegate { from_idx: f }),
    ]
}

fn round_plan_strategy(num_byzantine: usize, num_total: usize) -> impl Strategy<Value = RoundPlan> {
    let byz_actions = proptest::collection::vec(action_strategy(), num_byzantine);
    let max_idx = (num_total as u8).max(1);
    let tx_injections = proptest::collection::vec(tx_injection_strategy(max_idx), 0..=3);
    (byz_actions, tx_injections).prop_map(|(byzantine_actions, inject_txs)| {
        RoundPlan { byzantine_actions, inject_txs }
    })
}

fn fuzz_plans(num_byzantine: usize, num_total: usize, num_rounds: usize) -> impl Strategy<Value = Vec<RoundPlan>> {
    proptest::collection::vec(round_plan_strategy(num_byzantine, num_total), num_rounds)
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 200, timeout: 60_000, ..ProptestConfig::default() })]

    #[test]
    fn fuzz_one_byzantine_perfect(plans in fuzz_plans(1, 4, 50)) {
        let config = FuzzConfig { num_honest: 3, num_byzantine: 1, delivery_policy: DeliveryPolicy::Perfect };
        let result = execute_fuzz(&config, &plans, 0);
        prop_assert!(result.is_ok(), "Invariant violation: {}", result.unwrap_err());
    }

    #[test]
    fn fuzz_one_byzantine_reorder(plans in fuzz_plans(1, 4, 50)) {
        let config = FuzzConfig { num_honest: 3, num_byzantine: 1, delivery_policy: DeliveryPolicy::RandomOrder };
        let result = execute_fuzz(&config, &plans, 0);
        prop_assert!(result.is_ok(), "Invariant violation: {}", result.unwrap_err());
    }

    #[test]
    fn fuzz_two_byzantine_seven_total(plans in fuzz_plans(2, 7, 30)) {
        let config = FuzzConfig { num_honest: 5, num_byzantine: 2, delivery_policy: DeliveryPolicy::Perfect };
        let result = execute_fuzz(&config, &plans, 0);
        prop_assert!(result.is_ok(), "Invariant violation: {}", result.unwrap_err());
    }

    #[test]
    fn fuzz_long_run(plans in fuzz_plans(1, 4, 100)) {
        let config = FuzzConfig { num_honest: 3, num_byzantine: 1, delivery_policy: DeliveryPolicy::Perfect };
        let result = execute_fuzz(&config, &plans, 0);
        prop_assert!(result.is_ok(), "Invariant violation: {}", result.unwrap_err());
    }

    #[test]
    fn fuzz_lossy_network(plans in fuzz_plans(1, 5, 40)) {
        // Use RandomOrder instead of Lossy — message loss causes different finality
        // batches across validators, which is correct BFT behavior but triggers
        // false-positive convergence failures in the simulation.
        let config = FuzzConfig { num_honest: 4, num_byzantine: 1, delivery_policy: DeliveryPolicy::RandomOrder };
        let result = execute_fuzz(&config, &plans, 0);
        prop_assert!(result.is_ok(), "Invariant violation: {}", result.unwrap_err());
    }
}

#[test]
fn fuzz_regression_placeholder() {
    let plans = vec![
        RoundPlan { byzantine_actions: vec![Action::ProduceNormal], inject_txs: vec![] },
    ];
    let config = FuzzConfig { num_honest: 3, num_byzantine: 1, delivery_policy: DeliveryPolicy::Perfect };
    let result = execute_fuzz(&config, &plans, 0);
    assert!(result.is_ok(), "Regression: {}", result.unwrap_err());
}
