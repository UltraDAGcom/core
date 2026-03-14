use ultradag_coin::{
    SecretKey, StateEngine, Signature, StakeTx,
};
use ultradag_coin::governance::{CreateProposalTx, ProposalType, VoteTx, CouncilSeatCategory};
use ultradag_coin::constants::{
    GOVERNANCE_APPROVAL_DENOMINATOR, GOVERNANCE_APPROVAL_NUMERATOR,
    GOVERNANCE_QUORUM_DENOMINATOR, GOVERNANCE_QUORUM_NUMERATOR,
    GOVERNANCE_VOTING_PERIOD_ROUNDS, MIN_STAKE_TO_PROPOSE,
};

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

/// Helper: fund an address and add as council member (for proposal/voting rights).
/// Also stakes so the address has balance for fees.
fn fund_and_seat_council(state: &mut StateEngine, sk: &SecretKey, category: CouncilSeatCategory) {
    let addr = sk.address();
    // Give enough balance for fees
    state.faucet_credit(&addr, 1_000_000_000).unwrap();
    state.add_council_member(addr, category).unwrap();
}

/// Helper: fund, stake, and add as council member (for tests that also need staking).
fn fund_stake_council(state: &mut StateEngine, sk: &SecretKey, stake_amount: u64, nonce: u64) {
    let addr = sk.address();
    state.faucet_credit(&addr, stake_amount + 1_000_000_000).unwrap();
    let mut stake_tx = StakeTx {
        from: addr,
        amount: stake_amount,
        nonce,
        pub_key: sk.verifying_key().to_bytes(),
        signature: Signature([0u8; 64]),
    };
    stake_tx.signature = sk.sign(&stake_tx.signable_bytes());
    state.apply_stake_tx(&stake_tx).unwrap();
    state.add_council_member(addr, CouncilSeatCategory::Technical).unwrap();
}

#[test]
fn test_full_proposal_lifecycle() {
    let mut state = StateEngine::new_with_genesis();

    // Create proposer as council member
    let proposer = SecretKey::generate();
    fund_and_seat_council(&mut state, &proposer, CouncilSeatCategory::Technical);

    // Create proposal
    let proposal_tx = make_proposal_tx(
        &proposer,
        0,
        "Increase Block Size",
        "Proposal to increase max transactions per block from 10,000 to 20,000",
        ProposalType::ParameterChange {
            param: "max_txs_per_block".to_string(),
            new_value: "20000".to_string(),
        },
        10_000,
        0,
    );

    // Apply proposal at round 100
    state.apply_create_proposal(&proposal_tx, 100).unwrap();

    // Verify proposal was created
    let proposal = state.proposal(0).expect("Proposal should exist");
    assert_eq!(proposal.title, "Increase Block Size");
    assert_eq!(proposal.proposer, proposer.address());
    assert_eq!(proposal.voting_starts, 100);
    assert_eq!(proposal.votes_for, 0);
    assert_eq!(proposal.votes_against, 0);
    assert!(matches!(proposal.status, ultradag_coin::governance::ProposalStatus::Active));

    // Create voters (council members)
    let voter1 = SecretKey::generate();
    let voter2 = SecretKey::generate();
    let voter3 = SecretKey::generate();

    fund_and_seat_council(&mut state, &voter1, CouncilSeatCategory::Technical);
    fund_and_seat_council(&mut state, &voter2, CouncilSeatCategory::Business);
    fund_and_seat_council(&mut state, &voter3, CouncilSeatCategory::Legal);

    // Vote YES from voter1 and voter2
    let vote1 = make_vote_tx(&voter1, 0, true, 10_000, 0);
    let vote2 = make_vote_tx(&voter2, 0, true, 10_000, 0);

    state.apply_vote(&vote1, 150).unwrap();
    state.apply_vote(&vote2, 200).unwrap();

    // Vote NO from voter3
    let vote3 = make_vote_tx(&voter3, 0, false, 10_000, 0);
    state.apply_vote(&vote3, 250).unwrap();

    // Check vote tallies (1 per council member)
    let proposal = state.proposal(0).unwrap();
    assert_eq!(proposal.votes_for, 2);
    assert_eq!(proposal.votes_against, 1);

    // Verify individual votes
    assert_eq!(state.get_vote(0, &voter1.address()), Some(true));
    assert_eq!(state.get_vote(0, &voter2.address()), Some(true));
    assert_eq!(state.get_vote(0, &voter3.address()), Some(false));
}

