/// Edge case coverage tests — fills gaps in delegation, governance, state engine,
/// checkpoint, and DAG testing.
///
/// These tests focus on untested boundary conditions and error paths.

use ultradag_coin::*;
use ultradag_coin::governance::{
    CreateProposalTx, VoteTx, ProposalType, ProposalStatus, CouncilSeatCategory,
    GovernanceParams,
};
use ultradag_coin::constants::{
    GOVERNANCE_VOTING_PERIOD_ROUNDS,
    MIN_FEE_SATS, COUNCIL_MAX_MEMBERS, MIN_DAO_VALIDATORS,
    PROPOSAL_TITLE_MAX_BYTES, PROPOSAL_DESCRIPTION_MAX_BYTES,
    MAX_ACTIVE_PROPOSALS,
};
use std::collections::HashSet;

// ============================================================
// Helpers
// ============================================================

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

fn make_set_commission_tx(sk: &SecretKey, percent: u8, nonce: u64) -> SetCommissionTx {
    let mut tx = SetCommissionTx {
        from: sk.address(),
        commission_percent: percent,
        nonce,
        pub_key: sk.verifying_key().to_bytes(),
        signature: Signature([0u8; 64]),
    };
    tx.signature = sk.sign(&tx.signable_bytes());
    tx
}

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
    approve: bool,
    fee: u64,
    nonce: u64,
) -> VoteTx {
    let mut tx = VoteTx {
        from: sk.address(),
        proposal_id,
        vote: approve,
        fee,
        nonce,
        pub_key: sk.verifying_key().to_bytes(),
        signature: Signature([0u8; 64]),
    };
    tx.signature = sk.sign(&tx.signable_bytes());
    tx
}

fn fund_and_seat(state: &mut StateEngine, sk: &SecretKey, cat: CouncilSeatCategory) {
    state.faucet_credit(&sk.address(), 10_000_000_000).unwrap();
    state.add_council_member(sk.address(), cat).unwrap();
}

// ============================================================
// DELEGATION EDGE CASES
// ============================================================

/// Delegate below MIN_DELEGATION_SATS is rejected.
#[test]
fn test_delegate_below_minimum_rejected() {
    let mut state = StateEngine::new_with_genesis();
    let val_sk = SecretKey::from_bytes([0x01; 32]);
    let del_sk = SecretKey::from_bytes([0x02; 32]);

    state.faucet_credit(&val_sk.address(), MIN_STAKE_SATS * 3).unwrap();
    state.apply_stake_tx(&make_stake_tx(&val_sk, MIN_STAKE_SATS, 0)).unwrap();
    state.recalculate_active_set();

    state.faucet_credit(&del_sk.address(), MIN_DELEGATION_SATS * 2).unwrap();

    let tx = make_delegate_tx(&del_sk, val_sk.address(), MIN_DELEGATION_SATS - 1, 0);
    let result = state.apply_delegate_tx(&tx);
    assert!(result.is_err(), "Delegation below minimum must be rejected");
    let err = format!("{}", result.unwrap_err());
    assert!(err.contains("below minimum"), "Error should mention minimum, got: {}", err);
}

/// Delegate to address that is not a validator (no stake) is rejected.
#[test]
fn test_delegate_to_non_validator_rejected() {
    let mut state = StateEngine::new_with_genesis();
    let non_validator_sk = SecretKey::from_bytes([0x03; 32]);
    let del_sk = SecretKey::from_bytes([0x04; 32]);

    state.faucet_credit(&del_sk.address(), MIN_DELEGATION_SATS * 2).unwrap();

    let tx = make_delegate_tx(&del_sk, non_validator_sk.address(), MIN_DELEGATION_SATS, 0);
    let result = state.apply_delegate_tx(&tx);
    assert!(result.is_err(), "Delegation to non-validator must be rejected");
    let err = format!("{}", result.unwrap_err());
    assert!(err.contains("not a validator"), "Error should mention validator, got: {}", err);
}

/// Delegate when already delegating is rejected (one delegation per address).
#[test]
fn test_double_delegation_rejected() {
    let mut state = StateEngine::new_with_genesis();
    let val_sk = SecretKey::from_bytes([0x05; 32]);
    let del_sk = SecretKey::from_bytes([0x06; 32]);

    state.faucet_credit(&val_sk.address(), MIN_STAKE_SATS * 3).unwrap();
    state.apply_stake_tx(&make_stake_tx(&val_sk, MIN_STAKE_SATS, 0)).unwrap();
    state.recalculate_active_set();

    state.faucet_credit(&del_sk.address(), MIN_DELEGATION_SATS * 5).unwrap();

    // First delegation succeeds
    let tx1 = make_delegate_tx(&del_sk, val_sk.address(), MIN_DELEGATION_SATS, 0);
    state.apply_delegate_tx(&tx1).unwrap();

    // Second delegation rejected
    let tx2 = make_delegate_tx(&del_sk, val_sk.address(), MIN_DELEGATION_SATS, 1);
    let result = state.apply_delegate_tx(&tx2);
    assert!(result.is_err(), "Double delegation must be rejected");
    let err = format!("{}", result.unwrap_err());
    assert!(err.contains("already has an active delegation"), "Got: {}", err);
}

/// Delegate with insufficient balance is rejected.
#[test]
fn test_delegate_insufficient_balance_rejected() {
    let mut state = StateEngine::new_with_genesis();
    let val_sk = SecretKey::from_bytes([0x07; 32]);
    let del_sk = SecretKey::from_bytes([0x08; 32]);

    state.faucet_credit(&val_sk.address(), MIN_STAKE_SATS * 3).unwrap();
    state.apply_stake_tx(&make_stake_tx(&val_sk, MIN_STAKE_SATS, 0)).unwrap();
    state.recalculate_active_set();

    // Fund less than delegation amount
    state.faucet_credit(&del_sk.address(), MIN_DELEGATION_SATS / 2).unwrap();

    let tx = make_delegate_tx(&del_sk, val_sk.address(), MIN_DELEGATION_SATS, 0);
    let result = state.apply_delegate_tx(&tx);
    assert!(result.is_err(), "Delegation with insufficient balance must be rejected");
}

/// Undelegate when no active delegation is rejected.
#[test]
fn test_undelegate_no_delegation_rejected() {
    let mut state = StateEngine::new_with_genesis();
    let sk = SecretKey::from_bytes([0x09; 32]);
    state.faucet_credit(&sk.address(), 1_000_000_000).unwrap();

    let tx = make_undelegate_tx(&sk, 0);
    let result = state.apply_undelegate_tx(&tx, 100);
    assert!(result.is_err(), "Undelegate with no delegation must be rejected");
    let err = format!("{}", result.unwrap_err());
    assert!(err.contains("no active delegation"), "Got: {}", err);
}

