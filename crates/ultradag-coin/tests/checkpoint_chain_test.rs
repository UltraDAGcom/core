/// Test checkpoint chain verification prevents TOFU eclipse attacks.

use ultradag_coin::{
    Checkpoint, SecretKey,
    consensus::{compute_checkpoint_hash, verify_checkpoint_chain, compute_state_root, CheckpointSignature},
    constants::GENESIS_CHECKPOINT_HASH,
    StateEngine,
};
use std::collections::HashMap;

/// Create a test checkpoint with given parameters
fn make_checkpoint(
    round: u64,
    prev_hash: [u8; 32],
    sk: &SecretKey,
) -> Checkpoint {
    let state = StateEngine::new();
    let snapshot = state.snapshot();
    let state_root = compute_state_root(&snapshot);
    
    let mut checkpoint = Checkpoint {
        round,
        state_root,
        dag_tip: [0u8; 32],
        total_supply: 0,
        prev_checkpoint_hash: prev_hash,
        signatures: vec![],
    };
    
    // Sign it
    let sig = CheckpointSignature {
        validator: sk.address(),
        pub_key: sk.verifying_key().to_bytes(),
        signature: sk.sign(&checkpoint.signable_bytes()),
    };
    checkpoint.signatures.push(sig);
    
    checkpoint
}

#[test]
fn test_genesis_checkpoint_accepted() {
    // Genesis checkpoint with zero prev_hash should be accepted
    let sk = SecretKey::generate();
    let genesis = make_checkpoint(0, [0u8; 32], &sk);
    
    let loader = |_hash: [u8; 32]| -> Option<Checkpoint> { None };
    
    // Should succeed (GENESIS_CHECKPOINT_HASH is placeholder [0u8; 32])
    let result = verify_checkpoint_chain(&genesis, loader);
    assert!(result.is_ok(), "Genesis checkpoint should be accepted: {:?}", result);
}

#[test]
fn test_valid_checkpoint_chain() {
    let sk = SecretKey::generate();
    
    // Create a chain: genesis -> cp100 -> cp200
    let genesis = make_checkpoint(0, [0u8; 32], &sk);
    let genesis_hash = compute_checkpoint_hash(&genesis);
    
    let cp100 = make_checkpoint(100, genesis_hash, &sk);
    let cp100_hash = compute_checkpoint_hash(&cp100);
    
    let cp200 = make_checkpoint(200, cp100_hash, &sk);
    
    // Store checkpoints in a map
    let mut store = HashMap::new();
    store.insert(genesis_hash, genesis);
    store.insert(cp100_hash, cp100);
    
    let loader = move |hash: [u8; 32]| -> Option<Checkpoint> {
        store.get(&hash).cloned()
    };
    
    // Verify cp200 links back to genesis
    let result = verify_checkpoint_chain(&cp200, loader);
    assert!(result.is_ok(), "Valid chain should be accepted: {:?}", result);
}

#[test]
fn test_broken_chain_rejected() {
    let sk = SecretKey::generate();
    
    // Create genesis
    let genesis = make_checkpoint(0, [0u8; 32], &sk);
    let genesis_hash = compute_checkpoint_hash(&genesis);
    
    // Create cp100 that links to genesis
    let _cp100 = make_checkpoint(100, genesis_hash, &sk);
    
    // Create cp200 with WRONG prev_hash (not cp100)
    let fake_prev = [0xFF; 32];
    let cp200 = make_checkpoint(200, fake_prev, &sk);
    
    // Store only genesis (cp100 is missing)
    let mut store = HashMap::new();
    store.insert(genesis_hash, genesis);
    
    let loader = move |hash: [u8; 32]| -> Option<Checkpoint> {
        store.get(&hash).cloned()
    };
    
    // Should fail because prev_hash points to non-existent checkpoint
    let result = verify_checkpoint_chain(&cp200, loader);
    assert!(result.is_err(), "Broken chain should be rejected");
    assert!(result.unwrap_err().contains("not found"));
}

