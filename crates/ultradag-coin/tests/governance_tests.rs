use ultradag_coin::{StateEngine, SecretKey, Address};
use ultradag_coin::governance::{Proposal, ProposalType, ProposalStatus};

#[test]
fn test_proposal_creation() {
    let proposer = SecretKey::generate();

    let proposal = Proposal {
        id: 1,
        proposer: proposer.address(),
        title: "Reduce Block Time".to_string(),
        proposal_type: ProposalType::ParameterChange {
            param: "block_time".to_string(),
            new_value: "5".to_string(),
        },
        description: "Reduce block time to 5 seconds".to_string(),
        voting_starts: 100,
        voting_ends: 200,
        status: ProposalStatus::Active,
        votes_for: 0,
        votes_against: 0,
        snapshot_total_stake: 0,
    };

    assert_eq!(proposal.id, 1);
    assert_eq!(proposal.status, ProposalStatus::Active);
}

#[test]
fn test_proposal_voting_period() {
    let proposer = SecretKey::generate();

    let proposal = Proposal {
        id: 1,
        proposer: proposer.address(),
        title: "Increase TX Size".to_string(),
        proposal_type: ProposalType::ParameterChange {
            param: "max_tx_size".to_string(),
            new_value: "10000".to_string(),
        },
        description: "Increase max transaction size".to_string(),
        voting_starts: 100,
        voting_ends: 200,
        status: ProposalStatus::Active,
        votes_for: 0,
        votes_against: 0,
        snapshot_total_stake: 0,
    };

    assert!(proposal.voting_ends > proposal.voting_starts);
    assert_eq!(proposal.voting_ends - proposal.voting_starts, 100);
}

#[test]
fn test_proposal_types() {
    let proposer = Address([1u8; 32]);

    let param_change = Proposal {
        id: 1,
        proposer,
        title: "Change Fee".to_string(),
        proposal_type: ProposalType::ParameterChange {
            param: "fee".to_string(),
            new_value: "200".to_string(),
        },
        description: "Change fee".to_string(),
        voting_starts: 0,
        voting_ends: 100,
        status: ProposalStatus::Active,
        votes_for: 0,
        votes_against: 0,
        snapshot_total_stake: 0,
    };

    let text_proposal = Proposal {
        id: 2,
        proposer,
        title: "Community Initiative".to_string(),
        proposal_type: ProposalType::TextProposal,
        description: "Approve community initiative".to_string(),
        voting_starts: 0,
        voting_ends: 100,
        status: ProposalStatus::Active,
        votes_for: 0,
        votes_against: 0,
        snapshot_total_stake: 0,
    };

    match param_change.proposal_type {
        ProposalType::ParameterChange { .. } => {},
        _ => panic!("Wrong type"),
    }

    match text_proposal.proposal_type {
        ProposalType::TextProposal => {},
        _ => panic!("Wrong type"),
    }
}

#[test]
fn test_proposal_status_transitions() {
    let proposer = Address([1u8; 32]);

    let mut proposal = Proposal {
        id: 1,
        proposer,
        title: "Test Proposal".to_string(),
        proposal_type: ProposalType::ParameterChange {
            param: "test".to_string(),
            new_value: "value".to_string(),
        },
        description: "Test".to_string(),
        voting_starts: 0,
        voting_ends: 100,
        status: ProposalStatus::Active,
        votes_for: 0,
        votes_against: 0,
        snapshot_total_stake: 0,
    };

    assert_eq!(proposal.status, ProposalStatus::Active);

    proposal.status = ProposalStatus::PassedPending { execute_at_round: 300 };
    assert!(matches!(proposal.status, ProposalStatus::PassedPending { .. }));

    proposal.status = ProposalStatus::Rejected;
    assert_eq!(proposal.status, ProposalStatus::Rejected);

    proposal.status = ProposalStatus::Executed;
    assert_eq!(proposal.status, ProposalStatus::Executed);
}