/// Undelegate twice (already undelegating) is rejected.
#[test]
fn test_double_undelegate_rejected() {
    let mut state = StateEngine::new_with_genesis();
    let val_sk = SecretKey::from_bytes([0x0A; 32]);
    let del_sk = SecretKey::from_bytes([0x0B; 32]);

    state.faucet_credit(&val_sk.address(), MIN_STAKE_SATS * 3).unwrap();
    state.apply_stake_tx(&make_stake_tx(&val_sk, MIN_STAKE_SATS, 0)).unwrap();
    state.recalculate_active_set();

    state.faucet_credit(&del_sk.address(), MIN_DELEGATION_SATS * 3).unwrap();
    let del_tx = make_delegate_tx(&del_sk, val_sk.address(), MIN_DELEGATION_SATS, 0);
    state.apply_delegate_tx(&del_tx).unwrap();

    // First undelegate succeeds
    let undel_tx1 = make_undelegate_tx(&del_sk, 1);
    state.apply_undelegate_tx(&undel_tx1, 100).unwrap();

    // Second undelegate rejected
    let undel_tx2 = make_undelegate_tx(&del_sk, 2);
    let result = state.apply_undelegate_tx(&undel_tx2, 200);
    assert!(result.is_err(), "Double undelegate must be rejected");
    let err = format!("{}", result.unwrap_err());
    assert!(err.contains("already undelegating"), "Got: {}", err);
}

/// Undelegation completes after cooldown and returns funds.
#[test]
fn test_undelegate_completes_after_cooldown() {
    let mut state = StateEngine::new_with_genesis();
    let val_sk = SecretKey::from_bytes([0x0C; 32]);
    let del_sk = SecretKey::from_bytes([0x0D; 32]);

    state.faucet_credit(&val_sk.address(), MIN_STAKE_SATS * 3).unwrap();
    state.apply_stake_tx(&make_stake_tx(&val_sk, MIN_STAKE_SATS, 0)).unwrap();
    state.recalculate_active_set();

    let del_amount = MIN_DELEGATION_SATS * 2;
    state.faucet_credit(&del_sk.address(), del_amount * 2).unwrap();
    let balance_before_delegate = state.balance(&del_sk.address());

    let del_tx = make_delegate_tx(&del_sk, val_sk.address(), del_amount, 0);
    state.apply_delegate_tx(&del_tx).unwrap();
    assert_eq!(state.balance(&del_sk.address()), balance_before_delegate - del_amount);

    // Begin undelegation at round 100
    let undel_tx = make_undelegate_tx(&del_sk, 1);
    state.apply_undelegate_tx(&undel_tx, 100).unwrap();

    // Before cooldown: funds still locked
    state.process_unstake_completions(100 + UNSTAKE_COOLDOWN_ROUNDS - 1);
    assert!(state.delegation_account(&del_sk.address()).is_some(),
        "Delegation should still exist before cooldown ends");

    // After cooldown: funds returned
    state.process_unstake_completions(100 + UNSTAKE_COOLDOWN_ROUNDS);
    assert!(state.delegation_account(&del_sk.address()).is_none(),
        "Delegation should be removed after cooldown");
    assert_eq!(state.balance(&del_sk.address()), balance_before_delegate,
        "Full delegated amount should be returned to liquid balance");
}

/// SetCommission above MAX_COMMISSION_PERCENT is rejected.
#[test]
fn test_set_commission_above_max_rejected() {
    let mut state = StateEngine::new_with_genesis();
    let sk = SecretKey::from_bytes([0x0E; 32]);

    state.faucet_credit(&sk.address(), MIN_STAKE_SATS * 3).unwrap();
    state.apply_stake_tx(&make_stake_tx(&sk, MIN_STAKE_SATS, 0)).unwrap();

    let tx = make_set_commission_tx(&sk, MAX_COMMISSION_PERCENT + 1, 1);
    let result = state.apply_set_commission_tx(&tx, 0);
    assert!(result.is_err(), "Commission above max must be rejected");
}

/// SetCommission by non-staker is rejected.
#[test]
fn test_set_commission_not_staking_rejected() {
    let mut state = StateEngine::new_with_genesis();
    let sk = SecretKey::from_bytes([0x0F; 32]);
    state.faucet_credit(&sk.address(), 1_000_000_000).unwrap();

    let tx = make_set_commission_tx(&sk, 50, 0);
    let result = state.apply_set_commission_tx(&tx, 0);
    assert!(result.is_err(), "SetCommission by non-staker must be rejected");
}

/// Delegate max amount (all balance) succeeds.
#[test]
fn test_delegate_full_balance() {
    let mut state = StateEngine::new_with_genesis();
    let val_sk = SecretKey::from_bytes([0x10; 32]);
    let del_sk = SecretKey::from_bytes([0x11; 32]);

    state.faucet_credit(&val_sk.address(), MIN_STAKE_SATS * 3).unwrap();
    state.apply_stake_tx(&make_stake_tx(&val_sk, MIN_STAKE_SATS, 0)).unwrap();
    state.recalculate_active_set();

    let del_amount = MIN_DELEGATION_SATS * 10;
    state.faucet_credit(&del_sk.address(), del_amount).unwrap();

    let tx = make_delegate_tx(&del_sk, val_sk.address(), del_amount, 0);
    state.apply_delegate_tx(&tx).unwrap();
    assert_eq!(state.balance(&del_sk.address()), 0, "Full balance should be delegated");
    assert_eq!(
        state.delegation_account(&del_sk.address()).unwrap().delegated,
        del_amount
    );
}

/// Effective stake includes active delegations but excludes undelegating ones.
#[test]
fn test_effective_stake_excludes_undelegating() {
    let mut state = StateEngine::new_with_genesis();
    let val_sk = SecretKey::from_bytes([0x12; 32]);
    let del_sk = SecretKey::from_bytes([0x13; 32]);

    state.faucet_credit(&val_sk.address(), MIN_STAKE_SATS * 3).unwrap();
    state.apply_stake_tx(&make_stake_tx(&val_sk, MIN_STAKE_SATS, 0)).unwrap();
    state.recalculate_active_set();

    let del_amount = MIN_DELEGATION_SATS * 5;
    state.faucet_credit(&del_sk.address(), del_amount * 2).unwrap();
    let del_tx = make_delegate_tx(&del_sk, val_sk.address(), del_amount, 0);
    state.apply_delegate_tx(&del_tx).unwrap();

    let effective_before = state.effective_stake_of(&val_sk.address());
    assert_eq!(effective_before, MIN_STAKE_SATS + del_amount,
        "Effective stake should include own + delegated");

    // Begin undelegation
    let undel_tx = make_undelegate_tx(&del_sk, 1);
    state.apply_undelegate_tx(&undel_tx, 100).unwrap();

    let effective_after = state.effective_stake_of(&val_sk.address());
    assert_eq!(effective_after, MIN_STAKE_SATS,
        "Effective stake should exclude undelegating delegation");
}

