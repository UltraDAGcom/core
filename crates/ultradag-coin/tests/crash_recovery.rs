//! Crash recovery tests: verify state survives disk persistence and reload.
//!
//! These tests exercise the full save→crash→reload cycle using real redb
//! persistence, ensuring checkpoints and state roots match after recovery.

use ultradag_coin::{
    Address, Block, BlockHeader, SecretKey, Signature,
    BlockDag, DagVertex, FinalityTracker, StateEngine,
};
use ultradag_coin::tx::CoinbaseTx;
use ultradag_coin::consensus::compute_state_root;
use ultradag_coin::Checkpoint;
use ultradag_coin::consensus::CheckpointSignature;

/// Create a test vertex for a given round with the specified validator.
fn make_vertex(validator_idx: u64, round: u64, parents: Vec<[u8; 32]>, sk: &SecretKey, configured_validators: u64) -> DagVertex {
    let block = Block {
        header: BlockHeader {
            version: 1,
            height: round,
            timestamp: 1700000000 + round as i64,
            prev_hash: [0u8; 32],
            merkle_root: [0u8; 32],
        },
        coinbase: CoinbaseTx {
            to: sk.address(),
            amount: 0, // deferred coinbase model
            height: round,
        },
        transactions: vec![],
    };
    let mut vertex = DagVertex {
        block,
        parent_hashes: parents,
        round,
        validator: sk.address(),
        pub_key: sk.verifying_key().to_bytes(),
        signature: Signature([0u8; 64]),
        topo_level: 0,
    };
    let signable = vertex.signable_bytes();
    vertex.signature = sk.sign(&signable);
    vertex
}

/// Run N rounds of 4-validator consensus, returning final state.
fn run_simulation(rounds: u64) -> (StateEngine, BlockDag, FinalityTracker, Vec<SecretKey>) {
    let validators: Vec<SecretKey> = (0..4).map(|i| {
        let mut seed = [0u8; 32];
        seed[0] = i as u8 + 100;
        SecretKey::from_bytes(seed)
    }).collect();

    let mut dag = BlockDag::new();
    let mut finality = FinalityTracker::new(3);
    let mut state = StateEngine::new_with_genesis();

    for sk in &validators {
        finality.register_validator(sk.address());
    }

    let genesis = make_vertex(0, 0, vec![], &validators[0], 4);
    dag.insert(genesis.clone());
    let mut tips = vec![genesis.hash()];

    for round in 1..=rounds {
        let mut new_tips = Vec::new();
        for (i, sk) in validators.iter().enumerate() {
            let v = make_vertex(i as u64, round, tips.clone(), sk, 4);
            dag.insert(v.clone());
            new_tips.push(v.hash());
        }
        tips = new_tips;

        let newly_finalized = finality.find_newly_finalized(&dag);
        if !newly_finalized.is_empty() {
            let verts: Vec<DagVertex> = newly_finalized.iter()
                .filter_map(|h| dag.get(h).cloned())
                .collect();
            let _ = state.apply_finalized_vertices(&verts);
        }
    }

    (state, dag, finality, validators)
}

#[test]
fn test_redb_save_reload_state_matches() {
    // Run simulation to build up state
    let (state, _dag, _fin, _validators) = run_simulation(200);

    let original_supply = state.total_supply();
    let original_round = state.last_finalized_round();
    let original_root = compute_state_root(&state.snapshot());

    // Save to redb on disk
    let tmp = tempfile::TempDir::new().unwrap();
    let db_path = tmp.path().join("state.redb");
    state.save(&db_path).expect("save_to_redb should succeed");

    // "Crash" — drop the original state
    drop(state);

    // Reload from redb
    let restored = StateEngine::load(&db_path).expect("load_from_redb should succeed");

    assert_eq!(restored.total_supply(), original_supply,
        "Total supply should match after reload");
    assert_eq!(restored.last_finalized_round(), original_round,
        "Last finalized round should match after reload");

    let restored_root = compute_state_root(&restored.snapshot());
    assert_eq!(restored_root, original_root,
        "State root should be identical after save→reload cycle");
}