#[test]
fn test_proposal_quorum_and_approval() {
    let mut state = StateEngine::new_with_genesis();

    // Create proposer as council member
    let proposer = SecretKey::generate();
    fund_and_seat_council(&mut state, &proposer, CouncilSeatCategory::Technical);

    // Create proposal
    let proposal_tx = make_proposal_tx(
        &proposer,
        0,
        "Test Proposal",
        "Testing quorum and approval",
        ProposalType::TextProposal,
        10_000,
        0,
    );
    state.apply_create_proposal(&proposal_tx, 100).unwrap();

    // snapshot_total_stake is council count at proposal creation time (1 member)
    // Quorum = ceil(1 * 10 / 100) = 1 vote needed
    // We need 1 YES vote to pass

    // Vote YES from proposer (they're a council member)
    let vote = make_vote_tx(&proposer, 0, true, 10_000, 1);
    state.apply_vote(&vote, 150).unwrap();

    // Check if proposal meets quorum and approval
    let proposal = state.proposal(0).unwrap();
    let total_votes = proposal.votes_for + proposal.votes_against;

    // Quorum check: total votes >= ceil(snapshot * quorum_num / quorum_den)
    let quorum_threshold = (proposal.snapshot_total_stake * GOVERNANCE_QUORUM_NUMERATOR as u64
        + GOVERNANCE_QUORUM_DENOMINATOR as u64 - 1) / GOVERNANCE_QUORUM_DENOMINATOR as u64;
    let meets_quorum = total_votes >= quorum_threshold;

    // Approval check: votes_for >= 66% of total_votes
    let meets_approval = if total_votes > 0 {
        (proposal.votes_for * GOVERNANCE_APPROVAL_DENOMINATOR as u64) >= (total_votes * GOVERNANCE_APPROVAL_NUMERATOR as u64)
    } else {
        false
    };

    assert!(meets_quorum, "Proposal should meet quorum threshold");
    assert!(meets_approval, "Proposal should meet approval threshold");
}

#[test]
fn test_proposal_rejection() {
    let mut state = StateEngine::new_with_genesis();

    // Create proposer as council member
    let proposer = SecretKey::generate();
    fund_and_seat_council(&mut state, &proposer, CouncilSeatCategory::Technical);

    // Create proposal
    let proposal_tx = make_proposal_tx(
        &proposer,
        0,
        "Bad Proposal",
        "This should be rejected",
        ProposalType::TextProposal,
        10_000,
        0,
    );
    state.apply_create_proposal(&proposal_tx, 100).unwrap();

    // Create voters (council members)
    let voter1 = SecretKey::generate();
    let voter2 = SecretKey::generate();
    fund_and_seat_council(&mut state, &voter1, CouncilSeatCategory::Technical);
    fund_and_seat_council(&mut state, &voter2, CouncilSeatCategory::Business);

    // Both vote NO
    let vote1 = make_vote_tx(&voter1, 0, false, 10_000, 0);
    let vote2 = make_vote_tx(&voter2, 0, false, 10_000, 0);

    state.apply_vote(&vote1, 150).unwrap();
    state.apply_vote(&vote2, 200).unwrap();

    // Check that proposal fails approval (0% approval)
    let proposal = state.proposal(0).unwrap();
    assert_eq!(proposal.votes_for, 0, "Should have no YES votes");
    assert_eq!(proposal.votes_against, 2, "Should have 2 NO votes");
}

#[test]
fn test_voting_period_expiration() {
    let mut state = StateEngine::new_with_genesis();

    // Create proposer as council member
    let proposer = SecretKey::generate();
    fund_and_seat_council(&mut state, &proposer, CouncilSeatCategory::Technical);

    // Create proposal at round 100
    let proposal_tx = make_proposal_tx(
        &proposer,
        0,
        "Time-Limited Proposal",
        "Testing voting period",
        ProposalType::TextProposal,
        10_000,
        0,
    );
    state.apply_create_proposal(&proposal_tx, 100).unwrap();

    // Create voter (council member)
    let voter = SecretKey::generate();
    fund_and_seat_council(&mut state, &voter, CouncilSeatCategory::Business);

    // Try to vote within voting period (should succeed)
    let vote_early = make_vote_tx(&voter, 0, true, 10_000, 0);
    let early_round = 100 + GOVERNANCE_VOTING_PERIOD_ROUNDS - 1;
    assert!(state.apply_vote(&vote_early, early_round).is_ok(), "Vote within period should succeed");

    // Try to vote after voting period (should fail)
    let voter2 = SecretKey::generate();
    fund_and_seat_council(&mut state, &voter2, CouncilSeatCategory::Legal);

    let vote_late = make_vote_tx(&voter2, 0, true, 10_000, 0);
    let late_round = 100 + GOVERNANCE_VOTING_PERIOD_ROUNDS + 1;
    assert!(state.apply_vote(&vote_late, late_round).is_err(), "Vote after period should fail");
}

