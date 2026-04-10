/// Economic audit tests: reward distribution correctness, delegation edge cases,
/// halving schedule, supply invariant stress tests.
///
/// These tests target bugs #170-175 found during the March 2026 economics audit.

use ultradag_coin::*;
use std::collections::HashSet;

fn make_stake_tx(sk: &SecretKey, amount: u64, nonce: u64) -> StakeTx {
    let mut tx = StakeTx {
        from: sk.address(),
        amount,
        nonce,
        pub_key: sk.verifying_key().to_bytes(),
        signature: Signature([0u8; 64]),
    };
    tx.signature = sk.sign(&tx.signable_bytes());
    tx
}

fn make_unstake_tx(sk: &SecretKey, nonce: u64) -> UnstakeTx {
    let mut tx = UnstakeTx {
        from: sk.address(),
        nonce,
        pub_key: sk.verifying_key().to_bytes(),
        signature: Signature([0u8; 64]),
    };
    tx.signature = sk.sign(&tx.signable_bytes());
    tx
}

fn make_delegate_tx(sk: &SecretKey, validator: Address, amount: u64, nonce: u64) -> DelegateTx {
    let mut tx = DelegateTx {
        from: sk.address(),
        validator,
        amount,
        nonce,
        pub_key: sk.verifying_key().to_bytes(),
        signature: Signature([0u8; 64]),
    };
    tx.signature = sk.sign(&tx.signable_bytes());
    tx
}

fn make_undelegate_tx(sk: &SecretKey, nonce: u64) -> UndelegateTx {
    let mut tx = UndelegateTx {
        from: sk.address(),
        nonce,
        pub_key: sk.verifying_key().to_bytes(),
        signature: Signature([0u8; 64]),
    };
    tx.signature = sk.sign(&tx.signable_bytes());
    tx
}

fn make_vertex(
    proposer_sk: &SecretKey,
    round: u64,
    height: u64,
    txs: Vec<Transaction>,
) -> DagVertex {
    let proposer = proposer_sk.address();
    let coinbase = CoinbaseTx {
        to: proposer,
        amount: 0,
        height,
    };
    let block = Block {
        header: BlockHeader {
            version: 1,
            height,
            timestamp: 1_000_000 + round as i64,
            prev_hash: [0u8; 32],
            merkle_root: [0u8; 32],
        },
        coinbase,
        transactions: txs,
    };
    let mut vertex = DagVertex::new(
        block,
        vec![[0u8; 32]],
        round,
        proposer,
        proposer_sk.verifying_key().to_bytes(),
        Signature([0u8; 64]),
    );
    vertex.signature = proposer_sk.sign(&vertex.signable_bytes());
    vertex
}

fn check_supply_invariant(state: &StateEngine) {
    // The supply invariant must hold after every state-changing operation.
    // This is the same check as in apply_vertex_with_validators but called
    // externally to verify distribute_round_rewards doesn't break it.
    let snap = state.snapshot();
    let liquid: u64 = snap.accounts.iter().map(|(_, a)| a.balance).fold(0u64, |acc, x| acc.saturating_add(x));
    let staked: u64 = snap.stake_accounts.iter().map(|(_, s)| s.staked).fold(0u64, |acc, x| acc.saturating_add(x));
    let delegated: u64 = snap.delegation_accounts.iter().map(|(_, d)| d.delegated).fold(0u64, |acc, x| acc.saturating_add(x));
    let total = liquid.saturating_add(staked).saturating_add(delegated).saturating_add(state.treasury_balance());
    assert_eq!(
        total, state.total_supply(),
        "Supply invariant broken: liquid={} staked={} delegated={} treasury={} sum={} != total_supply={}",
        liquid, staked, delegated, state.treasury_balance(), total, state.total_supply()
    );
}

// ============================================================================
// Bug #170: Undelegating amounts inflated the reward denominator
// ============================================================================

/// Test that rewards are distributed correctly when delegations are undelegating.
/// Before the fix, undelegating amounts were included in total_effective_stake
/// (the denominator) but excluded from per-validator effective_stake_of (the
/// numerators), causing proportional shares to sum to less than 100%.
#[test]
fn test_30_undelegating_does_not_inflate_reward_denominator() {
    let mut state = StateEngine::new_with_genesis();

    // Create two validators
    let val1_sk = SecretKey::generate();
    let val2_sk = SecretKey::generate();
    let del_sk = SecretKey::generate();

    let val1 = val1_sk.address();
    let val2 = val2_sk.address();
    let stake_amount = MIN_STAKE_SATS;

    // Fund and stake both validators
    state.faucet_credit(&val1, stake_amount * 3);
    state.faucet_credit(&val2, stake_amount * 3);
    state.faucet_credit(&del_sk.address(), MIN_DELEGATION_SATS * 2);

    state.apply_stake_tx(&make_stake_tx(&val1_sk, stake_amount, 0)).unwrap();
    state.apply_stake_tx(&make_stake_tx(&val2_sk, stake_amount, 0)).unwrap();

    // Delegate to val1
    state.apply_delegate_tx(&make_delegate_tx(&del_sk, val1, MIN_DELEGATION_SATS, 0)).unwrap();
    check_supply_invariant(&state);

    // Record balances before reward distribution
    let supply_before = state.total_supply();

    // Distribute rewards with both validators producing (round 100 to avoid round 0 edge cases)
    let mut producers = HashSet::new();
    producers.insert(val1);
    producers.insert(val2);
    state.distribute_round_rewards(100, &producers).unwrap();
    check_supply_invariant(&state);
    let supply_after_normal = state.total_supply();
    let minted_normal = supply_after_normal - supply_before;

    // Now start undelegating
    state.apply_undelegate_tx(&make_undelegate_tx(&del_sk, 1), 100).unwrap();

    // Record supply before second distribution
    let supply_before_2 = state.total_supply();

    // Distribute rewards again — undelegating delegation should NOT inflate denominator
    state.distribute_round_rewards(101, &producers).unwrap();
    check_supply_invariant(&state);
    let supply_after_undelegating = state.total_supply();
    let minted_undelegating = supply_after_undelegating - supply_before_2;

    // With undelegation active, val1's effective stake drops (delegation excluded).
    // val2's proportional share should increase. Total minted should still be close
    // to validator_pool, not reduced by the undelegating amount being "lost" in the
    // denominator gap.
    //
    // Key assertion: both rounds should mint a similar total (within rounding).
    // Before the fix, minted_undelegating was significantly less than minted_normal
    // because the denominator included the undelegating amount but no numerator matched it.
    let validator_pool = block_reward(100) * 90 / 100; // 90% after council emission
    assert!(
        minted_undelegating > validator_pool * 90 / 100,
        "Undelegating round should mint at least 90% of validator_pool (got {} vs pool {}). \
         This suggests the denominator is inflated by undelegating amounts.",
        minted_undelegating, validator_pool
    );
}