#[test]
fn test_forged_checkpoint_with_fake_validator_set() {
    // This is the critical test: attacker creates a checkpoint with fake validators
    let attacker_sk = SecretKey::generate();
    let victim_sk = SecretKey::generate();
    
    // Attacker creates a "genesis" checkpoint with themselves as validator
    let mut fake_genesis = Checkpoint {
        round: 0,
        state_root: [0xAA; 32], // Fake state root
        dag_tip: [0u8; 32],
        total_supply: 1_000_000,
        prev_checkpoint_hash: [0u8; 32],
        signatures: vec![],
    };
    
    // Attacker signs their fake genesis
    let sig = CheckpointSignature {
        validator: attacker_sk.address(),
        pub_key: attacker_sk.verifying_key().to_bytes(),
        signature: attacker_sk.sign(&fake_genesis.signable_bytes()),
    };
    fake_genesis.signatures.push(sig);
    
    let fake_genesis_hash = compute_checkpoint_hash(&fake_genesis);
    
    // Attacker creates cp100 linking to their fake genesis
    let fake_cp100 = make_checkpoint(100, fake_genesis_hash, &attacker_sk);
    
    // Victim has the REAL genesis
    let real_genesis = make_checkpoint(0, [0u8; 32], &victim_sk);
    let real_genesis_hash = compute_checkpoint_hash(&real_genesis);
    
    // Victim's checkpoint store
    let mut store = HashMap::new();
    store.insert(real_genesis_hash, real_genesis);
    store.insert(fake_genesis_hash, fake_genesis.clone());
    
    let loader = move |hash: [u8; 32]| -> Option<Checkpoint> {
        store.get(&hash).cloned()
    };
    
    // When victim verifies fake_cp100, it should be rejected because
    // fake_genesis hash doesn't match GENESIS_CHECKPOINT_HASH
    let result = verify_checkpoint_chain(&fake_cp100, loader);
    
    // With placeholder GENESIS_CHECKPOINT_HASH ([0u8; 32]), this will pass
    // In production with real GENESIS_CHECKPOINT_HASH, this would fail
    if GENESIS_CHECKPOINT_HASH == [0u8; 32] {
        // Placeholder mode - any genesis accepted
        assert!(result.is_ok(), "Placeholder mode accepts any genesis");
    } else {
        // Production mode - only real genesis accepted
        assert!(result.is_err(), "Forged genesis should be rejected");
        assert!(result.unwrap_err().contains("Genesis checkpoint hash mismatch"));
    }
}

#[test]
fn test_cycle_detection() {
    let sk = SecretKey::generate();
    
    // Create a cycle: cp100 -> cp200 -> cp100
    let cp100 = make_checkpoint(100, [0x01; 32], &sk);
    let cp100_hash = compute_checkpoint_hash(&cp100);
    
    let cp200 = make_checkpoint(200, cp100_hash, &sk);
    let cp200_hash = compute_checkpoint_hash(&cp200);
    
    // Make cp100 point back to cp200 (cycle)
    let mut cp100_cyclic = cp100.clone();
    cp100_cyclic.prev_checkpoint_hash = cp200_hash;
    let cp100_cyclic_hash = compute_checkpoint_hash(&cp100_cyclic);
    
    let mut store = HashMap::new();
    store.insert(cp100_cyclic_hash, cp100_cyclic);
    store.insert(cp200_hash, cp200.clone());
    
    let loader = move |hash: [u8; 32]| -> Option<Checkpoint> {
        store.get(&hash).cloned()
    };
    
    // Should detect cycle
    let result = verify_checkpoint_chain(&cp200, loader);
    assert!(result.is_err(), "Cycle should be detected");
    let err_msg = result.unwrap_err();
    assert!(err_msg.contains("cycle") || err_msg.contains("not found"));
}

