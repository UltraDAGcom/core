use ultradag_coin::{
    Address, BlockDag, DagVertex, FinalityTracker, Mempool, SecretKey, Signature, StateEngine,
    Transaction,
};

fn make_signed_tx(sk: &SecretKey, to: Address, amount: u64, fee: u64, nonce: u64) -> Transaction {
    let mut tx = Transaction {
        from: sk.address(),
        to,
        amount,
        fee,
        nonce,
        pub_key: sk.verifying_key().to_bytes(),
        signature: Signature([0u8; 64]),
    };
    tx.signature = sk.sign(&tx.signable_bytes());
    tx
}

/// unique_id ensures different block hashes even with the same round
fn make_vertex(
    round: u64,
    validator: &SecretKey,
    parents: Vec<[u8; 32]>,
) -> DagVertex {
    make_vertex_unique(round, round, validator, parents)
}

fn make_vertex_unique(
    round: u64,
    unique_id: u64,
    validator: &SecretKey,
    parents: Vec<[u8; 32]>,
) -> DagVertex {
    use ultradag_coin::{Block, CoinbaseTx};

    // Calculate correct coinbase amount: block_reward + total_fees
    let block_reward = ultradag_coin::constants::block_reward(unique_id);
    let coinbase = CoinbaseTx {
        to: validator.address(),
        amount: block_reward, // No fees in this test, so just block_reward
        height: unique_id,
    };

    let header = ultradag_coin::block::header::BlockHeader {
        version: 1,
        height: unique_id,
        timestamp: unique_id as i64,
        prev_hash: parents.first().copied().unwrap_or([0u8; 32]),
        merkle_root: [0u8; 32],
    };

    let block = Block {
        header,
        coinbase,
        transactions: vec![],
    };

    let mut vertex = DagVertex {
        block,
        parent_hashes: parents,
        round,
        validator: validator.address(),
        pub_key: validator.verifying_key().to_bytes(),
        signature: Signature([0u8; 64]),
    };

    vertex.signature = validator.sign(&vertex.signable_bytes());
    vertex
}

#[test]
fn test_blockdag_persistence() {
    let temp_dir = std::env::temp_dir();
    let path = temp_dir.join("test_dag_persistence.json");
    
    // Create DAG with some vertices
    let mut dag = BlockDag::new();
    let sk1 = SecretKey::generate();
    let sk2 = SecretKey::generate();
    
    let v1 = make_vertex_unique(0, 0, &sk1, vec![]);
    let v2 = make_vertex_unique(0, 1, &sk2, vec![]);
    let v1_hash = v1.hash();
    let v2_hash = v2.hash();
    
    dag.insert(v1);
    dag.insert(v2);
    
    let v3 = make_vertex(1, &sk1, vec![v1_hash, v2_hash]);
    dag.insert(v3);
    
    // Save DAG
    dag.save(&path).expect("Failed to save DAG");
    
    // Load DAG
    let loaded_dag = BlockDag::load(&path).expect("Failed to load DAG");
    
    // Verify loaded DAG has same vertices
    assert_eq!(loaded_dag.vertices_in_round(0).len(), 2);
    assert_eq!(loaded_dag.vertices_in_round(1).len(), 1);
    assert!(loaded_dag.get(&v1_hash).is_some());
    assert!(loaded_dag.get(&v2_hash).is_some());
    
    // Cleanup
    std::fs::remove_file(&path).ok();
}

#[test]
fn test_finality_tracker_persistence() {
    let temp_dir = std::env::temp_dir();
    let path = temp_dir.join("test_finality_persistence.json");
    
    // Create finality tracker with validators
    let mut finality = FinalityTracker::new(3);
    let sk1 = SecretKey::generate();
    let sk2 = SecretKey::generate();
    let sk3 = SecretKey::generate();
    
    finality.register_validator(sk1.address());
    finality.register_validator(sk2.address());
    finality.register_validator(sk3.address());
    
    // Save finality tracker
    finality.save(&path).expect("Failed to save finality tracker");
    
    // Load finality tracker
    let loaded_finality = FinalityTracker::load(&path, 3).expect("Failed to load finality tracker");

    // Validators are NOT restored from snapshot (prevents stale phantom validators).
    // They must be rebuilt from DAG after loading.
    assert_eq!(loaded_finality.validator_count(), 0);
    
    // Cleanup
    std::fs::remove_file(&path).ok();
}