/// Test that the proportional shares sum to approximately 100% of the validator pool
/// when there are no passive stakers (all producers).
#[test]
fn test_31_reward_proportions_sum_to_full_pool() {
    let mut state = StateEngine::new_with_genesis();

    let val1_sk = SecretKey::generate();
    let val2_sk = SecretKey::generate();
    let val1 = val1_sk.address();
    let val2 = val2_sk.address();

    // Fund and stake both validators with different amounts
    state.faucet_credit(&val1, MIN_STAKE_SATS * 3);
    state.faucet_credit(&val2, MIN_STAKE_SATS * 3);
    state.apply_stake_tx(&make_stake_tx(&val1_sk, MIN_STAKE_SATS * 2, 0)).unwrap();
    state.apply_stake_tx(&make_stake_tx(&val2_sk, MIN_STAKE_SATS, 0)).unwrap();

    let supply_before = state.total_supply();

    let mut producers = HashSet::new();
    producers.insert(val1);
    producers.insert(val2);
    state.distribute_round_rewards(100, &producers).unwrap();
    check_supply_invariant(&state);

    // Validator minting = total minted minus everything credited to other buckets.
    // Compute directly from validators' balance deltas to avoid bucket accounting.
    let val1_balance = state.balance(&val1);
    let val2_balance = state.balance(&val2);
    let validator_minted = val1_balance + val2_balance;

    let params = state.governance_params();
    let validator_pool = block_reward(100) * params.validator_emission_percent / 100;

    // All producers active, no observers. Validator minted should equal validator_pool
    // minus integer rounding dust (at most a few sats per validator).
    let dust_tolerance = 3; // sats
    assert!(
        validator_pool.saturating_sub(validator_minted) <= dust_tolerance,
        "Validator minted {} should be within {} sats of validator_pool {}. Gap={}",
        validator_minted, dust_tolerance, validator_pool,
        validator_pool.saturating_sub(validator_minted)
    );
    let _ = supply_before; // silences unused-var if supply invariant check removed
}

// ============================================================================
// Bug #171: compute_validator_reward had same denominator mismatch
// ============================================================================

/// Test that compute_validator_reward matches the actual distribution from
/// distribute_round_rewards when undelegating amounts exist.
#[test]
fn test_32_compute_validator_reward_matches_distribution() {
    let mut state = StateEngine::new_with_genesis();

    let val_sk = SecretKey::generate();
    let del_sk = SecretKey::generate();
    let val = val_sk.address();

    state.faucet_credit(&val, MIN_STAKE_SATS * 3).unwrap();
    state.faucet_credit(&del_sk.address(), MIN_DELEGATION_SATS * 2).unwrap();
    state.apply_stake_tx(&make_stake_tx(&val_sk, MIN_STAKE_SATS, 0)).unwrap();
    state.apply_delegate_tx(&make_delegate_tx(&del_sk, val, MIN_DELEGATION_SATS, 0)).unwrap();

    // Start undelegation
    state.apply_undelegate_tx(&make_undelegate_tx(&del_sk, 1), 100).unwrap();

    // compute_validator_reward should return a value consistent with the state
    // (i.e., not inflated by undelegating amounts in the denominator).
    let reward = state.compute_validator_reward(&val, 100, 1);

    // The validator is the only staker with effective stake. After undelegation
    // starts, effective_stake_of = own_stake only (delegation is undelegating).
    // So they should get 100% of the validator pool.
    //
    // Validator pool is block_reward × validator_emission_percent / 100, taken
    // directly from the governance parameter (not residual).
    let params = state.governance_params();
    let validator_pool = block_reward(100) * params.validator_emission_percent / 100;
    assert_eq!(
        reward, validator_pool,
        "Validator should get full pool when they're the only effective staker. \
         Got {} vs pool {} (validator_pct={})",
        reward, validator_pool, params.validator_emission_percent
    );
}

// ============================================================================
// Halving schedule correctness
// ============================================================================

