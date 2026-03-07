use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::address::Address;
use crate::consensus::vertex::DagVertex;
use crate::persistence::{self, PersistenceError};

/// Serializable snapshot of BlockDag state
/// Uses Vec of tuples instead of HashMaps to avoid JSON serialization issues
#[derive(Serialize, Deserialize)]
pub struct DagSnapshot {
    pub vertices: Vec<([u8; 32], DagVertex)>,
    pub children: Vec<([u8; 32], Vec<[u8; 32]>)>,
    pub tips: Vec<[u8; 32]>,
    pub rounds: Vec<(u64, Vec<[u8; 32]>)>,
    pub current_round: u64,
    pub byzantine_validators: Vec<Address>,
    pub equivocation_evidence: Vec<((Address, u64), [[u8; 32]; 2])>,
    #[serde(default)]
    pub pruning_floor: u64,
    /// Permanent equivocation evidence store (survives pruning).
    #[serde(default)]
    pub evidence_store: Vec<(Address, crate::consensus::dag::EquivocationEvidence)>,
}

impl DagSnapshot {
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

/// Serializable snapshot of FinalityTracker state
#[derive(Serialize, Deserialize)]
pub struct FinalitySnapshot {
    pub finalized: Vec<[u8; 32]>,
    pub validators: Vec<Address>,
    pub min_validators: usize,
    #[serde(default)]
    pub last_finalized_round: u64,
}

impl FinalitySnapshot {
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
