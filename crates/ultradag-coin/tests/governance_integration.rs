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

    // Proposal should be Failed (unknown param), not Executed
    assert!(matches!(state.proposal(0).unwrap().status, ultradag_coin::governance::ProposalStatus::Failed { .. }));
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

    // Proposal should be Failed (validation bounds), not Executed
    assert!(matches!(state.proposal(0).unwrap().status, ultradag_coin::governance::ProposalStatus::Failed { .. }));
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

// --- CouncilMembership Proposal Lifecycle Tests ---

use ultradag_coin::governance::CouncilAction;
use ultradag_coin::Address;

/// Helper: set up a state with a passing CouncilMembership proposal.
/// Returns (state, voting_ends) with proposal at id=0 in Active status.
fn setup_passing_council_membership_proposal(
    action: CouncilAction,
    target_address: Address,
    category: CouncilSeatCategory,
) -> (StateEngine, u64) {
    let mut state = StateEngine::new_with_genesis();

    // Create proposer as council member
    let proposer = SecretKey::generate();
    fund_and_seat_council(&mut state, &proposer, CouncilSeatCategory::Technical);

    let proposal_tx = make_proposal_tx(
        &proposer, 0, "Council Membership", "Add or remove council member",
        ProposalType::CouncilMembership {
            action,
            address: target_address,
            category,
        },
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

    let voting_ends = state.proposal(0).unwrap().voting_ends;
    (state, voting_ends)
}

/// Helper: advance a proposal from Active -> PassedPending -> Executed via tick_governance.
fn execute_proposal(state: &mut StateEngine, proposal_id: u64, voting_ends: u64) {
    // Tick past voting end — proposal transitions to PassedPending
    state.tick_governance(voting_ends + 1);
    let execute_at = match state.proposal(proposal_id).unwrap().status {
        ultradag_coin::governance::ProposalStatus::PassedPending { execute_at_round } => execute_at_round,
        ref s => panic!("Expected PassedPending, got {:?}", s),
    };
    // Tick at execution round — proposal transitions to Executed
    state.tick_governance(execute_at);
}

#[test]
fn test_council_membership_add_via_proposal() {
    // Create a new address to be added as council member
    let new_member = SecretKey::generate();
    let new_addr = new_member.address();

    let (mut state, voting_ends) = setup_passing_council_membership_proposal(
        CouncilAction::Add,
        new_addr,
        CouncilSeatCategory::Academic,
    );

    // Before execution, new_addr should NOT be a council member
    assert!(!state.is_council_member(&new_addr), "Should not be council member before execution");

    // Execute the proposal
    execute_proposal(&mut state, 0, voting_ends);

    // Verify proposal is Executed
    assert_eq!(
        state.proposal(0).unwrap().status,
        ultradag_coin::governance::ProposalStatus::Executed,
    );

    // Verify member was added
    assert!(state.is_council_member(&new_addr), "Should be council member after execution");
    assert_eq!(
        state.council_seat_category(&new_addr),
        Some(CouncilSeatCategory::Academic),
    );
}

#[test]
fn test_council_membership_remove_via_proposal() {
    // Create member to be removed
    let target = SecretKey::generate();
    let target_addr = target.address();

    let (mut state, voting_ends) = setup_passing_council_membership_proposal(
        CouncilAction::Remove,
        target_addr,
        CouncilSeatCategory::Academic, // category in proposal (used for Add, ignored for Remove)
    );

    // Manually add the target as a council member before execution
    state.add_council_member(target_addr, CouncilSeatCategory::Academic).unwrap();
    assert!(state.is_council_member(&target_addr), "Should be council member before removal");

    // Execute the proposal
    execute_proposal(&mut state, 0, voting_ends);

    // Verify proposal is Executed
    assert_eq!(
        state.proposal(0).unwrap().status,
        ultradag_coin::governance::ProposalStatus::Executed,
    );

    // Verify member was removed
    assert!(!state.is_council_member(&target_addr), "Should not be council member after removal");
}

#[test]
fn test_council_membership_add_category_full() {
    // Fill all Technical(7) seats, then propose an 8th
    let eighth_member = SecretKey::generate();
    let eighth_addr = eighth_member.address();

    let (mut state, voting_ends) = setup_passing_council_membership_proposal(
        CouncilAction::Add,
        eighth_addr,
        CouncilSeatCategory::Technical,
    );

    // The proposer already occupies 1 Technical seat (from setup_passing_council_membership_proposal).
    // Fill the remaining 6 Technical seats (total = 7, which is max).
    for _ in 0..6 {
        let filler = SecretKey::generate();
        state.add_council_member(filler.address(), CouncilSeatCategory::Technical).unwrap();
    }

    // Verify Technical is full (7 seats)
    let tech_count = state.council_members()
        .filter(|(_, cat)| **cat == CouncilSeatCategory::Technical)
        .count();
    assert_eq!(tech_count, 7, "All 7 Technical seats should be filled");

    // Execute the proposal — should transition to Failed (category full)
    execute_proposal(&mut state, 0, voting_ends);

    assert!(
        matches!(state.proposal(0).unwrap().status, ultradag_coin::governance::ProposalStatus::Failed { .. }),
        "Should be Failed, got {:?}", state.proposal(0).unwrap().status,
    );

    // The 8th member should NOT have been added (category full)
    assert!(!state.is_council_member(&eighth_addr), "8th Technical member should not be added (category full)");
}

#[test]
fn test_council_membership_add_at_capacity() {
    // Fill all 21 council seats, then propose a 22nd
    let extra_member = SecretKey::generate();
    let extra_addr = extra_member.address();

    let (mut state, voting_ends) = setup_passing_council_membership_proposal(
        CouncilAction::Add,
        extra_addr,
        CouncilSeatCategory::Community,
    );

    // setup_passing_council_membership_proposal already created 3 members:
    // 1 Technical (proposer), 1 Business (voter1), 1 Legal (voter2)
    // new_with_genesis() also added 1 Foundation member (dev address) = 4 total
    // Fill remaining seats to reach 21
    // Technical: 1 filled, max 7, add 6 more
    for _ in 0..6 {
        let sk = SecretKey::generate();
        state.add_council_member(sk.address(), CouncilSeatCategory::Technical).unwrap();
    }
    // Business: 1 filled, max 4, add 3 more
    for _ in 0..3 {
        let sk = SecretKey::generate();
        state.add_council_member(sk.address(), CouncilSeatCategory::Business).unwrap();
    }
    // Legal: 1 filled, max 3, add 2 more
    for _ in 0..2 {
        let sk = SecretKey::generate();
        state.add_council_member(sk.address(), CouncilSeatCategory::Legal).unwrap();
    }
    // Academic: 0 filled, max 3, add 3
    for _ in 0..3 {
        let sk = SecretKey::generate();
        state.add_council_member(sk.address(), CouncilSeatCategory::Academic).unwrap();
    }
    // Community: 0 filled, max 2, add 2
    for _ in 0..2 {
        let sk = SecretKey::generate();
        state.add_council_member(sk.address(), CouncilSeatCategory::Community).unwrap();
    }
    // Foundation: 1 filled (genesis dev addr), max 2, add 1 more
    for _ in 0..1 {
        let sk = SecretKey::generate();
        state.add_council_member(sk.address(), CouncilSeatCategory::Foundation).unwrap();
    }

    assert_eq!(state.council_member_count(), 21, "Council should be at full capacity");

    // Execute the proposal — should transition to Failed (council at capacity)
    execute_proposal(&mut state, 0, voting_ends);

    assert!(
        matches!(state.proposal(0).unwrap().status, ultradag_coin::governance::ProposalStatus::Failed { .. }),
        "Should be Failed, got {:?}", state.proposal(0).unwrap().status,
    );

    // The extra member should NOT have been added
    assert!(!state.is_council_member(&extra_addr), "22nd member should not be added (council at capacity)");
    assert_eq!(state.council_member_count(), 21, "Council should still be at 21");
}

#[test]
fn test_removed_member_loses_governance_rights() {
    let mut state = StateEngine::new_with_genesis();

    // Create and seat a member
    let member = SecretKey::generate();
    fund_and_seat_council(&mut state, &member, CouncilSeatCategory::Technical);
    assert!(state.is_council_member(&member.address()));

    // Remove the member directly (simulating post-execution state)
    state.remove_council_member(&member.address());
    assert!(!state.is_council_member(&member.address()));

    // Verify removed member cannot create proposals
    let proposal_tx = make_proposal_tx(
        &member, 0, "Proposal After Removal", "Should fail",
        ProposalType::TextProposal, 10_000, 0,
    );
    let result = state.apply_create_proposal(&proposal_tx, 100);
    assert!(result.is_err(), "Removed member should not be able to create proposals");

    // Set up a proposal from another council member so the removed member can try to vote
    let active_member = SecretKey::generate();
    fund_and_seat_council(&mut state, &active_member, CouncilSeatCategory::Business);

    let proposal_tx2 = make_proposal_tx(
        &active_member, 0, "Valid Proposal", "From active member",
        ProposalType::TextProposal, 10_000, 0,
    );
    state.apply_create_proposal(&proposal_tx2, 200).unwrap();

    // Verify removed member cannot vote
    let vote_tx = make_vote_tx(&member, 0, true, 10_000, 0);
    let result = state.apply_vote(&vote_tx, 250);
    assert!(result.is_err(), "Removed member should not be able to vote");
}

#[test]
fn test_council_membership_new_member_gets_emission() {
    let mut state = StateEngine::new_with_genesis();

    // Genesis already has 1 Foundation council member (dev address).
    // Add 2 more council members = 3 total.
    let member1 = SecretKey::generate();
    let member2 = SecretKey::generate();
    fund_and_seat_council(&mut state, &member1, CouncilSeatCategory::Technical);
    fund_and_seat_council(&mut state, &member2, CouncilSeatCategory::Business);

    // Emission with 3 members (1 genesis + 2 added)
    // Use validator_count=1 to see per-round totals
    let (per_member_before, total_before) = state.compute_council_emission(100, 1);
    assert!(per_member_before > 0, "Should have non-zero emission");
    assert_eq!(total_before, per_member_before * 3, "Total should be per_member * 3");

    // Add a 4th member
    let member3 = SecretKey::generate();
    state.add_council_member(member3.address(), CouncilSeatCategory::Legal).unwrap();

    // Emission with 4 members — per_member should decrease, total should stay roughly the same
    let (per_member_after, total_after) = state.compute_council_emission(100, 1);
    assert!(per_member_after > 0, "Should have non-zero emission");
    assert!(per_member_after < per_member_before, "Per-member emission should decrease with more members");
    assert_eq!(total_after, per_member_after * 4, "Total should be per_member * 4");
}

#[test]
fn test_council_membership_executes_regardless_of_dao_hibernation() {
    // CouncilMembership proposals should execute even with <8 active validators (DAO hibernating)
    let new_member = SecretKey::generate();
    let new_addr = new_member.address();

    let (mut state, voting_ends) = setup_passing_council_membership_proposal(
        CouncilAction::Add,
        new_addr,
        CouncilSeatCategory::Academic,
    );

    // Ensure DAO is NOT active (no stakers)
    state.recalculate_active_set();
    assert!(!state.dao_is_active(), "DAO should be hibernating with 0 stakers");

    // Execute the proposal — CouncilMembership should still execute
    execute_proposal(&mut state, 0, voting_ends);

    assert_eq!(
        state.proposal(0).unwrap().status,
        ultradag_coin::governance::ProposalStatus::Executed,
        "CouncilMembership proposal should execute regardless of DAO hibernation",
    );

    // Verify member was actually added
    assert!(state.is_council_member(&new_addr), "Member should be added even with DAO hibernating");
}

#[test]
fn test_non_council_cannot_propose_membership() {
    let mut state = StateEngine::new_with_genesis();

    // Create address that is NOT a council member, but has funds
    let non_member = SecretKey::generate();
    state.faucet_credit(&non_member.address(), 1_000_000_000).unwrap();

    let target = SecretKey::generate();

    // Try to create CouncilMembership proposal (should fail — not a council member)
    let proposal_tx = make_proposal_tx(
        &non_member, 0, "Add My Friend", "Should be rejected",
        ProposalType::CouncilMembership {
            action: CouncilAction::Add,
            address: target.address(),
            category: CouncilSeatCategory::Community,
        },
        10_000, 0,
    );

    let result = state.apply_create_proposal(&proposal_tx, 100);
    assert!(result.is_err(), "Non-council member should not be able to propose council membership changes");
}

#[test]
fn test_council_membership_quorum_uses_snapshot() {
    let mut state = StateEngine::new_with_genesis();

    // Genesis already has 1 Foundation council member (dev address).
    // Create 5 more council members = 6 total.
    let proposer = SecretKey::generate();
    fund_and_seat_council(&mut state, &proposer, CouncilSeatCategory::Technical);

    let voter1 = SecretKey::generate();
    let voter2 = SecretKey::generate();
    let member4 = SecretKey::generate();
    let member5 = SecretKey::generate();

    fund_and_seat_council(&mut state, &voter1, CouncilSeatCategory::Business);
    fund_and_seat_council(&mut state, &voter2, CouncilSeatCategory::Legal);
    fund_and_seat_council(&mut state, &member4, CouncilSeatCategory::Academic);
    fund_and_seat_council(&mut state, &member5, CouncilSeatCategory::Community);

    assert_eq!(state.council_member_count(), 6);

    // Create proposal — snapshot_total_stake should capture council size = 6
    let new_member = SecretKey::generate();
    let proposal_tx = make_proposal_tx(
        &proposer, 0, "Add Member", "Test quorum snapshot",
        ProposalType::CouncilMembership {
            action: CouncilAction::Add,
            address: new_member.address(),
            category: CouncilSeatCategory::Foundation,
        },
        10_000, 0,
    );
    state.apply_create_proposal(&proposal_tx, 100).unwrap();

    // Verify snapshot captured council size of 6 (1 genesis + 5 added)
    let proposal = state.proposal(0).unwrap();
    assert_eq!(proposal.snapshot_total_stake, 6, "Snapshot should capture council size at proposal creation");

    // Remove 2 council members AFTER proposal creation
    state.remove_council_member(&member4.address());
    state.remove_council_member(&member5.address());
    assert_eq!(state.council_member_count(), 4, "Should have 4 members after removal");

    // Vote YES from voter1 (1 vote out of snapshot=6)
    // Quorum = ceil(6 * 10 / 100) = 1, so 1 vote meets quorum
    // Approval = 1/1 = 100% >= 66%
    let v1 = make_vote_tx(&voter1, 0, true, 10_000, 0);
    state.apply_vote(&v1, 150).unwrap();

    let voting_ends = state.proposal(0).unwrap().voting_ends;

    // Tick past voting — the quorum should use snapshot of 6, not current 4
    state.tick_governance(voting_ends + 1);

    let proposal = state.proposal(0).unwrap();
    // With snapshot=6, quorum=ceil(6*10/100)=1, we have 1 vote, so it passes
    assert!(
        matches!(proposal.status, ultradag_coin::governance::ProposalStatus::PassedPending { .. }),
        "Proposal should pass using snapshot quorum of 6 (not current 4). Got: {:?}",
        proposal.status,
    );

    // Execute the proposal
    let execute_at = match proposal.status {
        ultradag_coin::governance::ProposalStatus::PassedPending { execute_at_round } => execute_at_round,
        _ => panic!("Expected PassedPending"),
    };
    state.tick_governance(execute_at);

    assert_eq!(
        state.proposal(0).unwrap().status,
        ultradag_coin::governance::ProposalStatus::Executed,
    );
    assert!(state.is_council_member(&new_member.address()), "New member should be added after execution");
}

#[test]
fn test_self_nomination_allowed_for_non_council() {
    use ultradag_coin::governance::CouncilAction;

    let mut state = StateEngine::new_with_genesis();
    let outsider = SecretKey::generate();

    // Give the outsider some UDAG for the fee
    state.credit(&outsider.address(), 1_000_000_000);

    // Outsider is NOT a council member
    assert!(!state.is_council_member(&outsider.address()));

    // Self-nomination: outsider proposes to add themselves
    let self_nom_tx = make_proposal_tx(
        &outsider,
        state.next_proposal_id(),
        "Self-nomination: Developer",
        "I'd like to join the Technical council seat",
        ProposalType::CouncilMembership {
            action: CouncilAction::Add,
            address: outsider.address(),
            category: CouncilSeatCategory::Technical,
        },
        100_000,
        0,
    );
    let result = state.apply_create_proposal(&self_nom_tx, 100);
    assert!(result.is_ok(), "Self-nomination should be allowed: {:?}", result);

    // Verify proposal was created
    let proposal = state.proposal(0).unwrap();
    assert_eq!(proposal.proposer, outsider.address());
}

#[test]
fn test_non_council_non_self_nomination_rejected() {
    use ultradag_coin::governance::CouncilAction;

    let mut state = StateEngine::new_with_genesis();
    let outsider = SecretKey::generate();
    let other = SecretKey::generate();

    state.credit(&outsider.address(), 1_000_000_000);

    // Outsider tries to nominate someone else — should be rejected
    let tx = make_proposal_tx(
        &outsider,
        state.next_proposal_id(),
        "Nominate friend",
        "Adding a friend",
        ProposalType::CouncilMembership {
            action: CouncilAction::Add,
            address: other.address(),
            category: CouncilSeatCategory::Technical,
        },
        100_000,
        0,
    );
    let result = state.apply_create_proposal(&tx, 100);
    assert!(result.is_err(), "Non-council nominating someone else should be rejected");

    // Outsider tries to create a text proposal — should be rejected
    let tx2 = make_proposal_tx(
        &outsider,
        state.next_proposal_id(),
        "Some text proposal",
        "description",
        ProposalType::TextProposal,
        100_000,
        1,
    );
    let result2 = state.apply_create_proposal(&tx2, 100);
    assert!(result2.is_err(), "Non-council creating text proposal should be rejected");
}
