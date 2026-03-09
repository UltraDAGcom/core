/// Governance integration tests: proposal and vote transaction creation, hashing, and signatures.

use ultradag_coin::*;
use ultradag_coin::governance::{CreateProposalTx, VoteTx, ProposalType};

fn make_proposal_tx(
    sk: &SecretKey,
    proposal_id: u64,
    title: &str,
    description: &str,
    proposal_type: ProposalType,
    fee: u64,
    nonce: u64,
) -> CreateProposalTx {
    let mut tx = CreateProposalTx {
        from: sk.address(),
        proposal_id,
        title: title.to_string(),
        description: description.to_string(),
        proposal_type,
        fee,
        nonce,
        pub_key: sk.verifying_key().to_bytes(),
        signature: Signature([0u8; 64]),
    };
    tx.signature = sk.sign(&tx.signable_bytes());
    tx
}

fn make_vote_tx(
    sk: &SecretKey,
    proposal_id: u64,
    vote: bool,
    fee: u64,
    nonce: u64,
) -> VoteTx {
    let mut tx = VoteTx {
        from: sk.address(),
        proposal_id,
        vote,
        fee,
        nonce,
        pub_key: sk.verifying_key().to_bytes(),
        signature: Signature([0u8; 64]),
    };
    tx.signature = sk.sign(&tx.signable_bytes());
    tx
}

#[test]
fn proposal_tx_hash_and_signature_are_valid() {
    let sk = SecretKey::generate();
    let tx = make_proposal_tx(
        &sk,
        1,
        "Test Proposal",
        "A proposal for testing",
        ProposalType::TextProposal,
        10_000,
        0,
    );

    // Hash should be deterministic (non-zero)
    let hash = tx.hash();
    assert_ne!(hash, [0u8; 32], "Proposal hash should not be zero");

    // Same inputs produce the same hash
    let hash2 = tx.hash();
    assert_eq!(hash, hash2, "Proposal hash should be deterministic");

    // Signature should verify
    assert!(tx.verify_signature(), "Proposal signature should be valid");
}

#[test]
fn vote_tx_hash_and_signature_are_valid() {
    let sk = SecretKey::generate();
    let tx = make_vote_tx(&sk, 1, true, 10_000, 0);

    // Hash should be deterministic (non-zero)
    let hash = tx.hash();
    assert_ne!(hash, [0u8; 32], "Vote hash should not be zero");

    // Same inputs produce the same hash
    let hash2 = tx.hash();
    assert_eq!(hash, hash2, "Vote hash should be deterministic");

    // Signature should verify
    assert!(tx.verify_signature(), "Vote signature should be valid");
}

#[test]
fn different_proposal_types_produce_different_hashes() {
    let sk = SecretKey::generate();

    let text_tx = make_proposal_tx(
        &sk,
        1,
        "Same Title",
        "Same Description",
        ProposalType::TextProposal,
        10_000,
        0,
    );

    let param_tx = make_proposal_tx(
        &sk,
        1,
        "Same Title",
        "Same Description",
        ProposalType::ParameterChange {
            param: "round_ms".to_string(),
            new_value: "3000".to_string(),
        },
        10_000,
        0,
    );

    assert_ne!(
        text_tx.hash(),
        param_tx.hash(),
        "TextProposal and ParameterChange with same title/description should have different hashes"
    );

    // Both signatures should still be valid
    assert!(text_tx.verify_signature(), "Text proposal signature should be valid");
    assert!(param_tx.verify_signature(), "Parameter change proposal signature should be valid");
}
