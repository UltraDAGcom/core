use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::address::Address;
use crate::persistence::{self, PersistenceError};
use crate::state::engine::{AccountState, StakeAccount};

/// Serializable snapshot of StateEngine
/// Uses Vec of tuples instead of HashMap to avoid JSON serialization issues
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateSnapshot {
    pub accounts: Vec<(Address, AccountState)>,
    #[serde(default)]
    pub stake_accounts: Vec<(Address, StakeAccount)>,
    #[serde(default)]
    pub active_validator_set: Vec<Address>,
    #[serde(default)]
    pub current_epoch: u64,
    pub total_supply: u64,
    pub last_finalized_round: Option<u64>,
    #[serde(default)]
    pub proposals: Vec<(u64, crate::governance::Proposal)>,
    #[serde(default)]
    pub votes: Vec<((u64, Address), bool)>,
    #[serde(default)]
    pub next_proposal_id: u64,
    #[serde(default)]
    pub governance_params: crate::governance::GovernanceParams,
    /// Council of 21 members with seat categories.
    #[serde(default)]
    pub council_members: Vec<(crate::address::Address, crate::governance::CouncilSeatCategory)>,
    /// DAO treasury balance in sats. Controlled by Council of 21 via TreasurySpend proposals.
    #[serde(default)]
    pub treasury_balance: u64,
}

impl StateSnapshot {
    pub fn save(&self, path: &Path) -> Result<(), PersistenceError> {
        persistence::save(self, path)
    }

    pub fn load(path: &Path) -> Result<Self, PersistenceError> {
        persistence::load(path)
    }

    pub fn exists(path: &Path) -> bool {
        persistence::exists(path)
    }
}