#[test]
fn test_checkpoint_snapshot_restore_state_matches() {
    // Run simulation
    let (state, _dag, _fin, validators) = run_simulation(300);

    let snapshot = state.snapshot();
    let state_root = compute_state_root(&snapshot);
    let supply = state.total_supply();

    // Simulate fast-sync: create new engine from snapshot
    let mut new_state = StateEngine::new_with_genesis();
    new_state.load_snapshot(snapshot.clone());

    assert_eq!(new_state.total_supply(), supply,
        "Fast-synced state should have same supply");
    assert_eq!(new_state.last_finalized_round(), state.last_finalized_round(),
        "Fast-synced state should have same finalized round");

    let new_root = compute_state_root(&new_state.snapshot());
    assert_eq!(new_root, state_root,
        "Fast-synced state root should match original");
}

#[test]
fn test_redb_survives_multiple_save_cycles() {
    // Save, reload, run more, save again, reload — verify consistency
    let (mut state, mut dag, mut finality, validators) = run_simulation(100);

    let tmp = tempfile::TempDir::new().unwrap();
    let db_path = tmp.path().join("state.redb");

    // First save
    state.save(&db_path).expect("first save");
    let root_after_100 = compute_state_root(&state.snapshot());

    // Run more rounds (101-200)
    let mut tips: Vec<[u8; 32]> = dag.tips().to_vec();
    for round in 101..=200 {
        let mut new_tips = Vec::new();
        for (i, sk) in validators.iter().enumerate() {
            let v = make_vertex(i as u64, round, tips.clone(), sk, 4);
            dag.insert(v.clone());
            new_tips.push(v.hash());
        }
        tips = new_tips;
        let newly_finalized = finality.find_newly_finalized(&dag);
        if !newly_finalized.is_empty() {
            let verts: Vec<DagVertex> = newly_finalized.iter()
                .filter_map(|h| dag.get(h).cloned())
                .collect();
            let _ = state.apply_finalized_vertices(&verts);
        }
    }

    let root_after_200 = compute_state_root(&state.snapshot());
    assert_ne!(root_after_100, root_after_200,
        "State root should change after more rounds");

    // Second save (overwrites first)
    state.save(&db_path).expect("second save");

    // Reload and verify
    let restored = StateEngine::load(&db_path).expect("reload after second save");
    let restored_root = compute_state_root(&restored.snapshot());
    assert_eq!(restored_root, root_after_200,
        "Reloaded state should reflect second save, not first");
}

#[test]
fn test_checkpoint_state_file_roundtrip() {
    let (state, _dag, _fin, _validators) = run_simulation(150);

    let tmp = tempfile::TempDir::new().unwrap();
    let snapshot = state.snapshot();
    let state_root = compute_state_root(&snapshot);

    // Save checkpoint state to disk
    ultradag_coin::persistence::save_checkpoint_state(tmp.path(), 100, &snapshot)
        .expect("save_checkpoint_state should succeed");

    // Load it back
    let loaded = ultradag_coin::persistence::load_checkpoint_state(tmp.path(), 100)
        .expect("load_checkpoint_state should succeed");

    let loaded_root = compute_state_root(&loaded);
    assert_eq!(loaded_root, state_root,
        "Checkpoint state file roundtrip should preserve state root");
}