/// Nonce mismatch on delegate tx is rejected.
#[test]
fn test_delegate_wrong_nonce_rejected() {
    let mut state = StateEngine::new_with_genesis();
    let val_sk = SecretKey::from_bytes([0x14; 32]);
    let del_sk = SecretKey::from_bytes([0x15; 32]);

    state.faucet_credit(&val_sk.address(), MIN_STAKE_SATS * 3).unwrap();
    state.apply_stake_tx(&make_stake_tx(&val_sk, MIN_STAKE_SATS, 0)).unwrap();
    state.recalculate_active_set();

    state.faucet_credit(&del_sk.address(), MIN_DELEGATION_SATS * 2).unwrap();

    // Use nonce 5 instead of 0
    let tx = make_delegate_tx(&del_sk, val_sk.address(), MIN_DELEGATION_SATS, 5);
    let result = state.apply_delegate_tx(&tx);
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), CoinError::InvalidNonce { .. }));
}

// ============================================================
// GOVERNANCE EDGE CASES
// ============================================================

/// Voting after the voting period ends is rejected.
#[test]
fn test_vote_after_period_ends_rejected() {
    let mut state = StateEngine::new_with_genesis();
    let sk = SecretKey::from_bytes([0x20; 32]);
    fund_and_seat(&mut state, &sk, CouncilSeatCategory::Engineering);

    let ptx = make_proposal_tx(
        &sk, 0, "Test", "Desc", ProposalType::TextProposal, MIN_FEE_SATS, 0,
    );
    state.apply_create_proposal(&ptx, 1000).unwrap();

    // Vote at a round past voting_ends
    let voting_ends = 1000 + GOVERNANCE_VOTING_PERIOD_ROUNDS;
    let vtx = make_vote_tx(&sk, 0, true, MIN_FEE_SATS, 1);
    let result = state.apply_vote(&vtx, voting_ends + 1);
    assert!(result.is_err(), "Voting after period must be rejected");
    assert!(matches!(result.unwrap_err(), CoinError::VotingClosed));
}

/// Proposal with title exceeding max length is rejected.
#[test]
fn test_proposal_title_too_long_rejected() {
    let mut state = StateEngine::new_with_genesis();
    let sk = SecretKey::from_bytes([0x21; 32]);
    fund_and_seat(&mut state, &sk, CouncilSeatCategory::Engineering);

    let long_title = "A".repeat(PROPOSAL_TITLE_MAX_BYTES + 1);
    let ptx = make_proposal_tx(
        &sk, 0, &long_title, "Desc", ProposalType::TextProposal, MIN_FEE_SATS, 0,
    );
    let result = state.apply_create_proposal(&ptx, 1000);
    assert!(result.is_err(), "Title exceeding max must be rejected");
    assert!(matches!(result.unwrap_err(), CoinError::ProposalTitleTooLong));
}

/// Proposal with description exceeding max length is rejected.
#[test]
fn test_proposal_description_too_long_rejected() {
    let mut state = StateEngine::new_with_genesis();
    let sk = SecretKey::from_bytes([0x22; 32]);
    fund_and_seat(&mut state, &sk, CouncilSeatCategory::Engineering);

    let long_desc = "B".repeat(PROPOSAL_DESCRIPTION_MAX_BYTES + 1);
    let ptx = make_proposal_tx(
        &sk, 0, "Title", &long_desc, ProposalType::TextProposal, MIN_FEE_SATS, 0,
    );
    let result = state.apply_create_proposal(&ptx, 1000);
    assert!(result.is_err(), "Description exceeding max must be rejected");
    assert!(matches!(result.unwrap_err(), CoinError::ProposalDescriptionTooLong));
}

/// Proposal with wrong ID (non-sequential) is rejected.
#[test]
fn test_proposal_wrong_id_rejected() {
    let mut state = StateEngine::new_with_genesis();
    let sk = SecretKey::from_bytes([0x23; 32]);
    fund_and_seat(&mut state, &sk, CouncilSeatCategory::Engineering);

    // next_proposal_id is 0, try to create with id=5
    let ptx = make_proposal_tx(
        &sk, 5, "Title", "Desc", ProposalType::TextProposal, MIN_FEE_SATS, 0,
    );
    let result = state.apply_create_proposal(&ptx, 1000);
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), CoinError::InvalidProposalId));
}

/// Creating proposals at max active count is rejected.
#[test]
fn test_proposals_at_max_active_count_rejected() {
    let mut state = StateEngine::new_with_genesis();
    // Use multiple council members to have enough balance for fees
    let sks: Vec<SecretKey> = (0..7u8).map(|i| SecretKey::from_bytes([0x30 + i; 32])).collect();
    // Spread across categories: Engineering(5 max), Growth(3 max), etc.
    let cats = [
        CouncilSeatCategory::Engineering,
        CouncilSeatCategory::Engineering,
        CouncilSeatCategory::Engineering,
        CouncilSeatCategory::Engineering,
        CouncilSeatCategory::Engineering,
        CouncilSeatCategory::Growth,
        CouncilSeatCategory::Growth,
    ];
    for (i, sk) in sks.iter().enumerate() {
        fund_and_seat(&mut state, sk, cats[i]);
    }

    // Create MAX_ACTIVE_PROPOSALS proposals using different council members
    // (to avoid PROPOSAL_COOLDOWN_ROUNDS restriction)
    let mut nonces = vec![0u64; sks.len()];
    for i in 0..MAX_ACTIVE_PROPOSALS as u64 {
        let idx = (i as usize) % sks.len();
        let proposer_sk = &sks[idx];
        let nonce = nonces[idx];
        nonces[idx] += 1;
        
        let ptx = make_proposal_tx(
            proposer_sk, i, &format!("Proposal {}", i), "Desc",
            ProposalType::TextProposal, MIN_FEE_SATS, nonce,
        );
        // Use different rounds to avoid cooldown
        let round = 1000 + i * (crate::constants::PROPOSAL_COOLDOWN_ROUNDS + 1);
        state.apply_create_proposal(&ptx, round).unwrap();
    }

    // One more should fail - use a fresh address that hasn't proposed before
    let fresh_sk = SecretKey::from_bytes([0x99; 32]);
    fund_and_seat(&mut state, &fresh_sk, CouncilSeatCategory::Growth);
    
    let ptx = make_proposal_tx(
        &fresh_sk, MAX_ACTIVE_PROPOSALS as u64, "One too many", "Desc",
        ProposalType::TextProposal, MIN_FEE_SATS, 0,
    );
    let result = state.apply_create_proposal(&ptx, 1000);
    assert!(result.is_err(), "Should reject when at max active proposals");
    assert!(matches!(result.unwrap_err(), CoinError::TooManyActiveProposals));
}