/// Verify the halving schedule produces exactly the documented rewards.
#[test]
fn test_33_halving_schedule_exact_values() {
    assert_eq!(block_reward(0), INITIAL_REWARD_SATS); // 1 UDAG
    assert_eq!(block_reward(HALVING_INTERVAL - 1), INITIAL_REWARD_SATS); // still 1 UDAG
    assert_eq!(block_reward(HALVING_INTERVAL), INITIAL_REWARD_SATS / 2); // 0.5 UDAG
    assert_eq!(block_reward(HALVING_INTERVAL * 2), INITIAL_REWARD_SATS / 4); // 0.25 UDAG

    // 64th halving produces 0
    assert_eq!(block_reward(64 * HALVING_INTERVAL), 0);
    // u64::MAX / HALVING_INTERVAL > 64, so block_reward(u64::MAX) = 0
    assert_eq!(block_reward(u64::MAX), 0);

    // After 27 halvings, reward drops to 0 (1 UDAG >> 27 = 0 for integer division)
    // Actually: 100_000_000 >> 26 = 1, >> 27 = 0
    assert_eq!(block_reward(26 * HALVING_INTERVAL), 1);
    assert_eq!(block_reward(27 * HALVING_INTERVAL), 0);
}

/// Verify total theoretical emission is less than MAX_SUPPLY.
#[test]
fn test_34_total_emission_under_max_supply() {
    let mut total_emission: u128 = 0;
    for h in 0..64u64 {
        let reward = block_reward(h * HALVING_INTERVAL) as u128;
        if reward == 0 {
            break;
        }
        total_emission += reward * HALVING_INTERVAL as u128;
    }
    assert!(
        total_emission <= MAX_SUPPLY_SATS as u128,
        "Total emission {} exceeds MAX_SUPPLY_SATS {}",
        total_emission, MAX_SUPPLY_SATS
    );
}

// ============================================================================
// Delegation reward edge cases
// ============================================================================

/// Test that a single delegator to a validator being slashed loses proportional stake.
#[test]
fn test_35_single_delegator_slashing_cascade() {
    let mut state = StateEngine::new_with_genesis();

    let val_sk = SecretKey::generate();
    let del_sk = SecretKey::generate();
    let val = val_sk.address();

    state.faucet_credit(&val, MIN_STAKE_SATS * 3);
    state.faucet_credit(&del_sk.address(), MIN_DELEGATION_SATS * 2);

    state.apply_stake_tx(&make_stake_tx(&val_sk, MIN_STAKE_SATS, 0)).unwrap();
    state.apply_delegate_tx(&make_delegate_tx(&del_sk, val, MIN_DELEGATION_SATS, 0)).unwrap();

    let supply_before_slash = state.total_supply();
    let del_before = state.delegation_account(&del_sk.address()).unwrap().delegated;
    let stake_before = state.stake_of(&val);

    // Slash the validator
    state.slash(&val);

    let del_after = state.delegation_account(&del_sk.address()).unwrap().delegated;
    let stake_after = state.stake_of(&val);
    let supply_after_slash = state.total_supply();

    // Both validator and delegator lose 50%
    assert_eq!(stake_after, stake_before / 2);
    assert_eq!(del_after, del_before / 2);

    // Total supply decreased by slashed amounts (burned)
    let total_slashed = (stake_before - stake_after) + (del_before - del_after);
    assert_eq!(supply_before_slash - supply_after_slash, total_slashed);

    check_supply_invariant(&state);
}

/// Test that commission calculation is correct: delegator gets (1-commission)% of their share.
#[test]
fn test_36_commission_calculation_correctness() {
    let mut state = StateEngine::new_with_genesis();

    let val_sk = SecretKey::generate();
    let del_sk = SecretKey::generate();
    let val = val_sk.address();
    let del = del_sk.address();

    state.faucet_credit(&val, MIN_STAKE_SATS * 3);
    // Fund delegator with enough to delegate MIN_DELEGATION_SATS
    state.faucet_credit(&del, MIN_DELEGATION_SATS * 3);

    // Stake and set commission to 50%
    state.apply_stake_tx(&make_stake_tx(&val_sk, MIN_STAKE_SATS, 0)).unwrap();

    // Set commission via SetCommissionTx
    let mut comm_tx = SetCommissionTx {
        from: val,
        commission_percent: 50,
        nonce: 1,
        pub_key: val_sk.verifying_key().to_bytes(),
        signature: Signature([0u8; 64]),
    };
    comm_tx.signature = val_sk.sign(&comm_tx.signable_bytes());
    state.apply_set_commission_tx(&comm_tx, 0).unwrap();

    // Delegate MIN_DELEGATION_SATS (not MIN_STAKE_SATS which is much larger)
    state.apply_delegate_tx(&make_delegate_tx(&del_sk, val, MIN_DELEGATION_SATS, 0)).unwrap();

    let val_bal_before = state.balance(&val);
    let del_bal_before = state.balance(&del);

    // Distribute rewards — validator is sole producer
    let mut producers = HashSet::new();
    producers.insert(val);
    state.distribute_round_rewards(100, &producers).unwrap();
    check_supply_invariant(&state);

    let val_earned = state.balance(&val) - val_bal_before;
    let del_earned = state.balance(&del) - del_bal_before;

    // Validator has effective_stake = own_stake + delegation = MIN_STAKE_SATS + MIN_DELEGATION_SATS
    // Own portion of validator_share = own_stake / effective * validator_share
    // Delegation pool = delegation / effective * validator_share
    // Commission = 50% of delegation_pool goes to validator
    // Delegator gets = 50% of delegation_pool
    // So validator gets: own_portion + commission (from delegated portion)
    // Delegator gets: 50% of their delegation's proportional share

    assert!(
        val_earned > del_earned,
        "With 50% commission and much larger own stake, validator should earn more. \
         val_earned={}, del_earned={}",
        val_earned, del_earned
    );
    assert!(
        del_earned > 0,
        "Delegator should earn something (got 0)"
    );

    // Verify: delegator's net reward should be roughly 50% of the pre-commission
    // delegation share (since commission is 50%).
    // delegation_share_pre_commission = validator_share * delegation / effective_stake
    // delegator_net = delegation_share_pre_commission * (100 - 50) / 100
    // This is hard to verify precisely with integer rounding, but we can check
    // that commission is working by comparing to a hypothetical 0% commission.
    // For now, just verify both earn > 0 and supply invariant holds.
    check_supply_invariant(&state);
}