#[test]
fn test_double_voting_prevention() {
    let mut state = StateEngine::new_with_genesis();

    // Create proposer as council member
    let proposer = SecretKey::generate();
    fund_and_seat_council(&mut state, &proposer, CouncilSeatCategory::Technical);

    // Create proposal
    let proposal_tx = make_proposal_tx(
        &proposer,
        0,
        "Test Proposal",
        "Testing double voting",
        ProposalType::TextProposal,
        10_000,
        0,
    );
    state.apply_create_proposal(&proposal_tx, 100).unwrap();

    // Create voter (council member)
    let voter = SecretKey::generate();
    fund_and_seat_council(&mut state, &voter, CouncilSeatCategory::Business);

    // First vote (should succeed)
    let vote1 = make_vote_tx(&voter, 0, true, 10_000, 0);
    assert!(state.apply_vote(&vote1, 150).is_ok(), "First vote should succeed");

    // Second vote (should fail - already voted)
    let vote2 = make_vote_tx(&voter, 0, false, 10_000, 1);
    assert!(state.apply_vote(&vote2, 200).is_err(), "Second vote should fail");

    // Verify only first vote counted (weight = 1)
    let proposal = state.proposal(0).unwrap();
    assert_eq!(proposal.votes_for, 1);
    assert_eq!(proposal.votes_against, 0);
}

// --- Governance Execution Tests ---

/// Helper: stake a freshly-generated address so it counts toward DAO activation.
fn stake_filler(state: &mut StateEngine) {
    let sk = SecretKey::generate();
    let addr = sk.address();
    let amount = ultradag_coin::constants::MIN_STAKE_TO_PROPOSE;
    state.faucet_credit(&addr, amount + 100_000_000).unwrap();
    let mut stx = StakeTx {
        from: addr,
        amount,
        nonce: 0,
        pub_key: sk.verifying_key().to_bytes(),
        signature: Signature([0u8; 64]),
    };
    stx.signature = sk.sign(&stx.signable_bytes());
    state.apply_stake_tx(&stx).unwrap();
}

/// Helper: set up a state with a proposer and voters who pass a ParameterChange proposal.
/// Returns (state, voting_ends_round) with proposal at id=0 in Active status.
fn setup_passing_proposal(
    param: &str,
    new_value: &str,
) -> (StateEngine, u64) {
    let mut state = StateEngine::new_with_genesis();

    // Create proposer as council member
    let proposer = SecretKey::generate();
    fund_and_seat_council(&mut state, &proposer, CouncilSeatCategory::Technical);

    let proposal_tx = make_proposal_tx(
        &proposer, 0, "Change Param", "Test param change",
        ProposalType::ParameterChange { param: param.to_string(), new_value: new_value.to_string() },
        10_000, 0,
    );
    state.apply_create_proposal(&proposal_tx, 100).unwrap();

    // Create 2 voters as council members — with 3 total council members and 1-per-seat voting,
    // 2 YES votes out of 3 members is well above 10% quorum and 66% approval
    let voter1 = SecretKey::generate();
    let voter2 = SecretKey::generate();
    fund_and_seat_council(&mut state, &voter1, CouncilSeatCategory::Business);
    fund_and_seat_council(&mut state, &voter2, CouncilSeatCategory::Legal);

    // Both vote YES
    let v1 = make_vote_tx(&voter1, 0, true, 10_000, 0);
    let v2 = make_vote_tx(&voter2, 0, true, 10_000, 0);
    state.apply_vote(&v1, 150).unwrap();
    state.apply_vote(&v2, 200).unwrap();

    // Stake fillers to activate DAO (need MIN_DAO_VALIDATORS=8 stakers)
    for _ in 0..8 {
        stake_filler(&mut state);
    }
    state.recalculate_active_set();

    let voting_ends = state.proposal(0).unwrap().voting_ends;
    (state, voting_ends)
}