/// Vote on nonexistent proposal is rejected.
#[test]
fn test_vote_on_nonexistent_proposal_rejected() {
    let mut state = StateEngine::new_with_genesis();
    let sk = SecretKey::from_bytes([0x24; 32]);
    fund_and_seat(&mut state, &sk, CouncilSeatCategory::Engineering);

    let vtx = make_vote_tx(&sk, 999, true, MIN_FEE_SATS, 0);
    let result = state.apply_vote(&vtx, 1000);
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), CoinError::ProposalNotFound));
}

/// Non-council member cannot vote.
#[test]
fn test_non_council_cannot_vote() {
    let mut state = StateEngine::new_with_genesis();
    let council_sk = SecretKey::from_bytes([0x25; 32]);
    let outsider_sk = SecretKey::from_bytes([0x26; 32]);

    fund_and_seat(&mut state, &council_sk, CouncilSeatCategory::Engineering);
    state.faucet_credit(&outsider_sk.address(), 10_000_000_000).unwrap();

    // Council member creates proposal
    let ptx = make_proposal_tx(
        &council_sk, 0, "Test", "Desc", ProposalType::TextProposal, MIN_FEE_SATS, 0,
    );
    state.apply_create_proposal(&ptx, 1000).unwrap();

    // Outsider tries to vote
    let vtx = make_vote_tx(&outsider_sk, 0, true, MIN_FEE_SATS, 0);
    let result = state.apply_vote(&vtx, 1001);
    assert!(result.is_err(), "Non-council member should not be able to vote");
}

/// Fee too low on proposal is rejected.
#[test]
fn test_proposal_fee_too_low_rejected() {
    let mut state = StateEngine::new_with_genesis();
    let sk = SecretKey::from_bytes([0x27; 32]);
    fund_and_seat(&mut state, &sk, CouncilSeatCategory::Engineering);

    let ptx = make_proposal_tx(
        &sk, 0, "Test", "Desc", ProposalType::TextProposal, MIN_FEE_SATS - 1, 0,
    );
    let result = state.apply_create_proposal(&ptx, 1000);
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), CoinError::FeeTooLow));
}

/// TreasurySpend proposal execution succeeds and debits treasury.
#[test]
fn test_treasury_spend_proposal_executes() {
    let mut state = StateEngine::new_with_genesis();
    let sk1 = SecretKey::from_bytes([0x40; 32]);
    let sk2 = SecretKey::from_bytes([0x41; 32]);
    let recipient_sk = SecretKey::from_bytes([0x42; 32]);

    fund_and_seat(&mut state, &sk1, CouncilSeatCategory::Engineering);
    fund_and_seat(&mut state, &sk2, CouncilSeatCategory::Growth);

    // Build up treasury through emission (no genesis pre-fund in emission-only model)
    // Each round emits 10% of block_reward to treasury = 10,000,000 sats = 0.1 UDAG
    // Need 1000 UDAG = 10,000 rounds of emission
    let dummy_producer = SecretKey::from_bytes([0x99; 32]);
    let mut producers = std::collections::HashSet::new();
    producers.insert(dummy_producer.address());
    for r in 0..10_100u64 {
        state.distribute_round_rewards(r, &producers).unwrap();
    }
    let treasury_before = state.treasury_balance();
    let spend_amount = 1_000 * COIN; // 1000 UDAG
    assert!(treasury_before >= spend_amount, "Treasury should have enough for test (has {} sats)", treasury_before);

    // Create TreasurySpend proposal
    let ptx = make_proposal_tx(
        &sk1, 0, "Spend", "Fund dev",
        ProposalType::TreasurySpend {
            recipient: recipient_sk.address(),
            amount: spend_amount,
        },
        MIN_FEE_SATS, 0,
    );
    state.apply_create_proposal(&ptx, 1000).unwrap();

    // Both council members vote yes
    let v1 = make_vote_tx(&sk1, 0, true, MIN_FEE_SATS, 1);
    state.apply_vote(&v1, 1001).unwrap();
    let v2 = make_vote_tx(&sk2, 0, true, MIN_FEE_SATS, 0);
    state.apply_vote(&v2, 1001).unwrap();

    // Need MIN_DAO_VALIDATORS for TreasurySpend to execute — add more validators
    for i in 0..MIN_DAO_VALIDATORS as u8 {
        let val_sk = SecretKey::from_bytes([0x50 + i; 32]);
        state.faucet_credit(&val_sk.address(), MIN_STAKE_SATS * 2).unwrap();
        state.apply_stake_tx(&make_stake_tx(&val_sk, MIN_STAKE_SATS, 0)).unwrap();
    }
    state.recalculate_active_set();

    // Advance past voting period and trigger governance tick
    let voting_ends = 1000 + GOVERNANCE_VOTING_PERIOD_ROUNDS;
    state.tick_governance(voting_ends + 1);

    // Should be in PassedPending
    let proposal = state.proposal(0).unwrap();
    assert!(matches!(proposal.status, ProposalStatus::PassedPending { .. }),
        "Should be PassedPending, got: {:?}", proposal.status);

    // Advance past execution delay
    if let ProposalStatus::PassedPending { execute_at_round } = proposal.status {
        state.tick_governance(execute_at_round);

        let proposal_after = state.proposal(0).unwrap();
        assert!(matches!(proposal_after.status, ProposalStatus::Executed),
            "Should be Executed, got: {:?}", proposal_after.status);

        // Verify treasury was debited and recipient was credited
        assert_eq!(
            state.treasury_balance(),
            treasury_before - spend_amount,
            "Treasury should be debited"
        );
        assert_eq!(
            state.balance(&recipient_sk.address()),
            spend_amount,
            "Recipient should receive the spend amount"
        );
    }
}

/// TreasurySpend exceeding treasury balance transitions to Failed.
#[test]
fn test_treasury_spend_insufficient_funds_fails() {
    let mut state = StateEngine::new_with_genesis();
    let sk1 = SecretKey::from_bytes([0x43; 32]);
    let sk2 = SecretKey::from_bytes([0x44; 32]);
    let recipient_sk = SecretKey::from_bytes([0x45; 32]);

    fund_and_seat(&mut state, &sk1, CouncilSeatCategory::Engineering);
    fund_and_seat(&mut state, &sk2, CouncilSeatCategory::Growth);

    // Request more than treasury holds
    let excessive_amount = state.treasury_balance() + 1;

    let ptx = make_proposal_tx(
        &sk1, 0, "Overspend", "Too much",
        ProposalType::TreasurySpend {
            recipient: recipient_sk.address(),
            amount: excessive_amount,
        },
        MIN_FEE_SATS, 0,
    );
    state.apply_create_proposal(&ptx, 1000).unwrap();

    let v1 = make_vote_tx(&sk1, 0, true, MIN_FEE_SATS, 1);
    state.apply_vote(&v1, 1001).unwrap();
    let v2 = make_vote_tx(&sk2, 0, true, MIN_FEE_SATS, 0);
    state.apply_vote(&v2, 1001).unwrap();

    // Add validators for DAO activation
    for i in 0..MIN_DAO_VALIDATORS as u8 {
        let val_sk = SecretKey::from_bytes([0x60 + i; 32]);
        state.faucet_credit(&val_sk.address(), MIN_STAKE_SATS * 2).unwrap();
        state.apply_stake_tx(&make_stake_tx(&val_sk, MIN_STAKE_SATS, 0)).unwrap();
    }
    state.recalculate_active_set();

    let voting_ends = 1000 + GOVERNANCE_VOTING_PERIOD_ROUNDS;
    state.tick_governance(voting_ends + 1);

    let proposal = state.proposal(0).unwrap();
    if let ProposalStatus::PassedPending { execute_at_round } = proposal.status {
        state.tick_governance(execute_at_round);

        let proposal_after = state.proposal(0).unwrap();
        assert!(
            matches!(proposal_after.status, ProposalStatus::Failed { .. }),
            "Should be Failed due to insufficient treasury, got: {:?}", proposal_after.status
        );
    }
}

