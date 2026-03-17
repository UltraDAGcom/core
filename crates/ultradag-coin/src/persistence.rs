use std::fs;
use std::io::Write;
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

/// Write bytes to a file and fsync to ensure data is durable on disk.
/// Without fsync, a crash after write but before OS flush can leave
/// the file empty or partially written, even though write() returned Ok.
fn write_and_fsync(path: &Path, data: &[u8]) -> std::io::Result<()> {
    let mut file = fs::File::create(path)?;
    file.write_all(data)?;
    file.sync_all()?;
    Ok(())
}

/// Fsync the parent directory to ensure a rename/create is durable.
/// On POSIX systems, rename() is atomic for the directory entry but
/// the directory metadata itself may not be flushed without this.
fn fsync_directory(path: &Path) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        if let Ok(dir) = fs::File::open(parent) {
            // Best-effort: some platforms (e.g., Windows) don't support
            // syncing directories. Ignore errors on non-Unix.
            let _ = dir.sync_all();
        }
    }
    Ok(())
}

/// Save data to disk as postcard binary (atomic: write to .tmp, fsync, then rename).
/// The fsync ensures the temp file data is durable before rename replaces the old file.
pub fn save<T: Serialize>(data: &T, path: &Path) -> Result<(), PersistenceError> {
    let bytes = postcard::to_allocvec(data)?;
    let tmp_path = path.with_extension("tmp");
    write_and_fsync(&tmp_path, &bytes)?;
    fs::rename(&tmp_path, path)?;
    fsync_directory(path)?;
    Ok(())
}

/// Atomic write of raw bytes (used by monotonicity module).
/// Fsyncs temp file before rename to prevent data loss on crash.
pub fn atomic_write(path: &Path, data: &[u8]) -> std::io::Result<()> {
    let tmp_path = path.with_extension("tmp");
    write_and_fsync(&tmp_path, data)?;
    fs::rename(&tmp_path, path)?;
    fsync_directory(path)?;
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

/// Save a checkpoint to disk (postcard binary, atomic with fsync).
pub fn save_checkpoint(dir: &Path, checkpoint: &crate::consensus::Checkpoint) -> std::io::Result<()> {
    let path = dir.join(format!("checkpoint_{:010}.bin", checkpoint.round));
    let tmp = path.with_extension("tmp");
    let bytes = postcard::to_allocvec(checkpoint).map_err(|e| std::io::Error::other(e.to_string()))?;
    write_and_fsync(&tmp, &bytes)?;
    std::fs::rename(&tmp, &path)?;
    fsync_directory(&path)?;
    Ok(())
}

/// Save the state snapshot that corresponds to a checkpoint.
/// This must be called at checkpoint production time so GetCheckpoint
/// can serve the correct state (not the current, advanced state).
pub fn save_checkpoint_state(dir: &Path, round: u64, snapshot: &crate::state::persistence::StateSnapshot) -> std::io::Result<()> {
    let path = dir.join(format!("checkpoint_state_{:010}.bin", round));
    let tmp = path.with_extension("tmp");
    let bytes = postcard::to_allocvec(snapshot).map_err(|e| std::io::Error::other(e.to_string()))?;
    write_and_fsync(&tmp, &bytes)?;
    std::fs::rename(&tmp, &path)?;
    fsync_directory(&path)?;
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

/// Extract the round number from a checkpoint filename like "checkpoint_0000007500.bin".
/// Returns None if the filename doesn't match the expected pattern.
fn parse_checkpoint_round_from_filename(name: &str) -> Option<u64> {
    if !name.starts_with("checkpoint_") || !name.ends_with(".bin") || name.starts_with("checkpoint_state_") {
        return None;
    }
    // "checkpoint_" = 11 chars, ".bin" = 4 chars
    let round_str = &name[11..name.len() - 4];
    round_str.parse::<u64>().ok()
}

/// Load the latest checkpoint from disk, if any.
/// Parses round from filename to find the latest, then only deserializes that one file.
/// This avoids O(N) deserializations when many checkpoint files exist on disk.
pub fn load_latest_checkpoint(dir: &Path) -> Option<crate::consensus::Checkpoint> {
    let mut best_round: Option<u64> = None;
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if let Some(round) = parse_checkpoint_round_from_filename(&name) {
                if best_round.is_none_or(|r| round > r) {
                    best_round = Some(round);
                }
            }
        }
    }
    best_round.and_then(|round| load_checkpoint_by_round(dir, round))
}

