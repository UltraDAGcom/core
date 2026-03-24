use crate::address::Address;
use crate::constants::*;
use crate::governance::council::{CouncilAction, CouncilSeatCategory};
use serde::{Deserialize, Serialize};

use super::GovernanceParams;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProposalType {
    TextProposal,
    ParameterChange { param: String, new_value: String },
    /// Add or remove a council member. Executed on-chain when proposal passes.
    CouncilMembership {
        action: CouncilAction,
        address: Address,
        category: CouncilSeatCategory,
    },
    /// Spend from the DAO treasury. Council votes to send funds to a recipient.
    TreasurySpend {
        recipient: Address,
        amount: u64,
    },
    /// Emergency refund of a bridge deposit before the retention period expires.
    /// The council can authorize returning locked funds to the original sender
    /// if the Arbitrum-side claim failed or the bridge is stuck.
    BridgeRefund {
        nonce: u64,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProposalStatus {
    Active,
    PassedPending { execute_at_round: u64 },
    Executed,
    /// Proposal passed governance vote but execution failed (e.g., insufficient treasury balance).
    Failed { reason: String },
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
    /// Total votable stake at proposal creation time, used as quorum denominator.
    /// Prevents quorum manipulation via coordinated unstaking during voting period.
    #[serde(default)]
    pub snapshot_total_stake: u64,
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
        // Empty council/zero quorum denominator: proposals cannot pass with 0 eligible voters.
        // Prevents auto-pass when all council members are removed (quorum=0, 0>=0 would pass).
        if total_staked == 0 {
            return false;
        }
        // Use u128 to prevent overflow: total_staked * quorum_numerator can exceed u64
        let quorum = (total_staked as u128 * params.quorum_numerator as u128)
            .div_ceil(GOVERNANCE_QUORUM_DENOMINATOR as u128) as u64;
        let total = self.total_votes();
        if total < quorum {
            return false;
        }
        let threshold = (total as u128 * params.approval_numerator as u128)
            .div_ceil(GOVERNANCE_APPROVAL_DENOMINATOR as u128) as u64;
        self.votes_for >= threshold
    }

    /// Check if proposal passed using default constants (for tests and backward compat).
    pub fn has_passed(&self, total_staked: u64) -> bool {
        self.has_passed_with_params(total_staked, &GovernanceParams::default())
    }
}