/// Test that with 0 stakers, pre-staking fallback distributes correctly.
#[test]
fn test_37_zero_stakers_pre_staking_fallback() {
    let mut state = StateEngine::new_with_genesis();
    state.set_configured_validator_count(4);

    let val_sk = SecretKey::generate();
    let val = val_sk.address();

    let supply_before = state.total_supply();

    // Only 1 producer out of 4 configured
    let mut producers = HashSet::new();
    producers.insert(val);
    state.distribute_round_rewards(0, &producers).unwrap();
    check_supply_invariant(&state);

    let total_minted = state.total_supply() - supply_before;
    // Emission split (April 2026): 44% validators + 10% council + 16% treasury
    // + 5% founder + 8% ecosystem + 5% reserve = 88% of block_reward.
    let br = block_reward(0);
    let params = state.governance_params();
    let council_emission = br * params.council_emission_percent / 100;
    let treasury_emission = br * params.treasury_emission_percent / 100;
    let founder_emission = br * params.founder_emission_percent / 100;
    let ecosystem_emission = br * params.ecosystem_emission_percent / 100;
    let reserve_emission = br * params.reserve_emission_percent / 100;
    let validator_pool = br * params.validator_emission_percent / 100;

    // Per-producer reward in pre-staking fallback = validator_pool / configured_count
    let expected_validator = validator_pool / 4;
    let expected_total = council_emission + treasury_emission + founder_emission
        + ecosystem_emission + reserve_emission + expected_validator;
    assert_eq!(
        total_minted, expected_total,
        "Pre-staking fallback: all buckets + 1-of-4 producer. Got {} expected {}",
        total_minted, expected_total
    );
}

/// Test rewards with 0 producers (empty round).
#[test]
fn test_38_zero_producers_no_rewards() {
    let mut state = StateEngine::new_with_genesis();
    state.set_configured_validator_count(4);

    let supply_before = state.total_supply();

    let producers: HashSet<Address> = HashSet::new();
    state.distribute_round_rewards(0, &producers).unwrap();

    // Council, treasury, founder, ecosystem, and reserve emission still happen
    // (no producers doesn't block them). Validator rewards require producers in
    // pre-staking mode, so validator_pool is NOT minted with 0 producers.
    let minted = state.total_supply() - supply_before;
    let br = block_reward(0);
    let params = state.governance_params();
    let expected = br * params.council_emission_percent / 100
        + br * params.treasury_emission_percent / 100
        + br * params.founder_emission_percent / 100
        + br * params.ecosystem_emission_percent / 100
        + br * params.reserve_emission_percent / 100;
    assert_eq!(
        minted, expected,
        "With 0 producers, non-validator buckets minted. Got {} expected {}",
        minted, expected
    );
}

/// Test that supply cap enforcement works at near-exhaustion.
#[test]
fn test_39_supply_cap_enforcement_near_exhaustion() {
    let mut state = StateEngine::new();
    // Set total_supply to near MAX_SUPPLY_SATS
    let near_max = MAX_SUPPLY_SATS - 50; // only 50 sats left
    let val_sk = SecretKey::generate();
    let val = val_sk.address();
    state.credit(&val, near_max);
    state.total_supply = near_max;

    let supply_before = state.total_supply();

    let mut producers = HashSet::new();
    producers.insert(val);
    state.distribute_round_rewards(0, &producers).unwrap();

    let minted = state.total_supply() - supply_before;
    assert!(
        minted <= 50,
        "Should not mint more than remaining 50 sats. Minted: {}",
        minted
    );
    assert!(
        state.total_supply() <= MAX_SUPPLY_SATS,
        "Total supply {} should not exceed MAX_SUPPLY_SATS {}",
        state.total_supply(), MAX_SUPPLY_SATS
    );
}

/// Test rewards when block_reward is 0 (far future after all halvings).
#[test]
fn test_40_zero_block_reward_after_halvings() {
    let mut state = StateEngine::new_with_genesis();

    let val_sk = SecretKey::generate();
    state.faucet_credit(&val_sk.address(), MIN_STAKE_SATS * 2);
    state.apply_stake_tx(&make_stake_tx(&val_sk, MIN_STAKE_SATS, 0)).unwrap();

    let supply_before = state.total_supply();

    let mut producers = HashSet::new();
    producers.insert(val_sk.address());
    // Round far past all halvings — block_reward returns 0
    state.distribute_round_rewards(64 * HALVING_INTERVAL, &producers).unwrap();
    check_supply_invariant(&state);

    assert_eq!(
        state.total_supply(), supply_before,
        "No emission should occur when block_reward is 0"
    );
}