/// List all available checkpoint rounds.
/// Parses round from filename pattern "checkpoint_NNNNNNNNNN.bin" instead of
/// deserializing every file. O(N) directory scan with zero deserialization.
pub fn list_checkpoints(dir: &Path) -> Vec<u64> {
    let mut rounds = Vec::new();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if let Some(round) = parse_checkpoint_round_from_filename(&name) {
                rounds.push(round);
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
        let temp_dir = tempfile::TempDir::new().unwrap();
        let path = temp_dir.path().join("test_persistence.bin");

        let data = TestData {
            value: 42,
            name: "test".to_string(),
        };

        save(&data, &path).unwrap();
        let loaded: TestData = load(&path).unwrap();

        assert_eq!(data, loaded);
    }

    #[test]
    fn exists_returns_false_for_nonexistent_file() {
        let path = PathBuf::from("/nonexistent/file.json");
        assert!(!exists(&path));
    }

    #[test]
    fn save_removes_tmp_file_on_success() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let path = temp_dir.path().join("test_no_tmp.bin");
        let tmp_path = path.with_extension("tmp");

        let data = TestData { value: 1, name: "a".into() };
        save(&data, &path).unwrap();

        // The .tmp file should not remain after a successful save
        assert!(!tmp_path.exists(), "tmp file should not exist after successful save");
        assert!(path.exists(), "target file should exist");
    }

    #[test]
    fn save_overwrites_stale_tmp_file() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let path = temp_dir.path().join("test_stale_tmp.bin");
        let tmp_path = path.with_extension("tmp");

        // Create a stale .tmp file (simulates interrupted previous save)
        fs::write(&tmp_path, b"stale garbage data").unwrap();

        let data = TestData { value: 99, name: "fresh".into() };
        save(&data, &path).unwrap();

        // Stale tmp should be gone, data should be correct
        assert!(!tmp_path.exists());
        let loaded: TestData = load(&path).unwrap();
        assert_eq!(loaded, data);
    }

    #[test]
    fn atomic_write_is_durable() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let path = temp_dir.path().join("atomic_test.bin");

        atomic_write(&path, b"hello world").unwrap();
        let bytes = fs::read(&path).unwrap();
        assert_eq!(bytes, b"hello world");

        // No leftover .tmp file
        assert!(!path.with_extension("tmp").exists());
    }

    #[test]
    fn parse_checkpoint_round_from_filename_valid() {
        assert_eq!(parse_checkpoint_round_from_filename("checkpoint_0000007500.bin"), Some(7500));
        assert_eq!(parse_checkpoint_round_from_filename("checkpoint_0000000000.bin"), Some(0));
        assert_eq!(parse_checkpoint_round_from_filename("checkpoint_9999999999.bin"), Some(9999999999));
    }

    #[test]
    fn parse_checkpoint_round_from_filename_rejects_state_files() {
        assert_eq!(parse_checkpoint_round_from_filename("checkpoint_state_0000007500.bin"), None);
    }

    #[test]
    fn parse_checkpoint_round_from_filename_rejects_invalid() {
        assert_eq!(parse_checkpoint_round_from_filename("other_file.bin"), None);
        assert_eq!(parse_checkpoint_round_from_filename("checkpoint_abc.bin"), None);
        assert_eq!(parse_checkpoint_round_from_filename("checkpoint_0000007500.json"), None);
    }

    #[test]
    fn list_checkpoints_uses_filename_not_deserialization() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let dir = temp_dir.path();

        // Create checkpoint files with correct names but invalid content
        // (just enough to prove we parse filename, not content)
        let cp = crate::consensus::Checkpoint {
            round: 100,
            state_root: [0; 32],
            dag_tip: [0; 32],
            total_supply: 0,
            prev_checkpoint_hash: [0; 32],
            signatures: vec![],
        };
        save_checkpoint(dir, &cp).unwrap();

        let cp200 = crate::consensus::Checkpoint { round: 200, ..cp.clone() };
        save_checkpoint(dir, &cp200).unwrap();

        // Also create a checkpoint_state file that should be excluded
        let state_path = dir.join("checkpoint_state_0000000100.bin");
        fs::write(&state_path, b"state data").unwrap();

        let rounds = list_checkpoints(dir);
        assert_eq!(rounds, vec![100, 200]);
    }

    #[test]
    fn load_latest_checkpoint_returns_highest_round() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let dir = temp_dir.path();

        let cp100 = crate::consensus::Checkpoint {
            round: 100,
            state_root: [1; 32],
            dag_tip: [0; 32],
            total_supply: 0,
            prev_checkpoint_hash: [0; 32],
            signatures: vec![],
        };
        let cp300 = crate::consensus::Checkpoint { round: 300, state_root: [3; 32], ..cp100.clone() };
        let cp200 = crate::consensus::Checkpoint { round: 200, state_root: [2; 32], ..cp100.clone() };

        save_checkpoint(dir, &cp100).unwrap();
        save_checkpoint(dir, &cp300).unwrap();
        save_checkpoint(dir, &cp200).unwrap();

        let latest = load_latest_checkpoint(dir).unwrap();
        assert_eq!(latest.round, 300);
        assert_eq!(latest.state_root, [3; 32]);
    }

    #[test]
    fn checkpoint_save_and_load_roundtrip() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let dir = temp_dir.path();

        let cp = crate::consensus::Checkpoint {
            round: 500,
            state_root: [42; 32],
            dag_tip: [7; 32],
            total_supply: 1_000_000,
            prev_checkpoint_hash: [0; 32],
            signatures: vec![],
        };

        save_checkpoint(dir, &cp).unwrap();
        let loaded = load_checkpoint_by_round(dir, 500).unwrap();

        assert_eq!(loaded.round, 500);
        assert_eq!(loaded.state_root, [42; 32]);
        assert_eq!(loaded.total_supply, 1_000_000);
    }

    #[test]
    fn prune_old_checkpoints_keeps_newest() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let dir = temp_dir.path();

        // Create 5 checkpoints
        for round in [100, 200, 300, 400, 500] {
            let cp = crate::consensus::Checkpoint {
                round,
                state_root: [0; 32],
                dag_tip: [0; 32],
                total_supply: 0,
                prev_checkpoint_hash: [0; 32],
                signatures: vec![],
            };
            save_checkpoint(dir, &cp).unwrap();
        }

        assert_eq!(list_checkpoints(dir).len(), 5);

        // Prune keeping 3 (min 2)
        let deleted = prune_old_checkpoints(dir, 3).unwrap();
        assert_eq!(deleted, 2);

        let remaining = list_checkpoints(dir);
        assert_eq!(remaining, vec![300, 400, 500]);
    }
}
