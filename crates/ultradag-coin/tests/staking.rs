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

fn make_vertex(
    proposer_sk: &SecretKey,
    round: u64,
    height: u64,
    txs: Vec<Transaction>,
    reward: u64,
) -> DagVertex {
    let proposer = proposer_sk.address();
    let total_fees: u64 = txs.iter().map(|tx| tx.fee()).sum();
    let coinbase = CoinbaseTx {
        to: proposer,
        amount: reward + total_fees,
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
    let reward = block_reward(0);
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
        let total_round_reward = block_reward(round);
        let total_stake = state.total_staked();

        // All 4 validators produce in each round
        let mut round_vertices = Vec::new();
        for (i, sk) in sks.iter().enumerate() {
            let own_stake = state.stake_of(&sk.address());
            let validator_reward = ((total_round_reward as u128)
                * own_stake as u128 / total_stake as u128) as u64;
            let v = make_vertex(sk, round, round * 4 + i as u64, vec![], validator_reward);
            round_vertices.push(v);
        }

        // Sum of all validator rewards in this round should be <= block_reward
        let total_emitted: u64 = round_vertices.iter()
            .map(|v| v.block.coinbase.amount)
            .sum();
        assert!(
            total_emitted <= total_round_reward,
            "Round {}: emitted {} > reward {}", round, total_emitted, total_round_reward
        );

        state.apply_finalized_vertices(&round_vertices).unwrap();
    }

    // Total supply should be genesis amounts + sum of all rewards
    // (Each round emits slightly less than block_reward due to integer division rounding)
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
        let vertex = make_vertex(proposer, round, round, vec![], block_reward(round));
        state.apply_vertex(&vertex).unwrap();
        round += 1;
        
        // Safety: don't loop forever
        if round > 100_000 {
            break;
        }
    }

    // Now apply one more vertex - should be capped at MAX_SUPPLY
    let sk = &validators[0];
    let vertex = make_vertex(sk, round, round, vec![], block_reward(round));
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
    // With 3 equal stakers, proportional reward = block_reward / 3
    let total_stake = state.total_staked();
    let own_stake = state.stake_of(&sks[0].address());
    let reward = ((block_reward(0) as u128) * own_stake as u128 / total_stake as u128) as u64;
    let v = make_vertex(&sks[0], 0, 0, vec![], reward);
    state.apply_vertex(&v).unwrap();

    let active = state.active_validators();
    assert_eq!(active.len(), 3, "All 3 stakers should be in active set");
    for sk in &sks {
        assert!(active.contains(&sk.address()));
    }

    // Produce a vertex at the next epoch boundary
    // Engine uses vertex.round as height for block_reward, so use epoch2_round
    let epoch2_round = EPOCH_LENGTH_ROUNDS;
    let own_stake2 = state.stake_of(&sks[1].address());
    let total_stake2 = state.total_staked();
    let reward2 = ((block_reward(epoch2_round) as u128) * own_stake2 as u128 / total_stake2 as u128) as u64;
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
    let total_round_reward = block_reward(0);
    let base_reward = ((total_round_reward as u128) * MIN_STAKE_SATS as u128
        / total_stake as u128) as u64;
    let observer_reward = base_reward * OBSERVER_REWARD_PERCENT / 100;

    // Observer produces vertex — gets reduced reward
    let v_obs = make_vertex(observer_sk, 0, 0, vec![], observer_reward);
    let balance_before_obs = state.balance(&observer_sk.address());
    state.apply_vertex(&v_obs).unwrap();
    assert_eq!(
        state.balance(&observer_sk.address()),
        balance_before_obs + observer_reward,
        "Observer should earn reduced reward"
    );

    // Active validator produces vertex — gets full reward
    let v_act = make_vertex(active_sk, 1, 1, vec![], base_reward);
    let balance_before_act = state.balance(&active_sk.address());
    state.apply_vertex(&v_act).unwrap();
    assert_eq!(
        state.balance(&active_sk.address()),
        balance_before_act + base_reward,
        "Active validator should earn full reward"
    );

    // Observer reward should be exactly OBSERVER_REWARD_PERCENT% of active reward
    assert_eq!(observer_reward, base_reward * OBSERVER_REWARD_PERCENT / 100);
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
    let reward = ((block_reward(0) as u128) * own_stake as u128 / total_stake as u128) as u64;
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
    let reward2 = ((block_reward(1) as u128) * own_stake2 as u128 / total_stake2 as u128) as u64;
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
    let state_path = tmp.join("state.json");

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
    let r0 = ((block_reward(0) as u128) * own0 as u128 / total_stake as u128) as u64;
    let v0 = make_vertex(&sks[0], 0, 0, vec![], r0);
    state.apply_vertex(&v0).unwrap();
    assert_eq!(state.current_epoch(), 0);

    // Jump to round 210,001 (epoch 1) — past first halving at 210,000
    let epoch1_round = EPOCH_LENGTH_ROUNDS + 1;
    let own1 = state.stake_of(&sks[1].address());
    let ts1 = state.total_staked();
    let r1 = ((block_reward(epoch1_round) as u128) * own1 as u128 / ts1 as u128) as u64;
    let v1 = make_vertex(&sks[1], epoch1_round, 1, vec![], r1);
    state.apply_vertex(&v1).unwrap();
    assert_eq!(state.current_epoch(), 1);

    // Save state
    state.save(&state_path).unwrap();

    // Tamper: load the raw snapshot and set current_epoch back to 0
    let raw = std::fs::read_to_string(&state_path).unwrap();
    let tampered = raw.replacen("\"current_epoch\":1", "\"current_epoch\":0", 1);
    std::fs::write(&state_path, tampered).unwrap();

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

    let total_stake = state.total_staked();
    let total_round_reward = block_reward(0);
    let base_reward = ((total_round_reward as u128) * MIN_STAKE_SATS as u128
        / total_stake as u128) as u64;
    let observer_reward = base_reward * OBSERVER_REWARD_PERCENT / 100;

    let supply_before = state.total_supply();

    // Observer produces vertex
    let v_obs = make_vertex(observer_sk, 0, 0, vec![], observer_reward);
    state.apply_vertex(&v_obs).unwrap();

    // One active validator produces vertex
    let v_act = make_vertex(active_sks[0], 1, 1, vec![], base_reward);
    state.apply_vertex(&v_act).unwrap();

    let supply_after = state.total_supply();
    let supply_increase = supply_after - supply_before;

    // Supply increased by exactly observer_reward + active_reward
    assert_eq!(supply_increase, observer_reward + base_reward,
        "Supply should increase by exactly the sum of rewards");

    // Total emitted should be <= block_reward for each height
    assert!(observer_reward <= block_reward(0));
    assert!(base_reward <= block_reward(1));

    // Supply invariant: liquid + staked == total_supply
    // (This is also checked in debug builds inside apply_vertex, but verify manually)
    let liquid: u64 = (0..count).map(|i| state.balance(&sks[i].address())).sum();
    let staked = state.total_staked();
    let dev_balance = state.balance(&ultradag_coin::dev_address());
    let faucet_balance = state.balance(&ultradag_coin::faucet_keypair().address());
    assert_eq!(liquid + staked + dev_balance + faucet_balance, state.total_supply(),
        "Supply invariant: liquid({}) + staked({}) + dev({}) + faucet({}) != total_supply({})",
        liquid, staked, dev_balance, faucet_balance, state.total_supply());
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
    let sk = SecretKey::generate();
    let reward = block_reward(0);
    let v = make_vertex(&sk, 0, 0, vec![], reward);
    state.apply_vertex(&v).unwrap();

    // Supply invariant holds
    assert_eq!(
        state.balance(&sk.address())
            + state.balance(&faucet_keypair().address())
            + state.balance(&dev_address()),
        state.total_supply(),
        "Supply invariant must hold at genesis"
    );
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