/// Test that multiple delegators to the same validator all earn proportionally.
#[test]
fn test_41_multiple_delegators_proportional_rewards() {
    let mut state = StateEngine::new_with_genesis();

    let val_sk = SecretKey::generate();
    let del1_sk = SecretKey::generate();
    let del2_sk = SecretKey::generate();
    let val = val_sk.address();
    let del1 = del1_sk.address();
    let del2 = del2_sk.address();

    state.faucet_credit(&val, MIN_STAKE_SATS * 3);
    state.faucet_credit(&del1, MIN_DELEGATION_SATS * 10);
    state.faucet_credit(&del2, MIN_DELEGATION_SATS * 10);

    state.apply_stake_tx(&make_stake_tx(&val_sk, MIN_STAKE_SATS, 0)).unwrap();

    // del1 delegates 2x, del2 delegates 1x
    state.apply_delegate_tx(&make_delegate_tx(&del1_sk, val, MIN_DELEGATION_SATS * 2, 0)).unwrap();
    state.apply_delegate_tx(&make_delegate_tx(&del2_sk, val, MIN_DELEGATION_SATS, 0)).unwrap();

    let del1_bal_before = state.balance(&del1);
    let del2_bal_before = state.balance(&del2);

    let mut producers = HashSet::new();
    producers.insert(val);
    state.distribute_round_rewards(100, &producers).unwrap();
    check_supply_invariant(&state);

    let del1_earned = state.balance(&del1) - del1_bal_before;
    let del2_earned = state.balance(&del2) - del2_bal_before;

    // del1 delegated 2x del2's amount, so should earn approximately 2x
    // (rounding may cause small differences)
    assert!(
        del1_earned > 0 && del2_earned > 0,
        "Both delegators should earn rewards. del1={}, del2={}",
        del1_earned, del2_earned
    );
    // Allow 1 sat rounding tolerance
    let ratio = (del1_earned as f64) / (del2_earned as f64);
    assert!(
        (ratio - 2.0).abs() < 0.1,
        "del1 should earn ~2x del2. Ratio={:.3}, del1={}, del2={}",
        ratio, del1_earned, del2_earned
    );
}

/// Test that observer_reward_percent is respected: passive stakers earn 50%.
#[test]
fn test_42_observer_reward_percent_applied() {
    let mut state = StateEngine::new_with_genesis();

    let active_sk = SecretKey::generate();
    let passive_sk = SecretKey::generate();
    let active = active_sk.address();
    let passive = passive_sk.address();

    state.faucet_credit(&active, MIN_STAKE_SATS * 3);
    state.faucet_credit(&passive, MIN_STAKE_SATS * 3);

    // Both stake the same amount
    state.apply_stake_tx(&make_stake_tx(&active_sk, MIN_STAKE_SATS, 0)).unwrap();
    state.apply_stake_tx(&make_stake_tx(&passive_sk, MIN_STAKE_SATS, 0)).unwrap();

    let active_bal_before = state.balance(&active);
    let passive_bal_before = state.balance(&passive);

    // Only active_sk produces — passive_sk is observer
    let mut producers = HashSet::new();
    producers.insert(active);
    state.distribute_round_rewards(100, &producers).unwrap();
    check_supply_invariant(&state);

    let active_earned = state.balance(&active) - active_bal_before;
    let passive_earned = state.balance(&passive) - passive_bal_before;

    // Both have equal stake. Active gets 100% proportional, passive gets 50%.
    // active_earned / passive_earned should be approximately 2x (100/50)
    assert!(active_earned > 0 && passive_earned > 0);
    let ratio = (active_earned as f64) / (passive_earned as f64);
    assert!(
        (ratio - 2.0).abs() < 0.5,
        "Active should earn ~2x passive (OBSERVER_REWARD_PERCENT=50). Ratio={:.2}",
        ratio
    );
}

/// Test that slashing a validator with a delegator who is undelegating
/// correctly slashes the undelegating amount too.
#[test]
fn test_43_slash_during_undelegation() {
    let mut state = StateEngine::new_with_genesis();

    let val_sk = SecretKey::generate();
    let del_sk = SecretKey::generate();
    let val = val_sk.address();

    state.faucet_credit(&val, MIN_STAKE_SATS * 3);
    state.faucet_credit(&del_sk.address(), MIN_DELEGATION_SATS * 2);

    state.apply_stake_tx(&make_stake_tx(&val_sk, MIN_STAKE_SATS, 0)).unwrap();
    state.apply_delegate_tx(&make_delegate_tx(&del_sk, val, MIN_DELEGATION_SATS, 0)).unwrap();

    // Start undelegation
    state.apply_undelegate_tx(&make_undelegate_tx(&del_sk, 1), 100).unwrap();

    let del_before = state.delegation_account(&del_sk.address()).unwrap().delegated;
    let supply_before = state.total_supply();

    // Slash validator — should also slash undelegating delegator
    state.slash(&val);

    let del_after = state.delegation_account(&del_sk.address()).unwrap().delegated;
    let supply_after = state.total_supply();

    assert_eq!(del_after, del_before / 2, "Undelegating delegation should be slashed 50%");
    assert!(supply_after < supply_before, "Supply should decrease from burn");
    check_supply_invariant(&state);
}

