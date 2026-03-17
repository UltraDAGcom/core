//! Exhaustive property verification tests.
//! Every property that MUST hold for a correct DAG-BFT chain.

use ultradag_sim::harness::{SimConfig, SimHarness, Scenario};
use ultradag_sim::network::DeliveryPolicy;
use ultradag_sim::byzantine::ByzantineStrategy;
use ultradag_sim::properties;

/// All properties hold after 500 rounds with staking + transactions.
#[test]
fn properties_hold_with_staking() {
    let config = SimConfig {
        num_honest: 4, byzantine: vec![], num_rounds: 500,
        delivery_policy: DeliveryPolicy::Perfect, seed: 600,
        txs_per_round: 10, check_every_round: false,
        scenario: Some(Scenario::StakingLifecycle), max_finality_lag: 50,
    };
    let mut harness = SimHarness::new(&config);
    let result = harness.run(&config);
    assert!(result.passed, "{:?}", result.violations);
    let props = properties::check_all_properties(&harness.validators);
    assert!(props.is_ok(), "Property violation: {}", props.unwrap_err());
}

/// All properties hold after delegation + commission.
#[test]
fn properties_hold_with_delegation() {
    let config = SimConfig {
        num_honest: 4, byzantine: vec![], num_rounds: 300,
        delivery_policy: DeliveryPolicy::Perfect, seed: 601,
        txs_per_round: 5, check_every_round: false,
        scenario: Some(Scenario::DelegationRewards), max_finality_lag: 50,
    };
    let mut harness = SimHarness::new(&config);
    let result = harness.run(&config);
    assert!(result.passed, "{:?}", result.violations);
    let props = properties::check_all_properties(&harness.validators);
    assert!(props.is_ok(), "Property violation: {}", props.unwrap_err());
}

/// All properties hold after governance parameter change.
#[test]
fn properties_hold_with_governance() {
    let config = SimConfig {
        num_honest: 4, byzantine: vec![], num_rounds: 200,
        delivery_policy: DeliveryPolicy::Perfect, seed: 602,
        txs_per_round: 0, check_every_round: false,
        scenario: Some(Scenario::GovernanceParameterChange), max_finality_lag: 50,
    };
    let mut harness = SimHarness::new(&config);
    let result = harness.run(&config);
    assert!(result.passed, "{:?}", result.violations);
    let props = properties::check_all_properties(&harness.validators);
    assert!(props.is_ok(), "Property violation: {}", props.unwrap_err());
}

/// All properties hold after cross-feature scenario with equivocation.
#[test]
fn properties_hold_cross_feature_equivocation() {
    let config = SimConfig {
        num_honest: 5, byzantine: vec![ByzantineStrategy::Equivocator],
        num_rounds: 500, delivery_policy: DeliveryPolicy::Perfect,
        seed: 603, txs_per_round: 10, check_every_round: false,
        scenario: Some(Scenario::CrossFeature), max_finality_lag: 50,
    };
    let mut harness = SimHarness::new(&config);
    let result = harness.run(&config);
    assert!(result.passed, "{:?}", result.violations);
    let props = properties::check_all_properties(&harness.validators);
    assert!(props.is_ok(), "Property violation: {}", props.unwrap_err());
}

/// All properties hold after 1000 rounds with 21 validators.
#[test]
fn properties_hold_high_validator_count() {
    let config = SimConfig {
        num_honest: 21, byzantine: vec![], num_rounds: 1000,
        delivery_policy: DeliveryPolicy::Perfect, seed: 604,
        txs_per_round: 30, check_every_round: false,
        scenario: None, max_finality_lag: 50,
    };
    let mut harness = SimHarness::new(&config);
    let result = harness.run(&config);
    assert!(result.passed, "{:?}", result.violations);
    let props = properties::check_all_properties(&harness.validators);
    assert!(props.is_ok(), "Property violation: {}", props.unwrap_err());
}

/// Supply cap check: total_supply should never exceed MAX_SUPPLY_SATS
/// even after many rounds of reward distribution.
#[test]
fn supply_never_exceeds_max() {
    let config = SimConfig {
        num_honest: 4, byzantine: vec![], num_rounds: 500,
        delivery_policy: DeliveryPolicy::Perfect, seed: 605,
        txs_per_round: 20, check_every_round: false,
        scenario: Some(Scenario::StakingLifecycle), max_finality_lag: 50,
    };
    let mut harness = SimHarness::new(&config);
    let result = harness.run(&config);
    assert!(result.passed, "{:?}", result.violations);
    for v in &harness.validators {
        if v.honest {
            assert!(v.state.total_supply() <= ultradag_coin::constants::MAX_SUPPLY_SATS,
                "Supply {} exceeds max {}", v.state.total_supply(), ultradag_coin::constants::MAX_SUPPLY_SATS);
        }
    }
}

/// Active set consistency: all honest validators agree on the active set.
#[test]
fn active_set_identical_across_validators() {
    let config = SimConfig {
        num_honest: 6, byzantine: vec![], num_rounds: 300,
        delivery_policy: DeliveryPolicy::Perfect, seed: 606,
        txs_per_round: 0, check_every_round: false,
        scenario: Some(Scenario::EpochTransition), max_finality_lag: 50,
    };
    let mut harness = SimHarness::new(&config);
    let result = harness.run(&config);
    assert!(result.passed, "{:?}", result.violations);
    let check = properties::check_active_set_consistency(&harness.validators);
    assert!(check.is_ok(), "Active set diverged: {}", check.unwrap_err());
}