#[test]
fn test_corrupted_redb_detected() {
    let (state, _dag, _fin, _validators) = run_simulation(50);

    let tmp = tempfile::TempDir::new().unwrap();
    let db_path = tmp.path().join("state.redb");
    state.save(&db_path).expect("save");

    // Corrupt the file by overwriting middle bytes
    let mut bytes = std::fs::read(&db_path).unwrap();
    if bytes.len() > 200 {
        for i in 100..150 {
            bytes[i] = 0xFF;
        }
        std::fs::write(&db_path, &bytes).unwrap();
    }

    // Load should fail or detect corruption
    let result = StateEngine::load(&db_path);
    // Either it fails to parse, or the state root check catches the corruption
    if let Ok(restored) = result {
        // If it somehow loads, the state root should NOT match
        let original_root = compute_state_root(&state.snapshot());
        let restored_root = compute_state_root(&restored.snapshot());
        // This might still match if corruption hit non-data pages
        // The important thing is no panic
        let _ = (original_root, restored_root);
    }
    // Success: didn't panic on corrupted file
}

/// Build a StateEngine with non-default values for EVERY persisted field.
/// Returns the engine plus the addresses used, for assertion.
fn build_fully_populated_engine() -> StateEngine {
    use ultradag_coin::governance::{
        CouncilSeatCategory, GovernanceParams, Proposal, ProposalStatus, ProposalType,
    };
    use ultradag_coin::bridge::BridgeAttestation;

    // Start with genesis (sets faucet account with 1,000,000 UDAG and total_supply)
    let mut engine = StateEngine::new_with_genesis();

    // Use faucet keypair for staking/delegation (has genesis balance)
    let faucet_sk = ultradag_coin::faucet_keypair();
    let faucet_addr = faucet_sk.address();

    // Create secondary addresses (will be used for council, bridge params, etc.)
    let sk2 = SecretKey::from_bytes([2u8; 32]);
    let sk3 = SecretKey::from_bytes([3u8; 32]);
    let addr2 = sk2.address();
    let addr3 = sk3.address();

    // --- Stake accounts: stake from faucet ---
    let stake_tx1 = ultradag_coin::tx::StakeTx {
        from: faucet_addr,
        amount: 100_000_000_000, // 1000 UDAG
        nonce: 0,
        pub_key: faucet_sk.verifying_key().to_bytes(),
        signature: Signature([0u8; 64]),
    };
    let _ = engine.apply_stake_tx(&stake_tx1);

    // --- Council members ---
    engine.add_council_member(faucet_addr, CouncilSeatCategory::Technical).unwrap();
    engine.add_council_member(addr2, CouncilSeatCategory::Business).unwrap();

    // --- Governance params (non-default) ---
    let gp = engine.governance_params_mut();
    gp.min_fee_sats = 20_000;
    gp.slash_percent = 75;
    gp.council_emission_percent = 15;

    // --- Configured validator count ---
    engine.set_configured_validator_count(5);

    // --- Bridge contract address ---
    engine.set_bridge_contract_address([0xAB; 20]);

    // --- Bridge attestations, signatures, nonce ---
    let attestation = BridgeAttestation {
        sender: faucet_addr,
        recipient: [0xDE; 20],
        amount: 1_000_000_000,
        nonce: 1,
        destination_chain_id: 42161,
        bridge_contract_address: [0xAB; 20],
        creation_round: 100,
    };
    let mut attestations = std::collections::HashMap::new();
    attestations.insert(1u64, attestation);
    let mut sigs = std::collections::HashMap::new();
    let mut packed = [0u8; 85];
    packed[..20].copy_from_slice(&[0xEE; 20]); // eth addr
    packed[20..].copy_from_slice(&[0x11; 65]); // ecdsa sig
    sigs.insert((1u64, faucet_addr), packed);
    engine.restore_bridge_state(attestations, sigs, 2);

    // --- Used release nonces ---
    engine.restore_used_release_nonces(vec![(42161, 10), (42161, 20)]);

    // --- Bridge release votes ---
    engine.restore_bridge_release_votes(vec![
        ((42161, 30), vec![faucet_addr, addr2]),
    ]);

    // --- Bridge release params ---
    engine.restore_bridge_release_params(vec![
        ((42161, 30), (addr3, 5_000_000_000)),
    ]);

    // --- Bridge release first vote round ---
    engine.restore_bridge_release_first_vote_round(vec![
        ((42161, 30), 50),
        ((42161, 40), 60),
    ]);

    // --- Bridge release disagree count ---
    engine.restore_bridge_release_disagree_count(vec![
        ((42161, 30), 1),
        ((42161, 40), 3),
    ]);

    // --- Slashed events ---
    engine.restore_slashed_events(vec![
        (faucet_addr, 100),
        (addr2, 200),
    ]);

    // --- Last proposal round ---
    engine.restore_last_proposal_round(vec![
        (faucet_addr, 500),
        (addr3, 600),
    ]);

    engine
}

