/// Epoch transition integration tests.
/// Verifies that the active validator set transitions correctly at epoch boundaries
/// and that the FinalityTracker is synchronized.

use ultradag_coin::*;
use ultradag_coin::consensus::epoch::sync_epoch_validators;

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

fn make_vertex(sk: &SecretKey, round: u64, height: u64, _reward: u64) -> DagVertex {
    let proposer = sk.address();
    let block = Block {
        header: BlockHeader {
            version: 1,
            height,
            timestamp: 1_000_000 + round as i64,
            prev_hash: [0u8; 32],
            merkle_root: [0u8; 32],
        },
        coinbase: CoinbaseTx {
            to: proposer,
            amount: 0,
            height,
        },
        transactions: vec![],
    };
    let mut v = DagVertex::new(
        block,
        vec![],
        round,
        proposer,
        sk.verifying_key().to_bytes(),
        Signature([0u8; 64]),
    );
    v.signature = sk.sign(&v.signable_bytes());
    v
}

#[test]
fn epoch_boundary_recalculates_active_set() {
    let mut state = StateEngine::new_with_genesis();

    let sk1 = SecretKey::generate();
    let sk2 = SecretKey::generate();

    // Fund and stake
    state.faucet_credit(&sk1.address(), MIN_STAKE_SATS).unwrap();
    state.faucet_credit(&sk2.address(), MIN_STAKE_SATS).unwrap();
    state.apply_stake_tx(&make_stake_tx(&sk1, MIN_STAKE_SATS, 0)).unwrap();
    state.apply_stake_tx(&make_stake_tx(&sk2, MIN_STAKE_SATS, 0)).unwrap();

    assert!(state.active_validators().is_empty(), "No active set before first recalculation");

    // Manually trigger recalculation (normally happens at epoch boundary)
    state.recalculate_active_set();

    assert_eq!(state.active_validators().len(), 2);
    assert!(state.is_active_validator(&sk1.address()));
    assert!(state.is_active_validator(&sk2.address()));
}

#[test]
fn sync_restricts_finality_to_active_set() {
    let mut state = StateEngine::new_with_genesis();
    let mut ft = FinalityTracker::new(2);

    let sk1 = SecretKey::generate();
    let sk2 = SecretKey::generate();
    let sk3 = SecretKey::generate();

    // Register all 3 in FinalityTracker (permissionless)
    ft.register_validator(sk1.address());
    ft.register_validator(sk2.address());
    ft.register_validator(sk3.address());
    assert_eq!(ft.validator_count(), 3);

    // Only sk1 and sk2 stake
    state.faucet_credit(&sk1.address(), MIN_STAKE_SATS).unwrap();
    state.faucet_credit(&sk2.address(), MIN_STAKE_SATS).unwrap();
    state.apply_stake_tx(&make_stake_tx(&sk1, MIN_STAKE_SATS, 0)).unwrap();
    state.apply_stake_tx(&make_stake_tx(&sk2, MIN_STAKE_SATS, 0)).unwrap();
    state.recalculate_active_set();

    // Sync epoch validators
    sync_epoch_validators(&mut ft, &state);

    // FinalityTracker should now only have 2 validators
    assert_eq!(ft.validator_count(), 2);
    assert_eq!(ft.validator_set().configured_validators(), Some(2));

    // sk3 should be rejected if trying to register again
    assert!(!ft.validator_set().is_allowed(&sk3.address()));
}

#[test]
fn no_sync_when_staking_inactive() {
    let state = StateEngine::new();
    let mut ft = FinalityTracker::new(2);

    let sk1 = SecretKey::generate();
    let sk2 = SecretKey::generate();
    ft.register_validator(sk1.address());
    ft.register_validator(sk2.address());

    // No stakers — sync should be a no-op
    sync_epoch_validators(&mut ft, &state);

    assert_eq!(ft.validator_count(), 2);
    assert!(ft.validator_set().configured_validators().is_none());
}

#[test]
fn epoch_just_changed_detects_boundary() {
    let mut state = StateEngine::new_with_genesis();
    let sk = SecretKey::generate();
    // new_with_genesis() bootstraps 1 council member (dev address) at 10% emission,
    // so the validator receives 90% of block_reward.
    let validator_reward = |h: u64| block_reward(h) * 90 / 100;

    // Apply vertex at round 0
    let v = make_vertex(&sk, 0, 0, validator_reward(0));
    state.apply_vertex(&v).unwrap();
    assert!(!state.epoch_just_changed(None));

    // Simulate approaching epoch boundary
    // Set last_finalized_round to EPOCH_LENGTH_ROUNDS - 1
    let prev_round = Some(EPOCH_LENGTH_ROUNDS - 2);
    let v2 = make_vertex(&sk, EPOCH_LENGTH_ROUNDS - 1, EPOCH_LENGTH_ROUNDS - 1, validator_reward(EPOCH_LENGTH_ROUNDS - 1));
    state.apply_vertex(&v2).unwrap();
    assert!(!state.epoch_just_changed(prev_round));

    // Cross the boundary
    let prev_round2 = state.last_finalized_round();
    let v3 = make_vertex(&sk, EPOCH_LENGTH_ROUNDS, EPOCH_LENGTH_ROUNDS, validator_reward(EPOCH_LENGTH_ROUNDS));
    state.apply_vertex(&v3).unwrap();
    assert!(state.epoch_just_changed(prev_round2));
    assert_eq!(state.current_epoch(), 1);
}

#[test]
fn max_active_validators_caps_set() {
    let mut state = StateEngine::new_with_genesis();

    // Stake one more than MAX_ACTIVE_VALIDATORS to exercise the cap.
    let count = MAX_ACTIVE_VALIDATORS + 10;
    let sks: Vec<SecretKey> = (0..count).map(|_| SecretKey::generate()).collect();
    for (i, sk) in sks.iter().enumerate() {
        let amount = MIN_STAKE_SATS + (i as u64 * COIN); // varying stakes
        state.faucet_credit(&sk.address(), amount).unwrap();
        state.apply_stake_tx(&make_stake_tx(sk, amount, 0)).unwrap();
    }

    state.recalculate_active_set();

    assert_eq!(
        state.active_validators().len(),
        MAX_ACTIVE_VALIDATORS,
        "Active set should be capped at MAX_ACTIVE_VALIDATORS"
    );

    // Top staker (highest additional stake) should be in the set
    assert!(state.is_active_validator(&sks[count - 1].address()));
}