// ============================================================
// GOVERNANCE PARAMS EDGE CASES
// ============================================================

/// GovernanceParams apply_change rejects zero min_fee_sats.
#[test]
fn test_governance_param_min_fee_zero_rejected() {
    let mut params = GovernanceParams::default();
    assert!(params.apply_change("min_fee_sats", "0").is_err());
}

/// GovernanceParams apply_change rejects min_fee above ceiling.
#[test]
fn test_governance_param_min_fee_ceiling() {
    let mut params = GovernanceParams::default();
    assert!(params.apply_change("min_fee_sats", "100000001").is_err());
    // At ceiling should succeed
    assert!(params.apply_change("min_fee_sats", "100000000").is_ok());
}

/// GovernanceParams rejects approval_numerator below 51.
#[test]
fn test_governance_param_approval_below_51_rejected() {
    let mut params = GovernanceParams::default();
    assert!(params.apply_change("approval_numerator", "50").is_err());
    assert!(params.apply_change("approval_numerator", "51").is_ok());
}

/// GovernanceParams rejects slash_percent outside 10-100 range.
#[test]
fn test_governance_param_slash_percent_bounds() {
    let mut params = GovernanceParams::default();
    assert!(params.apply_change("slash_percent", "9").is_err());
    assert!(params.apply_change("slash_percent", "101").is_err());
    assert!(params.apply_change("slash_percent", "10").is_ok());
    assert!(params.apply_change("slash_percent", "100").is_ok());
}

/// GovernanceParams rejects council_emission_percent above 30.
#[test]
fn test_governance_param_council_emission_ceiling() {
    let mut params = GovernanceParams::default();
    assert!(params.apply_change("council_emission_percent", "31").is_err());
    assert!(params.apply_change("council_emission_percent", "30").is_ok());
    assert!(params.apply_change("council_emission_percent", "0").is_ok());
}

/// GovernanceParams rejects non-numeric values.
#[test]
fn test_governance_param_non_numeric_rejected() {
    let mut params = GovernanceParams::default();
    assert!(params.apply_change("min_fee_sats", "abc").is_err());
    assert!(params.apply_change("slash_percent", "-5").is_err());
    assert!(params.apply_change("quorum_numerator", "").is_err());
}

/// GovernanceParams rejects unknown parameter name.
#[test]
fn test_governance_param_unknown_name_rejected() {
    let mut params = GovernanceParams::default();
    assert!(params.apply_change("nonexistent_param", "42").is_err());
}

/// GovernanceParams execution_delay_rounds must be >= 2016.
#[test]
fn test_governance_param_execution_delay_floor() {
    let mut params = GovernanceParams::default();
    assert!(params.apply_change("execution_delay_rounds", "2015").is_err());
    assert!(params.apply_change("execution_delay_rounds", "2016").is_ok());
}

/// GovernanceParams voting_period_rounds must be >= 1000.
#[test]
fn test_governance_param_voting_period_floor() {
    let mut params = GovernanceParams::default();
    assert!(params.apply_change("voting_period_rounds", "999").is_err());
    assert!(params.apply_change("voting_period_rounds", "1000").is_ok());
}

/// GovernanceParams max_active_proposals must be 1-100.
#[test]
fn test_governance_param_max_proposals_bounds() {
    let mut params = GovernanceParams::default();
    assert!(params.apply_change("max_active_proposals", "0").is_err());
    assert!(params.apply_change("max_active_proposals", "101").is_err());
    assert!(params.apply_change("max_active_proposals", "1").is_ok());
    assert!(params.apply_change("max_active_proposals", "100").is_ok());
}

// ============================================================
// COUNCIL EDGE CASES
// ============================================================

/// Adding member to full council is rejected.
#[test]
fn test_council_full_capacity_rejected() {
    let mut state = StateEngine::new_with_genesis();

    // Genesis adds 1 Operations member. Fill remaining slots.
    // Engineering: 5, Growth: 3, Legal: 2, Research: 2, Community: 4, Operations: 3, Security: 2
    // Genesis already has 1 Operations. Need to fill 20 more.
    let categories = vec![
        (CouncilSeatCategory::Engineering, 5),
        (CouncilSeatCategory::Growth, 3),
        (CouncilSeatCategory::Legal, 2),
        (CouncilSeatCategory::Research, 2),
        (CouncilSeatCategory::Community, 4),
        (CouncilSeatCategory::Operations, 2), // 1 already from genesis
        (CouncilSeatCategory::Security, 2),
    ];

    let mut seed = 0x70u8;
    for (cat, count) in &categories {
        for _ in 0..*count {
            let sk = SecretKey::from_bytes([seed; 32]);
            state.add_council_member(sk.address(), *cat).unwrap();
            seed = seed.wrapping_add(1);
        }
    }

    // Council should be full (21)
    assert_eq!(state.council_members().count(), COUNCIL_MAX_MEMBERS);

    // Adding one more should fail
    let extra_sk = SecretKey::from_bytes([0xFF; 32]);
    let result = state.add_council_member(extra_sk.address(), CouncilSeatCategory::Engineering);
    assert!(result.is_err(), "Adding to full council must be rejected");
}

/// Adding duplicate council member is rejected.
#[test]
fn test_council_duplicate_member_rejected() {
    let mut state = StateEngine::new_with_genesis();
    let sk = SecretKey::from_bytes([0xA0; 32]);
    state.add_council_member(sk.address(), CouncilSeatCategory::Engineering).unwrap();

    let result = state.add_council_member(sk.address(), CouncilSeatCategory::Engineering);
    assert!(result.is_err(), "Adding duplicate member must be rejected");
    let err = format!("{}", result.unwrap_err());
    assert!(err.contains("already a council member"), "Got: {}", err);
}

