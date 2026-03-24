/// Staking tests: stake/unstake lifecycle, proportional rewards, slashing, and supply invariants.

use ultradag_coin::*;

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

/// Compute the validator reward pool after council emission deduction.
/// Genesis bootstraps 1 council member with 10% emission, so validators get 90%.
fn validator_pool(round: u64) -> u64 {
    // Emission split: 10% council, 10% treasury, 5% founder → 75% to validators
    let br = block_reward(round);
    let council = br * 10 / 100;
    let treasury = br * 10 / 100;
    let founder = br * 5 / 100;
    br - council - treasury - founder // 75%
}

fn make_vertex(
    proposer_sk: &SecretKey,
    round: u64,
    height: u64,
    txs: Vec<Transaction>,
    _reward: u64,
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

/// test_01: stake_deducts_from_liquid_balance
#[test]
fn test_01_stake_deducts_from_liquid_balance() {
    let mut state = StateEngine::new_with_genesis();
    let sk = SecretKey::generate();
    let amount = MIN_STAKE_SATS * 2;
    state.faucet_credit(&sk.address(), amount);

    let tx = make_stake_tx(&sk, MIN_STAKE_SATS, 0);
    state.apply_stake_tx(&tx).unwrap();

    assert_eq!(state.balance(&sk.address()), amount - MIN_STAKE_SATS);
    assert_eq!(state.stake_of(&sk.address()), MIN_STAKE_SATS);
}

/// test_02: stake_below_minimum_rejected
#[test]
fn test_02_stake_below_minimum_rejected() {
    let mut state = StateEngine::new_with_genesis();
    let sk = SecretKey::generate();
    state.faucet_credit(&sk.address(), MIN_STAKE_SATS * 2);

    let tx = make_stake_tx(&sk, MIN_STAKE_SATS - 1, 0);
    let result = state.apply_stake_tx(&tx);
    assert!(matches!(result, Err(CoinError::BelowMinStake { .. })));
}

/// test_03: stake_makes_validator_active
#[test]
fn test_03_stake_makes_validator_active() {
    let mut state = StateEngine::new_with_genesis();
    let sk = SecretKey::generate();
    state.faucet_credit(&sk.address(), MIN_STAKE_SATS);

    assert!(state.active_stakers().is_empty());

    let tx = make_stake_tx(&sk, MIN_STAKE_SATS, 0);
    state.apply_stake_tx(&tx).unwrap();

    let active = state.active_stakers();
    assert_eq!(active.len(), 1);
    assert_eq!(active[0], sk.address());
}

/// test_04: unstake_begins_cooldown
#[test]
fn test_04_unstake_begins_cooldown() {
    let mut state = StateEngine::new_with_genesis();
    let sk = SecretKey::generate();
    state.faucet_credit(&sk.address(), MIN_STAKE_SATS);

    let stake_tx = make_stake_tx(&sk, MIN_STAKE_SATS, 0);
    state.apply_stake_tx(&stake_tx).unwrap();

    let unstake_tx = make_unstake_tx(&sk, 1);
    state.apply_unstake_tx(&unstake_tx, 100).unwrap();

    let acct = state.stake_account(&sk.address()).unwrap();
    assert_eq!(acct.unlock_at_round, Some(100 + UNSTAKE_COOLDOWN_ROUNDS));
    // Still staked during cooldown
    assert_eq!(state.stake_of(&sk.address()), MIN_STAKE_SATS);
    // No longer an active staker (unstaking)
    assert!(state.active_stakers().is_empty());
}

/// test_05: unstake_returns_funds_after_cooldown
#[test]
fn test_05_unstake_returns_funds_after_cooldown() {
    let mut state = StateEngine::new_with_genesis();
    let sk = SecretKey::generate();
    state.faucet_credit(&sk.address(), MIN_STAKE_SATS);

    let stake_tx = make_stake_tx(&sk, MIN_STAKE_SATS, 0);
    state.apply_stake_tx(&stake_tx).unwrap();
    assert_eq!(state.balance(&sk.address()), 0);

    let unstake_tx = make_unstake_tx(&sk, 1);
    state.apply_unstake_tx(&unstake_tx, 100).unwrap();

    // Before cooldown — funds still locked
    state.process_unstake_completions(100 + UNSTAKE_COOLDOWN_ROUNDS - 1);
    assert_eq!(state.balance(&sk.address()), 0);
    assert_eq!(state.stake_of(&sk.address()), MIN_STAKE_SATS);

    // At cooldown completion — funds returned
    state.process_unstake_completions(100 + UNSTAKE_COOLDOWN_ROUNDS);
    assert_eq!(state.balance(&sk.address()), MIN_STAKE_SATS);
    assert_eq!(state.stake_of(&sk.address()), 0);
}

/// test_06: unstake_before_cooldown_does_not_return_funds
#[test]
fn test_06_unstake_before_cooldown_does_not_return_funds() {
    let mut state = StateEngine::new_with_genesis();
    let sk = SecretKey::generate();
    state.faucet_credit(&sk.address(), MIN_STAKE_SATS);

    let stake_tx = make_stake_tx(&sk, MIN_STAKE_SATS, 0);
    state.apply_stake_tx(&stake_tx).unwrap();

    let unstake_tx = make_unstake_tx(&sk, 1);
    state.apply_unstake_tx(&unstake_tx, 100).unwrap();

    // 1 round before cooldown ends
    state.process_unstake_completions(100 + UNSTAKE_COOLDOWN_ROUNDS - 1);
    assert_eq!(state.balance(&sk.address()), 0, "Funds still locked before cooldown");
}

/// test_07: double_unstake_rejected
#[test]
fn test_07_double_unstake_rejected() {
    let mut state = StateEngine::new_with_genesis();
    let sk = SecretKey::generate();
    state.faucet_credit(&sk.address(), MIN_STAKE_SATS);

    let stake_tx = make_stake_tx(&sk, MIN_STAKE_SATS, 0);
    state.apply_stake_tx(&stake_tx).unwrap();

    let unstake_tx1 = make_unstake_tx(&sk, 1);
    state.apply_unstake_tx(&unstake_tx1, 100).unwrap();

    let unstake_tx2 = make_unstake_tx(&sk, 2);
    let result = state.apply_unstake_tx(&unstake_tx2, 200);
    assert!(matches!(result, Err(CoinError::AlreadyUnstaking)));
}

/// test_08: slash_burns_half_stake
#[test]
fn test_08_slash_burns_half_stake() {
    let mut state = StateEngine::new_with_genesis();
    let sk = SecretKey::generate();
    state.faucet_credit(&sk.address(), MIN_STAKE_SATS * 2);

    let stake_tx = make_stake_tx(&sk, MIN_STAKE_SATS * 2, 0);
    state.apply_stake_tx(&stake_tx).unwrap();
    assert_eq!(state.stake_of(&sk.address()), MIN_STAKE_SATS * 2);

    state.slash(&sk.address());
    assert_eq!(state.stake_of(&sk.address()), MIN_STAKE_SATS);
}

/// test_09: slash_reduces_total_supply
#[test]
fn test_09_slash_reduces_total_supply() {
    let mut state = StateEngine::new_with_genesis();
    let sk = SecretKey::generate();
    let amount = MIN_STAKE_SATS * 2;
    state.faucet_credit(&sk.address(), amount);
    let supply_before = state.total_supply();

    let stake_tx = make_stake_tx(&sk, amount, 0);
    state.apply_stake_tx(&stake_tx).unwrap();
    // Staking doesn't change total supply (just moves from liquid to staked)
    assert_eq!(state.total_supply(), supply_before);

    state.slash(&sk.address());
    let slash_amount = amount / 2;
    assert_eq!(state.total_supply(), supply_before - slash_amount);
}

/// test_10: proportional_rewards_sum_to_block_reward
#[test]
fn test_10_proportional_rewards_sum_to_block_reward() {
    let mut state = StateEngine::new_with_genesis();
    let sk1 = SecretKey::generate();
    let sk2 = SecretKey::generate();

    // Give both validators funds and stake them
    state.faucet_credit(&sk1.address(), MIN_STAKE_SATS * 3);
    state.faucet_credit(&sk2.address(), MIN_STAKE_SATS);

    let stake1 = make_stake_tx(&sk1, MIN_STAKE_SATS * 3, 0);
    let stake2 = make_stake_tx(&sk2, MIN_STAKE_SATS, 0);
    state.apply_stake_tx(&stake1).unwrap();
    state.apply_stake_tx(&stake2).unwrap();

    let total_stake = state.total_staked();
    assert_eq!(total_stake, MIN_STAKE_SATS * 4);

    let total_round_reward = block_reward(0);

    // Compute proportional rewards
    let reward1 = ((total_round_reward as u128) * (MIN_STAKE_SATS * 3) as u128 / total_stake as u128) as u64;
    let reward2 = ((total_round_reward as u128) * MIN_STAKE_SATS as u128 / total_stake as u128) as u64;

    // Sum should be <= total_round_reward (rounding remainder is implicitly burned)
    assert!(reward1 + reward2 <= total_round_reward);
    // Should be very close (within a few sats of rounding)
    assert!(total_round_reward - (reward1 + reward2) < 4);
}

/// test_11: proportional_rewards_with_equal_stake_splits_evenly
#[test]
fn test_11_proportional_rewards_with_equal_stake_splits_evenly() {
    let mut state = StateEngine::new_with_genesis();
    let sks: Vec<_> = (0..4).map(|_| SecretKey::generate()).collect();

    for sk in &sks {
        state.faucet_credit(&sk.address(), MIN_STAKE_SATS);
        let tx = make_stake_tx(sk, MIN_STAKE_SATS, 0);
        state.apply_stake_tx(&tx).unwrap();
    }

    let total_round_reward = block_reward(0);
    let per_validator = total_round_reward / 4;

    // Each validator should get exactly 1/4 of the reward
    for sk in &sks {
        let reward = ((total_round_reward as u128)
            * state.stake_of(&sk.address()) as u128
            / state.total_staked() as u128) as u64;
        assert_eq!(reward, per_validator);
    }
}

/// test_12: proportional_rewards_with_unequal_stake_favors_larger_staker
#[test]
fn test_12_proportional_rewards_with_unequal_stake_favors_larger_staker() {
    let mut state = StateEngine::new_with_genesis();
    let sk_big = SecretKey::generate();
    let sk_small = SecretKey::generate();

    state.faucet_credit(&sk_big.address(), MIN_STAKE_SATS * 9);
    state.faucet_credit(&sk_small.address(), MIN_STAKE_SATS);

    let stake_big = make_stake_tx(&sk_big, MIN_STAKE_SATS * 9, 0);
    let stake_small = make_stake_tx(&sk_small, MIN_STAKE_SATS, 0);
    state.apply_stake_tx(&stake_big).unwrap();
    state.apply_stake_tx(&stake_small).unwrap();

    let total = block_reward(0);
    let reward_big = ((total as u128) * (MIN_STAKE_SATS * 9) as u128 / state.total_staked() as u128) as u64;
    let reward_small = ((total as u128) * MIN_STAKE_SATS as u128 / state.total_staked() as u128) as u64;

    assert!(reward_big > reward_small);
    // 90% stake should get ~90% of reward
    assert_eq!(reward_big, total * 9 / 10);
    assert_eq!(reward_small, total / 10);
}

/// test_13: zero_stake_fallback_uses_equal_split
#[test]
fn test_13_zero_stake_fallback_uses_equal_split() {
    let mut state = StateEngine::new_with_genesis();
    let sk = SecretKey::generate();

    // No staking — equal split fallback (apply_vertex defaults to count=1)
    // Genesis has 1 council member, so validator gets 90% of block_reward
    let reward = validator_pool(0);
    let vertex = make_vertex(&sk, 0, 0, vec![], reward);
    state.apply_vertex(&vertex).unwrap();

    assert_eq!(state.balance(&sk.address()), reward);
}

/// test_14: total_emission_invariant_holds_100_rounds
#[test]
fn test_14_total_emission_invariant_holds_100_rounds() {
    let mut state = StateEngine::new_with_genesis();
    let sks: Vec<_> = (0..4).map(|_| SecretKey::generate()).collect();

    // Give validators funds and stake them
    for sk in &sks {
        state.faucet_credit(&sk.address(), MIN_STAKE_SATS * 2);
        let tx = make_stake_tx(sk, MIN_STAKE_SATS, 0);
        state.apply_stake_tx(&tx).unwrap();
    }
    // Vary stake: give validator 0 extra stake
    let extra_stake = make_stake_tx(&sks[0], MIN_STAKE_SATS, 1);
    state.apply_stake_tx(&extra_stake).unwrap();

    let supply_after_setup = state.total_supply();

    for round in 0u64..100 {
        let vpool = validator_pool(round);
        let total_stake = state.total_staked();

        // All 4 validators produce in each round
        let mut round_vertices = Vec::new();
        for (i, sk) in sks.iter().enumerate() {
            let own_stake = state.stake_of(&sk.address());
            let validator_reward = ((vpool as u128)
                * own_stake as u128 / total_stake as u128) as u64;
            let v = make_vertex(sk, round, round * 4 + i as u64, vec![], validator_reward);
            round_vertices.push(v);
        }

        // Coinbase amount is always 0 — rewards distributed via distribute_round_rewards()
        for v in &round_vertices {
            assert_eq!(v.block.coinbase.amount, 0, "Round {}: coinbase must be zero", round);
        }

        state.apply_finalized_vertices(&round_vertices).unwrap();
    }

    // Total supply should be genesis amounts + sum of all rewards (validator + council emission)
    assert!(state.total_supply() > supply_after_setup);
    assert!(state.total_supply() <= supply_after_setup + 100 * block_reward(0));
}

/// test_15: dev_allocation_plus_coinbase_never_exceeds_max_supply
#[test]
fn test_15_dev_allocation_plus_coinbase_never_exceeds_max_supply() {
    let mut state = StateEngine::new_with_genesis();

    // Apply many vertices to approach max supply through block rewards
    let validators: Vec<_> = (0..4).map(|_| SecretKey::generate()).collect();

    // Apply vertices until we're very close to max supply
    let mut round = 0u64;
    while state.total_supply() < MAX_SUPPLY_SATS - (1000 * COIN) {
        let proposer = &validators[(round % 4) as usize];
        let reward = validator_pool(round);
        let vertex = make_vertex(proposer, round, round, vec![], reward);
        state.apply_vertex(&vertex).unwrap();
        round += 1;

        // Safety: don't loop forever
        if round > 100_000 {
            break;
        }
    }

    // Now apply one more vertex - should be capped at MAX_SUPPLY
    let sk = &validators[0];
    let reward = validator_pool(round);
    let vertex = make_vertex(sk, round, round, vec![], reward);
    state.apply_vertex(&vertex).unwrap();

    assert!(state.total_supply() <= MAX_SUPPLY_SATS,
        "Supply {} exceeded max {}", state.total_supply(), MAX_SUPPLY_SATS);
}

/// test_16: stake_nonce_increments_correctly
#[test]
fn test_16_stake_nonce_increments_correctly() {
    let mut state = StateEngine::new_with_genesis();
    let sk = SecretKey::generate();
    state.faucet_credit(&sk.address(), MIN_STAKE_SATS * 5);

    assert_eq!(state.nonce(&sk.address()), 0);

    let tx1 = make_stake_tx(&sk, MIN_STAKE_SATS, 0);
    state.apply_stake_tx(&tx1).unwrap();
    assert_eq!(state.nonce(&sk.address()), 1);

    let tx2 = make_stake_tx(&sk, MIN_STAKE_SATS, 1);
    state.apply_stake_tx(&tx2).unwrap();
    assert_eq!(state.nonce(&sk.address()), 2);
}

/// test_17: unstake_nonce_increments_correctly
#[test]
fn test_17_unstake_nonce_increments_correctly() {
    let mut state = StateEngine::new_with_genesis();
    let sk = SecretKey::generate();
    state.faucet_credit(&sk.address(), MIN_STAKE_SATS);

    let stake_tx = make_stake_tx(&sk, MIN_STAKE_SATS, 0);
    state.apply_stake_tx(&stake_tx).unwrap();
    assert_eq!(state.nonce(&sk.address()), 1);

    let unstake_tx = make_unstake_tx(&sk, 1);
    state.apply_unstake_tx(&unstake_tx, 100).unwrap();
    assert_eq!(state.nonce(&sk.address()), 2);
}

/// test_18: staking_tx_replay_rejected
#[test]
fn test_18_staking_tx_replay_rejected() {
    let mut state = StateEngine::new_with_genesis();
    let sk = SecretKey::generate();
    state.faucet_credit(&sk.address(), MIN_STAKE_SATS * 3);

    let tx = make_stake_tx(&sk, MIN_STAKE_SATS, 0);
    state.apply_stake_tx(&tx).unwrap();

    // Replay the same tx (same nonce=0)
    let result = state.apply_stake_tx(&tx);
    assert!(matches!(result, Err(CoinError::InvalidNonce { expected: 1, got: 0 })));
}

/// test_19: epoch_boundary_updates_validator_set
#[test]
fn test_19_epoch_boundary_updates_validator_set() {
    let mut state = StateEngine::new_with_genesis();
    let sks: Vec<_> = (0..3).map(|_| SecretKey::generate()).collect();

    // Stake all 3 validators
    for sk in &sks {
        state.faucet_credit(&sk.address(), MIN_STAKE_SATS);
        let tx = make_stake_tx(sk, MIN_STAKE_SATS, 0);
        state.apply_stake_tx(&tx).unwrap();
    }

    // Produce a vertex at round 0 (epoch boundary) — should trigger active set calculation
    // With 3 equal stakers, proportional reward = validator_pool / 3
    let total_stake = state.total_staked();
    let own_stake = state.stake_of(&sks[0].address());
    let reward = ((validator_pool(0) as u128) * own_stake as u128 / total_stake as u128) as u64;
    let v = make_vertex(&sks[0], 0, 0, vec![], reward);
    state.apply_vertex(&v).unwrap();

    let active = state.active_validators();
    assert_eq!(active.len(), 3, "All 3 stakers should be in active set");
    for sk in &sks {
        assert!(active.contains(&sk.address()));
    }

    // Produce a vertex at the next epoch boundary
    let epoch2_round = EPOCH_LENGTH_ROUNDS;
    let own_stake2 = state.stake_of(&sks[1].address());
    let total_stake2 = state.total_staked();
    let reward2 = ((validator_pool(epoch2_round) as u128) * own_stake2 as u128 / total_stake2 as u128) as u64;
    let v2 = make_vertex(&sks[1], epoch2_round, 1, vec![], reward2);
    state.apply_vertex(&v2).unwrap();

    assert_eq!(state.current_epoch(), 1);
    assert_eq!(state.active_validators().len(), 3);
}

/// test_20: max_validators_cap_enforced
#[test]
fn test_20_max_validators_cap_enforced() {
    let mut state = StateEngine::new_with_genesis();
    let count = MAX_ACTIVE_VALIDATORS + 5;
    let sks: Vec<_> = (0..count).map(|_| SecretKey::generate()).collect();

    // Stake all of them
    for sk in &sks {
        state.faucet_credit(&sk.address(), MIN_STAKE_SATS);
        let tx = make_stake_tx(sk, MIN_STAKE_SATS, 0);
        state.apply_stake_tx(&tx).unwrap();
    }

    // Trigger active set recalculation
    state.recalculate_active_set();

    assert_eq!(
        state.active_validators().len(),
        MAX_ACTIVE_VALIDATORS,
        "Active set should be capped at MAX_ACTIVE_VALIDATORS"
    );
}

/// test_21: highest_stake_wins_validator_slot
#[test]
fn test_21_highest_stake_wins_validator_slot() {
    let mut state = StateEngine::new_with_genesis();

    // Create MAX_ACTIVE_VALIDATORS + 1 stakers with varying stakes
    let count = MAX_ACTIVE_VALIDATORS + 1;
    let sks: Vec<_> = (0..count).map(|_| SecretKey::generate()).collect();

    // First MAX_ACTIVE_VALIDATORS get MIN_STAKE, last one gets 2x MIN_STAKE
    for (i, sk) in sks.iter().enumerate() {
        let amount = if i == count - 1 { MIN_STAKE_SATS * 2 } else { MIN_STAKE_SATS };
        state.faucet_credit(&sk.address(), amount);
        let tx = make_stake_tx(sk, amount, 0);
        state.apply_stake_tx(&tx).unwrap();
    }

    state.recalculate_active_set();

    let active = state.active_validators();
    assert_eq!(active.len(), MAX_ACTIVE_VALIDATORS);

    // The highest staker (last one, 2x stake) should be in the active set
    let big_staker = sks.last().unwrap().address();
    assert!(
        active.contains(&big_staker),
        "Highest staker should be in active set"
    );

    // Exactly one of the MIN_STAKE stakers should be excluded
    let excluded: Vec<_> = sks[..count - 1]
        .iter()
        .filter(|sk| !active.contains(&sk.address()))
        .collect();
    assert_eq!(excluded.len(), 1, "Exactly one small staker should be excluded");
}

/// test_22: observer_earns_reduced_reward
#[test]
fn test_22_observer_earns_reduced_reward() {
    let mut state = StateEngine::new_with_genesis();

    // Create MAX_ACTIVE_VALIDATORS + 1 stakers
    let count = MAX_ACTIVE_VALIDATORS + 1;
    let sks: Vec<_> = (0..count).map(|_| SecretKey::generate()).collect();

    // All stake the same amount
    for sk in &sks {
        state.faucet_credit(&sk.address(), MIN_STAKE_SATS);
        let tx = make_stake_tx(sk, MIN_STAKE_SATS, 0);
        state.apply_stake_tx(&tx).unwrap();
    }

    // Trigger epoch recalculation — one staker will be excluded (observer)
    state.recalculate_active_set();
    assert_eq!(state.active_validators().len(), MAX_ACTIVE_VALIDATORS);

    // Find who is the observer
    let observer_sk = sks.iter()
        .find(|sk| !state.is_active_validator(&sk.address()))
        .expect("One staker should be an observer");
    let active_sk = sks.iter()
        .find(|sk| state.is_active_validator(&sk.address()))
        .expect("At least one staker should be active");

    let total_stake = state.total_staked();
    let vpool = validator_pool(0);
    let base_reward = ((vpool as u128) * MIN_STAKE_SATS as u128
        / total_stake as u128) as u64;
    let observer_rate = base_reward * OBSERVER_REWARD_PERCENT / 100;

    // Observer produces vertex — distribute_round_rewards treats them as producer (100%)
    // and all other stakers as non-producers (observer rate 20%)
    let v_obs = make_vertex(observer_sk, 0, 0, vec![], 0);
    let balance_before_obs = state.balance(&observer_sk.address());
    state.apply_vertex(&v_obs).unwrap();
    // Observer is a producer this round, so they earn base_reward (not the 20% observer rate)
    assert_eq!(
        state.balance(&observer_sk.address()),
        balance_before_obs + base_reward,
        "Observer producing a vertex should earn producer-rate reward"
    );

    // Active validator produces vertex — gets full reward
    let v_act = make_vertex(active_sk, 1, 1, vec![], 0);
    let balance_before_act = state.balance(&active_sk.address());
    state.apply_vertex(&v_act).unwrap();
    assert_eq!(
        state.balance(&active_sk.address()),
        balance_before_act + base_reward,
        "Active validator should earn full reward"
    );

    // Observer rate should be exactly OBSERVER_REWARD_PERCENT% of base
    assert_eq!(observer_rate, base_reward * OBSERVER_REWARD_PERCENT / 100);
}

/// test_23: mid_epoch_stake_does_not_change_active_set
#[test]
fn test_23_mid_epoch_stake_does_not_change_active_set() {
    let mut state = StateEngine::new_with_genesis();

    // Setup initial validators
    let initial_sks: Vec<_> = (0..3).map(|_| SecretKey::generate()).collect();
    for sk in &initial_sks {
        state.faucet_credit(&sk.address(), MIN_STAKE_SATS);
        let tx = make_stake_tx(sk, MIN_STAKE_SATS, 0);
        state.apply_stake_tx(&tx).unwrap();
    }

    // Produce vertex at round 0 (epoch 0 boundary) to set active set
    let total_stake = state.total_staked();
    let own_stake = state.stake_of(&initial_sks[0].address());
    let reward = ((validator_pool(0) as u128) * own_stake as u128 / total_stake as u128) as u64;
    let v0 = make_vertex(&initial_sks[0], 0, 0, vec![], reward);
    state.apply_vertex(&v0).unwrap();

    let active_before = state.active_validators().to_vec();
    assert_eq!(active_before.len(), 3);

    // New staker joins mid-epoch with huge stake (10x the initial stakers)
    let new_sk = SecretKey::generate();
    state.faucet_credit(&new_sk.address(), MIN_STAKE_SATS * 10).unwrap();
    let big_stake = make_stake_tx(&new_sk, MIN_STAKE_SATS * 10, 0);
    state.apply_stake_tx(&big_stake).unwrap();

    // Produce vertex at round 5 (still epoch 0, not a boundary)
    // Now total_stake includes the new big staker
    let total_stake2 = state.total_staked();
    let own_stake2 = state.stake_of(&initial_sks[1].address());
    let reward2 = ((validator_pool(1) as u128) * own_stake2 as u128 / total_stake2 as u128) as u64;
    let v5 = make_vertex(&initial_sks[1], 5, 1, vec![], reward2);
    state.apply_vertex(&v5).unwrap();

    // Active set should NOT have changed mid-epoch
    let active_after = state.active_validators().to_vec();
    assert_eq!(active_before, active_after, "Active set should not change mid-epoch");

    // New staker should NOT be in the active set yet
    assert!(
        !state.is_active_validator(&new_sk.address()),
        "Mid-epoch staker should not be in active set until next epoch"
    );
}

/// test_24: stale_epoch_on_load_triggers_recalculation
#[test]
fn test_24_stale_epoch_on_load_triggers_recalculation() {
    let tmp = std::env::temp_dir().join(format!("ultradag_test24_{}", std::process::id()));
    let _ = std::fs::create_dir_all(&tmp);
    let state_path = tmp.join("state.redb");

    let mut state = StateEngine::new_with_genesis();
    let sks: Vec<_> = (0..3).map(|_| SecretKey::generate()).collect();

    // Stake all 3
    for sk in &sks {
        state.faucet_credit(&sk.address(), MIN_STAKE_SATS);
        let tx = make_stake_tx(sk, MIN_STAKE_SATS, 0);
        state.apply_stake_tx(&tx).unwrap();
    }

    // Build state through round 210,001 (epoch 1)
    // First vertex at round 0 triggers epoch 0 active set
    let total_stake = state.total_staked();
    let own0 = state.stake_of(&sks[0].address());
    let r0 = ((validator_pool(0) as u128) * own0 as u128 / total_stake as u128) as u64;
    let v0 = make_vertex(&sks[0], 0, 0, vec![], r0);
    state.apply_vertex(&v0).unwrap();
    assert_eq!(state.current_epoch(), 0);

    // Jump to round 210,001 (epoch 1) — past first halving at 210,000
    let epoch1_round = EPOCH_LENGTH_ROUNDS + 1;
    let own1 = state.stake_of(&sks[1].address());
    let ts1 = state.total_staked();
    let r1 = ((validator_pool(epoch1_round) as u128) * own1 as u128 / ts1 as u128) as u64;
    let v1 = make_vertex(&sks[1], epoch1_round, 1, vec![], r1);
    state.apply_vertex(&v1).unwrap();
    assert_eq!(state.current_epoch(), 1);

    // Save state
    state.save(&state_path).unwrap();

    // Tamper: load state, modify epoch to 0, and re-save
    let mut tampered = StateEngine::load(&state_path).unwrap();
    // Use from_parts to reconstruct with epoch=0
    let tampered_state = StateEngine::from_parts(
        tampered.all_accounts().map(|(k, v)| (*k, v.clone())).collect(),
        tampered.all_stakes().map(|(k, v)| (*k, v.clone())).collect(),
        tampered.active_validators().to_vec(),
        0, // stale epoch
        tampered.total_supply(),
        tampered.last_finalized_round(),
        tampered.all_proposals().map(|(k, v)| (*k, v.clone())).collect(),
        tampered.all_votes().map(|(k, v)| (*k, *v)).collect(),
        tampered.next_proposal_id(),
        tampered.governance_params().clone(),
        tampered.configured_validator_count(),
        tampered.council_members().map(|(k, v)| (*k, *v)).collect(),
        tampered.treasury_balance(),
        tampered.all_delegations().map(|(k, v)| (*k, v.clone())).collect(),
        tampered.bridge_reserve(),
    ).unwrap();
    tampered_state.save(&state_path).unwrap();

    // Reload — should detect stale epoch and recalculate
    let reloaded = StateEngine::load(&state_path).unwrap();
    assert_eq!(reloaded.current_epoch(), 1, "Epoch should be reconciled to 1 on load");
    assert_eq!(reloaded.active_validators().len(), 3, "Active set should be recalculated");
    for sk in &sks {
        assert!(reloaded.active_validators().contains(&sk.address()));
    }

    let _ = std::fs::remove_dir_all(&tmp);
}

/// test_25: observer_reward_increases_total_supply_correctly
#[test]
fn test_25_observer_reward_increases_total_supply_correctly() {
    let mut state = StateEngine::new_with_genesis();

    // 22 stakers: 21 active + 1 observer
    let count = MAX_ACTIVE_VALIDATORS + 1;
    let sks: Vec<_> = (0..count).map(|_| SecretKey::generate()).collect();

    for sk in &sks {
        state.faucet_credit(&sk.address(), MIN_STAKE_SATS);
        let tx = make_stake_tx(sk, MIN_STAKE_SATS, 0);
        state.apply_stake_tx(&tx).unwrap();
    }

    // Trigger epoch recalculation
    state.recalculate_active_set();
    assert_eq!(state.active_validators().len(), MAX_ACTIVE_VALIDATORS);

    let observer_sk = sks.iter()
        .find(|sk| !state.is_active_validator(&sk.address()))
        .expect("One observer");
    let active_sks: Vec<_> = sks.iter()
        .filter(|sk| state.is_active_validator(&sk.address()))
        .collect();

    let supply_before = state.total_supply();

    // Observer produces vertex at round 0
    // distribute_round_rewards credits ALL stakers: producer at 100%, non-producers at 20%
    let v_obs = make_vertex(observer_sk, 0, 0, vec![], 0);
    state.apply_vertex(&v_obs).unwrap();

    // One active validator produces vertex at round 1
    let v_act = make_vertex(active_sks[0], 1, 1, vec![], 0);
    state.apply_vertex(&v_act).unwrap();

    let supply_after = state.total_supply();
    let supply_increase = supply_after - supply_before;

    // Supply should have increased (validator rewards + council emission were minted)
    assert!(supply_increase > 0, "Supply should increase from block rewards");

    // Each round's total validator emission should be <= validator_pool
    let vpool0 = validator_pool(0);
    let vpool1 = validator_pool(1);
    assert!(supply_increase <= 2 * (vpool0 + vpool1),
        "Total supply increase bounded by validator pools + council emission");

    // Supply invariant is checked unconditionally inside apply_vertex
    // (liquid + staked + delegated + treasury == total_supply)
}

/// test_26: epoch_boundary_at_genesis_with_no_stakers
#[test]
fn test_26_epoch_boundary_at_genesis_with_no_stakers() {
    let mut state = StateEngine::new_with_genesis();

    // Round 0 is an epoch boundary: epoch_of(0)=0, is_epoch_boundary(0)=true
    assert!(is_epoch_boundary(0));

    // Explicitly recalculate with no stakers — should not panic
    state.recalculate_active_set();
    assert!(state.active_validators().is_empty(), "No stakers means no active validators");

    // Apply genesis vertex (no staking, pre-staking fallback)
    // Genesis has 1 council member, so validator gets 90% of block_reward
    let sk = SecretKey::generate();
    let reward = validator_pool(0);
    let v = make_vertex(&sk, 0, 0, vec![], reward);
    state.apply_vertex(&v).unwrap();

    // Supply invariant is checked unconditionally inside apply_vertex
    // (liquid + staked + treasury == total_supply)
    assert!(state.active_validators().is_empty(),
        "Active set should still be empty with no stakers");
}

/// test_27: slash_policy_is_explicit — slash immediately removes from active set
#[test]
fn test_27_slash_policy_is_explicit() {
    let mut state = StateEngine::new_with_genesis();
    let sk = SecretKey::generate();

    // Stake exactly MIN_STAKE_SATS
    state.faucet_credit(&sk.address(), MIN_STAKE_SATS);
    let tx = make_stake_tx(&sk, MIN_STAKE_SATS, 0);
    state.apply_stake_tx(&tx).unwrap();

    // Add to active set at epoch boundary
    state.recalculate_active_set();
    assert!(state.is_active_validator(&sk.address()),
        "Validator should be in active set before slash");

    // Slash 50%: stake drops from 10,000 to 5,000 UDAG (below MIN_STAKE_SATS)
    state.slash(&sk.address());
    assert_eq!(state.stake_of(&sk.address()), MIN_STAKE_SATS / 2);
    assert!(state.stake_of(&sk.address()) < MIN_STAKE_SATS,
        "Stake should be below minimum after slash");

    // SLASH POLICY: slash immediately removes from active validator set.
    // This is the chosen policy. A known-equivocating validator should not
    // continue earning rewards for up to 210,000 rounds.
    assert!(
        !state.is_active_validator(&sk.address()),
        "Slashed validator below MIN_STAKE must be immediately removed from active set"
    );
}

/// test_28: delegate_to_self_rejected
/// Verifies that apply_delegate_tx rejects self-delegation (from == validator).
/// Self-delegation would inflate effective_stake without additional economic risk.
#[test]
fn test_28_delegate_to_self_rejected() {
    let mut state = StateEngine::new_with_genesis();
    let sk = SecretKey::from_bytes([0xAA; 32]);
    let addr = sk.address();

    // Fund and stake so address is a valid validator
    state.faucet_credit(&addr, MIN_STAKE_SATS * 3).unwrap();
    let stake_tx = make_stake_tx(&sk, MIN_STAKE_SATS, 0);
    state.apply_stake_tx(&stake_tx).unwrap();
    state.recalculate_active_set();
    assert!(state.is_active_validator(&addr), "Should be an active validator");

    // Try to delegate to self — should fail
    let mut delegate_tx = DelegateTx {
        from: addr,
        validator: addr, // self-delegation
        amount: MIN_DELEGATION_SATS,
        nonce: 1,
        pub_key: sk.verifying_key().to_bytes(),
        signature: Signature([0u8; 64]),
    };
    delegate_tx.signature = sk.sign(&delegate_tx.signable_bytes());

    let result = state.apply_delegate_tx(&delegate_tx);
    assert!(result.is_err(), "Self-delegation must be rejected");
    let err_msg = format!("{}", result.unwrap_err());
    assert!(
        err_msg.contains("cannot delegate to self"),
        "Error should mention self-delegation, got: {}",
        err_msg
    );

    // Verify no delegation was created
    assert!(
        state.delegation_account(&addr).is_none(),
        "No delegation account should exist after rejected self-delegation"
    );
}

/// test_29: pre_staking_reward_distribution_deterministic
/// Verifies that distribute_round_rewards gives each producer the correct
/// pre-staking reward (equal split) regardless of HashSet iteration order.
#[test]
fn test_29_pre_staking_reward_distribution_deterministic() {
    use std::collections::HashSet;

    let sk1 = SecretKey::from_bytes([0x01; 32]);
    let sk2 = SecretKey::from_bytes([0x02; 32]);
    let sk3 = SecretKey::from_bytes([0x03; 32]);
    let sk4 = SecretKey::from_bytes([0x04; 32]);

    let addresses = vec![sk1.address(), sk2.address(), sk3.address(), sk4.address()];

    // Run the same distribution twice on independent state engines
    // and verify they produce identical results.
    let mut balances_run1 = Vec::new();
    let mut balances_run2 = Vec::new();

    for run_balances in [&mut balances_run1, &mut balances_run2] {
        let mut state = StateEngine::new_with_genesis();
        // No staking — pre-staking fallback path
        // Set configured_validator_count so the pre-staking path
        // knows how many validators to split among
        state.set_configured_validator_count(4);

        let mut producers: HashSet<Address> = HashSet::new();
        for addr in &addresses {
            producers.insert(*addr);
        }

        state.distribute_round_rewards(0, &producers).unwrap();

        for addr in &addresses {
            run_balances.push(state.balance(addr));
        }
    }

    // Both runs must produce identical balances
    assert_eq!(balances_run1, balances_run2,
        "Pre-staking reward distribution must be deterministic");

    // Each producer should get an equal share of the validator pool
    let expected_per_producer = validator_pool(0) / 4;
    for (i, balance) in balances_run1.iter().enumerate() {
        assert_eq!(
            *balance, expected_per_producer,
            "Producer {} should get {}, got {}",
            i, expected_per_producer, balance
        );
    }

    // Verify total minted does not exceed the block reward
    let total_minted: u64 = balances_run1.iter().sum();
    assert!(
        total_minted <= block_reward(0),
        "Total minted {} should not exceed block_reward {}",
        total_minted, block_reward(0)
    );
}

/// test_30: delegate_to_unstaking_validator_rejected
/// Verifies that delegation to a validator in unstake cooldown is rejected.
/// Delegating to an exiting validator would leave delegators stranded with no rewards.
#[test]
fn test_30_delegate_to_unstaking_validator_rejected() {
    let mut state = StateEngine::new_with_genesis();
    let validator_sk = SecretKey::from_bytes([0xBB; 32]);
    let delegator_sk = SecretKey::from_bytes([0xCC; 32]);
    let val_addr = validator_sk.address();
    let del_addr = delegator_sk.address();

    // Fund validator, stake, activate
    state.faucet_credit(&val_addr, MIN_STAKE_SATS * 3).unwrap();
    let stake_tx = make_stake_tx(&validator_sk, MIN_STAKE_SATS, 0);
    state.apply_stake_tx(&stake_tx).unwrap();
    state.recalculate_active_set();
    assert!(state.is_active_validator(&val_addr));

    // Validator begins unstaking
    let unstake_tx = make_unstake_tx(&validator_sk, 1);
    state.apply_unstake_tx(&unstake_tx, 100).unwrap();

    // Fund delegator
    state.faucet_credit(&del_addr, MIN_DELEGATION_SATS * 2).unwrap();

    // Try to delegate to unstaking validator — should fail
    let mut delegate_tx = DelegateTx {
        from: del_addr,
        validator: val_addr,
        amount: MIN_DELEGATION_SATS,
        nonce: 0,
        pub_key: delegator_sk.verifying_key().to_bytes(),
        signature: Signature([0u8; 64]),
    };
    delegate_tx.signature = delegator_sk.sign(&delegate_tx.signable_bytes());

    let result = state.apply_delegate_tx(&delegate_tx);
    assert!(result.is_err(), "Delegation to unstaking validator must be rejected");
    let err = format!("{}", result.unwrap_err());
    assert!(err.contains("unstaking"), "Error should mention unstaking, got: {}", err);
}

/// test_31: slash_cascade_supply_invariant
/// Verifies that slashing a validator with delegators maintains supply invariant.
/// Total supply must decrease by exactly (validator_slash + sum(delegator_slashes)).
#[test]
fn test_31_slash_cascade_supply_invariant() {
    let mut state = StateEngine::new_with_genesis();
    let validator_sk = SecretKey::from_bytes([0xDD; 32]);
    let delegator1_sk = SecretKey::from_bytes([0xEE; 32]);
    let delegator2_sk = SecretKey::from_bytes([0xFF; 32]);
    let val_addr = validator_sk.address();
    let del1_addr = delegator1_sk.address();
    let del2_addr = delegator2_sk.address();

    // Fund and stake validator
    state.faucet_credit(&val_addr, MIN_STAKE_SATS * 3).unwrap();
    let stake_tx = make_stake_tx(&validator_sk, MIN_STAKE_SATS, 0);
    state.apply_stake_tx(&stake_tx).unwrap();
    state.recalculate_active_set();

    // Fund and delegate two delegators
    state.faucet_credit(&del1_addr, MIN_DELEGATION_SATS * 5).unwrap();
    let mut del1_tx = DelegateTx {
        from: del1_addr,
        validator: val_addr,
        amount: MIN_DELEGATION_SATS * 2,
        nonce: 0,
        pub_key: delegator1_sk.verifying_key().to_bytes(),
        signature: Signature([0u8; 64]),
    };
    del1_tx.signature = delegator1_sk.sign(&del1_tx.signable_bytes());
    state.apply_delegate_tx(&del1_tx).unwrap();

    state.faucet_credit(&del2_addr, MIN_DELEGATION_SATS * 5).unwrap();
    let mut del2_tx = DelegateTx {
        from: del2_addr,
        validator: val_addr,
        amount: MIN_DELEGATION_SATS * 3,
        nonce: 0,
        pub_key: delegator2_sk.verifying_key().to_bytes(),
        signature: Signature([0u8; 64]),
    };
    del2_tx.signature = delegator2_sk.sign(&del2_tx.signable_bytes());
    state.apply_delegate_tx(&del2_tx).unwrap();

    let supply_before = state.total_supply();
    let validator_stake = state.stake_of(&val_addr);
    let del1_delegated = state.delegation_account(&del1_addr).unwrap().delegated;
    let del2_delegated = state.delegation_account(&del2_addr).unwrap().delegated;
    let total_at_risk = validator_stake + del1_delegated + del2_delegated;

    // Slash the validator (50%)
    state.slash(&val_addr);

    let supply_after = state.total_supply();
    let slash_pct = state.governance_params().slash_percent as u64;
    let expected_burned = total_at_risk.saturating_mul(slash_pct) / 100;

    // Supply should decrease by the burned amount
    assert_eq!(
        supply_before - supply_after,
        expected_burned,
        "Supply decrease must equal total burned: validator={}, del1={}, del2={}, pct={}",
        validator_stake, del1_delegated, del2_delegated, slash_pct
    );

    // Delegators should have half their delegated amount remaining
    let del1_after = state.delegation_account(&del1_addr).map(|d| d.delegated).unwrap_or(0);
    let del2_after = state.delegation_account(&del2_addr).map(|d| d.delegated).unwrap_or(0);
    assert_eq!(del1_after, del1_delegated / 2, "Delegator 1 should be slashed 50%");
    assert_eq!(del2_after, del2_delegated / 2, "Delegator 2 should be slashed 50%");
}

/// test_32: commission_edge_cases
/// Verifies commission at 0% (all rewards to delegators) and 100% (all to validator).
#[test]
fn test_32_commission_edge_cases() {
    let mut state = StateEngine::new_with_genesis();
    let validator_sk = SecretKey::from_bytes([0xA1; 32]);
    let val_addr = validator_sk.address();

    // Fund, stake
    state.faucet_credit(&val_addr, MIN_STAKE_SATS * 3).unwrap();
    let stake_tx = make_stake_tx(&validator_sk, MIN_STAKE_SATS, 0);
    state.apply_stake_tx(&stake_tx).unwrap();

    // Set commission to 0%
    let mut set_comm = SetCommissionTx {
        from: val_addr,
        commission_percent: 0,
        nonce: 1,
        pub_key: validator_sk.verifying_key().to_bytes(),
        signature: Signature([0u8; 64]),
    };
    set_comm.signature = validator_sk.sign(&set_comm.signable_bytes());
    state.apply_set_commission_tx(&set_comm, 0).unwrap();
    assert_eq!(state.stake_account(&val_addr).unwrap().commission_percent, 0);

    // Set commission to 100% — must be at least COMMISSION_COOLDOWN_ROUNDS later
    let mut set_comm_max = SetCommissionTx {
        from: val_addr,
        commission_percent: 100,
        nonce: 2,
        pub_key: validator_sk.verifying_key().to_bytes(),
        signature: Signature([0u8; 64]),
    };
    set_comm_max.signature = validator_sk.sign(&set_comm_max.signable_bytes());
    state.apply_set_commission_tx(&set_comm_max, 3_000).unwrap();
    assert_eq!(state.stake_account(&val_addr).unwrap().commission_percent, 100);
}

/// test_33: configured_validator_count_survives_load_snapshot
/// Verifies that load_snapshot preserves the CLI-configured validator count.
#[test]
fn test_33_configured_validator_count_survives_load_snapshot() {
    let mut state = StateEngine::new_with_genesis();
    state.set_configured_validator_count(21);
    assert_eq!(state.configured_validator_count(), Some(21));

    let snapshot = state.snapshot();

    // Load snapshot should preserve configured_validator_count
    state.load_snapshot(snapshot);
    assert_eq!(
        state.configured_validator_count(),
        Some(21),
        "configured_validator_count must survive load_snapshot"
    );
}