#[test]
fn test_state_engine_persistence() {
    let temp_dir = std::env::temp_dir();
    let path = temp_dir.join("test_state_persistence.json");
    
    // Create state engine and apply some vertices
    let mut state = StateEngine::new();
    let sk1 = SecretKey::generate();
    let sk2 = SecretKey::generate();
    
    let v1 = make_vertex(0, &sk1, vec![]);
    state.apply_vertex(&v1).expect("Failed to apply vertex");
    
    let initial_balance = state.balance(&sk1.address());
    assert!(initial_balance > 0);
    
    // Save state
    state.save(&path).expect("Failed to save state");
    
    // Load state
    let loaded_state = StateEngine::load(&path).expect("Failed to load state");
    
    // Verify loaded state has same balances
    assert_eq!(loaded_state.balance(&sk1.address()), initial_balance);
    assert_eq!(loaded_state.balance(&sk2.address()), 0);
    assert_eq!(loaded_state.total_supply(), state.total_supply());
    
    // Cleanup
    std::fs::remove_file(&path).ok();
}

#[test]
fn test_mempool_persistence() {
    let temp_dir = std::env::temp_dir();
    let path = temp_dir.join("test_mempool_persistence.json");
    
    // Create mempool with some transactions
    let mut mempool = Mempool::new();
    let sk1 = SecretKey::generate();
    let sk2 = SecretKey::generate();
    
    let tx1 = make_signed_tx(&sk1, sk2.address(), 100, 10, 0);
    let tx2 = make_signed_tx(&sk1, sk2.address(), 200, 20, 1);
    let tx3 = make_signed_tx(&sk2, sk1.address(), 50, 5, 0);
    
    mempool.insert(tx1.clone());
    mempool.insert(tx2.clone());
    mempool.insert(tx3.clone());
    
    assert_eq!(mempool.len(), 3);
    
    // Save mempool
    mempool.save(&path).expect("Failed to save mempool");
    
    // Load mempool
    let loaded_mempool = Mempool::load(&path).expect("Failed to load mempool");
    
    // Verify loaded mempool has same transactions
    assert_eq!(loaded_mempool.len(), 3);
    assert!(loaded_mempool.contains(&tx1.hash()));
    assert!(loaded_mempool.contains(&tx2.hash()));
    assert!(loaded_mempool.contains(&tx3.hash()));
    
    // Cleanup
    std::fs::remove_file(&path).ok();
}

#[test]
fn test_complete_node_state_persistence() {
    let temp_dir = std::env::temp_dir();
    let dag_path = temp_dir.join("test_node_dag.json");
    let finality_path = temp_dir.join("test_node_finality.json");
    let state_path = temp_dir.join("test_node_state.json");
    let mempool_path = temp_dir.join("test_node_mempool.json");
    
    // Create complete node state
    let mut dag = BlockDag::new();
    let mut finality = FinalityTracker::new(3);
    let mut state = StateEngine::new();
    let mut mempool = Mempool::new();
    
    let sk1 = SecretKey::generate();
    let sk2 = SecretKey::generate();
    let sk3 = SecretKey::generate();
    
    finality.register_validator(sk1.address());
    finality.register_validator(sk2.address());
    finality.register_validator(sk3.address());
    
    // Add vertices to DAG
    let v1 = make_vertex_unique(0, 0, &sk1, vec![]);
    let v2 = make_vertex_unique(0, 1, &sk2, vec![]);
    let v3 = make_vertex_unique(0, 2, &sk3, vec![]);
    
    dag.insert(v1.clone());
    dag.insert(v2.clone());
    dag.insert(v3.clone());
    
    // Apply to state
    state.apply_vertex(&v1).unwrap();
    state.apply_vertex(&v2).unwrap();
    state.apply_vertex(&v3).unwrap();
    
    // Add transactions to mempool
    let tx = make_signed_tx(&sk1, sk2.address(), 100, 10, 1);
    mempool.insert(tx.clone());
    
    // Save all components
    dag.save(&dag_path).expect("Failed to save DAG");
    finality.save(&finality_path).expect("Failed to save finality");
    state.save(&state_path).expect("Failed to save state");
    mempool.save(&mempool_path).expect("Failed to save mempool");
    
    // Load all components
    let loaded_dag = BlockDag::load(&dag_path).expect("Failed to load DAG");
    let loaded_finality = FinalityTracker::load(&finality_path, 3).expect("Failed to load finality");
    let loaded_state = StateEngine::load(&state_path).expect("Failed to load state");
    let loaded_mempool = Mempool::load(&mempool_path).expect("Failed to load mempool");
    
    // Verify complete state restoration
    assert_eq!(loaded_dag.vertices_in_round(0).len(), 3);
    // Validators are rebuilt from DAG, not from finality snapshot
    assert_eq!(loaded_finality.validator_count(), 0);
    // After rebuilding from DAG, we'd have 3 validators:
    let dag_validators = loaded_dag.all_validators();
    assert_eq!(dag_validators.len(), 3);
    assert!(loaded_state.balance(&sk1.address()) > 0);
    assert_eq!(loaded_mempool.len(), 1);
    assert!(loaded_mempool.contains(&tx.hash()));
    
    // Cleanup
    std::fs::remove_file(&dag_path).ok();
    std::fs::remove_file(&finality_path).ok();
    std::fs::remove_file(&state_path).ok();
    std::fs::remove_file(&mempool_path).ok();
}