#[test]
fn test_vote_counting() {
    let proposer = Address([1u8; 32]);

    let mut proposal = Proposal {
        id: 1,
        proposer,
        title: "Test Proposal".to_string(),
        proposal_type: ProposalType::ParameterChange {
            param: "test".to_string(),
            new_value: "value".to_string(),
        },
        description: "Test".to_string(),
        voting_starts: 0,
        voting_ends: 100,
        status: ProposalStatus::Active,
        votes_for: 0,
        votes_against: 0,
        snapshot_total_stake: 0,
    };

    proposal.votes_for = 10;
    proposal.votes_against = 5;

    assert_eq!(proposal.votes_for, 10);
    assert_eq!(proposal.votes_against, 5);
    assert!(proposal.votes_for > proposal.votes_against);
}

#[test]
fn test_proposal_quorum_calculation() {
    let proposer = Address([1u8; 32]);

    let proposal = Proposal {
        id: 1,
        proposer,
        title: "Test Proposal".to_string(),
        proposal_type: ProposalType::ParameterChange {
            param: "test".to_string(),
            new_value: "value".to_string(),
        },
        description: "Test".to_string(),
        voting_starts: 0,
        voting_ends: 100,
        status: ProposalStatus::Active,
        votes_for: 7,
        votes_against: 3,
        snapshot_total_stake: 0,
    };

    let total_votes = proposal.votes_for + proposal.votes_against;
    assert_eq!(total_votes, 10);

    let approval_rate = (proposal.votes_for as f64) / (total_votes as f64);
    assert!(approval_rate > 0.5);
}

#[test]
fn test_state_engine_governance_integration() {
    let state = StateEngine::new();

    assert_eq!(state.current_epoch(), u64::MAX); // sentinel: epoch never initialized
}

#[test]
fn test_proposal_id_uniqueness() {
    let proposer = Address([1u8; 32]);

    let proposal1 = Proposal {
        id: 1,
        proposer,
        title: "First Proposal".to_string(),
        proposal_type: ProposalType::ParameterChange {
            param: "a".to_string(),
            new_value: "1".to_string(),
        },
        description: "First".to_string(),
        voting_starts: 0,
        voting_ends: 100,
        status: ProposalStatus::Active,
        votes_for: 0,
        votes_against: 0,
        snapshot_total_stake: 0,
    };

    let proposal2 = Proposal {
        id: 2,
        proposer,
        title: "Second Proposal".to_string(),
        proposal_type: ProposalType::ParameterChange {
            param: "b".to_string(),
            new_value: "2".to_string(),
        },
        description: "Second".to_string(),
        voting_starts: 0,
        voting_ends: 100,
        status: ProposalStatus::Active,
        votes_for: 0,
        votes_against: 0,
        snapshot_total_stake: 0,
    };

    assert_ne!(proposal1.id, proposal2.id);
}

#[test]
fn test_proposal_has_passed_with_quorum() {
    let proposer = Address([1u8; 32]);

    let proposal = Proposal {
        id: 1,
        proposer,
        title: "Test Quorum".to_string(),
        proposal_type: ProposalType::ParameterChange {
            param: "test".to_string(),
            new_value: "value".to_string(),
        },
        description: "Test quorum logic".to_string(),
        voting_starts: 0,
        voting_ends: 100,
        status: ProposalStatus::Active,
        votes_for: 70,
        votes_against: 30,
        snapshot_total_stake: 0,
    };

    let total_staked = 100;
    assert!(proposal.has_passed(total_staked));
}

#[test]
fn test_proposal_fails_without_quorum() {
    let proposer = Address([1u8; 32]);

    let proposal = Proposal {
        id: 1,
        proposer,
        title: "Test No Quorum".to_string(),
        proposal_type: ProposalType::ParameterChange {
            param: "test".to_string(),
            new_value: "value".to_string(),
        },
        description: "Test quorum failure".to_string(),
        voting_starts: 0,
        voting_ends: 100,
        status: ProposalStatus::Active,
        votes_for: 5,
        votes_against: 2,
        snapshot_total_stake: 0,
    };

    let total_staked = 1000;
    assert!(!proposal.has_passed(total_staked));
}