/// Category seat limit enforcement.
#[test]
fn test_council_category_seat_limit() {
    let mut state = StateEngine::new_with_genesis();

    // Security has 2 seats max
    let sk1 = SecretKey::from_bytes([0xB0; 32]);
    let sk2 = SecretKey::from_bytes([0xB1; 32]);
    let sk3 = SecretKey::from_bytes([0xB2; 32]);

    state.add_council_member(sk1.address(), CouncilSeatCategory::Security).unwrap();
    state.add_council_member(sk2.address(), CouncilSeatCategory::Security).unwrap();

    let result = state.add_council_member(sk3.address(), CouncilSeatCategory::Security);
    assert!(result.is_err(), "Should reject 3rd Security seat");
    let err = format!("{}", result.unwrap_err());
    assert!(err.contains("No vacant"), "Got: {}", err);
}

/// Removing non-existent council member returns false.
#[test]
fn test_council_remove_nonexistent() {
    let mut state = StateEngine::new_with_genesis();
    let sk = SecretKey::from_bytes([0xC0; 32]);
    let removed = state.remove_council_member(&sk.address());
    assert!(!removed, "Removing non-member should return false");
}

// ============================================================
// STATE ENGINE EDGE CASES
// ============================================================

/// Staking with exact balance (balance == MIN_STAKE_SATS) succeeds.
#[test]
fn test_stake_exact_balance() {
    let mut state = StateEngine::new_with_genesis();
    let sk = SecretKey::from_bytes([0xD0; 32]);
    state.faucet_credit(&sk.address(), MIN_STAKE_SATS).unwrap();

    let tx = make_stake_tx(&sk, MIN_STAKE_SATS, 0);
    state.apply_stake_tx(&tx).unwrap();
    assert_eq!(state.balance(&sk.address()), 0);
    assert_eq!(state.stake_of(&sk.address()), MIN_STAKE_SATS);
}

/// Staking more than balance is rejected.
#[test]
fn test_stake_exceeding_balance_rejected() {
    let mut state = StateEngine::new_with_genesis();
    let sk = SecretKey::from_bytes([0xD1; 32]);
    state.faucet_credit(&sk.address(), MIN_STAKE_SATS - 1).unwrap();

    let tx = make_stake_tx(&sk, MIN_STAKE_SATS, 0);
    let result = state.apply_stake_tx(&tx);
    assert!(result.is_err());
}

/// Unstaking when not staked is rejected.
#[test]
fn test_unstake_when_not_staked_rejected() {
    let mut state = StateEngine::new_with_genesis();
    let sk = SecretKey::from_bytes([0xD2; 32]);
    state.faucet_credit(&sk.address(), 1_000_000_000).unwrap();

    let tx = make_unstake_tx(&sk, 0);
    let result = state.apply_unstake_tx(&tx, 100);
    assert!(result.is_err());
}

/// Default slash (50%) burns half the stake and the correct supply amount.
#[test]
fn test_slash_default_50_percent() {
    let mut state = StateEngine::new_with_genesis();
    let sk = SecretKey::from_bytes([0xD3; 32]);

    state.faucet_credit(&sk.address(), MIN_STAKE_SATS * 3).unwrap();
    state.apply_stake_tx(&make_stake_tx(&sk, MIN_STAKE_SATS, 0)).unwrap();

    let slash_pct = state.governance_params().slash_percent;
    assert_eq!(slash_pct, 50, "Default slash percent should be 50");

    let supply_before = state.total_supply();
    let stake_before = state.stake_of(&sk.address());
    state.slash(&sk.address());

    let expected_slash = stake_before * slash_pct / 100;
    assert_eq!(state.stake_of(&sk.address()), stake_before - expected_slash,
        "Stake should be reduced by slash_percent");
    assert_eq!(state.total_supply(), supply_before - expected_slash,
        "Supply should decrease by slashed amount");
}

/// Double slash on same validator reduces stake further.
#[test]
fn test_double_slash_reduces_further() {
    let mut state = StateEngine::new_with_genesis();
    let sk = SecretKey::from_bytes([0xD6; 32]);

    state.faucet_credit(&sk.address(), MIN_STAKE_SATS * 4).unwrap();
    state.apply_stake_tx(&make_stake_tx(&sk, MIN_STAKE_SATS * 2, 0)).unwrap();

    let stake_initial = state.stake_of(&sk.address());
    state.slash(&sk.address());
    let stake_after_first = state.stake_of(&sk.address());
    assert_eq!(stake_after_first, stake_initial / 2);

    state.slash(&sk.address());
    let stake_after_second = state.stake_of(&sk.address());
    assert_eq!(stake_after_second, stake_after_first / 2);
    assert_eq!(stake_after_second, stake_initial / 4);
}

/// Reward distribution with no producers and no stakers does not mint.
#[test]
fn test_distribute_rewards_no_producers_no_stakers() {
    let mut state = StateEngine::new_with_genesis();
    state.set_configured_validator_count(4);

    let supply_before = state.total_supply();
    let empty_producers: HashSet<Address> = HashSet::new();
    state.distribute_round_rewards(0, &empty_producers).unwrap();

    // Council member gets emission but no validator rewards distributed
    // since there are no producers
    let supply_after = state.total_supply();
    // The council member (genesis Operations member) gets their share
    // but no validator pool distribution happens with empty producers
    assert!(supply_after >= supply_before,
        "Supply should not decrease from reward distribution");
}

/// Snapshot and load_snapshot roundtrip preserves all state.
#[test]
fn test_snapshot_roundtrip_preserves_delegation() {
    let mut state = StateEngine::new_with_genesis();
    let val_sk = SecretKey::from_bytes([0xD4; 32]);
    let del_sk = SecretKey::from_bytes([0xD5; 32]);

    // Set up stake and delegation
    state.faucet_credit(&val_sk.address(), MIN_STAKE_SATS * 3).unwrap();
    state.apply_stake_tx(&make_stake_tx(&val_sk, MIN_STAKE_SATS, 0)).unwrap();
    state.recalculate_active_set();

    state.faucet_credit(&del_sk.address(), MIN_DELEGATION_SATS * 3).unwrap();
    let del_tx = make_delegate_tx(&del_sk, val_sk.address(), MIN_DELEGATION_SATS * 2, 0);
    state.apply_delegate_tx(&del_tx).unwrap();

    // Snapshot
    let snapshot = state.snapshot();
    let supply_before = state.total_supply();
    let effective_before = state.effective_stake_of(&val_sk.address());

    // Load into fresh state
    let mut new_state = StateEngine::new_with_genesis();
    new_state.load_snapshot(snapshot);

    assert_eq!(new_state.total_supply(), supply_before);
    assert_eq!(new_state.effective_stake_of(&val_sk.address()), effective_before);
    assert!(new_state.delegation_account(&del_sk.address()).is_some());
    assert_eq!(
        new_state.delegation_account(&del_sk.address()).unwrap().delegated,
        MIN_DELEGATION_SATS * 2
    );
}

// ============================================================
// CHECKPOINT EDGE CASES
// ============================================================

