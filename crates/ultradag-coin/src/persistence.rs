use std::fs;
use std::path::Path;
use serde::{Serialize, Deserialize};

pub mod monotonicity;

/// Persistence error types
#[derive(Debug, thiserror::Error)]
pub enum PersistenceError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
}

/// Save data to disk as JSON (atomic: write to .tmp then rename)
pub fn save<T: Serialize>(data: &T, path: &Path) -> Result<(), PersistenceError> {
    let json = serde_json::to_string_pretty(data)?;
    let tmp_path = path.with_extension("tmp");
    fs::write(&tmp_path, json)?;
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

/// Load data from disk
pub fn load<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<T, PersistenceError> {
    let json = fs::read_to_string(path)?;
    let data = serde_json::from_str(&json)?;
    Ok(data)
}

/// Check if persistence file exists
pub fn exists(path: &Path) -> bool {
    path.exists()
}

/// Save a checkpoint to disk.
pub fn save_checkpoint(dir: &Path, checkpoint: &crate::consensus::Checkpoint) -> std::io::Result<()> {
    let path = dir.join(format!("checkpoint_{:010}.json", checkpoint.round));
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, serde_json::to_vec(checkpoint)?)?;
    std::fs::rename(tmp, path)?;
    Ok(())
}

/// Load the latest checkpoint from disk, if any.
pub fn load_latest_checkpoint(dir: &Path) -> Option<crate::consensus::Checkpoint> {
    let mut latest: Option<(u64, crate::consensus::Checkpoint)> = None;
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if name.starts_with("checkpoint_") && name.ends_with(".json") {
                if let Ok(bytes) = std::fs::read(entry.path()) {
                    if let Ok(cp) = serde_json::from_slice::<crate::consensus::Checkpoint>(&bytes) {
                        if latest.as_ref().map_or(true, |(r, _)| cp.round > *r) {
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
            if name.starts_with("checkpoint_") && name.ends_with(".json") {
                if let Ok(bytes) = std::fs::read(entry.path()) {
                    if let Ok(cp) = serde_json::from_slice::<crate::consensus::Checkpoint>(&bytes) {
                        rounds.push(cp.round);
                    }
                }
            }
        }
    }
    rounds.sort_unstable();
    rounds
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
