use crate::constants::*;
use serde::{Deserialize, Serialize};

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
                if value < 5 || value > 100 {
                    return Err("quorum_numerator must be 5-100".to_string());
                }
                self.quorum_numerator = value;
            }
            "approval_numerator" => {
                if !(51..=100).contains(&value) {
                    return Err("approval_numerator must be 51-100".to_string());
                }
                self.approval_numerator = value;
            }
            "voting_period_rounds" => {
                if value < 1000 {
                    return Err("voting_period_rounds must be >= 1000".to_string());
                }
                self.voting_period_rounds = value;
            }
            "execution_delay_rounds" => {
                if value < 100 {
                    return Err("execution_delay_rounds must be >= 100".to_string());
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
