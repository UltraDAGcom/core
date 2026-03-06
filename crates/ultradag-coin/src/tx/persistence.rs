use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::persistence::{self, PersistenceError};
use crate::tx::transaction::Transaction;

/// Serializable snapshot of Mempool
#[derive(Serialize, Deserialize)]
pub struct MempoolSnapshot {
    pub transactions: Vec<Transaction>,
}

impl MempoolSnapshot {
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
