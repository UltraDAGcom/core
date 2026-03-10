pub mod transactions;

use crate::constants::*;
use crate::address::Address;
use serde::{Deserialize, Serialize};

pub use transactions::{CreateProposalTx, VoteTx};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProposalType {
    TextProposal,
    ParameterChange { param: String, new_value: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProposalStatus {
    Active,
    PassedPending { execute_at_round: u64 },
    Executed,
    Rejected,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Proposal {
    pub id: u64,
    pub proposer: Address,
    pub title: String,
    pub description: String,
    pub proposal_type: ProposalType,
    pub voting_starts: u64,
    pub voting_ends: u64,
    pub votes_for: u64,
    pub votes_against: u64,
    pub status: ProposalStatus,
}

impl Proposal {
    pub fn is_voting_open(&self, current_round: u64) -> bool {
        matches!(self.status, ProposalStatus::Active)
            && current_round >= self.voting_starts
            && current_round <= self.voting_ends
    }

    pub fn total_votes(&self) -> u64 {
        self.votes_for.saturating_add(self.votes_against)
    }

    pub fn has_passed(&self, total_staked: u64) -> bool {
        // Ceiling division for quorum: total_votes must be >= ceil(total_staked * 10 / 100)
        let quorum = total_staked
            .saturating_mul(GOVERNANCE_QUORUM_NUMERATOR)
            .saturating_add(GOVERNANCE_QUORUM_DENOMINATOR - 1)
            / GOVERNANCE_QUORUM_DENOMINATOR;
        let total = self.total_votes();
        if total < quorum {
            return false;
        }
        // Ceiling division for approval: votes_for must be >= ceil(total_votes * 66 / 100)
        let threshold = total
            .saturating_mul(GOVERNANCE_APPROVAL_NUMERATOR)
            .saturating_add(GOVERNANCE_APPROVAL_DENOMINATOR - 1)
            / GOVERNANCE_APPROVAL_DENOMINATOR;
        self.votes_for >= threshold
    }
}
