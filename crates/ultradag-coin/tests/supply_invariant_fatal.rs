//! Tests that the supply invariant check correctly fires and returns
//! `SupplyInvariantBroken` when state is corrupted.
//!
//! The supply invariant is the most critical safety mechanism in UltraDAG.
//! Any violation means state is corrupt and the node must halt immediately.
//! In production (server.rs), `SupplyInvariantBroken` triggers `process::exit(101)`.
//!
//! These tests verify the detection mechanism at the coin-crate level.
//! The server-level exit behavior is tested separately via subprocess.

use ultradag_coin::*;

fn make_vertex(
    proposer_sk: &SecretKey,
    round: u64,
    height: u64,
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
        transactions: vec![],
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

/// Corrupt total_supply upward → invariant fires (sum < total_supply).
#[test]
fn test_supply_invariant_detects_inflated_total_supply() {
    let mut state = StateEngine::new_with_genesis();
    state.set_configured_validator_count(1);
    let sk = SecretKey::generate();

    // Corrupt: inflate total_supply by 1 sat
    state.total_supply = state.total_supply.saturating_add(1);

    let v = make_vertex(&sk, 1, 0);
    let result = state.apply_finalized_vertices(&[v]);
    assert!(result.is_err(), "Supply invariant must detect corruption");
    let err_msg = format!("{}", result.unwrap_err());
    assert!(
        err_msg.contains("supply invariant broken"),
        "Error must be SupplyInvariantBroken, got: {}", err_msg
    );
}

/// Corrupt total_supply downward → invariant fires (sum > total_supply).
#[test]
fn test_supply_invariant_detects_deflated_total_supply() {
    let mut state = StateEngine::new_with_genesis();
    state.set_configured_validator_count(1);
    let sk = SecretKey::generate();

    // Corrupt: deflate total_supply by 1 sat
    state.total_supply = state.total_supply.saturating_sub(1);

    let v = make_vertex(&sk, 1, 0);
    let result = state.apply_finalized_vertices(&[v]);
    assert!(result.is_err(), "Supply invariant must detect corruption");
    let err_msg = format!("{}", result.unwrap_err());
    assert!(
        err_msg.contains("supply invariant broken"),
        "Error must be SupplyInvariantBroken, got: {}", err_msg
    );
}

/// Verify that uncorrupted state passes the invariant (sanity check).
#[test]
fn test_supply_invariant_passes_on_healthy_state() {
    let mut state = StateEngine::new_with_genesis();
    state.set_configured_validator_count(1);
    let sk = SecretKey::generate();

    let v = make_vertex(&sk, 1, 0);
    let result = state.apply_finalized_vertices(&[v]);
    assert!(result.is_ok(), "Healthy state must pass invariant: {:?}", result);
}

/// Verify the error message includes the diagnostic details
/// (liquid, staked, delegated, treasury, sum, total_supply).
#[test]
fn test_supply_invariant_error_includes_diagnostics() {
    let mut state = StateEngine::new_with_genesis();
    state.set_configured_validator_count(1);
    let sk = SecretKey::generate();

    state.total_supply = state.total_supply.saturating_add(42);

    let v = make_vertex(&sk, 1, 0);
    let result = state.apply_finalized_vertices(&[v]);
    let err_msg = format!("{}", result.unwrap_err());

    // The error should contain the breakdown values for debugging
    assert!(err_msg.contains("liquid="), "Error must include liquid balance");
    assert!(err_msg.contains("staked="), "Error must include staked amount");
    assert!(err_msg.contains("total_supply="), "Error must include total_supply");
}

/// Test that is_fatal() returns true for SupplyInvariantBroken errors.
/// server.rs uses is_fatal() to decide whether to call process::exit(101).
#[test]
fn test_supply_invariant_is_fatal() {
    let mut state = StateEngine::new_with_genesis();
    state.set_configured_validator_count(1);
    let sk = SecretKey::generate();

    state.total_supply = state.total_supply.saturating_add(1);

    let v = make_vertex(&sk, 1, 0);
    let result = state.apply_finalized_vertices(&[v]);
    let err = result.unwrap_err();

    // server.rs uses e.is_fatal() to decide whether to halt.
    assert!(err.is_fatal(), "SupplyInvariantBroken must be fatal");
}
