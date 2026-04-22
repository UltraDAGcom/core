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
const BRIDGE_ATTESTATIONS: TableDefinition<u64, &[u8]> = TableDefinition::new("bridge_attestations");
// Bridge signatures key: 8 bytes nonce + 20 bytes validator address = 28 bytes
// Value: 85 bytes = eth_address (20) + ecdsa_sig (65, r||s||v)
const BRIDGE_SIGNATURES: TableDefinition<&[u8], &[u8]> = TableDefinition::new("bridge_signatures");
// SmartAccount configs: address (20 bytes) -> postcard-serialized SmartAccountConfig
const SMART_ACCOUNTS: TableDefinition<&[u8; 20], &[u8]> = TableDefinition::new("smart_accounts");
// Name Registry: name string -> postcard-serialized (address, expiry, created_at, profile)
const NAME_REGISTRY: TableDefinition<&str, &[u8]> = TableDefinition::new("name_registry");
// Streams: stream_id (32 bytes) -> postcard-serialized Stream
const STREAMS: TableDefinition<&[u8; 32], &[u8]> = TableDefinition::new("streams");

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

    // Bridge attestations
    {
        let mut table = txn.open_table(BRIDGE_ATTESTATIONS)?;
        let snapshot = engine.snapshot();
        for (nonce, attestation) in &snapshot.bridge_attestations {
            let bytes = postcard::to_allocvec(attestation)
                .map_err(|e| PersistenceError::Serialization(e.to_string()))?;
            table.insert(*nonce, bytes.as_slice())?;
        }
    }

    // Bridge signatures (stored as 85 bytes: eth_address (20) + ecdsa_sig (65))
    {
        let mut table = txn.open_table(BRIDGE_SIGNATURES)?;
        let snapshot = engine.snapshot();
        for ((nonce, validator), combined_bytes) in &snapshot.bridge_signatures {
            let mut key = [0u8; 28];
            key[..8].copy_from_slice(&nonce.to_le_bytes());
            key[8..].copy_from_slice(&validator.0);
            // combined_bytes is 85 bytes (20 eth_addr + 65 ecdsa_sig) from snapshot()
            table.insert(key.as_slice(), combined_bytes.as_slice())?;
        }
    }

    // SmartAccounts
    {
        let mut table = txn.open_table(SMART_ACCOUNTS)?;
        for (addr, config) in engine.smart_accounts() {
            let bytes = postcard::to_allocvec(config)
                .map_err(|e| PersistenceError::Serialization(e.to_string()))?;
            table.insert(&addr.0, bytes.as_slice())?;
        }
    }

    // Name Registry
    {
        let mut table = txn.open_table(NAME_REGISTRY)?;
        let (names, expiry, created, profiles) = engine.name_registry_snapshot();
        // Store each name as a combined entry: (address, expiry, created_at, optional profile)
        for (name, addr) in &names {
            let exp = expiry.iter().find(|(n, _)| n == name).map(|(_, e)| *e).unwrap_or(0);
            let cre = created.iter().find(|(n, _)| n == name).map(|(_, c)| *c).unwrap_or(0);
            let prof = profiles.iter().find(|(n, _)| n == name).map(|(_, p)| p.clone());
            let entry = (addr, exp, cre, prof);
            let bytes = postcard::to_allocvec(&entry)
                .map_err(|e| PersistenceError::Serialization(e.to_string()))?;
            table.insert(name.as_str(), bytes.as_slice())?;
        }
    }

    // Streams
    {
        let mut table = txn.open_table(STREAMS)?;
        for (id, stream) in engine.all_streams() {
            let bytes = postcard::to_allocvec(stream)
                .map_err(|e| PersistenceError::Serialization(e.to_string()))?;
            table.insert(id, bytes.as_slice())?;
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
        table.insert("bridge_nonce", engine.get_bridge_nonce().to_le_bytes().as_slice())?;
        table.insert("bridge_contract_address", engine.bridge_contract_address().as_slice())?;

        // Persist used_release_nonces
        {
            let nonces_bytes = postcard::to_allocvec(&engine.snapshot().used_release_nonces)
                .map_err(|e| PersistenceError::Serialization(e.to_string()))?;
            table.insert("used_release_nonces", nonces_bytes.as_slice())?;
        }

        // Persist bridge_release_votes
        {
            let votes_bytes = postcard::to_allocvec(&engine.snapshot().bridge_release_votes)
                .map_err(|e| PersistenceError::Serialization(e.to_string()))?;
            table.insert("bridge_release_votes", votes_bytes.as_slice())?;
        }

        // Persist bridge_release_params (canonical recipient+amount for in-progress releases)
        {
            let snap = engine.snapshot();
            if let Some(ref params) = snap.bridge_release_params {
                let params_bytes = postcard::to_allocvec(params)
                    .map_err(|e| PersistenceError::Serialization(e.to_string()))?;
                table.insert("bridge_release_params", params_bytes.as_slice())?;
            }
        }

        // Persist bridge_release_first_vote_round (age tracking for stale vote pruning)
        {
            if !engine.bridge_release_first_vote_round_snapshot().is_empty() {
                let fvr_bytes = postcard::to_allocvec(&engine.bridge_release_first_vote_round_snapshot())
                    .map_err(|e| PersistenceError::Serialization(e.to_string()))?;
                table.insert("bridge_release_first_vote_round", fvr_bytes.as_slice())?;
            }
        }

        // Persist bridge_release_disagree_count (disagreement tracking for bridge releases)
        {
            if !engine.bridge_release_disagree_count_snapshot().is_empty() {
                let dc_bytes = postcard::to_allocvec(&engine.bridge_release_disagree_count_snapshot())
                    .map_err(|e| PersistenceError::Serialization(e.to_string()))?;
                table.insert("bridge_release_disagree_count", dc_bytes.as_slice())?;
            }
        }

        // Persist last_proposal_round (spam prevention cooldown)
        {
            let snap = engine.snapshot();
            if !snap.last_proposal_round.is_empty() {
                let lpr_bytes = postcard::to_allocvec(&snap.last_proposal_round)
                    .map_err(|e| PersistenceError::Serialization(e.to_string()))?;
                table.insert("last_proposal_round", lpr_bytes.as_slice())?;
            }
        }

        // Persist slashed_events (idempotency guard for slash_at_round)
        {
            let slashed = engine.slashed_events_snapshot();
            if !slashed.is_empty() {
                let slashed_bytes = postcard::to_allocvec(&slashed)
                    .map_err(|e| PersistenceError::Serialization(e.to_string()))?;
                table.insert("slashed_events", slashed_bytes.as_slice())?;
            }
        }

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
            let id_bytes: [u8; 8] = key_bytes[..8].try_into()
                .map_err(|_| PersistenceError::Serialization("vote key conversion failed".into()))?;
            let id = u64::from_le_bytes(id_bytes);
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

    // Bridge attestations
    let mut bridge_attestations = std::collections::HashMap::new();
    if let Ok(ba_table) = txn.open_table(BRIDGE_ATTESTATIONS) {
        for entry in ba_table.iter()? {
            let (k, v) = entry?;
            let nonce = k.value();
            let attestation: crate::bridge::BridgeAttestation = postcard::from_bytes(v.value())
                .map_err(|e| PersistenceError::Serialization(e.to_string()))?;
            bridge_attestations.insert(nonce, attestation);
        }
    }

    // Bridge signatures (85 bytes: eth_address (20) + ecdsa_sig (65))
    let mut bridge_sigs = std::collections::HashMap::new();
    if let Ok(bs_table) = txn.open_table(BRIDGE_SIGNATURES) {
        for entry in bs_table.iter()? {
            let (k, v) = entry?;
            let key_bytes = k.value();
            let val_bytes = v.value();
            if key_bytes.len() == 28 && val_bytes.len() == 85 {
                let nonce = u64::from_le_bytes(key_bytes[..8].try_into().unwrap());
                let mut addr_bytes = [0u8; 20];
                addr_bytes.copy_from_slice(&key_bytes[8..]);
                let addr = Address(addr_bytes);
                let mut packed = [0u8; 85];
                packed.copy_from_slice(val_bytes);
                bridge_sigs.insert((nonce, addr), packed);
            }
            // Skip entries with wrong length (legacy or corrupted)
        }
    }

    // SmartAccounts (graceful fallback for databases without this table)
    let mut smart_accounts_vec: Vec<(Address, crate::tx::smart_account::SmartAccountConfig)> = Vec::new();
    if let Ok(sa_table) = txn.open_table(SMART_ACCOUNTS) {
        for entry in sa_table.iter()? {
            let (k, v) = entry?;
            let addr = Address(*k.value());
            let config: crate::tx::smart_account::SmartAccountConfig = postcard::from_bytes(v.value())
                .map_err(|e| PersistenceError::Serialization(e.to_string()))?;
            smart_accounts_vec.push((addr, config));
        }
    }

    // Name Registry (graceful fallback for databases without this table)
    let mut name_entries: Vec<(String, Address)> = Vec::new();
    let mut name_expiry_entries: Vec<(String, u64)> = Vec::new();
    let mut name_created_entries: Vec<(String, u64)> = Vec::new();
    let mut name_profile_entries: Vec<(String, crate::tx::name_registry::NameProfile)> = Vec::new();
    if let Ok(nr_table) = txn.open_table(NAME_REGISTRY) {
        for entry in nr_table.iter()? {
            let (k, v) = entry?;
            let name = k.value().to_string();
            let (addr, exp, cre, prof): (
                Address, u64, u64, Option<crate::tx::name_registry::NameProfile>,
            ) = postcard::from_bytes(v.value())
                .map_err(|e| PersistenceError::Serialization(e.to_string()))?;
            name_entries.push((name.clone(), addr));
            name_expiry_entries.push((name.clone(), exp));
            name_created_entries.push((name.clone(), cre));
            if let Some(p) = prof {
                name_profile_entries.push((name, p));
            }
        }
    }

    // Streams (graceful fallback for databases without this table)
    let mut streams_vec: Vec<([u8; 32], crate::tx::stream::Stream)> = Vec::new();
    if let Ok(st_table) = txn.open_table(STREAMS) {
        for entry in st_table.iter()? {
            let (k, v) = entry?;
            let id = *k.value();
            let stream: crate::tx::stream::Stream = postcard::from_bytes(v.value())
                .map_err(|e| PersistenceError::Serialization(e.to_string()))?;
            streams_vec.push((id, stream));
        }
    }

    let bridge_nonce = read_u64(&meta, "bridge_nonce")?;

    // Load bridge contract address (20 bytes, default [0u8; 20] for legacy databases)
    let bridge_contract_address: [u8; 20] = meta.get("bridge_contract_address")?
        .and_then(|v| {
            let bytes = v.value();
            if bytes.len() == 20 {
                let mut arr = [0u8; 20];
                arr.copy_from_slice(bytes);
                Some(arr)
            } else {
                None
            }
        })
        .unwrap_or([0u8; 20]);

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

    // Restore bridge state that from_parts initializes to empty/0
    engine.restore_bridge_state(bridge_attestations, bridge_sigs, bridge_nonce);
    engine.set_bridge_contract_address(bridge_contract_address);

    // Restore SmartAccounts. The pocket_to_parent reverse-index is derived
    // state — it lives in memory only, not in redb — so it must be rebuilt
    // from each config's pockets list immediately after restore. Without
    // this, every pocket on the node becomes unspendable after a restart
    // (verify_smart_transfer's parent-fallback finds no entry) and spending
    // policies silently stop being enforced (check_spending_policy's
    // pocket→parent resolution falls through to the pocket's empty config).
    if !smart_accounts_vec.is_empty() {
        engine.restore_smart_accounts(smart_accounts_vec);
        engine.rebuild_pocket_map();
    }

    // Restore Name Registry
    if !name_entries.is_empty() {
        engine.restore_name_registry(name_entries, name_expiry_entries, name_created_entries, name_profile_entries);
    }

    // Restore Streams
    if !streams_vec.is_empty() {
        engine.restore_streams(streams_vec);
    }

    // Restore used_release_nonces from METADATA
    if let Some(nonces_val) = meta.get("used_release_nonces")? {
        let nonces: Vec<(u64, u64)> = postcard::from_bytes(nonces_val.value())
            .map_err(|e| PersistenceError::Serialization(e.to_string()))?;
        engine.restore_used_release_nonces(nonces);
    }

    // Restore bridge_release_votes from METADATA
    if let Some(votes_val) = meta.get("bridge_release_votes")? {
        let votes: Vec<((u64, u64), Vec<Address>)> = postcard::from_bytes(votes_val.value())
            .map_err(|e| PersistenceError::Serialization(e.to_string()))?;
        engine.restore_bridge_release_votes(votes);
    }

    // Restore bridge_release_params from METADATA (canonical recipient+amount for in-progress releases)
    if let Some(params_val) = meta.get("bridge_release_params")? {
        let params: Vec<((u64, u64), (Address, u64))> = postcard::from_bytes(params_val.value())
            .map_err(|e| PersistenceError::Serialization(e.to_string()))?;
        engine.restore_bridge_release_params(params);
    }

    // Restore bridge_release_first_vote_round (age tracking for stale vote pruning)
    if let Some(fvr_val) = meta.get("bridge_release_first_vote_round")? {
        let fvr: Vec<((u64, u64), u64)> = postcard::from_bytes(fvr_val.value())
            .map_err(|e| PersistenceError::Serialization(e.to_string()))?;
        engine.restore_bridge_release_first_vote_round(fvr);
    }

    // Restore bridge_release_disagree_count from METADATA
    if let Some(dc_val) = meta.get("bridge_release_disagree_count")? {
        let dc: Vec<((u64, u64), u64)> = postcard::from_bytes(dc_val.value())
            .map_err(|e| PersistenceError::Serialization(e.to_string()))?;
        engine.restore_bridge_release_disagree_count(dc);
    }

    // Restore last_proposal_round from METADATA (spam prevention cooldown)
    if let Some(lpr_val) = meta.get("last_proposal_round")? {
        let lpr: Vec<(Address, u64)> = postcard::from_bytes(lpr_val.value())
            .map_err(|e| PersistenceError::Serialization(e.to_string()))?;
        engine.restore_last_proposal_round(lpr);
    }

    // Restore slashed_events from METADATA (idempotency guard for slash_at_round)
    if let Some(se_val) = meta.get("slashed_events")? {
        let slashed: Vec<(Address, u64)> = postcard::from_bytes(se_val.value())
            .map_err(|e| PersistenceError::Serialization(e.to_string()))?;
        engine.restore_slashed_events(slashed);
    }

    // Verify supply invariant after all restores (including streams, bridge state, etc.).
    // Catches state divergence that wouldn't surface until the next finalized vertex.
    if let Err(e) = engine.verify_supply_invariant() {
        return Err(PersistenceError::Serialization(format!(
            "Supply invariant violation after loading from redb: {}. \
             Persisted state may be corrupted. Delete state.redb and restart with fast-sync.",
            e
        )));
    }

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
        Engineering => 0,
        Growth => 1,
        Legal => 2,
        Research => 3,
        Community => 4,
        Operations => 5,
        Security => 6,
    }
}

fn u8_to_council_category(val: u8) -> Option<crate::governance::CouncilSeatCategory> {
    use crate::governance::CouncilSeatCategory::*;
    match val {
        0 => Some(Engineering),
        1 => Some(Growth),
        2 => Some(Legal),
        3 => Some(Research),
        4 => Some(Community),
        5 => Some(Operations),
        6 => Some(Security),
        _ => None,
    }
}

fn read_u64(table: &redb::ReadOnlyTable<&str, &[u8]>, key: &str) -> Result<u64, PersistenceError> {
    match table.get(key)? {
        Some(v) => {
            let bytes = v.value();
            if bytes.len() >= 8 {
                let bytes_array: [u8; 8] = bytes[..8].try_into()
                    .map_err(|_| PersistenceError::Serialization("metadata value conversion failed".into()))?;
                Ok(u64::from_le_bytes(bytes_array))
            } else {
                Ok(0)
            }
        }
        None => Ok(0),
    }
}
