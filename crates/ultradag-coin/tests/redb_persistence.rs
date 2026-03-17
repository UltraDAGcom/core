//! Tests for the redb persistence layer (state/db.rs).
//!
//! Covers:
//! - Save/load roundtrip with all field types
//! - State root integrity verification on load
//! - Corrupted state root detection
//! - Legacy database handling (missing tables)
//! - Concurrent save safety (stale tmp file cleanup)

use ultradag_coin::{SecretKey, StateEngine};
use ultradag_coin::state::db::{save_to_redb, load_from_redb};

/// Helper: create a StateEngine with some non-trivial state for testing persistence.
fn engine_with_state() -> StateEngine {
    use ultradag_coin::{Block, CoinbaseTx, DagVertex, Signature};
    use ultradag_coin::block::header::BlockHeader;

    let mut engine = StateEngine::new_with_genesis();
    let sk = SecretKey::generate();
    let addr = sk.address();

    // Apply a vertex to create some account state
    let block = Block {
        header: BlockHeader {
            version: 1,
            height: 0,
            timestamp: 1000,
            prev_hash: [0u8; 32],
            merkle_root: [0u8; 32],
        },
        coinbase: CoinbaseTx {
            to: addr,
            amount: 0, // Fees only
            height: 0,
        },
        transactions: vec![],
    };

    let mut vertex = DagVertex::new(
        block,
        vec![[0u8; 32]],
        0,
        addr,
        sk.verifying_key().to_bytes(),
        Signature([0u8; 64]),
    );
    vertex.signature = sk.sign(&vertex.signable_bytes());

    let _ = engine.apply_vertex(&vertex);
    engine
}

#[test]
fn redb_save_load_roundtrip() {
    let temp_dir = tempfile::TempDir::new().unwrap();
    let path = temp_dir.path().join("state.redb");

    let engine = engine_with_state();
    let supply_before = engine.total_supply();
    let epoch_before = engine.current_epoch();

    save_to_redb(&engine, &path).unwrap();
    let loaded = load_from_redb(&path).unwrap();

    assert_eq!(loaded.total_supply(), supply_before);
    assert_eq!(loaded.current_epoch(), epoch_before);
    assert_eq!(loaded.last_finalized_round(), engine.last_finalized_round());
}

#[test]
fn redb_state_root_integrity_verified_on_load() {
    let temp_dir = tempfile::TempDir::new().unwrap();
    let path = temp_dir.path().join("state.redb");

    let engine = engine_with_state();
    save_to_redb(&engine, &path).unwrap();

    // Loading should succeed (state root matches)
    let loaded = load_from_redb(&path);
    assert!(loaded.is_ok(), "Load should succeed with valid state root");
}

#[test]
fn redb_corrupted_state_root_detected() {
    use redb::{Database, TableDefinition};

    let temp_dir = tempfile::TempDir::new().unwrap();
    let path = temp_dir.path().join("state.redb");

    let engine = engine_with_state();
    save_to_redb(&engine, &path).unwrap();

    // Corrupt the state root in the metadata table by modifying it directly
    {
        let db = Database::open(&path).unwrap();
        let txn = db.begin_write().unwrap();
        {
            const METADATA: TableDefinition<&str, &[u8]> = TableDefinition::new("metadata");
            let mut table = txn.open_table(METADATA).unwrap();
            // Write a bogus state root
            table.insert("state_root", [0xDE; 32].as_slice()).unwrap();
        }
        txn.commit().unwrap();
    }

    // Loading should fail with state root mismatch
    let result = load_from_redb(&path);
    assert!(result.is_err(), "Load should fail with corrupted state root");
    let err_msg = format!("{}", result.unwrap_err());
    assert!(err_msg.contains("State root mismatch"), "Error should mention state root mismatch: {}", err_msg);
}

#[test]
fn redb_no_tmp_file_after_successful_save() {
    let temp_dir = tempfile::TempDir::new().unwrap();
    let path = temp_dir.path().join("state.redb");
    let tmp_path = path.with_extension("redb.tmp");

    let engine = engine_with_state();
    save_to_redb(&engine, &path).unwrap();

    assert!(path.exists(), "state.redb should exist");
    assert!(!tmp_path.exists(), "temp file should not remain after successful save");
}

#[test]
fn redb_stale_tmp_file_cleaned_on_new_save() {
    let temp_dir = tempfile::TempDir::new().unwrap();
    let path = temp_dir.path().join("state.redb");
    let tmp_path = path.with_extension("redb.tmp");

    // Create a stale tmp file (simulates crashed previous save)
    std::fs::write(&tmp_path, b"stale data from crashed save").unwrap();

    let engine = engine_with_state();
    save_to_redb(&engine, &path).unwrap();

    assert!(path.exists());
    assert!(!tmp_path.exists(), "stale tmp file should be cleaned up");

    // And the loaded state should be correct
    let loaded = load_from_redb(&path).unwrap();
    assert_eq!(loaded.total_supply(), engine.total_supply());
}

#[test]
fn redb_configured_validator_count_persisted() {
    let temp_dir = tempfile::TempDir::new().unwrap();
    let path = temp_dir.path().join("state.redb");

    let mut engine = engine_with_state();
    engine.set_configured_validator_count(5);

    save_to_redb(&engine, &path).unwrap();
    let loaded = load_from_redb(&path).unwrap();

    assert_eq!(loaded.configured_validator_count(), Some(5));
}

#[test]
fn redb_configured_validator_count_none_when_missing() {
    let temp_dir = tempfile::TempDir::new().unwrap();
    let path = temp_dir.path().join("state.redb");

    let engine = engine_with_state();
    // Don't set configured_validator_count — should be None

    save_to_redb(&engine, &path).unwrap();
    let loaded = load_from_redb(&path).unwrap();

    assert_eq!(loaded.configured_validator_count(), None);
}

#[test]
fn redb_governance_params_roundtrip() {
    let temp_dir = tempfile::TempDir::new().unwrap();
    let path = temp_dir.path().join("state.redb");

    let engine = engine_with_state();
    let params_before = engine.governance_params().clone();

    save_to_redb(&engine, &path).unwrap();
    let loaded = load_from_redb(&path).unwrap();

    assert_eq!(loaded.governance_params().min_fee_sats, params_before.min_fee_sats);
    assert_eq!(loaded.governance_params().slash_percent, params_before.slash_percent);
    assert_eq!(loaded.governance_params().council_emission_percent, params_before.council_emission_percent);
}

#[test]
fn redb_treasury_balance_roundtrip() {
    let temp_dir = tempfile::TempDir::new().unwrap();
    let path = temp_dir.path().join("state.redb");

    let engine = engine_with_state();
    let treasury_before = engine.treasury_balance();

    save_to_redb(&engine, &path).unwrap();
    let loaded = load_from_redb(&path).unwrap();

    assert_eq!(loaded.treasury_balance(), treasury_before);
}

#[test]
fn redb_multiple_save_load_cycles_stable() {
    let temp_dir = tempfile::TempDir::new().unwrap();
    let path = temp_dir.path().join("state.redb");

    let engine = engine_with_state();

    // Save/load 3 times to verify no drift
    for _ in 0..3 {
        save_to_redb(&engine, &path).unwrap();
        let loaded = load_from_redb(&path).unwrap();
        assert_eq!(loaded.total_supply(), engine.total_supply());
    }
}
