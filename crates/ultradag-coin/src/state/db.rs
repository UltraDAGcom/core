use std::path::Path;

use redb::{Database, ReadableTable, TableDefinition};

use crate::address::Address;
use crate::persistence::PersistenceError;
use crate::state::engine::{AccountState, DelegationAccount, StakeAccount, StateEngine};

// Table definitions for redb
const ACCOUNTS: TableDefinition<&[u8; 20], (u64, u64)> = TableDefinition::new("accounts");
const STAKES: TableDefinition<&[u8; 20], &[u8]> = TableDefinition::new("stakes_v2");
const PROPOSALS: TableDefinition<u64, &[u8]> = TableDefinition::new("proposals");
const VOTES: TableDefinition<&[u8], u8> = TableDefinition::new("votes");
const METADATA: TableDefinition<&str, &[u8]> = TableDefinition::new("metadata");
const ACTIVE_VALIDATORS: TableDefinition<u64, &[u8; 20]> = TableDefinition::new("active_validators");
const COUNCIL_MEMBERS: TableDefinition<&[u8; 20], u8> = TableDefinition::new("council_members");
const DELEGATIONS: TableDefinition<&[u8; 20], &[u8]> = TableDefinition::new("delegations");

impl From<redb::Error> for PersistenceError {
    fn from(e: redb::Error) -> Self {
        PersistenceError::Serialization(e.to_string())
    }
}

impl From<redb::DatabaseError> for PersistenceError {
    fn from(e: redb::DatabaseError) -> Self {
        PersistenceError::Serialization(e.to_string())
    }
}

impl From<redb::TransactionError> for PersistenceError {
    fn from(e: redb::TransactionError) -> Self {
        PersistenceError::Serialization(e.to_string())
    }
}

impl From<redb::TableError> for PersistenceError {
    fn from(e: redb::TableError) -> Self {
        PersistenceError::Serialization(e.to_string())
    }
}

impl From<redb::StorageError> for PersistenceError {
    fn from(e: redb::StorageError) -> Self {
        PersistenceError::Serialization(e.to_string())
    }
}

impl From<redb::CommitError> for PersistenceError {
    fn from(e: redb::CommitError) -> Self {
        PersistenceError::Serialization(e.to_string())
    }
}

/// Save StateEngine to a redb database file in a single ACID transaction.
/// Deletes the old file and creates a fresh database each time for simplicity.
/// This is called every ~10 rounds (50 seconds), so the overhead is negligible.
pub fn save_to_redb(engine: &StateEngine, path: &Path) -> Result<(), PersistenceError> {
    // Atomic: write to temp file, then rename
    let tmp_path = path.with_extension("redb.tmp");
    if tmp_path.exists() {
        let _ = std::fs::remove_file(&tmp_path);
    }

    let db = Database::create(&tmp_path)?;
    let txn = db.begin_write()?;

    // Accounts
    {
        let mut table = txn.open_table(ACCOUNTS)?;
        for (addr, acct) in engine.all_accounts() {
            table.insert(&addr.0, (acct.balance, acct.nonce))?;
        }
    }

    // Stakes (postcard serialized StakeAccount — includes commission_percent)
    {
        let mut table = txn.open_table(STAKES)?;
        for (addr, stake) in engine.all_stakes() {
            let bytes = postcard::to_allocvec(stake)
                .map_err(|e| PersistenceError::Serialization(e.to_string()))?;
            table.insert(&addr.0, bytes.as_slice())?;
        }
    }

    // Delegations
    {
        let mut table = txn.open_table(DELEGATIONS)?;
        for (addr, delegation) in engine.all_delegations() {
            let bytes = postcard::to_allocvec(delegation)
                .map_err(|e| PersistenceError::Serialization(e.to_string()))?;
            table.insert(&addr.0, bytes.as_slice())?;
        }
    }

    // Active validators
    {
        let mut table = txn.open_table(ACTIVE_VALIDATORS)?;
        for (i, addr) in engine.active_validators().iter().enumerate() {
            table.insert(i as u64, &addr.0)?;
        }
    }

    // Proposals
    {
        let mut table = txn.open_table(PROPOSALS)?;
        for (id, proposal) in engine.all_proposals() {
            let bytes = postcard::to_allocvec(proposal)
                .map_err(|e| PersistenceError::Serialization(e.to_string()))?;
            table.insert(*id, bytes.as_slice())?;
        }
    }

    // Votes
    {
        let mut table = txn.open_table(VOTES)?;
        for ((id, addr), vote) in engine.all_votes() {
            let mut key = [0u8; 28];
            key[..8].copy_from_slice(&id.to_le_bytes());
            key[8..].copy_from_slice(&addr.0);
            table.insert(key.as_slice(), if *vote { 1u8 } else { 0u8 })?;
        }
    }

    // Council members
    {
        let mut table = txn.open_table(COUNCIL_MEMBERS)?;
        for (addr, category) in engine.council_members() {
            let cat_byte = council_category_to_u8(category);
            table.insert(&addr.0, cat_byte)?;
        }
    }

    // Metadata
    {
        let mut table = txn.open_table(METADATA)?;
        table.insert("total_supply", engine.total_supply().to_le_bytes().as_slice())?;

        let lfr = engine.last_finalized_round().unwrap_or(u64::MAX);
        table.insert("last_finalized_round", lfr.to_le_bytes().as_slice())?;

        table.insert("current_epoch", engine.current_epoch().to_le_bytes().as_slice())?;
        table.insert("next_proposal_id", engine.next_proposal_id().to_le_bytes().as_slice())?;

        let gp_bytes = postcard::to_allocvec(engine.governance_params())
            .map_err(|e| PersistenceError::Serialization(e.to_string()))?;
        table.insert("governance_params", gp_bytes.as_slice())?;

        if let Some(cvc) = engine.configured_validator_count() {
            table.insert("configured_validator_count", cvc.to_le_bytes().as_slice())?;
        }

        table.insert("treasury_balance", engine.treasury_balance().to_le_bytes().as_slice())?;
        table.insert("bridge_reserve", engine.bridge_reserve().to_le_bytes().as_slice())?;

        // Compute and store state root for integrity verification on reload.
        // This catches silent corruption from disk errors, partial writes, or bugs.
        let snapshot = engine.snapshot();
        let state_root = crate::consensus::compute_state_root(&snapshot);
        table.insert("state_root", state_root.as_slice())?;
    }

    txn.commit()?;
    drop(db);

    // Fsync the temp file to ensure committed data is durable on disk before rename.
    // redb commits to its own WAL, but we need the OS to flush the file data
    // so that rename doesn't point to a partially-written database after a crash.
    match std::fs::File::open(&tmp_path) {
        Ok(f) => {
            if let Err(e) = f.sync_all() {
                tracing::error!("Failed to fsync redb temp file before rename: {}", e);
                let _ = std::fs::remove_file(&tmp_path);
                return Err(e.into());
            }
        }
        Err(e) => {
            tracing::error!("Failed to open redb temp file for fsync: {}", e);
            return Err(e.into());
        }
    }

    // Atomic rename
    if let Err(e) = std::fs::rename(&tmp_path, path) {
        // Clean up temp file on rename failure to avoid stale state
        let _ = std::fs::remove_file(&tmp_path);
        return Err(PersistenceError::Io(e));
    }

    // Fsync parent directory to ensure the rename is durable
    if let Some(parent) = path.parent() {
        if let Ok(dir) = std::fs::File::open(parent) {
            let _ = dir.sync_all();
        }
    }

    Ok(())
}

