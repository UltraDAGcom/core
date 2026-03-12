use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::address::Address;
use crate::consensus::vertex::DagVertex;
use crate::persistence::{self, PersistenceError};

/// Type alias for equivocation evidence to simplify complex types
pub type EquivocationEvidenceTuple = ((Address, u64), [[u8; 32]; 2]);

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
    pub equivocation_evidence: Vec<EquivocationEvidenceTuple>,
    #[serde(default)]
    pub pruning_floor: u64,
    /// Permanent equivocation evidence store (survives pruning).
    #[serde(default)]
    pub evidence_store: Vec<(Address, Vec<crate::consensus::dag::EquivocationEvidence>)>,
    /// Rejected equivocation vertices (not in DAG, needed for evidence broadcasting).
    #[serde(default)]
    pub equivocation_vertices: Vec<([u8; 32], DagVertex)>,
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
