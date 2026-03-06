use std::fs;
use std::path::Path;
use serde::{Serialize, Deserialize};

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
