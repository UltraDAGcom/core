use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::address::Address;
use crate::persistence::{self, PersistenceError};
use crate::state::engine::{AccountState, DelegationAccount, StakeAccount};

/// Serializable snapshot of StateEngine
/// Uses Vec of tuples instead of HashMap for deterministic serialization order
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
    /// Delegated staking accounts: delegator address → delegation details.
    #[serde(default)]
    pub delegation_accounts: Vec<(crate::address::Address, DelegationAccount)>,
    /// Configured validator count from --validators N CLI flag.
    /// Affects pre-staking reward distribution (divisor). Must be in state root
    /// to prevent divergence between nodes with different --validators N values.
    #[serde(default)]
    pub configured_validator_count: Option<u64>,
    /// Bridge reserve: UDAG locked for bridging to Arbitrum.
    #[serde(default)]
    pub bridge_reserve: u64,
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