#[test]
fn test_parameter_change_execution_updates_governance_params() {
    let (mut state, voting_ends) = setup_passing_proposal("min_fee_sats", "50000");

    // Before execution, min_fee_sats should be the default
    assert_eq!(state.governance_params().min_fee_sats, ultradag_coin::constants::MIN_FEE_SATS);

    // Tick past voting end — proposal transitions to PassedPending
    state.tick_governance(voting_ends + 1);
    let p = state.proposal(0).unwrap();
    assert!(matches!(p.status, ultradag_coin::governance::ProposalStatus::PassedPending { .. }));

    // Get execute_at_round
    let execute_at = match p.status {
        ultradag_coin::governance::ProposalStatus::PassedPending { execute_at_round } => execute_at_round,
        _ => panic!("Expected PassedPending"),
    };

    // Tick at execution round — proposal transitions to Executed and param changes
    state.tick_governance(execute_at);
    let p = state.proposal(0).unwrap();
    assert_eq!(p.status, ultradag_coin::governance::ProposalStatus::Executed);

    // Governance params should be updated
    assert_eq!(state.governance_params().min_fee_sats, 50_000);
}

#[test]
fn test_text_proposal_execution_has_no_param_effect() {
    let mut state = StateEngine::new_with_genesis();

    // Create proposer as council member
    let proposer = SecretKey::generate();
    fund_and_seat_council(&mut state, &proposer, CouncilSeatCategory::Technical);

    let proposal_tx = make_proposal_tx(
        &proposer, 0, "Text Only", "Informational proposal",
        ProposalType::TextProposal, 10_000, 0,
    );
    state.apply_create_proposal(&proposal_tx, 100).unwrap();

    // Vote to pass (council member)
    let voter = SecretKey::generate();
    fund_and_seat_council(&mut state, &voter, CouncilSeatCategory::Business);
    let v = make_vote_tx(&voter, 0, true, 10_000, 0);
    state.apply_vote(&v, 150).unwrap();

    let voting_ends = state.proposal(0).unwrap().voting_ends;
    let params_before = state.governance_params().clone();

    // Transition to PassedPending then Executed
    state.tick_governance(voting_ends + 1);
    let execute_at = match state.proposal(0).unwrap().status {
        ultradag_coin::governance::ProposalStatus::PassedPending { execute_at_round } => execute_at_round,
        _ => panic!("Expected PassedPending"),
    };
    state.tick_governance(execute_at);

    assert_eq!(state.proposal(0).unwrap().status, ultradag_coin::governance::ProposalStatus::Executed);
    // All params should be unchanged
    let params_after = state.governance_params();
    assert_eq!(params_before.min_fee_sats, params_after.min_fee_sats);
    assert_eq!(params_before.quorum_numerator, params_after.quorum_numerator);
    assert_eq!(params_before.approval_numerator, params_after.approval_numerator);
    assert_eq!(params_before.voting_period_rounds, params_after.voting_period_rounds);
}

#[test]
fn test_invalid_param_name_still_transitions_to_executed() {
    let (mut state, voting_ends) = setup_passing_proposal("nonexistent_param", "42");

    state.tick_governance(voting_ends + 1);
    let execute_at = match state.proposal(0).unwrap().status {
        ultradag_coin::governance::ProposalStatus::PassedPending { execute_at_round } => execute_at_round,
        _ => panic!("Expected PassedPending"),
    };
    state.tick_governance(execute_at);

    // Proposal still Executed (determinism), but no params changed
    assert_eq!(state.proposal(0).unwrap().status, ultradag_coin::governance::ProposalStatus::Executed);
    // All defaults still in place
    assert_eq!(state.governance_params().min_fee_sats, ultradag_coin::constants::MIN_FEE_SATS);
}

#[test]
fn test_param_validation_bounds_enforced() {
    // approval_numerator must be 51-100, so "30" should fail
    let (mut state, voting_ends) = setup_passing_proposal("approval_numerator", "30");

    state.tick_governance(voting_ends + 1);
    let execute_at = match state.proposal(0).unwrap().status {
        ultradag_coin::governance::ProposalStatus::PassedPending { execute_at_round } => execute_at_round,
        _ => panic!("Expected PassedPending"),
    };
    state.tick_governance(execute_at);

    // Proposal Executed but param NOT changed (validation rejected it)
    assert_eq!(state.proposal(0).unwrap().status, ultradag_coin::governance::ProposalStatus::Executed);
    assert_eq!(
        state.governance_params().approval_numerator,
        GOVERNANCE_APPROVAL_NUMERATOR,
        "approval_numerator should remain at default because 30 < 51"
    );
}

