use crate::address::Address;
use crate::constants::*;
use serde::{Deserialize, Serialize};

use super::GovernanceParams;

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

    /// Check if proposal passed with governance-adjustable thresholds.
    /// Uses u128 for intermediate calculations to prevent overflow with large staked values.
    pub fn has_passed_with_params(&self, total_staked: u64, params: &GovernanceParams) -> bool {
        // Use u128 to prevent overflow: total_staked * quorum_numerator can exceed u64
        let quorum = ((total_staked as u128 * params.quorum_numerator as u128
            + GOVERNANCE_QUORUM_DENOMINATOR as u128 - 1)
            / GOVERNANCE_QUORUM_DENOMINATOR as u128) as u64;
        let total = self.total_votes();
        if total < quorum {
            return false;
        }
        let threshold = ((total as u128 * params.approval_numerator as u128
            + GOVERNANCE_APPROVAL_DENOMINATOR as u128 - 1)
            / GOVERNANCE_APPROVAL_DENOMINATOR as u128) as u64;
        self.votes_for >= threshold
    }

    /// Check if proposal passed using default constants (for tests and backward compat).
    pub fn has_passed(&self, total_staked: u64) -> bool {
        self.has_passed_with_params(total_staked, &GovernanceParams::default())
    }
}
