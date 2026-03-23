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
    /// Council emission share percentage (param: "council_emission_percent")
    #[serde(default = "default_council_emission")]
    pub council_emission_percent: u64,
    /// Slash percentage on equivocation (param: "slash_percent")
    #[serde(default = "default_slash_percent")]
    pub slash_percent: u64,
    /// Treasury emission share percentage (param: "treasury_emission_percent")
    #[serde(default = "default_treasury_emission")]
    pub treasury_emission_percent: u64,
    /// Founder emission share percentage (param: "founder_emission_percent")
    #[serde(default = "default_founder_emission")]
    pub founder_emission_percent: u64,
}

fn default_council_emission() -> u64 {
    COUNCIL_EMISSION_PERCENT
}

fn default_slash_percent() -> u64 {
    SLASH_PERCENTAGE
}

fn default_treasury_emission() -> u64 {
    TREASURY_EMISSION_PERCENT
}

fn default_founder_emission() -> u64 {
    FOUNDER_EMISSION_PERCENT
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
            council_emission_percent: COUNCIL_EMISSION_PERCENT,
            slash_percent: SLASH_PERCENTAGE,
            treasury_emission_percent: TREASURY_EMISSION_PERCENT,
            founder_emission_percent: FOUNDER_EMISSION_PERCENT,
        }
    }
}

impl GovernanceParams {
    /// Apply a parameter change. Returns Err if param name is unknown or value is invalid.
    /// 
    /// SECURITY: Enforces BFT safety minimums to prevent governance from changing
    /// parameters to values that would compromise consensus safety or enable attacks.
    pub fn apply_change(&mut self, param: &str, new_value: &str) -> Result<(), String> {
        let value: u64 = new_value.parse::<u64>()
            .map_err(|_| format!("Invalid value '{}': must be a positive integer", new_value))?;

        match param {
            "min_fee_sats" => {
                // Floor: 1 sat (no free transactions)
                // Ceiling: 1 UDAG (100M sats) — prevents governance from making txs prohibitively expensive
                if value == 0 {
                    return Err("min_fee_sats cannot be zero".to_string());
                }
                if value > 100_000_000 {
                    return Err("min_fee_sats cannot exceed 1 UDAG (100_000_000 sats)".to_string());
                }
                self.min_fee_sats = value;
            }
            "min_stake_to_propose" => {
                // SECURITY: BFT safety minimum prevents dust attacks on governance.
                // Floor: BFT_MIN_STAKE_SATS (1000 sats) — meaningful stake required
                // Ceiling: 1M UDAG — prevents governance from being locked to whales
                if value < BFT_MIN_STAKE_SATS {
                    return Err(format!(
                        "min_stake_to_propose cannot be below BFT safety minimum of {} sats",
                        BFT_MIN_STAKE_SATS
                    ));
                }
                if value > 1_000_000 * 100_000_000 {
                    return Err("min_stake_to_propose cannot exceed 1,000,000 UDAG".to_string());
                }
                self.min_stake_to_propose = value;
            }
            "quorum_numerator" => {
                // SECURITY: BFT safety bounds prevent quorum manipulation.
                // Floor: BFT_MIN_QUORUM_NUMERATOR (10%) — prevents tiny fraction from passing proposals
                // Ceiling: BFT_MAX_QUORUM_NUMERATOR (50%) — prevents impossible quorum requirements
                if value < BFT_MIN_QUORUM_NUMERATOR {
                    return Err(format!(
                        "quorum_numerator cannot be below BFT safety minimum of {}",
                        BFT_MIN_QUORUM_NUMERATOR
                    ));
                }
                if value > BFT_MAX_QUORUM_NUMERATOR {
                    return Err(format!(
                        "quorum_numerator cannot exceed BFT safety maximum of {}",
                        BFT_MAX_QUORUM_NUMERATOR
                    ));
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
                // Floor: 1000 rounds (~1.4 hours at 5s) — meaningful governance window
                // Ceiling: 1,000,000 rounds (~58 days at 5s) — prevents indefinite voting
                if value < 1000 {
                    return Err("voting_period_rounds must be >= 1000".to_string());
                }
                if value > 1_000_000 {
                    return Err("voting_period_rounds cannot exceed 1,000,000".to_string());
                }
                self.voting_period_rounds = value;
            }
            "execution_delay_rounds" => {
                // Hard floor matches UNSTAKE_COOLDOWN_ROUNDS (2,016 rounds / ~2.8 hours).
                // Prevents coordinated attacks from executing before community notices.
                // Ceiling: 100,000 rounds (~5.8 days at 5s) — prevents indefinite delay
                if value < 2016 {
                    return Err("execution_delay_rounds must be >= 2016".to_string());
                }
                if value > 100_000 {
                    return Err("execution_delay_rounds cannot exceed 100,000".to_string());
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
            "council_emission_percent" => {
                // Floor: 0 (council gets nothing — effectively disabled)
                // Ceiling: 30% — prevents council from capturing most of the emission
                if value > 30 {
                    return Err("council_emission_percent must be 0-30".to_string());
                }
                self.council_emission_percent = value;
            }
            "slash_percent" => {
                // Floor: 10% — must be meaningful deterrent against equivocation
                // Ceiling: 100% — full stake burn (maximum punishment)
                if !(10..=100).contains(&value) {
                    return Err("slash_percent must be 10-100".to_string());
                }
                self.slash_percent = value;
            }
            "treasury_emission_percent" => {
                // Floor: 0% — treasury gets nothing (disabled)
                // Ceiling: 20% — prevents treasury from capturing too much emission
                // Combined with council (10%) + founder (5%) leaves at least 65% for validators.
                if value > 20 {
                    return Err("treasury_emission_percent must be 0-20".to_string());
                }
                self.treasury_emission_percent = value;
            }
            "founder_emission_percent" => {
                // Floor: 0% — founder gets nothing (can be sunset)
                // Ceiling: 10% — prevents founder from capturing too much emission
                if value > 10 {
                    return Err("founder_emission_percent must be 0-10".to_string());
                }
                self.founder_emission_percent = value;
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
            "council_emission_percent",
            "slash_percent",
            "treasury_emission_percent",
            "founder_emission_percent",
        ]
    }
}
