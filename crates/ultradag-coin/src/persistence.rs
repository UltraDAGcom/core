use std::fs;
use std::path::Path;
use serde::{Serialize, Deserialize};

pub mod monotonicity;
pub mod wal;

/// Persistence error types
#[derive(Debug, thiserror::Error)]
pub enum PersistenceError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Serialization error: {0}")]
    Serialization(String),
}

impl From<serde_json::Error> for PersistenceError {
    fn from(e: serde_json::Error) -> Self {
        PersistenceError::Serialization(e.to_string())
    }
}

impl From<postcard::Error> for PersistenceError {
    fn from(e: postcard::Error) -> Self {
        PersistenceError::Serialization(e.to_string())
    }
}

/// Save data to disk as postcard binary (atomic: write to .tmp then rename)
pub fn save<T: Serialize>(data: &T, path: &Path) -> Result<(), PersistenceError> {
    let bytes = postcard::to_allocvec(data)?;
    let tmp_path = path.with_extension("tmp");
    fs::write(&tmp_path, bytes)?;
    fs::rename(&tmp_path, path)?;
    Ok(())
}

/// Atomic write of raw bytes (used by monotonicity module)
pub fn atomic_write(path: &Path, data: &[u8]) -> std::io::Result<()> {
    let tmp_path = path.with_extension("tmp");
    fs::write(&tmp_path, data)?;
    fs::rename(&tmp_path, path)?;
    Ok(())
}

/// Load data from disk (postcard binary)
pub fn load<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<T, PersistenceError> {
    let bytes = fs::read(path)?;
    let data = postcard::from_bytes(&bytes)?;
    Ok(data)
}

/// Check if persistence file exists
pub fn exists(path: &Path) -> bool {
    path.exists()
}

/// Save a checkpoint to disk (postcard binary).
pub fn save_checkpoint(dir: &Path, checkpoint: &crate::consensus::Checkpoint) -> std::io::Result<()> {
    let path = dir.join(format!("checkpoint_{:010}.bin", checkpoint.round));
    let tmp = path.with_extension("tmp");
    let bytes = postcard::to_allocvec(checkpoint).map_err(|e| std::io::Error::other(e.to_string()))?;
    std::fs::write(&tmp, bytes)?;
    std::fs::rename(tmp, path)?;
    Ok(())
}

/// Save the state snapshot that corresponds to a checkpoint.
/// This must be called at checkpoint production time so GetCheckpoint
/// can serve the correct state (not the current, advanced state).
pub fn save_checkpoint_state(dir: &Path, round: u64, snapshot: &crate::state::persistence::StateSnapshot) -> std::io::Result<()> {
    let path = dir.join(format!("checkpoint_state_{:010}.bin", round));
    let tmp = path.with_extension("tmp");
    let bytes = postcard::to_allocvec(snapshot).map_err(|e| std::io::Error::other(e.to_string()))?;
    std::fs::write(&tmp, bytes)?;
    std::fs::rename(tmp, path)?;
    Ok(())
}

/// Load the state snapshot for a specific checkpoint round.
pub fn load_checkpoint_state(dir: &Path, round: u64) -> Option<crate::state::persistence::StateSnapshot> {
    let path = dir.join(format!("checkpoint_state_{:010}.bin", round));
    if path.exists() {
        let bytes = std::fs::read(&path).ok()?;
        postcard::from_bytes(&bytes).ok()
    } else {
        None
    }
}

/// Load a checkpoint for a specific round from disk.
pub fn load_checkpoint_by_round(dir: &Path, round: u64) -> Option<crate::consensus::Checkpoint> {
    let path = dir.join(format!("checkpoint_{:010}.bin", round));
    if path.exists() {
        let bytes = std::fs::read(&path).ok()?;
        postcard::from_bytes(&bytes).ok()
    } else {
        None
    }
}

/// Load the latest checkpoint from disk, if any.
pub fn load_latest_checkpoint(dir: &Path) -> Option<crate::consensus::Checkpoint> {
    let mut latest: Option<(u64, crate::consensus::Checkpoint)> = None;
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if name.starts_with("checkpoint_") && name.ends_with(".bin") {
                if let Ok(bytes) = std::fs::read(entry.path()) {
                    if let Ok(cp) = postcard::from_bytes::<crate::consensus::Checkpoint>(&bytes) {
                        if latest.as_ref().is_none_or(|(r, _)| cp.round > *r) {
                            latest = Some((cp.round, cp));
                        }
                    }
                }
            }
        }
    }
    latest.map(|(_, cp)| cp)
}

/// List all available checkpoint rounds.
pub fn list_checkpoints(dir: &Path) -> Vec<u64> {
    let mut rounds = Vec::new();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if name.starts_with("checkpoint_") && name.ends_with(".bin") {
                if let Ok(bytes) = std::fs::read(entry.path()) {
                    if let Ok(cp) = postcard::from_bytes::<crate::consensus::Checkpoint>(&bytes) {
                        rounds.push(cp.round);
                    }
                }
            }
        }
    }
    rounds.sort_unstable();
    rounds
}

/// Prune old checkpoints, keeping only the most recent ones.
/// 
/// Strategy:
/// - Keep the latest `keep_count` checkpoints
/// - Always keep at least 2 checkpoints for safety (even if keep_count=1)
/// - Delete older checkpoints to limit disk usage
/// 
/// Returns the number of checkpoints deleted.
pub fn prune_old_checkpoints(dir: &Path, keep_count: usize) -> std::io::Result<usize> {
    let keep_count = keep_count.max(2); // Always keep at least 2 for safety
    let mut rounds = list_checkpoints(dir);
    
    if rounds.len() <= keep_count {
        return Ok(0); // Nothing to prune
    }
    
    // Sort in descending order (newest first)
    rounds.sort_unstable_by(|a, b| b.cmp(a));
    
    // Keep the newest `keep_count` checkpoints, delete the rest
    let to_delete = &rounds[keep_count..];
    let mut deleted = 0;
    
    for round in to_delete {
        let path = dir.join(format!("checkpoint_{:010}.bin", round));
        if path.exists() {
            if let Err(e) = std::fs::remove_file(&path) {
                tracing::warn!("Failed to delete checkpoint at round {}: {}", round, e);
            } else {
                deleted += 1;
            }
        }
        // Also delete the corresponding state snapshot
        let state_path = dir.join(format!("checkpoint_state_{:010}.bin", round));
        if state_path.exists() {
            let _ = std::fs::remove_file(&state_path);
        }
    }
    
    Ok(deleted)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Serialize, Deserialize};
    use std::path::PathBuf;

    #[derive(Serialize, Deserialize, PartialEq, Debug)]
    struct TestData {
        value: u64,
        name: String,
    }

    #[test]
    fn save_and_load_roundtrip() {
        let temp_dir = std::env::temp_dir();
        let path = temp_dir.join("test_persistence.json");

        let data = TestData {
            value: 42,
            name: "test".to_string(),
        };

        save(&data, &path).unwrap();
        let loaded: TestData = load(&path).unwrap();

        assert_eq!(data, loaded);
        fs::remove_file(&path).unwrap();
    }

    #[test]
    fn exists_returns_false_for_nonexistent_file() {
        let path = PathBuf::from("/nonexistent/file.json");
        assert!(!exists(&path));
    }
}
