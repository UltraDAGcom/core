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

/// Parameters that can be changed via governance proposals.
/// Each field corresponds to a `param` string in ParameterChange proposals.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GovernanceParams {
    /// Minimum transaction fee in sats (param: "min_fee_sats")
    pub min_fee_sats: u64,
    /// Minimum stake to submit a governance proposal (param: "min_stake_to_propose")
    pub min_stake_to_propose: u64,
    /// Quorum numerator — fraction of total stake that must vote (param: "quorum_numerator")
    pub quorum_numerator: u64,
    /// Approval numerator — fraction of votes that must be "for" (param: "approval_numerator")
    pub approval_numerator: u64,
    /// Voting period in rounds (param: "voting_period_rounds")
    pub voting_period_rounds: u64,
    /// Execution delay in rounds after passing (param: "execution_delay_rounds")
    pub execution_delay_rounds: u64,
    /// Maximum simultaneous active proposals (param: "max_active_proposals")
    pub max_active_proposals: u64,
    /// Observer reward percentage (param: "observer_reward_percent")
    pub observer_reward_percent: u64,
}

impl Default for GovernanceParams {
    fn default() -> Self {
        Self {
            min_fee_sats: MIN_FEE_SATS,
            min_stake_to_propose: MIN_STAKE_TO_PROPOSE,
            quorum_numerator: GOVERNANCE_QUORUM_NUMERATOR,
            approval_numerator: GOVERNANCE_APPROVAL_NUMERATOR,
            voting_period_rounds: GOVERNANCE_VOTING_PERIOD_ROUNDS,
            execution_delay_rounds: GOVERNANCE_EXECUTION_DELAY_ROUNDS,
            max_active_proposals: MAX_ACTIVE_PROPOSALS as u64,
            observer_reward_percent: OBSERVER_REWARD_PERCENT,
        }
    }
}

impl GovernanceParams {
    /// Apply a parameter change. Returns Err if param name is unknown or value is invalid.
    pub fn apply_change(&mut self, param: &str, new_value: &str) -> Result<(), String> {
        let value: u64 = new_value.parse::<u64>()
            .map_err(|_| format!("Invalid value '{}': must be a positive integer", new_value))?;

        match param {
            "min_fee_sats" => {
                if value == 0 {
                    return Err("min_fee_sats cannot be zero".to_string());
                }
                self.min_fee_sats = value;
            }
            "min_stake_to_propose" => {
                if value == 0 {
                    return Err("min_stake_to_propose cannot be zero".to_string());
                }
                self.min_stake_to_propose = value;
            }
            "quorum_numerator" => {
                if value == 0 || value > 100 {
                    return Err("quorum_numerator must be 1-100".to_string());
                }
                self.quorum_numerator = value;
            }
            "approval_numerator" => {
                if value < 51 || value > 100 {
                    return Err("approval_numerator must be 51-100".to_string());
                }
                self.approval_numerator = value;
            }
            "voting_period_rounds" => {
                if value < 100 {
                    return Err("voting_period_rounds must be >= 100".to_string());
                }
                self.voting_period_rounds = value;
            }
            "execution_delay_rounds" => {
                if value < 10 {
                    return Err("execution_delay_rounds must be >= 10".to_string());
                }
                self.execution_delay_rounds = value;
            }
            "max_active_proposals" => {
                if value == 0 || value > 100 {
                    return Err("max_active_proposals must be 1-100".to_string());
                }
                self.max_active_proposals = value;
            }
            "observer_reward_percent" => {
                if value > 100 {
                    return Err("observer_reward_percent must be 0-100".to_string());
                }
                self.observer_reward_percent = value;
            }
            _ => {
                return Err(format!("Unknown governable parameter: '{}'", param));
            }
        }

        Ok(())
    }

    /// List all governable parameter names.
    pub fn param_names() -> &'static [&'static str] {
        &[
            "min_fee_sats",
            "min_stake_to_propose",
            "quorum_numerator",
            "approval_numerator",
            "voting_period_rounds",
            "execution_delay_rounds",
            "max_active_proposals",
            "observer_reward_percent",
        ]
    }
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
    pub fn has_passed_with_params(&self, total_staked: u64, params: &GovernanceParams) -> bool {
        let quorum = total_staked
            .saturating_mul(params.quorum_numerator)
            .saturating_add(GOVERNANCE_QUORUM_DENOMINATOR - 1)
            / GOVERNANCE_QUORUM_DENOMINATOR;
        let total = self.total_votes();
        if total < quorum {
            return false;
        }
        let threshold = total
            .saturating_mul(params.approval_numerator)
            .saturating_add(GOVERNANCE_APPROVAL_DENOMINATOR - 1)
            / GOVERNANCE_APPROVAL_DENOMINATOR;
        self.votes_for >= threshold
    }

    /// Check if proposal passed using default constants (for tests and backward compat).
    pub fn has_passed(&self, total_staked: u64) -> bool {
        self.has_passed_with_params(total_staked, &GovernanceParams::default())
    }
}