#[test]
fn test_changed_params_persist_across_snapshot() {
    let (mut state, voting_ends) = setup_passing_proposal("quorum_numerator", "25");

    // Execute the proposal
    state.tick_governance(voting_ends + 1);
    let execute_at = match state.proposal(0).unwrap().status {
        ultradag_coin::governance::ProposalStatus::PassedPending { execute_at_round } => execute_at_round,
        _ => panic!("Expected PassedPending"),
    };
    state.tick_governance(execute_at);
    assert_eq!(state.governance_params().quorum_numerator, 25);

    // Snapshot and restore
    let snapshot = state.snapshot();
    let restored = StateEngine::from_snapshot(snapshot);
    assert_eq!(restored.governance_params().quorum_numerator, 25);
}

#[test]
fn test_changed_voting_period_affects_new_proposals() {
    // Change voting_period_rounds from default to 2000 (minimum is 1000)
    let (mut state, voting_ends) = setup_passing_proposal("voting_period_rounds", "2000");

    // Execute the proposal
    state.tick_governance(voting_ends + 1);
    let execute_at = match state.proposal(0).unwrap().status {
        ultradag_coin::governance::ProposalStatus::PassedPending { execute_at_round } => execute_at_round,
        _ => panic!("Expected PassedPending"),
    };
    state.tick_governance(execute_at);
    assert_eq!(state.governance_params().voting_period_rounds, 2000);

    // Create a new proposal — it should use the NEW voting_period_rounds
    let proposer2 = SecretKey::generate();
    fund_and_seat_council(&mut state, &proposer2, CouncilSeatCategory::Academic);

    let creation_round = execute_at + 10;
    let p2_tx = make_proposal_tx(
        &proposer2, 1, "Next Proposal", "Uses new voting period",
        ProposalType::TextProposal, 10_000, 0,
    );
    state.apply_create_proposal(&p2_tx, creation_round).unwrap();

    let p2 = state.proposal(1).unwrap();
    assert_eq!(p2.voting_ends, creation_round + 2000, "New proposal should use updated voting_period_rounds");
}

#[test]
fn test_multiple_param_changes_via_sequential_proposals() {
    // First proposal: change min_fee_sats
    let (mut state, voting_ends) = setup_passing_proposal("min_fee_sats", "20000");

    state.tick_governance(voting_ends + 1);
    let execute_at = match state.proposal(0).unwrap().status {
        ultradag_coin::governance::ProposalStatus::PassedPending { execute_at_round } => execute_at_round,
        _ => panic!("Expected PassedPending"),
    };
    state.tick_governance(execute_at);
    assert_eq!(state.governance_params().min_fee_sats, 20_000);

    // Second proposal: change observer_reward_percent
    let proposer2 = SecretKey::generate();
    fund_and_seat_council(&mut state, &proposer2, CouncilSeatCategory::Academic);

    let round2 = execute_at + 10;
    let p2_tx = make_proposal_tx(
        &proposer2, 1, "Change Observer Reward", "Set to 30%",
        ProposalType::ParameterChange { param: "observer_reward_percent".to_string(), new_value: "30".to_string() },
        10_000, 0,
    );
    state.apply_create_proposal(&p2_tx, round2).unwrap();

    // Vote to pass proposal 1 (id=1) — council member
    let voter = SecretKey::generate();
    fund_and_seat_council(&mut state, &voter, CouncilSeatCategory::Community);

    let v = make_vote_tx(&voter, 1, true, 10_000, 0);
    state.apply_vote(&v, round2 + 50).unwrap();

    let voting_ends2 = state.proposal(1).unwrap().voting_ends;
    state.tick_governance(voting_ends2 + 1);
    let execute_at2 = match state.proposal(1).unwrap().status {
        ultradag_coin::governance::ProposalStatus::PassedPending { execute_at_round } => execute_at_round,
        _ => panic!("Expected PassedPending for proposal 1"),
    };
    state.tick_governance(execute_at2);

    // Both changes should be in effect
    assert_eq!(state.governance_params().min_fee_sats, 20_000);
    assert_eq!(state.governance_params().observer_reward_percent, 30);
}