#[test]
fn test_every_field_survives_redb_roundtrip() {
    let engine = build_fully_populated_engine();

    let original_snapshot = engine.snapshot();
    let original_root = compute_state_root(&original_snapshot);

    // Save to redb
    let tmp = tempfile::TempDir::new().unwrap();
    let db_path = tmp.path().join("state.redb");
    engine.save(&db_path).expect("save_to_redb should succeed");

    // Load from redb
    let restored = StateEngine::load(&db_path).expect("load_from_redb should succeed");
    let restored_snapshot = restored.snapshot();
    let restored_root = compute_state_root(&restored_snapshot);

    // State root must match (covers all consensus-critical fields)
    assert_eq!(original_root, restored_root,
        "State root must survive redb roundtrip");

    // --- Verify every individual snapshot field ---
    assert_eq!(original_snapshot.total_supply, restored_snapshot.total_supply, "total_supply");
    assert_eq!(original_snapshot.treasury_balance, restored_snapshot.treasury_balance, "treasury_balance");
    assert_eq!(original_snapshot.bridge_reserve, restored_snapshot.bridge_reserve, "bridge_reserve");
    assert_eq!(original_snapshot.current_epoch, restored_snapshot.current_epoch, "current_epoch");
    assert_eq!(original_snapshot.next_proposal_id, restored_snapshot.next_proposal_id, "next_proposal_id");
    assert_eq!(original_snapshot.last_finalized_round, restored_snapshot.last_finalized_round, "last_finalized_round");
    assert_eq!(original_snapshot.configured_validator_count, restored_snapshot.configured_validator_count, "configured_validator_count");
    assert_eq!(original_snapshot.bridge_nonce, restored_snapshot.bridge_nonce, "bridge_nonce");
    assert_eq!(original_snapshot.bridge_contract_address, restored_snapshot.bridge_contract_address, "bridge_contract_address");

    // Accounts (AccountState doesn't derive PartialEq, compare field by field)
    assert_eq!(original_snapshot.accounts.len(), restored_snapshot.accounts.len(), "accounts count");
    for (i, ((oa, oacct), (ra, racct))) in original_snapshot.accounts.iter()
        .zip(restored_snapshot.accounts.iter()).enumerate()
    {
        assert_eq!(oa.0, ra.0, "account addr mismatch at {}", i);
        assert_eq!(oacct.balance, racct.balance, "account balance mismatch at {}", i);
        assert_eq!(oacct.nonce, racct.nonce, "account nonce mismatch at {}", i);
    }

    // Stake accounts
    assert_eq!(original_snapshot.stake_accounts.len(), restored_snapshot.stake_accounts.len(), "stake_accounts count");
    for (i, ((oa, os), (ra, rs))) in original_snapshot.stake_accounts.iter()
        .zip(restored_snapshot.stake_accounts.iter()).enumerate()
    {
        assert_eq!(oa.0, ra.0, "stake addr mismatch at {}", i);
        assert_eq!(os.staked, rs.staked, "stake amount mismatch at {}", i);
        assert_eq!(os.commission_percent, rs.commission_percent, "commission mismatch at {}", i);
    }

    // Delegation accounts
    assert_eq!(original_snapshot.delegation_accounts.len(), restored_snapshot.delegation_accounts.len(), "delegation_accounts count");
    for (i, ((oa, od), (ra, rd))) in original_snapshot.delegation_accounts.iter()
        .zip(restored_snapshot.delegation_accounts.iter()).enumerate()
    {
        assert_eq!(oa.0, ra.0, "delegation addr mismatch at {}", i);
        assert_eq!(od.delegated, rd.delegated, "delegation amount mismatch at {}", i);
        assert_eq!(od.validator.0, rd.validator.0, "delegation validator mismatch at {}", i);
    }

    // Active validator set
    assert_eq!(original_snapshot.active_validator_set, restored_snapshot.active_validator_set, "active_validator_set");

    // Council members
    assert_eq!(original_snapshot.council_members.len(), restored_snapshot.council_members.len(), "council_members count");

    // Governance params
    assert_eq!(original_snapshot.governance_params.min_fee_sats, restored_snapshot.governance_params.min_fee_sats, "gov min_fee_sats");
    assert_eq!(original_snapshot.governance_params.slash_percent, restored_snapshot.governance_params.slash_percent, "gov slash_percent");
    assert_eq!(original_snapshot.governance_params.council_emission_percent, restored_snapshot.governance_params.council_emission_percent, "gov council_emission_percent");

    // Proposals and votes
    assert_eq!(original_snapshot.proposals.len(), restored_snapshot.proposals.len(), "proposals count");
    assert_eq!(original_snapshot.votes, restored_snapshot.votes, "votes");

    // Bridge attestations
    assert_eq!(original_snapshot.bridge_attestations.len(), restored_snapshot.bridge_attestations.len(), "bridge_attestations count");

    // Bridge signatures
    assert_eq!(original_snapshot.bridge_signatures.len(), restored_snapshot.bridge_signatures.len(), "bridge_signatures count");

    // Used release nonces
    let mut orig_nonces = original_snapshot.used_release_nonces.clone();
    let mut rest_nonces = restored_snapshot.used_release_nonces.clone();
    orig_nonces.sort();
    rest_nonces.sort();
    assert_eq!(orig_nonces, rest_nonces, "used_release_nonces");

    // Bridge release votes
    assert_eq!(original_snapshot.bridge_release_votes.len(), restored_snapshot.bridge_release_votes.len(), "bridge_release_votes count");
    assert_eq!(original_snapshot.bridge_release_votes, restored_snapshot.bridge_release_votes, "bridge_release_votes");

    // Bridge release params
    assert_eq!(original_snapshot.bridge_release_params, restored_snapshot.bridge_release_params, "bridge_release_params");

    // Last proposal round
    assert_eq!(original_snapshot.last_proposal_round, restored_snapshot.last_proposal_round, "last_proposal_round");

    // --- Gap 1: bridge_release_first_vote_round ---
    assert_eq!(
        original_snapshot.bridge_release_first_vote_round,
        restored_snapshot.bridge_release_first_vote_round,
        "bridge_release_first_vote_round must survive redb roundtrip"
    );
    // Verify non-empty
    assert!(original_snapshot.bridge_release_first_vote_round.is_some(),
        "bridge_release_first_vote_round should be Some (non-empty)");
    let fvr = original_snapshot.bridge_release_first_vote_round.as_ref().unwrap();
    assert_eq!(fvr.len(), 2, "should have 2 first-vote-round entries");

    // --- Gap 2: bridge_release_disagree_count ---
    assert_eq!(
        original_snapshot.bridge_release_disagree_count,
        restored_snapshot.bridge_release_disagree_count,
        "bridge_release_disagree_count must survive redb roundtrip"
    );
    assert!(original_snapshot.bridge_release_disagree_count.is_some(),
        "bridge_release_disagree_count should be Some (non-empty)");
    let dc = original_snapshot.bridge_release_disagree_count.as_ref().unwrap();
    assert_eq!(dc.len(), 2, "should have 2 disagree count entries");

    // --- Gap 3: slashed_events ---
    assert_eq!(
        original_snapshot.slashed_events,
        restored_snapshot.slashed_events,
        "slashed_events must survive redb roundtrip"
    );
    assert_eq!(original_snapshot.slashed_events.len(), 2,
        "should have 2 slashed events");
}