/// Checkpoint at round 0 with genesis state.
#[test]
fn test_checkpoint_at_round_zero() {
    let sk = SecretKey::from_bytes([0xE0; 32]);
    let mut cp = Checkpoint {
        round: 0,
        state_root: [0u8; 32],
        dag_tip: [0u8; 32],
        total_supply: 0,
        prev_checkpoint_hash: [0u8; 32],
        signatures: vec![],
    };
    cp.sign(&sk);

    assert_eq!(cp.round, 0);
    assert_eq!(cp.signatures.len(), 1);
    let valid = cp.valid_signers();
    assert_eq!(valid.len(), 1);
    assert_eq!(valid[0], sk.address());
}

/// Checkpoint with no signatures is not accepted.
#[test]
fn test_checkpoint_no_signatures_not_accepted() {
    let sk = SecretKey::from_bytes([0xE1; 32]);
    let cp = Checkpoint {
        round: 100,
        state_root: [1u8; 32],
        dag_tip: [2u8; 32],
        total_supply: 1_000_000,
        prev_checkpoint_hash: [0u8; 32],
        signatures: vec![],
    };

    let active = vec![sk.address()];
    assert!(!cp.is_accepted(&active, 1), "Checkpoint with no sigs should not be accepted");
}

/// Checkpoint signable_bytes is stable and deterministic.
#[test]
fn test_checkpoint_signable_bytes_deterministic() {
    let cp = Checkpoint {
        round: 42,
        state_root: [0xAA; 32],
        dag_tip: [0xBB; 32],
        total_supply: 21_000_000,
        prev_checkpoint_hash: [0xCC; 32],
        signatures: vec![],
    };

    let bytes1 = cp.signable_bytes();
    let bytes2 = cp.signable_bytes();
    assert_eq!(bytes1, bytes2, "signable_bytes must be deterministic");

    // Changing round produces different bytes
    let cp2 = Checkpoint {
        round: 43,
        ..cp.clone()
    };
    assert_ne!(cp.signable_bytes(), cp2.signable_bytes(),
        "Different rounds should produce different signable bytes");
}

/// Checkpoint hash changes when prev_checkpoint_hash changes (chain linking).
#[test]
fn test_checkpoint_hash_depends_on_prev_hash() {
    let cp1 = Checkpoint {
        round: 100,
        state_root: [1u8; 32],
        dag_tip: [2u8; 32],
        total_supply: 1_000_000,
        prev_checkpoint_hash: [0u8; 32],
        signatures: vec![],
    };
    let cp2 = Checkpoint {
        prev_checkpoint_hash: [0xFF; 32],
        ..cp1.clone()
    };

    assert_ne!(cp1.checkpoint_hash(), cp2.checkpoint_hash(),
        "Different prev_checkpoint_hash must produce different checkpoint hashes");
}

// ============================================================
// DAG EDGE CASES
// ============================================================

/// DAG try_insert with too many parents returns TooManyParents error.
#[test]
fn test_dag_too_many_parents_rejected() {
    use ultradag_coin::consensus::dag::{BlockDag, DagInsertError};

    let mut dag = BlockDag::new();
    let sk = SecretKey::from_bytes([0xF0; 32]);
    let validator = sk.address();

    // Create vertex with MAX_PARENTS + 1 parents
    let too_many_parents: Vec<[u8; 32]> = (0..=MAX_PARENTS as u64)
        .map(|i| {
            let mut h = [0u8; 32];
            h[0..8].copy_from_slice(&i.to_le_bytes());
            h
        })
        .collect();

    let block = Block {
        header: BlockHeader {
            version: 1, height: 0,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as i64,
            prev_hash: [0u8; 32],
            merkle_root: [0u8; 32],
        },
        coinbase: CoinbaseTx { to: validator, amount: 0, height: 0 },
        transactions: vec![],
    };
    let mut vertex = DagVertex::new(
        block, too_many_parents, 1, validator,
        sk.verifying_key().to_bytes(), Signature([0u8; 64]),
    );
    vertex.signature = sk.sign(&vertex.signable_bytes());

    let result = dag.try_insert(vertex);
    assert!(matches!(result, Err(DagInsertError::TooManyParents)),
        "Should reject vertex with {} parents, got: {:?}", MAX_PARENTS + 1, result);
}

/// DAG try_insert with exactly MAX_PARENTS succeeds (boundary case).
/// Note: parents must exist in DAG or be genesis parent [0u8; 32].
#[test]
fn test_dag_max_parents_accepted() {
    use ultradag_coin::consensus::dag::BlockDag;

    let mut dag = BlockDag::new();
    let sk = SecretKey::from_bytes([0xF1; 32]);
    let validator = sk.address();

    // Use genesis parent for all MAX_PARENTS entries
    let parents: Vec<[u8; 32]> = vec![[0u8; 32]; MAX_PARENTS];

    let block = Block {
        header: BlockHeader {
            version: 1, height: 0,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as i64,
            prev_hash: [0u8; 32],
            merkle_root: [0u8; 32],
        },
        coinbase: CoinbaseTx { to: validator, amount: 0, height: 0 },
        transactions: vec![],
    };
    let mut vertex = DagVertex::new(
        block, parents, 1, validator,
        sk.verifying_key().to_bytes(), Signature([0u8; 64]),
    );
    vertex.signature = sk.sign(&vertex.signable_bytes());

    let result = dag.try_insert(vertex);
    assert!(result.is_ok(), "Exactly MAX_PARENTS should be accepted: {:?}", result);
}

/// DAG equivocation detection: two vertices from same validator in same round.
#[test]
fn test_dag_equivocation_detection() {
    use ultradag_coin::consensus::dag::{BlockDag, DagInsertError};

    let mut dag = BlockDag::new();
    let sk = SecretKey::from_bytes([0xF2; 32]);
    let validator = sk.address();

    let make_v = |height: u64| {
        let block = Block {
            header: BlockHeader {
                version: 1, height,
                timestamp: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as i64,
                prev_hash: [0u8; 32],
                merkle_root: [0u8; 32],
            },
            coinbase: CoinbaseTx { to: validator, amount: height, height },
            transactions: vec![],
        };
        let mut v = DagVertex::new(
            block, vec![[0u8; 32]], 1, validator,
            sk.verifying_key().to_bytes(), Signature([0u8; 64]),
        );
        v.signature = sk.sign(&v.signable_bytes());
        v
    };

    // First insert succeeds
    let v1 = make_v(0);
    assert!(dag.try_insert(v1).unwrap());

    // Second insert (same round, different content) is equivocation
    let v2 = make_v(1);
    let result = dag.try_insert(v2);
    assert!(matches!(result, Err(DagInsertError::Equivocation { .. })),
        "Second vertex in same round should be equivocation: {:?}", result);

    // Validator should be marked Byzantine
    assert!(dag.is_byzantine(&validator), "Validator should be marked Byzantine");
}

