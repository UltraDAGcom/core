use std::path::{Path, PathBuf};
use std::fs;
use serde::{Serialize, Deserialize};
use thiserror::Error;

use crate::persistence::atomic_write;

#[derive(Debug, Error)]
pub enum MonotonicityError {
    #[error("State rollback detected: attempting to load round {attempting} but high-water mark is {current}")]
    StateRollbackDetected {
        current: u64,
        attempting: u64,
    },
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Serialization error: {0}")]
    Serialization(String),
}

impl From<postcard::Error> for MonotonicityError {
    fn from(e: postcard::Error) -> Self {
        MonotonicityError::Serialization(e.to_string())
    }
}

/// High-water mark tracking the highest round ever finalized.
/// This prevents loading old state files that would cause a rollback.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HighWaterMark {
    /// Highest round number ever finalized
    pub max_round: u64,
    /// Timestamp when this round was reached (Unix timestamp)
    pub timestamp: i64,
    /// Blake3 hash of the state at this round (for verification)
    pub state_hash: [u8; 32],
}

impl HighWaterMark {
    /// Create a new high-water mark at genesis
    pub fn new() -> Self {
        Self {
            max_round: 0,
            timestamp: chrono::Utc::now().timestamp(),
            state_hash: [0; 32],
        }
    }

    /// Load from disk or create new if doesn't exist
    pub fn load_or_create(path: &Path) -> Result<Self, MonotonicityError> {
        if path.exists() {
            let data = fs::read(path)?;
            let hwm: HighWaterMark = postcard::from_bytes(&data)?;
            Ok(hwm)
        } else {
            Ok(Self::new())
        }
    }

    /// Verify that new_round is >= max_round (monotonicity check)
    /// Returns error if attempting to go backwards
    pub fn verify_monotonic(&self, new_round: u64) -> Result<(), MonotonicityError> {
        if new_round < self.max_round {
            return Err(MonotonicityError::StateRollbackDetected {
                current: self.max_round,
                attempting: new_round,
            });
        }
        Ok(())
    }

    /// Update to new high-water mark
    /// Only updates if new round is >= current max
    pub fn update(&mut self, round: u64, state_hash: [u8; 32]) {
        if round >= self.max_round {
            self.max_round = round;
            self.timestamp = chrono::Utc::now().timestamp();
            self.state_hash = state_hash;
        }
    }

    /// Save to disk atomically
    pub fn save(&self, path: &Path) -> Result<(), MonotonicityError> {
        let data = postcard::to_allocvec(self)?;
        atomic_write(path, &data)?;
        Ok(())
    }

    /// Get the current high-water mark round
    pub fn current_round(&self) -> u64 {
        self.max_round
    }

    /// Get the path for the high-water mark file in a data directory
    pub fn path_in_dir(data_dir: &Path) -> PathBuf {
        data_dir.join("high_water_mark.bin")
    }
}

impl Default for HighWaterMark {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    use tempfile::TempDir;

    #[test]
    fn test_new_high_water_mark() {
        let hwm = HighWaterMark::new();
        assert_eq!(hwm.max_round, 0);
        assert_eq!(hwm.state_hash, [0; 32]);
    }

    #[test]
    fn test_verify_monotonic_allows_forward() {
        let hwm = HighWaterMark {
            max_round: 100,
            timestamp: 0,
            state_hash: [0; 32],
        };

        assert!(hwm.verify_monotonic(100).is_ok());
        assert!(hwm.verify_monotonic(101).is_ok());
        assert!(hwm.verify_monotonic(1000).is_ok());
    }

    #[test]
    fn test_verify_monotonic_rejects_backward() {
        let hwm = HighWaterMark {
            max_round: 100,
            timestamp: 0,
            state_hash: [0; 32],
        };

        let result = hwm.verify_monotonic(99);
        assert!(result.is_err());
        
        if let Err(MonotonicityError::StateRollbackDetected { current, attempting }) = result {
            assert_eq!(current, 100);
            assert_eq!(attempting, 99);
        } else {
            panic!("Expected StateRollbackDetected error");
        }
    }

    #[test]
    fn test_update_advances_forward() {
        let mut hwm = HighWaterMark::new();
        let hash1 = [1; 32];
        let hash2 = [2; 32];

        hwm.update(50, hash1);
        assert_eq!(hwm.max_round, 50);
        assert_eq!(hwm.state_hash, hash1);

        hwm.update(100, hash2);
        assert_eq!(hwm.max_round, 100);
        assert_eq!(hwm.state_hash, hash2);
    }

    #[test]
    fn test_update_ignores_backward() {
        let mut hwm = HighWaterMark {
            max_round: 100,
            timestamp: 0,
            state_hash: [1; 32],
        };

        let hash2 = [2; 32];
        hwm.update(50, hash2);
        
        // Should not update
        assert_eq!(hwm.max_round, 100);
        assert_eq!(hwm.state_hash, [1; 32]);
    }

    #[test]
    fn test_save_and_load() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("hwm.json");

        let mut hwm = HighWaterMark::new();
        hwm.update(42, [7; 32]);
        hwm.save(&path).unwrap();

        let loaded = HighWaterMark::load_or_create(&path).unwrap();
        assert_eq!(loaded.max_round, 42);
        assert_eq!(loaded.state_hash, [7; 32]);
    }

    #[test]
    fn test_load_or_create_when_missing() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("missing.json");

        let hwm = HighWaterMark::load_or_create(&path).unwrap();
        assert_eq!(hwm.max_round, 0);
    }

    #[test]
    fn test_path_in_dir() {
        let dir = Path::new("/data");
        let path = HighWaterMark::path_in_dir(dir);
        assert_eq!(path, Path::new("/data/high_water_mark.bin"));
    }
}