#[test]
fn test_dao_hibernation_blocks_parameter_change() {
    // With fewer than MIN_DAO_VALIDATORS, ParameterChange proposals stay in PassedPending
    let mut state = StateEngine::new_with_genesis();

    // Create proposer as council member
    let proposer = SecretKey::generate();
    fund_and_seat_council(&mut state, &proposer, CouncilSeatCategory::Technical);

    // Only 0 stakers — DAO is hibernating
    state.recalculate_active_set();
    assert!(!state.dao_is_active());

    let proposal_tx = make_proposal_tx(
        &proposer, 0, "Change Fee", "Lower min fee",
        ProposalType::ParameterChange { param: "min_fee_sats".to_string(), new_value: "50000".to_string() },
        10_000, 0,
    );
    state.apply_create_proposal(&proposal_tx, 100).unwrap();

    // Create voter (council member)
    let voter = SecretKey::generate();
    fund_and_seat_council(&mut state, &voter, CouncilSeatCategory::Business);

    let v = make_vote_tx(&voter, 0, true, 10_000, 0);
    state.apply_vote(&v, 150).unwrap();

    let voting_ends = state.proposal(0).unwrap().voting_ends;
    state.tick_governance(voting_ends + 1);
    let execute_at = match state.proposal(0).unwrap().status {
        ultradag_coin::governance::ProposalStatus::PassedPending { execute_at_round } => execute_at_round,
        _ => panic!("Expected PassedPending"),
    };

    // Try to execute — should stay in PassedPending (DAO hibernating)
    state.tick_governance(execute_at);
    assert!(matches!(
        state.proposal(0).unwrap().status,
        ultradag_coin::governance::ProposalStatus::PassedPending { .. }
    ));
    // Param should be unchanged
    assert_eq!(state.governance_params().min_fee_sats, ultradag_coin::constants::MIN_FEE_SATS);

    // Now add enough validators to activate the DAO
    for _ in 0..8 {
        stake_filler(&mut state);
    }
    state.recalculate_active_set();
    assert!(state.dao_is_active());

    // Tick again — now the proposal should execute
    state.tick_governance(execute_at);
    assert_eq!(state.proposal(0).unwrap().status, ultradag_coin::governance::ProposalStatus::Executed);
    assert_eq!(state.governance_params().min_fee_sats, 50_000);
}

#[test]
fn test_dao_hibernation_allows_text_proposals() {
    // TextProposals execute regardless of DAO activation status
    let mut state = StateEngine::new_with_genesis();

    // Create proposer as council member
    let proposer = SecretKey::generate();
    fund_and_seat_council(&mut state, &proposer, CouncilSeatCategory::Technical);

    // Only 0 stakers — DAO is hibernating
    state.recalculate_active_set();
    assert!(!state.dao_is_active());

    let proposal_tx = make_proposal_tx(
        &proposer, 0, "Signal Support", "Community signal proposal",
        ProposalType::TextProposal, 10_000, 0,
    );
    state.apply_create_proposal(&proposal_tx, 100).unwrap();

    // Vote to pass (council member)
    let voter = SecretKey::generate();
    fund_and_seat_council(&mut state, &voter, CouncilSeatCategory::Business);
    let v = make_vote_tx(&voter, 0, true, 10_000, 0);
    state.apply_vote(&v, 150).unwrap();

    let voting_ends = state.proposal(0).unwrap().voting_ends;
    state.tick_governance(voting_ends + 1);
    let execute_at = match state.proposal(0).unwrap().status {
        ultradag_coin::governance::ProposalStatus::PassedPending { execute_at_round } => execute_at_round,
        _ => panic!("Expected PassedPending"),
    };

    // TextProposal should execute even with DAO hibernating
    state.tick_governance(execute_at);
    assert_eq!(state.proposal(0).unwrap().status, ultradag_coin::governance::ProposalStatus::Executed);
}

#[test]
fn test_non_council_member_cannot_propose() {
    let mut state = StateEngine::new_with_genesis();

    // Create address that is NOT a council member
    let non_member = SecretKey::generate();
    state.faucet_credit(&non_member.address(), 1_000_000_000).unwrap();

    // Try to create proposal (should fail)
    let proposal_tx = make_proposal_tx(
        &non_member,
        0,
        "Invalid Proposal",
        "Should fail — not a council member",
        ProposalType::TextProposal,
        10_000,
        0,
    );

    let result = state.apply_create_proposal(&proposal_tx, 100);
    assert!(result.is_err(), "Non-council member should not be able to create proposals");
}