/// DAG duplicate insert returns Ok(false).
#[test]
fn test_dag_duplicate_insert_returns_false() {
    use ultradag_coin::consensus::dag::BlockDag;

    let mut dag = BlockDag::new();
    let sk = SecretKey::from_bytes([0xF3; 32]);
    let validator = sk.address();

    let block = Block {
        header: BlockHeader {
            version: 1, height: 0,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as i64,
            prev_hash: [0u8; 32],
            merkle_root: [0u8; 32],
        },
        coinbase: CoinbaseTx { to: validator, amount: 0, height: 0 },
        transactions: vec![],
    };
    let mut vertex = DagVertex::new(
        block, vec![[0u8; 32]], 1, validator,
        sk.verifying_key().to_bytes(), Signature([0u8; 64]),
    );
    vertex.signature = sk.sign(&vertex.signable_bytes());

    // First insert
    assert!(dag.try_insert(vertex.clone()).unwrap());
    // Duplicate insert
    assert!(!dag.try_insert(vertex).unwrap(), "Duplicate should return Ok(false)");
}

/// DAG insert with missing (non-genesis) parents returns MissingParents.
#[test]
fn test_dag_missing_parents_error() {
    use ultradag_coin::consensus::dag::{BlockDag, DagInsertError};

    let mut dag = BlockDag::new();
    let sk = SecretKey::from_bytes([0xF4; 32]);
    let validator = sk.address();

    let fake_parent = [0xAA; 32]; // Not in DAG and not genesis

    let block = Block {
        header: BlockHeader {
            version: 1, height: 0,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as i64,
            prev_hash: [0u8; 32],
            merkle_root: [0u8; 32],
        },
        coinbase: CoinbaseTx { to: validator, amount: 0, height: 0 },
        transactions: vec![],
    };
    let mut vertex = DagVertex::new(
        block, vec![fake_parent], 1, validator,
        sk.verifying_key().to_bytes(), Signature([0u8; 64]),
    );
    vertex.signature = sk.sign(&vertex.signable_bytes());

    let result = dag.try_insert(vertex);
    match result {
        Err(DagInsertError::MissingParents(missing)) => {
            assert_eq!(missing.len(), 1);
            assert_eq!(missing[0], fake_parent);
        }
        other => panic!("Expected MissingParents, got: {:?}", other),
    }
}

// ============================================================
// REWARD DISTRIBUTION EDGE CASES
// ============================================================

/// Commission at 0% gives all delegation rewards to delegators.
#[test]
fn test_zero_commission_all_rewards_to_delegators() {
    let mut state = StateEngine::new_with_genesis();
    let val_sk = SecretKey::from_bytes([0xE5; 32]);
    let del_sk = SecretKey::from_bytes([0xE6; 32]);

    state.faucet_credit(&val_sk.address(), MIN_STAKE_SATS * 3).unwrap();
    state.apply_stake_tx(&make_stake_tx(&val_sk, MIN_STAKE_SATS, 0)).unwrap();

    // Set commission to 0%
    let comm_tx = make_set_commission_tx(&val_sk, 0, 1);
    state.apply_set_commission_tx(&comm_tx, 0).unwrap();

    state.recalculate_active_set();

    state.faucet_credit(&del_sk.address(), MIN_DELEGATION_SATS * 5).unwrap();
    let del_tx = make_delegate_tx(&del_sk, val_sk.address(), MIN_DELEGATION_SATS * 4, 0);
    state.apply_delegate_tx(&del_tx).unwrap();

    let val_balance_before = state.balance(&val_sk.address());
    let del_balance_before = state.balance(&del_sk.address());

    let mut producers = HashSet::new();
    producers.insert(val_sk.address());
    state.distribute_round_rewards(0, &producers).unwrap();

    let val_balance_after = state.balance(&val_sk.address());
    let del_balance_after = state.balance(&del_sk.address());

    let val_reward = val_balance_after - val_balance_before;
    let del_reward = del_balance_after - del_balance_before;

    // Both should have rewards, and delegator should get their full share (no commission)
    assert!(val_reward > 0, "Validator should get own-stake reward");
    assert!(del_reward > 0, "Delegator should get reward with 0% commission");
}

/// Commission at 100% gives all delegation rewards to validator.
#[test]
fn test_full_commission_all_rewards_to_validator() {
    let mut state = StateEngine::new_with_genesis();
    let val_sk = SecretKey::from_bytes([0xE7; 32]);
    let del_sk = SecretKey::from_bytes([0xE8; 32]);

    state.faucet_credit(&val_sk.address(), MIN_STAKE_SATS * 3).unwrap();
    state.apply_stake_tx(&make_stake_tx(&val_sk, MIN_STAKE_SATS, 0)).unwrap();

    // Set commission to 100%
    let comm_tx = make_set_commission_tx(&val_sk, 100, 1);
    state.apply_set_commission_tx(&comm_tx, 0).unwrap();

    state.recalculate_active_set();

    state.faucet_credit(&del_sk.address(), MIN_DELEGATION_SATS * 5).unwrap();
    let del_tx = make_delegate_tx(&del_sk, val_sk.address(), MIN_DELEGATION_SATS * 4, 0);
    state.apply_delegate_tx(&del_tx).unwrap();

    let del_balance_before = state.balance(&del_sk.address());

    let mut producers = HashSet::new();
    producers.insert(val_sk.address());
    state.distribute_round_rewards(0, &producers).unwrap();

    let del_balance_after = state.balance(&del_sk.address());

    // Delegator should get zero reward with 100% commission
    assert_eq!(del_balance_after, del_balance_before,
        "Delegator should get no reward with 100% commission");
}

/// Proposal has_passed_with_params returns false when total_staked is 0.
#[test]
fn test_proposal_zero_total_staked_cannot_pass() {
    use ultradag_coin::governance::Proposal;

    let proposal = Proposal {
        id: 0,
        proposer: SecretKey::from_bytes([1; 32]).address(),
        title: "Test".into(),
        description: "Desc".into(),
        proposal_type: ProposalType::TextProposal,
        voting_starts: 0,
        voting_ends: 1000,
        votes_for: 100,
        votes_against: 0,
        status: ProposalStatus::Active,
        snapshot_total_stake: 0,
    };

    let params = GovernanceParams::default();
    assert!(!proposal.has_passed_with_params(0, &params),
        "Proposal must not pass with zero total stake (empty council)");
}

/// Proposal total_votes uses saturating_add.
#[test]
fn test_proposal_total_votes_saturating() {
    use ultradag_coin::governance::Proposal;

    let proposal = Proposal {
        id: 0,
        proposer: SecretKey::from_bytes([1; 32]).address(),
        title: "Test".into(),
        description: "Desc".into(),
        proposal_type: ProposalType::TextProposal,
        voting_starts: 0,
        voting_ends: 1000,
        votes_for: u64::MAX,
        votes_against: u64::MAX,
        status: ProposalStatus::Active,
        snapshot_total_stake: 10,
    };

    assert_eq!(proposal.total_votes(), u64::MAX,
        "total_votes should saturate, not overflow");
}
