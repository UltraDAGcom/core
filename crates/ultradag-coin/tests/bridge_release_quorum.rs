//! Regression tests for GHSA-6gwf-frh8-ppw7.
//!
//! Before the fix, a single validator (n=1) could drain the bridge reserve
//! because the dynamic quorum `ceil(2n/3)` collapses to 1 when n=1. These
//! tests lock in the two floors that close the exploit:
//!
//!   - `MIN_BRIDGE_VALIDATORS`: refuse releases when the active set is too small.
//!   - `MIN_BRIDGE_QUORUM`: floor on the number of distinct validator votes
//!     required, independent of active-set size.

use ultradag_coin::{
    StateEngine, SecretKey,
    tx::{BridgeDepositTx, StakeTx, MIN_STAKE_SATS},
    tx::bridge::BridgeReleaseTx,
    address::Signature,
    constants::{COIN, MIN_BRIDGE_QUORUM, MIN_BRIDGE_VALIDATORS, SUPPORTED_BRIDGE_CHAIN_IDS},
};

fn signed_stake(sk: &SecretKey, amount: u64, nonce: u64) -> StakeTx {
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

fn signed_bridge_deposit(sk: &SecretKey, amount: u64, nonce: u64, fee: u64) -> BridgeDepositTx {
    let mut tx = BridgeDepositTx {
        from: sk.address(),
        recipient: [0x11u8; 20],
        amount,
        destination_chain_id: SUPPORTED_BRIDGE_CHAIN_IDS[0],
        nonce,
        fee,
        pub_key: sk.verifying_key().to_bytes(),
        signature: Signature([0u8; 64]),
    };
    tx.signature = sk.sign(&tx.signable_bytes());
    tx
}

fn signed_bridge_release(
    sk: &SecretKey,
    recipient: ultradag_coin::address::Address,
    amount: u64,
    deposit_nonce: u64,
    nonce: u64,
) -> BridgeReleaseTx {
    let mut tx = BridgeReleaseTx {
        from: sk.address(),
        recipient,
        amount,
        source_chain_id: SUPPORTED_BRIDGE_CHAIN_IDS[0],
        deposit_nonce,
        nonce,
        pub_key: sk.verifying_key().to_bytes(),
        signature: Signature([0u8; 64]),
    };
    tx.signature = sk.sign(&tx.signable_bytes());
    tx
}

/// Regression: GHSA-6gwf-frh8-ppw7.
/// A sole active validator must NOT be able to release funds from the bridge reserve.
#[test]
fn single_validator_cannot_drain_bridge() {
    let mut state = StateEngine::new_with_genesis();

    let attacker = SecretKey::generate();
    let victim = SecretKey::generate();

    state.faucet_credit(&attacker.address(), MIN_STAKE_SATS).unwrap();
    state.faucet_credit(&victim.address(), 5 * COIN).unwrap();

    // Attacker becomes sole active validator (n = 1).
    let stake_tx = signed_stake(&attacker, MIN_STAKE_SATS, 0);
    state.apply_stake_tx(&stake_tx).unwrap();
    state.recalculate_active_set();
    assert!(state.is_active_validator(&attacker.address()));
    assert_eq!(state.active_validators().len(), 1);

    // Legit deposit seeds the bridge reserve.
    let deposit_amount = 3 * COIN;
    let deposit_fee = 10_000;
    let dep = signed_bridge_deposit(&victim, deposit_amount, 0, deposit_fee);
    state.apply_bridge_lock_tx(&dep, None, None).unwrap();
    assert_eq!(state.bridge_reserve(), deposit_amount);

    // Attempted drain: fabricated deposit_nonce, self-recipient.
    let attacker_addr = attacker.address();
    let reserve_before = state.bridge_reserve();
    let attacker_before = state.balance(&attacker_addr);
    let rel = signed_bridge_release(&attacker, attacker_addr, deposit_amount, 999_999, 1);
    let result = state.apply_bridge_release_tx(&rel);

    // Must be rejected by the MIN_BRIDGE_VALIDATORS gate.
    assert!(result.is_err(), "drain attempt with n=1 must be rejected, got: {:?}", result);
    let err_msg = format!("{:?}", result.err().unwrap());
    assert!(
        err_msg.contains("active validators"),
        "expected rejection to cite active-validator floor, got: {err_msg}"
    );

    // No state mutation on the reserve or attacker balance.
    assert_eq!(state.bridge_reserve(), reserve_before);
    assert_eq!(state.balance(&attacker_addr), attacker_before);
}

/// With exactly `MIN_BRIDGE_VALIDATORS - 1` active validators, releases are still blocked.
#[test]
fn releases_blocked_just_below_min_bridge_validators() {
    let mut state = StateEngine::new_with_genesis();

    let validators: Vec<SecretKey> = (0..MIN_BRIDGE_VALIDATORS - 1)
        .map(|_| SecretKey::generate())
        .collect();

    for sk in &validators {
        state.faucet_credit(&sk.address(), MIN_STAKE_SATS).unwrap();
        let stake_tx = signed_stake(sk, MIN_STAKE_SATS, 0);
        state.apply_stake_tx(&stake_tx).unwrap();
    }
    state.recalculate_active_set();
    assert_eq!(state.active_validators().len(), MIN_BRIDGE_VALIDATORS - 1);

    // Seed the reserve.
    let donor = SecretKey::generate();
    state.faucet_credit(&donor.address(), 10 * COIN).unwrap();
    let dep = signed_bridge_deposit(&donor, 5 * COIN, 0, 10_000);
    state.apply_bridge_lock_tx(&dep, None, None).unwrap();

    let v0 = &validators[0];
    let rel = signed_bridge_release(v0, v0.address(), 1 * COIN, 42, 1);
    let result = state.apply_bridge_release_tx(&rel);
    assert!(result.is_err(), "release must be blocked below MIN_BRIDGE_VALIDATORS");
}

/// Normal path: with a healthy set, a single vote is not enough (MIN_BRIDGE_QUORUM floor)
/// but quorum is reachable once enough independent validators attest.
#[test]
fn healthy_set_requires_min_quorum_votes() {
    let mut state = StateEngine::new_with_genesis();

    // Use MIN_BRIDGE_VALIDATORS validators — small enough that ceil(2n/3) <= MIN_BRIDGE_QUORUM,
    // so the MIN_BRIDGE_QUORUM floor is the effective threshold.
    let n = MIN_BRIDGE_VALIDATORS;
    let validators: Vec<SecretKey> = (0..n).map(|_| SecretKey::generate()).collect();
    for sk in &validators {
        state.faucet_credit(&sk.address(), MIN_STAKE_SATS).unwrap();
        let stake_tx = signed_stake(sk, MIN_STAKE_SATS, 0);
        state.apply_stake_tx(&stake_tx).unwrap();
    }
    state.recalculate_active_set();
    assert_eq!(state.active_validators().len(), n);

    // Seed the reserve.
    let donor = SecretKey::generate();
    state.faucet_credit(&donor.address(), 10 * COIN).unwrap();
    let dep = signed_bridge_deposit(&donor, 5 * COIN, 0, 10_000);
    state.apply_bridge_lock_tx(&dep, None, None).unwrap();

    let recipient = SecretKey::generate().address();
    let release_amount = 1 * COIN;
    let deposit_nonce = 777;

    // First (MIN_BRIDGE_QUORUM - 1) validators vote: release must NOT execute yet.
    // Each validator's account nonce is 1 after their StakeTx, so bridge vote uses nonce 1.
    let reserve_before = state.bridge_reserve();
    for sk in validators.iter().take(MIN_BRIDGE_QUORUM - 1) {
        let rel = signed_bridge_release(sk, recipient, release_amount, deposit_nonce, 1);
        state.apply_bridge_release_tx(&rel).unwrap();
    }
    assert_eq!(
        state.bridge_reserve(),
        reserve_before,
        "reserve must be untouched before MIN_BRIDGE_QUORUM votes"
    );

    // The MIN_BRIDGE_QUORUM-th vote crosses the floor: release executes.
    let crossing_voter = &validators[MIN_BRIDGE_QUORUM - 1];
    let rel_cross = signed_bridge_release(crossing_voter, recipient, release_amount, deposit_nonce, 1);
    state.apply_bridge_release_tx(&rel_cross).unwrap();

    assert_eq!(
        state.bridge_reserve(),
        reserve_before - release_amount,
        "reserve must decrement once MIN_BRIDGE_QUORUM is reached"
    );
    assert_eq!(state.balance(&recipient), release_amount);
}