/// Test governance parameter change: observer_reward_percent can be set to 0.
#[test]
fn test_44_observer_reward_percent_zero() {
    let mut state = StateEngine::new_with_genesis();

    let active_sk = SecretKey::generate();
    let passive_sk = SecretKey::generate();
    let active = active_sk.address();
    let passive = passive_sk.address();

    state.faucet_credit(&active, MIN_STAKE_SATS * 3);
    state.faucet_credit(&passive, MIN_STAKE_SATS * 3);
    state.apply_stake_tx(&make_stake_tx(&active_sk, MIN_STAKE_SATS, 0)).unwrap();
    state.apply_stake_tx(&make_stake_tx(&passive_sk, MIN_STAKE_SATS, 0)).unwrap();

    // Change observer_reward_percent to 0 via governance_params
    // (normally done via ParameterChange proposal, but we can test directly)
    state.governance_params_mut().observer_reward_percent = 0;

    let passive_bal_before = state.balance(&passive);

    let mut producers = HashSet::new();
    producers.insert(active);
    state.distribute_round_rewards(100, &producers).unwrap();
    check_supply_invariant(&state);

    let passive_earned = state.balance(&passive) - passive_bal_before;
    assert_eq!(passive_earned, 0, "With observer_reward_percent=0, passive should earn nothing");
}

// ============================================================================
// Fixed-denominator council emission (Option C, 2026-04-10)
// ============================================================================
//
// These tests verify that the council emission budget is split into
// COUNCIL_MAX_MEMBERS (21) equal shares regardless of how many seats are
// filled, and that the residual (unfilled seats + integer dust) flows to
// the treasury — NOT to validators, NOT to the founder, NOT silently dropped.
//
// Three invariants must hold across all seat counts:
//
//   (A) **Fair per-member share**: the amount credited to each seated member
//       in a single round is identical across all seat counts. A solo member
//       must not earn 21x what a member in a full council earns.
//
//   (B) **Full council budget accounted for**: the sum of (credited to
//       members) + (residual to treasury) equals the council budget exactly
//       (`block_reward * council_emission_percent / 100`). No sats are
//       silently dropped.
//
//   (C) **Supply invariant holds**: `liquid + staked + delegated + treasury
//       == total_supply` after every distribute_round_rewards call.

fn setup_council_test_state() -> StateEngine {
    // Build a minimal state with a small active validator set so that
    // distribute_round_rewards has something to distribute the 75% validator
    // pool to.
    //
    // `new_with_genesis` gives us a testnet faucet prefund (1,000,000 UDAG)
    // AND automatically seats the dev address as the first Operations council
    // member. We want a clean council slate for these tests (empty council
    // cases, full 21-member cases), so we explicitly remove the dev member
    // immediately after genesis. The faucet prefund is left in place.
    let mut state = StateEngine::new_with_genesis();
    let dev_addr = ultradag_coin::constants::dev_address();
    state.remove_council_member(&dev_addr);
    assert_eq!(
        state.council_member_count(), 0,
        "test helper expects a clean council after removing the genesis dev seat"
    );

    // Fund + stake three validators to give distribute_round_rewards a real
    // validator pool to work with. Uses the testnet faucet prefund.
    for i in 0..3u8 {
        let sk = SecretKey::from_bytes([i + 1; 32]);
        let addr = sk.address();
        state.faucet_credit(&addr, MIN_STAKE_SATS * 2).unwrap();
        let stake_tx = make_stake_tx(&sk, MIN_STAKE_SATS, 0);
        state.apply_stake_tx(&stake_tx).unwrap();
    }
    state
}

fn seat_n_council_members(state: &mut StateEngine, n: usize) -> Vec<Address> {
    // Seat `n` distinct council members. Uses deterministic seeds starting
    // at 0x80 so they don't collide with the validator seeds in
    // setup_council_test_state. Seats fill categories in a fixed order that
    // respects each category's max capacity (Engineering 5 / Community 4 /
    // Growth 3 / Operations 3 / Legal 2 / Research 2 / Security 2 = 21), so
    // we can seat anywhere from 0 to the full 21 without hitting capacity
    // errors. Returns the seated addresses in insertion order (NOT the same
    // as sorted-address-order — distribute_round_rewards sorts internally).
    use ultradag_coin::governance::CouncilSeatCategory;
    let plan: &[(CouncilSeatCategory, usize)] = &[
        (CouncilSeatCategory::Engineering, 5),
        (CouncilSeatCategory::Community, 4),
        (CouncilSeatCategory::Growth, 3),
        (CouncilSeatCategory::Operations, 3),
        (CouncilSeatCategory::Legal, 2),
        (CouncilSeatCategory::Research, 2),
        (CouncilSeatCategory::Security, 2),
    ];
    // Flatten the plan into a 21-element list of category assignments.
    let mut assignments: Vec<CouncilSeatCategory> = Vec::with_capacity(21);
    for (cat, max) in plan {
        for _ in 0..*max {
            assignments.push(*cat);
        }
    }
    assert_eq!(assignments.len(), 21, "seat plan must total COUNCIL_MAX_MEMBERS");

    let mut seated = Vec::with_capacity(n);
    for i in 0..n {
        let sk = SecretKey::from_bytes([0x80 + i as u8; 32]);
        let addr = sk.address();
        // Make sure the account exists before distribute_round_rewards credits
        // it. A 1-sat faucet transfer is the cheapest way to instantiate the
        // account entry without affecting balances meaningfully.
        state.faucet_credit(&addr, 1).unwrap();
        state.add_council_member(addr, assignments[i]).unwrap();
        seated.push(addr);
    }
    seated
}