/// Load StateEngine from a redb database file.
pub fn load_from_redb(path: &Path) -> Result<StateEngine, PersistenceError> {
    let db = Database::open(path)?;
    let txn = db.begin_read()?;

    // Metadata
    let meta = txn.open_table(METADATA)?;
    let total_supply = read_u64(&meta, "total_supply")?;
    let lfr_raw = read_u64(&meta, "last_finalized_round")?;
    let last_finalized_round = if lfr_raw == u64::MAX { None } else { Some(lfr_raw) };
    let current_epoch = read_u64(&meta, "current_epoch")?;
    let next_proposal_id = read_u64(&meta, "next_proposal_id")?;
    let governance_params = if let Some(gp) = meta.get("governance_params")? {
        postcard::from_bytes(gp.value())
            .map_err(|e| PersistenceError::Serialization(e.to_string()))?
    } else {
        crate::governance::GovernanceParams::default()
    };

    // Accounts
    let mut accounts = std::collections::HashMap::new();
    let acct_table = txn.open_table(ACCOUNTS)?;
    for entry in acct_table.iter()? {
        let (k, v) = entry?;
        let addr = Address(*k.value());
        let (balance, nonce) = v.value();
        accounts.insert(addr, AccountState { balance, nonce });
    }

    // Stakes (postcard serialized StakeAccount)
    let mut stake_accounts = std::collections::HashMap::new();
    let stake_table = txn.open_table(STAKES)?;
    for entry in stake_table.iter()? {
        let (k, v) = entry?;
        let addr = Address(*k.value());
        let stake: StakeAccount = postcard::from_bytes(v.value())
            .map_err(|e| PersistenceError::Serialization(e.to_string()))?;
        stake_accounts.insert(addr, stake);
    }

    // Active validators
    let av_table = txn.open_table(ACTIVE_VALIDATORS)?;
    let mut av_entries: Vec<(u64, Address)> = Vec::new();
    for entry in av_table.iter()? {
        let (k, v) = entry?;
        av_entries.push((k.value(), Address(*v.value())));
    }
    av_entries.sort_by_key(|(idx, _)| *idx);
    let active_validator_set: Vec<Address> = av_entries.into_iter().map(|(_, addr)| addr).collect();

    // Proposals
    let mut proposals = std::collections::HashMap::new();
    let prop_table = txn.open_table(PROPOSALS)?;
    for entry in prop_table.iter()? {
        let (k, v) = entry?;
        let proposal: crate::governance::Proposal = postcard::from_bytes(v.value())
            .map_err(|e| PersistenceError::Serialization(e.to_string()))?;
        proposals.insert(k.value(), proposal);
    }

    // Votes
    let mut votes = std::collections::HashMap::new();
    let vote_table = txn.open_table(VOTES)?;
    for entry in vote_table.iter()? {
        let (k, v) = entry?;
        let key_bytes = k.value();
        if key_bytes.len() == 28 {
            let id = u64::from_le_bytes(key_bytes[..8].try_into().unwrap());
            let mut addr_bytes = [0u8; 20];
            addr_bytes.copy_from_slice(&key_bytes[8..]);
            let addr = Address(addr_bytes);
            votes.insert((id, addr), v.value() == 1);
        }
    }

    let configured_validator_count = {
        let raw = read_u64(&meta, "configured_validator_count")?;
        if raw > 0 { Some(raw) } else { None }
    };

    let treasury_balance = read_u64(&meta, "treasury_balance")?;
    let bridge_reserve = read_u64(&meta, "bridge_reserve")?;

    // Delegations
    let mut delegation_accounts = std::collections::HashMap::new();
    if let Ok(deleg_table) = txn.open_table(DELEGATIONS) {
        for entry in deleg_table.iter()? {
            let (k, v) = entry?;
            let addr = Address(*k.value());
            let delegation: DelegationAccount = postcard::from_bytes(v.value())
                .map_err(|e| PersistenceError::Serialization(e.to_string()))?;
            delegation_accounts.insert(addr, delegation);
        }
    }
    // Legacy databases without DELEGATIONS table will have empty delegation_accounts

    // Council members (graceful fallback for legacy databases without this table)
    let mut council_members = std::collections::HashMap::new();
    if let Ok(cm_table) = txn.open_table(COUNCIL_MEMBERS) {
        for entry in cm_table.iter()? {
            let (k, v) = entry?;
            let addr = Address(*k.value());
            if let Some(category) = u8_to_council_category(v.value()) {
                council_members.insert(addr, category);
            }
        }
    }
    // Legacy databases without COUNCIL_MEMBERS table will have empty council_members

    let mut engine = StateEngine::from_parts(
        accounts,
        stake_accounts,
        active_validator_set,
        current_epoch,
        total_supply,
        last_finalized_round,
        proposals,
        votes,
        next_proposal_id,
        governance_params,
        configured_validator_count,
        council_members,
        treasury_balance,
        delegation_accounts,
        bridge_reserve,
    ).map_err(|e| PersistenceError::Serialization(e.to_string()))?;

    // Verify state integrity: recompute state root and compare against stored value.
    // Catches silent corruption from disk errors, partial writes, or software bugs.
    {
        let stored_root: Option<[u8; 32]> = meta.get("state_root")?
            .and_then(|v| {
                let bytes = v.value();
                if bytes.len() == 32 {
                    let mut arr = [0u8; 32];
                    arr.copy_from_slice(bytes);
                    Some(arr)
                } else {
                    None
                }
            });

        if let Some(expected) = stored_root {
            let snapshot = engine.snapshot();
            let computed = crate::consensus::compute_state_root(&snapshot);
            if computed != expected {
                return Err(PersistenceError::Serialization(format!(
                    "State root mismatch after loading from redb: stored={:02x}{:02x}{:02x}{:02x}.. computed={:02x}{:02x}{:02x}{:02x}.. \
                     Persisted state may be corrupted. Delete state.redb and restart with fast-sync.",
                    expected[0], expected[1], expected[2], expected[3],
                    computed[0], computed[1], computed[2], computed[3]
                )));
            }
        }
        // No stored root = legacy database from before this check was added; skip verification
    }

    // Reconcile epoch after loading
    if let Some(round) = engine.last_finalized_round() {
        let expected_epoch = crate::constants::epoch_of(round);
        if expected_epoch != engine.current_epoch() {
            engine.recalculate_active_set();
            engine.set_current_epoch(expected_epoch);
        }
    }

    Ok(engine)
}

fn council_category_to_u8(cat: &crate::governance::CouncilSeatCategory) -> u8 {
    use crate::governance::CouncilSeatCategory::*;
    match cat {
        Technical => 0,
        Business => 1,
        Legal => 2,
        Academic => 3,
        Community => 4,
        Foundation => 5,
    }
}

fn u8_to_council_category(val: u8) -> Option<crate::governance::CouncilSeatCategory> {
    use crate::governance::CouncilSeatCategory::*;
    match val {
        0 => Some(Technical),
        1 => Some(Business),
        2 => Some(Legal),
        3 => Some(Academic),
        4 => Some(Community),
        5 => Some(Foundation),
        _ => None,
    }
}

fn read_u64(table: &redb::ReadOnlyTable<&str, &[u8]>, key: &str) -> Result<u64, PersistenceError> {
    match table.get(key)? {
        Some(v) => {
            let bytes = v.value();
            if bytes.len() >= 8 {
                Ok(u64::from_le_bytes(bytes[..8].try_into().unwrap()))
            } else {
                Ok(0)
            }
        }
        None => Ok(0),
    }
}
