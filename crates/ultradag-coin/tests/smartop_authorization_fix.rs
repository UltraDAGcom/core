//! Regression tests for the SmartOp authorization-before-mutation fix.
//!
//! These tests verify that:
//! 1. Unauthorized SmartOp transactions do NOT mutate state (no fee burn, no nonce advance)
//! 2. Failed SmartOp transactions do NOT cause double nonce increment
//! 3. Supply invariant is preserved after rejected SmartOp transactions
//!
//! Related vulnerability: Non-council attacker could submit Vote SmartOp that passes
//! signature/nonce/balance checks but fails authorization after state mutation,
//! causing supply accounting mismatch and fatal node halt.

use p256::ecdsa::{SigningKey, signature::Signer};
use ultradag_coin::address::{Address, SecretKey};
use ultradag_coin::consensus::vertex::DagVertex;
use ultradag_coin::state::StateEngine;
use ultradag_coin::tx::smart_account::{AuthorizedKey, KeyType, SmartOpTx, SmartOpType};
use ultradag_coin::tx::Transaction;
use ultradag_coin::tx::CoinbaseTx;
use ultradag_coin::block::{Block, BlockHeader};
use ultradag_coin::constants::MIN_FEE_SATS;

/// Helper: derive SmartAccount address from P256 public key (same as production).
fn derive_smart_account_address_from_p256(pubkey: &[u8]) -> Address {
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"smart_account_p256");
    hasher.update(pubkey);
    let hash = hasher.finalize();
    let mut addr = [0u8; 20];
    addr.copy_from_slice(&hash.as_bytes()[..20]);
    Address(addr)
}

/// Helper: create a DagVertex with the given transactions.
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
        ultradag_coin::Signature([0u8; 64]),
    );
    vertex.signature = proposer_sk.sign(&vertex.signable_bytes());
    vertex
}

/// Test: Non-council member Vote SmartOp should be rejected BEFORE any state mutation.
/// This is the primary regression test for the authorization-before-mutation fix.
#[test]
fn test_non_council_vote_rejected_before_mutation() {
    let mut state = StateEngine::new_with_genesis();
    state.set_configured_validator_count(1);

    // Create an attacker with a fresh P256 key pair.
    let p256_sk = SigningKey::random(&mut rand::thread_rng());
    let p256_pubkey = p256_sk.verifying_key().to_encoded_point(true).as_bytes().to_vec();
    let attacker = derive_smart_account_address_from_p256(&p256_pubkey);
    let key_id = AuthorizedKey::compute_key_id(KeyType::P256, &p256_pubkey);

    // Fund attacker so balance checks pass.
    state.faucet_credit(&attacker, 1_000_000).unwrap();

    let balance_before = state.balance(&attacker);
    let nonce_before = state.nonce(&attacker);

    // Create a Vote SmartOp (requires council membership).
    let mut op = SmartOpTx {
        from: attacker,
        operation: SmartOpType::Vote {
            proposal_id: 1,
            approve: true,
        },
        fee: MIN_FEE_SATS,
        nonce: 0,
        signing_key_id: key_id,
        signature: vec![],
        webauthn: None,
        p256_pubkey: Some(p256_pubkey.clone()),
    };
    let sig: p256::ecdsa::Signature = p256_sk.sign(&op.signable_bytes());
    op.signature = sig.to_bytes().to_vec();

    // Apply the SmartOp directly (bypasses DAG validation).
    let result = state.apply_smart_op_tx(&op, 1);

    // Should fail with authorization error.
    assert!(result.is_err(), "Vote should fail for non-council member");
    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("only council members"),
        "Expected council membership error, got: {}",
        err_msg
    );

    // CRITICAL: State must NOT have changed.
    assert_eq!(
        state.balance(&attacker),
        balance_before,
        "Fee should NOT have been debited for unauthorized operation"
    );
    assert_eq!(
        state.nonce(&attacker),
        nonce_before,
        "Nonce should NOT have been incremented for unauthorized operation"
    );
}