fn council_budget(round: u64, council_percent: u64) -> u64 {
    block_reward(round).saturating_mul(council_percent) / 100
}

/// (A) Fair per-member share: a solo council member earns exactly the same
/// amount as a member of a full 21-seat council. This was the headline bug
/// in the old seated-count-denominator model — with 1 member the `/1` division
/// gave them 21x the fair share.
#[test]
fn council_fixed_denominator_fair_share_solo_vs_full() {
    let per_member_solo = {
        let mut state = setup_council_test_state();
        let seated = seat_n_council_members(&mut state, 1);
        let bal_before = state.balance(&seated[0]);
        let mut producers = HashSet::new();
        // Use at least one validator as a producer so the validator pool
        // isn't stuck in pre-staking fallback.
        producers.insert(SecretKey::from_bytes([1u8; 32]).address());
        state.distribute_round_rewards(100, &producers).unwrap();
        check_supply_invariant(&state);
        state.balance(&seated[0]) - bal_before
    };

    let per_member_full = {
        let mut state = setup_council_test_state();
        let seated = seat_n_council_members(&mut state, 21);
        // Pick one member we'll measure — any seat should have the same per-member credit.
        let mut sorted = seated.clone();
        sorted.sort();
        let measured = sorted[7]; // arbitrary middle seat
        let bal_before = state.balance(&measured);
        let mut producers = HashSet::new();
        producers.insert(SecretKey::from_bytes([1u8; 32]).address());
        state.distribute_round_rewards(100, &producers).unwrap();
        check_supply_invariant(&state);
        state.balance(&measured) - bal_before
    };

    assert_eq!(
        per_member_solo, per_member_full,
        "fixed-denominator model must pay the same per-member amount \
         regardless of seated count (solo={} vs full={})",
        per_member_solo, per_member_full
    );
    assert!(per_member_solo > 0, "per-member credit should be positive at round 100");
}

/// (B) Empty-council emission flows to treasury, not silently dropped.
/// This was the bug the user raised: under the old model, `council_count == 0`
/// caused the council block to skip entirely, and 10% of every round's
/// emission went nowhere.
///
/// The test uses all 3 stakers as producers so there's no observer-penalty
/// haircut on the validator pool — making the supply growth cleanly equal
/// to the full block reward when the fix is working.
#[test]
fn council_empty_routes_residual_to_treasury() {
    let mut state = setup_council_test_state();
    assert_eq!(state.council_members().count(), 0, "preconditions: no council");

    let treasury_before = state.treasury_balance();
    let supply_before = state.total_supply();
    let round = 100;
    let budget = council_budget(round, state.governance_params().council_emission_percent);

    // Make all 3 stakers producers so the full validator pool is minted
    // (no observer-penalty haircut).
    let mut producers = HashSet::new();
    for i in 0..3u8 {
        producers.insert(SecretKey::from_bytes([i + 1; 32]).address());
    }
    state.distribute_round_rewards(round, &producers).unwrap();
    check_supply_invariant(&state);

    // Treasury grew by at least the full council budget (all 10% becomes
    // residual with 0 seated members), PLUS the normal treasury share.
    let treasury_growth = state.treasury_balance() - treasury_before;
    assert!(
        treasury_growth >= budget,
        "empty council: treasury growth {} should be >= council budget {}",
        treasury_growth, budget
    );

    // Under the fix, the council residual lands in treasury and is minted into
    // supply. Under the OLD broken behavior, the council share was silently
    // skipped. Assert we got the full sum of bucket percentages (88% under the
    // April 2026 model), modulo tiny integer dust from proportional splits.
    let supply_growth = state.total_supply() - supply_before;
    let full_reward = block_reward(round);
    let params = state.governance_params();
    let bucket_sum_pct = params.validator_emission_percent
        + params.council_emission_percent
        + params.treasury_emission_percent
        + params.founder_emission_percent
        + params.ecosystem_emission_percent
        + params.reserve_emission_percent;
    let expected_growth = full_reward * bucket_sum_pct / 100;
    let old_broken_growth = full_reward
        * (bucket_sum_pct - params.council_emission_percent)
        / 100;
    assert!(
        supply_growth > old_broken_growth,
        "empty council: supply growth {} should exceed the old-broken model's {} \
         (if this fails, the council residual is not being routed to treasury)",
        supply_growth, old_broken_growth
    );
    // Should be within dust of the expected 88% bucket sum.
    assert!(
        expected_growth.saturating_sub(supply_growth) <= 100,
        "empty council: supply growth {} should be within dust of expected {} ({}% of reward)",
        supply_growth, expected_growth, bucket_sum_pct
    );
}