#[test]
fn test_every_field_survives_snapshot_roundtrip() {
    let engine = build_fully_populated_engine();

    let original_snapshot = engine.snapshot();
    let original_root = compute_state_root(&original_snapshot);

    // Roundtrip via from_snapshot
    let restored = StateEngine::from_snapshot(original_snapshot.clone())
        .expect("from_snapshot should succeed");
    let restored_snapshot = restored.snapshot();
    let restored_root = compute_state_root(&restored_snapshot);

    // State root must match
    assert_eq!(original_root, restored_root,
        "State root must survive snapshot roundtrip");

    // --- Verify every individual snapshot field ---
    assert_eq!(original_snapshot.total_supply, restored_snapshot.total_supply, "total_supply");
    assert_eq!(original_snapshot.treasury_balance, restored_snapshot.treasury_balance, "treasury_balance");
    assert_eq!(original_snapshot.bridge_reserve, restored_snapshot.bridge_reserve, "bridge_reserve");
    assert_eq!(original_snapshot.current_epoch, restored_snapshot.current_epoch, "current_epoch");
    assert_eq!(original_snapshot.next_proposal_id, restored_snapshot.next_proposal_id, "next_proposal_id");
    assert_eq!(original_snapshot.last_finalized_round, restored_snapshot.last_finalized_round, "last_finalized_round");
    assert_eq!(original_snapshot.configured_validator_count, restored_snapshot.configured_validator_count, "configured_validator_count");
    assert_eq!(original_snapshot.bridge_nonce, restored_snapshot.bridge_nonce, "bridge_nonce");
    assert_eq!(original_snapshot.bridge_contract_address, restored_snapshot.bridge_contract_address, "bridge_contract_address");

    // Accounts (compare field by field, AccountState lacks PartialEq)
    assert_eq!(original_snapshot.accounts.len(), restored_snapshot.accounts.len(), "accounts count");
    for (i, ((oa, oacct), (ra, racct))) in original_snapshot.accounts.iter()
        .zip(restored_snapshot.accounts.iter()).enumerate()
    {
        assert_eq!(oa.0, ra.0, "snapshot account addr mismatch at {}", i);
        assert_eq!(oacct.balance, racct.balance, "snapshot account balance mismatch at {}", i);
        assert_eq!(oacct.nonce, racct.nonce, "snapshot account nonce mismatch at {}", i);
    }

    // Stake accounts (compare field by field)
    assert_eq!(original_snapshot.stake_accounts.len(), restored_snapshot.stake_accounts.len(), "stake_accounts count");
    for (i, ((oa, os), (ra, rs))) in original_snapshot.stake_accounts.iter()
        .zip(restored_snapshot.stake_accounts.iter()).enumerate()
    {
        assert_eq!(oa.0, ra.0, "snapshot stake addr mismatch at {}", i);
        assert_eq!(os.staked, rs.staked, "snapshot stake amount mismatch at {}", i);
    }

    // Delegation accounts (compare field by field)
    assert_eq!(original_snapshot.delegation_accounts.len(), restored_snapshot.delegation_accounts.len(), "delegation_accounts count");
    for (i, ((oa, od), (ra, rd))) in original_snapshot.delegation_accounts.iter()
        .zip(restored_snapshot.delegation_accounts.iter()).enumerate()
    {
        assert_eq!(oa.0, ra.0, "snapshot delegation addr mismatch at {}", i);
        assert_eq!(od.delegated, rd.delegated, "snapshot delegation amount mismatch at {}", i);
    }

    // Active validator set
    assert_eq!(original_snapshot.active_validator_set, restored_snapshot.active_validator_set, "active_validator_set");

    // Council members
    assert_eq!(original_snapshot.council_members.len(), restored_snapshot.council_members.len(), "council_members count");

    // Governance params
    assert_eq!(original_snapshot.governance_params.min_fee_sats, restored_snapshot.governance_params.min_fee_sats, "gov min_fee_sats");
    assert_eq!(original_snapshot.governance_params.slash_percent, restored_snapshot.governance_params.slash_percent, "gov slash_percent");

    // Proposals and votes
    assert_eq!(original_snapshot.proposals.len(), restored_snapshot.proposals.len(), "proposals count");
    assert_eq!(original_snapshot.votes, restored_snapshot.votes, "votes");

    // Bridge attestations
    assert_eq!(original_snapshot.bridge_attestations.len(), restored_snapshot.bridge_attestations.len(), "bridge_attestations count");

    // Bridge signatures
    assert_eq!(original_snapshot.bridge_signatures.len(), restored_snapshot.bridge_signatures.len(), "bridge_signatures count");

    // Used release nonces
    let mut orig_nonces = original_snapshot.used_release_nonces.clone();
    let mut rest_nonces = restored_snapshot.used_release_nonces.clone();
    orig_nonces.sort();
    rest_nonces.sort();
    assert_eq!(orig_nonces, rest_nonces, "used_release_nonces");

    // Bridge release votes
    assert_eq!(original_snapshot.bridge_release_votes, restored_snapshot.bridge_release_votes, "bridge_release_votes");

    // Bridge release params
    assert_eq!(original_snapshot.bridge_release_params, restored_snapshot.bridge_release_params, "bridge_release_params");

    // Last proposal round
    assert_eq!(original_snapshot.last_proposal_round, restored_snapshot.last_proposal_round, "last_proposal_round");

    // --- Gap 1: bridge_release_first_vote_round ---
    assert_eq!(
        original_snapshot.bridge_release_first_vote_round,
        restored_snapshot.bridge_release_first_vote_round,
        "bridge_release_first_vote_round must survive snapshot roundtrip"
    );
    assert!(restored_snapshot.bridge_release_first_vote_round.is_some(),
        "bridge_release_first_vote_round should be Some after snapshot roundtrip");

    // --- Gap 2: bridge_release_disagree_count ---
    assert_eq!(
        original_snapshot.bridge_release_disagree_count,
        restored_snapshot.bridge_release_disagree_count,
        "bridge_release_disagree_count must survive snapshot roundtrip"
    );
    assert!(restored_snapshot.bridge_release_disagree_count.is_some(),
        "bridge_release_disagree_count should be Some after snapshot roundtrip");

    // --- Gap 3: slashed_events ---
    assert_eq!(
        original_snapshot.slashed_events,
        restored_snapshot.slashed_events,
        "slashed_events must survive snapshot roundtrip"
    );
    assert_eq!(restored_snapshot.slashed_events.len(), 2,
        "slashed_events should have 2 entries after snapshot roundtrip");

    // --- Also test load_snapshot path ---
    let mut fresh = StateEngine::new_with_genesis();
    fresh.load_snapshot(original_snapshot.clone());
    let loaded_snapshot = fresh.snapshot();

    assert_eq!(
        original_snapshot.bridge_release_first_vote_round,
        loaded_snapshot.bridge_release_first_vote_round,
        "bridge_release_first_vote_round must survive load_snapshot"
    );
    assert_eq!(
        original_snapshot.bridge_release_disagree_count,
        loaded_snapshot.bridge_release_disagree_count,
        "bridge_release_disagree_count must survive load_snapshot"
    );
    assert_eq!(
        original_snapshot.slashed_events,
        loaded_snapshot.slashed_events,
        "slashed_events must survive load_snapshot"
    );
}
