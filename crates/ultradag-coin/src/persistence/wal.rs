use std::fs::{self, File, OpenOptions};
use std::io::{self, BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::consensus::vertex::DagVertex;

/// A single WAL entry recording a batch of finalized vertices applied to state.
#[derive(Serialize, Deserialize, Debug)]
pub struct WalEntry {
    /// Monotonically increasing sequence number.
    pub sequence: u64,
    /// The last finalized round after applying this batch.
    pub finalized_round: u64,
    /// The finalized vertices in application order.
    pub vertices: Vec<DagVertex>,
    /// Blake3 state root after applying this batch (for verification).
    pub state_root: [u8; 32],
}

/// WAL header written atomically after each snapshot truncation.
#[derive(Serialize, Deserialize, Debug)]
pub struct WalHeader {
    /// Round of the last successful full snapshot.
    pub snapshot_round: u64,
    /// Next sequence number to write.
    pub next_sequence: u64,
    /// State root at snapshot time (integrity check on replay).
    pub snapshot_state_root: [u8; 32],
}

/// Append-only finality write-ahead log.
///
/// Records finalized vertex batches between full snapshots so that
/// crash recovery can replay them instead of losing progress.
///
/// File layout:
/// - `wal_header.json` — atomic metadata (snapshot round, sequence counter)
/// - `wal.jsonl` — append-only JSON Lines, one `WalEntry` per line
pub struct FinalityWal {
    wal_path: PathBuf,
    header_path: PathBuf,
    header: WalHeader,
    file: Option<File>,
}

impl FinalityWal {
    /// Open or create the WAL in the given data directory.
    pub fn open(data_dir: &Path) -> io::Result<Self> {
        let wal_path = data_dir.join("wal.jsonl");
        let header_path = data_dir.join("wal_header.json");

        let header = if header_path.exists() {
            let bytes = fs::read(&header_path)?;
            serde_json::from_slice(&bytes).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?
        } else {
            WalHeader {
                snapshot_round: 0,
                next_sequence: 0,
                snapshot_state_root: [0u8; 32],
            }
        };

        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&wal_path)?;

        Ok(Self {
            wal_path,
            header_path,
            header,
            file: Some(file),
        })
    }

    /// Append a finalized vertex batch to the WAL.
    /// Called after `apply_finalized_vertices` succeeds.
    pub fn append(&mut self, vertices: &[DagVertex], finalized_round: u64, state_root: [u8; 32]) -> io::Result<()> {
        let entry = WalEntry {
            sequence: self.header.next_sequence,
            finalized_round,
            vertices: vertices.to_vec(),
            state_root,
        };

        let mut line = serde_json::to_string(&entry)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        line.push('\n');

        if let Some(ref mut file) = self.file {
            file.write_all(line.as_bytes())?;
            file.flush()?;
            // fsync for durability — ensures the entry is on disk before we return
            file.sync_data()?;
        }

        self.header.next_sequence += 1;
        Ok(())
    }

    /// Truncate the WAL after a successful full snapshot.
    /// Atomically writes a new header, then truncates the log file.
    pub fn truncate_after_snapshot(&mut self, snapshot_round: u64, state_root: [u8; 32]) -> io::Result<()> {
        // Update header
        self.header.snapshot_round = snapshot_round;
        self.header.snapshot_state_root = state_root;
        // Don't reset sequence — keep it monotonic across truncations
        // (not strictly necessary, but aids debugging)

        // Atomically write header
        let header_json = serde_json::to_vec_pretty(&self.header)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        let tmp = self.header_path.with_extension("tmp");
        fs::write(&tmp, &header_json)?;
        fs::rename(&tmp, &self.header_path)?;

        // Close and truncate WAL file
        self.file = None;
        let file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&self.wal_path)?;
        self.file = Some(file);

        Ok(())
    }

    /// Read all WAL entries for replay on startup.
    /// Skips corrupted trailing entries (simulated crash mid-write).
    pub fn replay(data_dir: &Path) -> io::Result<(WalHeader, Vec<WalEntry>)> {
        let header_path = data_dir.join("wal_header.json");
        let wal_path = data_dir.join("wal.jsonl");

        let header = if header_path.exists() {
            let bytes = fs::read(&header_path)?;
            serde_json::from_slice(&bytes)
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?
        } else {
            WalHeader {
                snapshot_round: 0,
                next_sequence: 0,
                snapshot_state_root: [0u8; 32],
            }
        };

        let mut entries = Vec::new();
        if wal_path.exists() {
            let file = File::open(&wal_path)?;
            let reader = BufReader::new(file);
            for line in reader.lines() {
                let line = match line {
                    Ok(l) => l,
                    Err(_) => break, // I/O error — stop replay
                };
                if line.trim().is_empty() {
                    continue;
                }
                match serde_json::from_str::<WalEntry>(&line) {
                    Ok(entry) => entries.push(entry),
                    Err(_) => break, // Corrupted entry (crash mid-write) — stop here
                }
            }
        }

        Ok((header, entries))
    }

    /// Check if there are WAL entries to replay.
    pub fn has_entries(data_dir: &Path) -> bool {
        let wal_path = data_dir.join("wal.jsonl");
        wal_path.exists() && fs::metadata(&wal_path).map(|m| m.len() > 0).unwrap_or(false)
    }

    /// Get the current header (for diagnostics).
    pub fn header(&self) -> &WalHeader {
        &self.header
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::address::{SecretKey, Signature};
    use crate::block::{Block, BlockHeader};
    use crate::consensus::vertex::DagVertex;
    use crate::tx::CoinbaseTx;
    use std::path::PathBuf;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("ultradag_wal_test_{}_{}", name, std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        dir
    }

    fn make_test_vertex(round: u64) -> DagVertex {
        let sk = SecretKey::generate();
        let addr = sk.address();
        let block = Block {
            header: BlockHeader {
                version: 1,
                height: round,
                timestamp: 1_000_000 + round as i64,
                prev_hash: [0u8; 32],
                merkle_root: [0u8; 32],
            },
            coinbase: CoinbaseTx {
                to: addr,
                amount: 5_000_000_000,
                height: round,
            },
            transactions: vec![],
        };
        let mut v = DagVertex::new(
            block,
            vec![],
            round,
            addr,
            sk.verifying_key().to_bytes(),
            Signature([0u8; 64]),
        );
        v.signature = sk.sign(&v.signable_bytes());
        v
    }

    #[test]
    fn append_and_replay_roundtrip() {
        let dir = temp_dir("roundtrip");
        let mut wal = FinalityWal::open(&dir).unwrap();

        let v1 = make_test_vertex(1);
        let v2 = make_test_vertex(2);
        let root1 = [1u8; 32];
        let root2 = [2u8; 32];

        wal.append(&[v1.clone()], 1, root1).unwrap();
        wal.append(&[v2.clone()], 2, root2).unwrap();
        drop(wal);

        let (header, entries) = FinalityWal::replay(&dir).unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].sequence, 0);
        assert_eq!(entries[0].finalized_round, 1);
        assert_eq!(entries[0].state_root, root1);
        assert_eq!(entries[0].vertices.len(), 1);
        assert_eq!(entries[1].sequence, 1);
        assert_eq!(entries[1].finalized_round, 2);
        assert_eq!(header.next_sequence, 0); // Header not yet updated on disk

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn truncate_clears_entries() {
        let dir = temp_dir("truncate");
        let mut wal = FinalityWal::open(&dir).unwrap();

        let v1 = make_test_vertex(1);
        wal.append(&[v1], 1, [1u8; 32]).unwrap();
        wal.truncate_after_snapshot(1, [1u8; 32]).unwrap();
        drop(wal);

        let (header, entries) = FinalityWal::replay(&dir).unwrap();
        assert!(entries.is_empty());
        assert_eq!(header.snapshot_round, 1);
        assert_eq!(header.snapshot_state_root, [1u8; 32]);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn partial_line_ignored() {
        let dir = temp_dir("partial");
        let mut wal = FinalityWal::open(&dir).unwrap();

        let v1 = make_test_vertex(1);
        wal.append(&[v1], 1, [1u8; 32]).unwrap();
        drop(wal);

        // Simulate crash: append a partial JSON line
        let wal_path = dir.join("wal.jsonl");
        let mut f = OpenOptions::new().append(true).open(&wal_path).unwrap();
        f.write_all(b"{\"sequence\":1,\"finalized_round\":2,\"verti").unwrap();
        drop(f);

        let (_, entries) = FinalityWal::replay(&dir).unwrap();
        assert_eq!(entries.len(), 1); // Only the complete entry is returned
        assert_eq!(entries[0].sequence, 0);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn sequence_is_monotonic() {
        let dir = temp_dir("monotonic");
        let mut wal = FinalityWal::open(&dir).unwrap();

        for i in 0..5 {
            let v = make_test_vertex(i);
            wal.append(&[v], i, [i as u8; 32]).unwrap();
        }
        drop(wal);

        let (_, entries) = FinalityWal::replay(&dir).unwrap();
        for (i, entry) in entries.iter().enumerate() {
            assert_eq!(entry.sequence, i as u64);
        }

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn no_wal_returns_empty() {
        let dir = temp_dir("empty");
        assert!(!FinalityWal::has_entries(&dir));

        let (header, entries) = FinalityWal::replay(&dir).unwrap();
        assert!(entries.is_empty());
        assert_eq!(header.snapshot_round, 0);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn append_after_truncate_starts_fresh() {
        let dir = temp_dir("after_truncate");
        let mut wal = FinalityWal::open(&dir).unwrap();

        let v1 = make_test_vertex(1);
        wal.append(&[v1], 1, [1u8; 32]).unwrap();
        wal.truncate_after_snapshot(1, [1u8; 32]).unwrap();

        let v2 = make_test_vertex(2);
        wal.append(&[v2], 2, [2u8; 32]).unwrap();
        drop(wal);

        let (header, entries) = FinalityWal::replay(&dir).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].finalized_round, 2);
        assert_eq!(header.snapshot_round, 1);

        let _ = std::fs::remove_dir_all(&dir);
    }
}