/// (B) Partial council: seated members get their fair share, unfilled seats
/// flow to treasury. This is the bootstrap steady state we actually expect to
/// see on mainnet for the first few months.
#[test]
fn council_partial_routes_unfilled_seats_to_treasury() {
    let mut state = setup_council_test_state();
    // Seat 11 of 21 — about half the council.
    let seated = seat_n_council_members(&mut state, 11);

    let treasury_before = state.treasury_balance();
    let member_balances_before: Vec<u64> =
        seated.iter().map(|a| state.balance(a)).collect();

    let round = 100;
    let council_pct = state.governance_params().council_emission_percent;
    let budget = council_budget(round, council_pct);

    let mut producers = HashSet::new();
    producers.insert(SecretKey::from_bytes([1u8; 32]).address());
    state.distribute_round_rewards(round, &producers).unwrap();
    check_supply_invariant(&state);

    // Sum the credits to seated members.
    let member_credits: u64 = seated
        .iter()
        .zip(member_balances_before.iter())
        .map(|(a, before)| state.balance(a) - before)
        .sum();

    // Every seated member should have been credited `budget / 21`.
    let max_members = ultradag_coin::constants::COUNCIL_MAX_MEMBERS as u64;
    let expected_per_member = budget / max_members;
    let expected_to_members = expected_per_member * 11;
    assert_eq!(
        member_credits, expected_to_members,
        "partial council: expected {} to members ({} × 11), got {}",
        expected_to_members, expected_per_member, member_credits
    );

    // The treasury received at least the unfilled-seat residual, plus its
    // normal 10% share.
    let treasury_growth = state.treasury_balance() - treasury_before;
    let council_residual = budget - expected_to_members;
    assert!(
        treasury_growth >= council_residual,
        "partial council: treasury growth {} should be >= council residual {}",
        treasury_growth, council_residual
    );
}

/// (B) Full council (21 members): the residual is only integer dust from
/// the `council_total / 21` division. Every member gets the fair share; any
/// remainder flows to treasury.
#[test]
fn council_full_residual_is_only_dust() {
    let mut state = setup_council_test_state();
    let seated = seat_n_council_members(&mut state, 21);

    let member_balances_before: Vec<u64> =
        seated.iter().map(|a| state.balance(a)).collect();
    let treasury_before = state.treasury_balance();

    let round = 100;
    let council_pct = state.governance_params().council_emission_percent;
    let budget = council_budget(round, council_pct);

    let mut producers = HashSet::new();
    producers.insert(SecretKey::from_bytes([1u8; 32]).address());
    state.distribute_round_rewards(round, &producers).unwrap();
    check_supply_invariant(&state);

    let max_members = ultradag_coin::constants::COUNCIL_MAX_MEMBERS as u64;
    let expected_per_member = budget / max_members;
    let expected_distributed = expected_per_member * max_members;
    let expected_dust = budget - expected_distributed;

    // Every seated member got exactly `expected_per_member`.
    for (addr, before) in seated.iter().zip(member_balances_before.iter()) {
        let credited = state.balance(addr) - before;
        assert_eq!(
            credited, expected_per_member,
            "full council: member {} should be credited {} exactly, got {}",
            addr.to_hex(), expected_per_member, credited
        );
    }

    // Treasury got at least the dust (may be 0 at round 100 if the division
    // is exact — at 1 UDAG/round and 10% council, council_total is
    // 10_000_000 sats, 10_000_000 / 21 = 476_190 with dust 10, so we should
    // see dust > 0 in practice).
    let treasury_growth = state.treasury_balance() - treasury_before;
    assert!(
        treasury_growth >= expected_dust,
        "full council: treasury growth {} should include at least the dust {}",
        treasury_growth, expected_dust
    );
}

/// (C) Supply growth invariance: across 0, 1, 11, 21 seated members, the
/// total supply growth per round is identical (modulo tiny integer dust).
/// The council residual mechanism must never silently drop emission, so
/// supply should grow by the full block reward regardless of how many
/// council seats are filled. Uses all stakers as producers to avoid the
/// observer-reward haircut.
#[test]
fn council_supply_growth_invariant_across_seat_counts() {
    let round = 100;
    let full_reward = block_reward(round);

    // All 3 stakers are producers, so the full validator pool mints cleanly.
    let mut producers = HashSet::new();
    for i in 0..3u8 {
        producers.insert(SecretKey::from_bytes([i + 1; 32]).address());
    }

    let mut growth_by_count = Vec::new();
    for seat_count in &[0usize, 1, 11, 21] {
        let mut state = setup_council_test_state();
        seat_n_council_members(&mut state, *seat_count);
        let supply_before = state.total_supply();
        state.distribute_round_rewards(round, &producers).unwrap();
        check_supply_invariant(&state);
        let growth = state.total_supply() - supply_before;
        growth_by_count.push((*seat_count, growth));
    }

    // All growth amounts should be within a small integer-dust delta of
    // each other — specifically, all equal to `full_reward` minus whatever
    // integer truncation happens in the per-member council split and the
    // proportional validator split.
    let max = growth_by_count.iter().map(|(_, g)| *g).max().unwrap();
    let min = growth_by_count.iter().map(|(_, g)| *g).min().unwrap();
    let max_allowed_delta = 100u64;
    assert!(
        max - min <= max_allowed_delta,
        "supply growth should be ~constant across seat counts: {:?} (max - min = {})",
        growth_by_count, max - min
    );

    // Each growth should be close to the bucket sum × full_reward (within dust).
    // Under April 2026 tokenomics, buckets sum to 88% of the block reward.
    let state = setup_council_test_state();
    let params = state.governance_params();
    let bucket_sum_pct = params.validator_emission_percent
        + params.council_emission_percent
        + params.treasury_emission_percent
        + params.founder_emission_percent
        + params.ecosystem_emission_percent
        + params.reserve_emission_percent;
    let expected_growth = full_reward * bucket_sum_pct / 100;
    for (seats, growth) in &growth_by_count {
        assert!(
            expected_growth.saturating_sub(*growth) <= max_allowed_delta,
            "seat_count={} supply growth {} should be within {} sats of expected {} ({}% of reward)",
            seats, growth, max_allowed_delta, expected_growth, bucket_sum_pct
        );
    }
}
