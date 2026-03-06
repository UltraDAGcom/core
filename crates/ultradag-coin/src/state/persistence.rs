use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::address::Address;
use crate::persistence::{self, PersistenceError};
use crate::state::engine::AccountState;

/// Serializable snapshot of StateEngine
/// Uses Vec of tuples instead of HashMap to avoid JSON serialization issues
#[derive(Serialize, Deserialize)]
pub struct StateSnapshot {
    pub accounts: Vec<(Address, AccountState)>,
    pub total_supply: u64,
    pub last_finalized_round: Option<u64>,
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
