//! Runtime verification that the genesis state this binary computes at
//! startup matches the hardcoded `GENESIS_CHECKPOINT_HASH` constant in
//! `constants.rs`. This check is defense-in-depth against:
//!
//!   1. A genesis-affecting constant being changed without recomputing the
//!      hash (stale constant).
//!   2. An environment variable silently overriding a hardcoded protocol
//!      address at runtime (e.g., a leftover `ULTRADAG_DEV_ADDRESS` Fly
//!      secret — this is exactly what bit us on 2026-04-10).
//!
//! See `StateEngine::verify_genesis_matches_constant()`.

use ultradag_coin::StateEngine;

/// On testnet, `GENESIS_CHECKPOINT_HASH` is still the `[0u8; 32]` placeholder,
/// so the runtime check must be a no-op. `new_with_genesis()` should return
/// a valid engine without panicking.
#[cfg(not(feature = "mainnet"))]
#[test]
fn new_with_genesis_succeeds_on_testnet_placeholder() {
    assert_eq!(
        ultradag_coin::constants::GENESIS_CHECKPOINT_HASH,
        [0u8; 32],
        "testnet GENESIS_CHECKPOINT_HASH is no longer [0u8; 32] — update \
         this test to cover the non-placeholder branch"
    );
    let engine = StateEngine::new_with_genesis();
    // If the check had panicked we wouldn't get here.
    assert!(engine.total_supply() > 0);
}

/// `new_with_genesis_no_check()` must never panic regardless of the constant
/// state — it's the workflow tool used to compute a new hash when the
/// constant is intentionally being updated.
#[test]
fn new_with_genesis_no_check_never_panics() {
    let engine = StateEngine::new_with_genesis_no_check();
    assert!(engine.total_supply() > 0);
    // And it should return a valid usable engine.
    let snapshot = engine.snapshot();
    let _state_root =
        ultradag_coin::consensus::checkpoint::compute_state_root(&snapshot);
}

/// Calling the verifier explicitly on a testnet placeholder engine must not
/// panic (the verifier short-circuits when the constant is zero).
#[test]
fn verify_genesis_matches_constant_is_noop_on_placeholder() {
    if ultradag_coin::constants::GENESIS_CHECKPOINT_HASH != [0u8; 32] {
        // Non-testnet build — skip. Covered separately below.
        return;
    }
    let engine = StateEngine::new_with_genesis_no_check();
    engine.verify_genesis_matches_constant(); // must not panic
}

/// When the constant is non-zero (e.g., mainnet) the verifier must accept
/// an engine that was constructed from the same code it was computed from.
/// This is the happy path: `new_with_genesis()` -> verifier -> ok.
#[cfg(feature = "mainnet")]
#[test]
fn verify_genesis_matches_constant_accepts_fresh_mainnet_genesis() {
    assert_ne!(
        ultradag_coin::constants::GENESIS_CHECKPOINT_HASH,
        [0u8; 32],
        "mainnet GENESIS_CHECKPOINT_HASH must not be [0u8; 32]"
    );
    let engine = StateEngine::new_with_genesis_no_check();
    engine.verify_genesis_matches_constant(); // must not panic
}

/// Regression test: the fix for the 2026-04-10 Fly-secret override bug.
///
/// Simulates an engine whose genesis state differs from the one the constant
/// was computed against (e.g., because a runtime env var resolved a different
/// founder address). The verifier must catch this and panic.
///
/// We build an engine via `_no_check`, tamper with the total_supply so the
/// computed genesis hash differs from the constant, and assert the verifier
/// panics. (We can't override dev_address() mid-process because of the
/// OnceLock cache, but tampering with state after construction produces the
/// same class of divergence — the verifier doesn't care *why* the hash is
/// wrong, only that it is.)
#[cfg(feature = "mainnet")]
#[test]
#[should_panic(expected = "GENESIS_CHECKPOINT_HASH mismatch")]
fn verify_genesis_panics_on_state_drift() {
    let mut engine = StateEngine::new_with_genesis_no_check();
    // Tamper: bump total_supply by 1 sat. This alone changes the checkpoint
    // hash because total_supply is part of the Checkpoint struct hashed by
    // compute_checkpoint_hash. Mimics any class of silent divergence.
    engine.total_supply = engine.total_supply.saturating_add(1);
    engine.verify_genesis_matches_constant();
}