/// Test: Non-council member CreateProposal SmartOp should be rejected BEFORE state mutation.
#[test]
fn test_non_council_proposal_rejected_before_mutation() {
    let mut state = StateEngine::new_with_genesis();

    let p256_sk = SigningKey::random(&mut rand::thread_rng());
    let p256_pubkey = p256_sk.verifying_key().to_encoded_point(true).as_bytes().to_vec();
    let attacker = derive_smart_account_address_from_p256(&p256_pubkey);
    let key_id = AuthorizedKey::compute_key_id(KeyType::P256, &p256_pubkey);

    state.faucet_credit(&attacker, 1_000_000).unwrap();

    let balance_before = state.balance(&attacker);
    let nonce_before = state.nonce(&attacker);

    let mut op = SmartOpTx {
        from: attacker,
        operation: SmartOpType::CreateProposal {
            title: "Malicious Proposal".to_string(),
            description: "This should fail".to_string(),
            proposal_type_tag: 0, // TextProposal
            param: "".to_string(),
            new_value: 0,
        },
        fee: MIN_FEE_SATS,
        nonce: 0,
        signing_key_id: key_id,
        signature: vec![],
        webauthn: None,
        p256_pubkey: Some(p256_pubkey.clone()),
    };
    let sig: p256::ecdsa::Signature = p256_sk.sign(&op.signable_bytes());
    op.signature = sig.to_bytes().to_vec();

    let result = state.apply_smart_op_tx(&op, 1);

    assert!(result.is_err(), "CreateProposal should fail for non-council member");
    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("only council members"),
        "Expected council membership error, got: {}",
        err_msg
    );

    assert_eq!(
        state.balance(&attacker),
        balance_before,
        "Fee should NOT have been debited"
    );
    assert_eq!(
        state.nonce(&attacker),
        nonce_before,
        "Nonce should NOT have been incremented"
    );
}

/// Test: Failed SmartOp in finalized vertex path does NOT double-increment nonce.
/// This tests the outer error handler fix.
#[test]
fn test_failed_smartop_no_double_nonce_increment() {
    let mut state = StateEngine::new_with_genesis();
    state.set_configured_validator_count(1);

    let proposer_sk = SecretKey::from_bytes([0x11; 32]);

    // Create attacker with P256 key.
    let p256_sk = SigningKey::random(&mut rand::thread_rng());
    let p256_pubkey = p256_sk.verifying_key().to_encoded_point(true).as_bytes().to_vec();
    let attacker = derive_smart_account_address_from_p256(&p256_pubkey);
    let key_id = AuthorizedKey::compute_key_id(KeyType::P256, &p256_pubkey);

    state.faucet_credit(&attacker, 1_000_000).unwrap();

    let balance_before = state.balance(&attacker);
    let nonce_before = state.nonce(&attacker);

    // Create unauthorized Vote SmartOp.
    let mut op = SmartOpTx {
        from: attacker,
        operation: SmartOpType::Vote {
            proposal_id: 1,
            approve: true,
        },
        fee: MIN_FEE_SATS,
        nonce: 0,
        signing_key_id: key_id,
        signature: vec![],
        webauthn: None,
        p256_pubkey: Some(p256_pubkey.clone()),
    };
    let sig: p256::ecdsa::Signature = p256_sk.sign(&op.signable_bytes());
    op.signature = sig.to_bytes().to_vec();

    // Apply through finalized vertex path (triggers outer error handler).
    let vertex = make_vertex(&proposer_sk, 1, 0, vec![Transaction::SmartOp(op)]);
    let result = state.apply_finalized_vertices(&[vertex]);

    // Should succeed (error is handled gracefully, not fatal).
    assert!(result.is_ok(), "apply_finalized_vertices should not fail fatally");

    // CRITICAL FIX: With the authorization-before-mutation fix, the nonce
    // should NOT be incremented at all because the operation fails authorization
    // BEFORE the nonce increment.
    assert_eq!(
        state.nonce(&attacker),
        nonce_before,
        "Nonce should NOT be incremented for unauthorized operation (fails before nonce increment)"
    );

    // Fee should NOT be debited (fails before fee debit).
    assert_eq!(
        state.balance(&attacker),
        balance_before,
        "Fee should NOT be debited for unauthorized operation"
    );
}

