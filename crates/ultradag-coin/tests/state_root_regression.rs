//! Regression tests for canonical state root computation.
//!
//! These tests build state from known fixtures and assert the EXACT hash output.
//! If `compute_state_root` changes in any way (field order, encoding, version prefix),
//! these tests will fail — which is the point. Any change to the state root algorithm
//! is a consensus-breaking change that requires GENESIS_CHECKPOINT_HASH recomputation
//! and a clean network restart.

use ultradag_coin::address::Address;
use ultradag_coin::consensus::checkpoint::compute_state_root;
use ultradag_coin::governance::GovernanceParams;
use ultradag_coin::state::engine::{AccountState, DelegationAccount, StakeAccount};
use ultradag_coin::state::persistence::StateSnapshot;

/// Build a minimal but non-trivial state fixture with known values.
/// Every field is set to a deterministic value so the hash is reproducible.
fn known_fixture() -> StateSnapshot {
    let addr_a = Address([0x01; 20]);
    let addr_b = Address([0x02; 20]);
    let addr_c = Address([0x03; 20]);

    StateSnapshot {
        accounts: vec![
            (addr_a, AccountState { balance: 1_000_000_000, nonce: 5 }),
            (addr_b, AccountState { balance: 500_000_000, nonce: 2 }),
        ],
        stake_accounts: vec![
            (addr_a, StakeAccount {
                staked: 100_000_000_000,
                unlock_at_round: None,
                commission_percent: 10,
                commission_last_changed: None,
                locked_stake: 0,
            }),
        ],
        active_validator_set: vec![addr_a],
        current_epoch: 1,
        total_supply: 2_100_000_000_000_000,
        last_finalized_round: Some(500),
        proposals: vec![],
        votes: vec![],
        next_proposal_id: 0,
        governance_params: GovernanceParams::default(),
        council_members: vec![
            (addr_c, ultradag_coin::governance::CouncilSeatCategory::Operations),
        ],
        bridge_reserve: 0,
        treasury_balance: 210_000_000_000_000,
        delegation_accounts: vec![
            (addr_b, DelegationAccount {
                delegated: 50_000_000_000,
                validator: addr_a,
                unlock_at_round: None,
            }),
        ],
        configured_validator_count: None,
        bridge_attestations: vec![],
        bridge_signatures: vec![],
        bridge_nonce: 0,
        bridge_contract_address: [0u8; 20],
        used_release_nonces: vec![],
        bridge_release_votes: vec![],
        bridge_release_params: None,
        last_proposal_round: vec![],
        bridge_release_first_vote_round: None,
        bridge_release_disagree_count: None,
        slashed_events: vec![],
    }
}

/// Regression anchor: the exact hash of the known fixture.
/// If this test fails, `compute_state_root` has changed — which is a consensus-breaking
/// change requiring GENESIS_CHECKPOINT_HASH recomputation and clean restart.
#[test]
fn state_root_regression_known_fixture() {
    let snapshot = known_fixture();
    let root = compute_state_root(&snapshot);

    // This is the canonical hash of the known fixture.
    // To update: run this test, copy the printed hash, and replace the assertion.
    // WARNING: Updating this value means every node must be restarted with the new code.
    // Updated 2026-03-26 after adding bridge_release_first_vote_round, bridge_release_disagree_count,
    // slashed_events, and bridge_release_params fields to StateSnapshot
    let expected: [u8; 32] = [
        0xc5, 0xeb, 0xc4, 0x8c, 0xb8, 0xb1, 0xff, 0xc9,
        0x90, 0x16, 0xef, 0xd3, 0x21, 0xbf, 0x6c, 0xaf,
        0x89, 0xc8, 0x6b, 0x09, 0x31, 0x07, 0x9e, 0x97,
        0x45, 0xfd, 0x84, 0xde, 0xd4, 0xa9, 0xd8, 0x49,
    ];

    if expected == [0x00; 32] {
        // First run: print the hash so it can be checked in
        let hex: String = root.iter().map(|b| format!("0x{b:02x}")).collect::<Vec<_>>().join(", ");
        panic!(
            "STATE ROOT REGRESSION ANCHOR NOT SET.\n\
             Computed hash: [{}]\n\
             Copy this into the expected array in this test.",
            hex
        );
    }

    assert_eq!(
        root, expected,
        "State root regression failed! compute_state_root has changed. \
         This is a consensus-breaking change."
    );
}

/// Verify that genesis state root is deterministic across invocations.
#[test]
fn state_root_genesis_deterministic() {
    let engine1 = ultradag_coin::state::engine::StateEngine::new_with_genesis();
    let engine2 = ultradag_coin::state::engine::StateEngine::new_with_genesis();
    let root1 = compute_state_root(&engine1.snapshot());
    let root2 = compute_state_root(&engine2.snapshot());
    assert_eq!(root1, root2, "Genesis state root must be deterministic");
}

/// Verify that different states produce different roots (no collision).
#[test]
fn state_root_different_states_differ() {
    let snapshot1 = known_fixture();
    let mut snapshot2 = known_fixture();
    snapshot2.total_supply += 1; // Tiny change

    let root1 = compute_state_root(&snapshot1);
    let root2 = compute_state_root(&snapshot2);
    assert_ne!(root1, root2, "Different total_supply must produce different roots");

    // Also check account balance change
    let mut snapshot3 = known_fixture();
    snapshot3.accounts[0].1.balance += 1;
    let root3 = compute_state_root(&snapshot3);
    assert_ne!(root1, root3, "Different balance must produce different root");
}

/// Verify field ordering matters: swapping two accounts produces different root.
#[test]
fn state_root_order_sensitive() {
    let mut snapshot1 = known_fixture();
    let mut snapshot2 = known_fixture();
    // Reverse account order
    snapshot2.accounts.reverse();

    let root1 = compute_state_root(&snapshot1);
    let root2 = compute_state_root(&snapshot2);
    assert_ne!(root1, root2, "Account ordering must affect state root");
}

/// Verify empty state produces a valid (non-zero) hash.
#[test]
fn state_root_empty_state() {
    let snapshot = StateSnapshot {
        accounts: vec![],
        stake_accounts: vec![],
        active_validator_set: vec![],
        current_epoch: 0,
        total_supply: 0,
        last_finalized_round: None,
        proposals: vec![],
        votes: vec![],
        next_proposal_id: 0,
        governance_params: GovernanceParams::default(),
        council_members: vec![],
        bridge_reserve: 0,
        treasury_balance: 0,
        delegation_accounts: vec![],
        configured_validator_count: None,
        bridge_attestations: vec![],
        bridge_signatures: vec![],
        bridge_nonce: 0,
        bridge_contract_address: [0u8; 20],
        used_release_nonces: vec![],
        bridge_release_votes: vec![],
        bridge_release_params: None,
        last_proposal_round: vec![],
        bridge_release_first_vote_round: None,
        bridge_release_disagree_count: None,
        slashed_events: vec![],
    };
    let root = compute_state_root(&snapshot);
    assert_ne!(root, [0u8; 32], "Empty state root should not be all zeros");
}

/// Verify that Optional fields (None vs Some) produce different roots.
#[test]
fn state_root_option_discrimination() {
    let mut snapshot1 = known_fixture();
    let mut snapshot2 = known_fixture();
    snapshot1.last_finalized_round = None;
    snapshot2.last_finalized_round = Some(0);

    let root1 = compute_state_root(&snapshot1);
    let root2 = compute_state_root(&snapshot2);
    assert_ne!(root1, root2, "None vs Some(0) must produce different roots");
}