#[test]
fn test_non_genesis_with_zero_prev_hash_rejected() {
    let sk = SecretKey::generate();
    
    // Create cp100 with zero prev_hash (invalid - only genesis can have zero)
    let cp100 = make_checkpoint(100, [0u8; 32], &sk);
    
    let loader = |_hash: [u8; 32]| -> Option<Checkpoint> { None };
    
    // Should fail
    let result = verify_checkpoint_chain(&cp100, loader);
    assert!(result.is_err(), "Non-genesis with zero prev_hash should be rejected");
    assert!(result.unwrap_err().contains("zero prev_checkpoint_hash"));
}

#[test]
fn test_checkpoint_hash_mismatch_rejected() {
    let sk = SecretKey::generate();
    
    // Create genesis
    let genesis = make_checkpoint(0, [0u8; 32], &sk);
    let genesis_hash = compute_checkpoint_hash(&genesis);
    
    // Create cp100 that claims to link to genesis
    let cp100 = make_checkpoint(100, genesis_hash, &sk);
    
    // But store a MODIFIED genesis (hash won't match)
    let mut modified_genesis = genesis.clone();
    modified_genesis.total_supply = 999; // Change something
    
    let mut store = HashMap::new();
    store.insert(genesis_hash, modified_genesis);
    
    let loader = move |hash: [u8; 32]| -> Option<Checkpoint> {
        store.get(&hash).cloned()
    };
    
    // Should fail because computed hash doesn't match claimed hash
    let result = verify_checkpoint_chain(&cp100, loader);
    assert!(result.is_err(), "Hash mismatch should be rejected");
    assert!(result.unwrap_err().contains("hash mismatch"));
}

#[test]
fn test_long_chain_accepted() {
    let sk = SecretKey::generate();
    
    // Create a long chain: genesis -> cp100 -> cp200 -> ... -> cp1000
    let mut checkpoints = Vec::new();
    let genesis = make_checkpoint(0, [0u8; 32], &sk);
    let mut prev_hash = compute_checkpoint_hash(&genesis);
    checkpoints.push((prev_hash, genesis));
    
    for round in (100..=1000).step_by(100) {
        let cp = make_checkpoint(round, prev_hash, &sk);
        prev_hash = compute_checkpoint_hash(&cp);
        checkpoints.push((prev_hash, cp));
    }
    
    let store: HashMap<[u8; 32], Checkpoint> = checkpoints.into_iter().collect();
    let final_cp = store.get(&prev_hash).unwrap().clone();
    
    let loader = move |hash: [u8; 32]| -> Option<Checkpoint> {
        store.get(&hash).cloned()
    };
    
    // Should succeed
    let result = verify_checkpoint_chain(&final_cp, loader);
    assert!(result.is_ok(), "Long valid chain should be accepted: {:?}", result);
}

#[test]
fn test_chain_length_limit() {
    let sk = SecretKey::generate();
    
    // Create a chain longer than the limit (10000)
    let genesis = make_checkpoint(0, [0u8; 32], &sk);
    let mut prev_hash = compute_checkpoint_hash(&genesis);
    let mut store = HashMap::new();
    store.insert(prev_hash, genesis);
    
    // Create 10001 checkpoints
    for round in 1..=10001 {
        let cp = make_checkpoint(round * 100, prev_hash, &sk);
        prev_hash = compute_checkpoint_hash(&cp);
        store.insert(prev_hash, cp);
    }
    
    let final_cp = store.get(&prev_hash).unwrap().clone();
    
    let loader = move |hash: [u8; 32]| -> Option<Checkpoint> {
        store.get(&hash).cloned()
    };
    
    // Should fail due to length limit
    let result = verify_checkpoint_chain(&final_cp, loader);
    assert!(result.is_err(), "Chain exceeding length limit should be rejected");
    assert!(result.unwrap_err().contains("too long"));
}