/// Test: Supply invariant is preserved after rejected SmartOp.
#[test]
fn test_supply_invariant_preserved_after_rejected_smartop() {
    let mut state = StateEngine::new_with_genesis();
    state.set_configured_validator_count(1);

    let proposer_sk = SecretKey::from_bytes([0x11; 32]);

    let p256_sk = SigningKey::random(&mut rand::thread_rng());
    let p256_pubkey = p256_sk.verifying_key().to_encoded_point(true).as_bytes().to_vec();
    let attacker = derive_smart_account_address_from_p256(&p256_pubkey);
    let key_id = AuthorizedKey::compute_key_id(KeyType::P256, &p256_pubkey);

    state.faucet_credit(&attacker, 1_000_000).unwrap();

    let balance_before = state.balance(&attacker);

    let mut op = SmartOpTx {
        from: attacker,
        operation: SmartOpType::Vote {
            proposal_id: 1,
            approve: true,
        },
        fee: MIN_FEE_SATS,
        nonce: 0,
        signing_key_id: key_id,
        signature: vec![],
        webauthn: None,
        p256_pubkey: Some(p256_pubkey.clone()),
    };
    let sig: p256::ecdsa::Signature = p256_sk.sign(&op.signable_bytes());
    op.signature = sig.to_bytes().to_vec();

    let vertex = make_vertex(&proposer_sk, 1, 0, vec![Transaction::SmartOp(op)]);
    let result = state.apply_finalized_vertices(&[vertex]);

    // Should succeed without fatal error.
    assert!(result.is_ok(), "Should not trigger fatal supply invariant error");

    // Attacker balance must remain unchanged (no fee debit).
    assert_eq!(
        state.balance(&attacker),
        balance_before,
        "Attacker balance must not change (no fee debit for unauthorized op)"
    );
    
    // The key invariant: no supply invariant broken error was triggered.
    // The test passing means the node did NOT halt with exit code 101.
}

/// Test: Valid council member Vote succeeds and mutates state correctly.
#[test]
fn test_council_vote_succeeds_and_mutates_state() {
    let mut state = StateEngine::new_with_genesis();

    // Create a council member.
    let council_sk = SecretKey::from_bytes([0x22; 32]);
    let council_addr = council_sk.address();
    state.faucet_credit(&council_addr, 1_000_000).unwrap();
    state.ensure_smart_account(&council_addr);

    // Add to council (simulate council membership).
    state.add_council_member(council_addr, ultradag_coin::governance::CouncilSeatCategory::Engineering);

    let balance_before = state.balance(&council_addr);
    let nonce_before = state.nonce(&council_addr);

    let mut op = SmartOpTx {
        from: council_addr,
        operation: SmartOpType::Vote {
            proposal_id: 1,
            approve: true,
        },
        fee: MIN_FEE_SATS,
        nonce: 0,
        signing_key_id: [0u8; 8],
        signature: vec![],
        webauthn: None,
        p256_pubkey: None,
    };

    // Should succeed.
    let result = state.apply_smart_op_tx(&op, 1);
    assert!(result.is_ok(), "Council member vote should succeed");

    // Fee debited and nonce incremented.
    assert_eq!(
        state.balance(&council_addr),
        balance_before - MIN_FEE_SATS
    );
    assert_eq!(state.nonce(&council_addr), nonce_before + 1);
}

/// Test: Valid council member CreateProposal succeeds.
#[test]
fn test_council_proposal_succeeds() {
    let mut state = StateEngine::new_with_genesis();

    let council_sk = SecretKey::from_bytes([0x33; 32]);
    let council_addr = council_sk.address();
    state.faucet_credit(&council_addr, 1_000_000).unwrap();
    state.ensure_smart_account(&council_addr);
    state.add_council_member(council_addr, ultradag_coin::governance::CouncilSeatCategory::Engineering);

    let balance_before = state.balance(&council_addr);

    let mut op = SmartOpTx {
        from: council_addr,
        operation: SmartOpType::CreateProposal {
            title: "Valid Proposal".to_string(),
            description: "This is a valid proposal".to_string(),
            proposal_type_tag: 0,
            param: "".to_string(),
            new_value: 0,
        },
        fee: MIN_FEE_SATS,
        nonce: 0,
        signing_key_id: [0u8; 8],
        signature: vec![],
        webauthn: None,
        p256_pubkey: None,
    };

    let result = state.apply_smart_op_tx(&op, 1);
    assert!(result.is_ok(), "Council member proposal should succeed");

    assert_eq!(
        state.balance(&council_addr),
        balance_before - MIN_FEE_SATS
    );
    assert_eq!(state.nonce(&council_addr), 1);
}
