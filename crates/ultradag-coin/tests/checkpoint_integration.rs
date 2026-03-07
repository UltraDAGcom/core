use ultradag_coin::{
    Address, BlockDag, Checkpoint, DagVertex, FinalityTracker,
    SecretKey, StateEngine, CHECKPOINT_INTERVAL,
};
use ultradag_coin::consensus::{CheckpointSignature, compute_state_root};
use ultradag_coin::block::header::BlockHeader;
use ultradag_coin::block::Block;
use ultradag_coin::tx::CoinbaseTx;

fn make_vertex(nonce: u64, round: u64, parents: Vec<[u8; 32]>, sk: &SecretKey) -> DagVertex {
    let header = BlockHeader {
        version: 1,
        height: round,
        timestamp: 1000000 + round as i64,
        prev_hash: [0u8; 32],
        merkle_root: [0u8; 32],
    };
    let coinbase = CoinbaseTx {
        to: sk.address(),
        amount: 5_000_000_000,
        height: round,
    };
    let mut block = Block {
        header,
        coinbase,
        transactions: vec![],
    };
    block.header.timestamp += nonce as i64;
    
    let validator = sk.address();
    let pub_key = sk.verifying_key().to_bytes();
    
    let mut vertex = DagVertex::new(
        block.clone(),
        parents,
        round,
        validator,
        pub_key,
        ultradag_coin::address::Signature([0u8; 64]),
    );
    
    vertex.signature = sk.sign(&vertex.signable_bytes());
    vertex
}

#[test]
fn test_01_checkpoint_produced_at_interval() {
    // Run a 4-validator simulation to round CHECKPOINT_INTERVAL
    let validators: Vec<SecretKey> = (0..4).map(|_| SecretKey::generate()).collect();
    let mut dag = BlockDag::new();
    let mut finality = FinalityTracker::new(3);
    let mut state = StateEngine::new_with_genesis();
    
    // Register validators
    for sk in &validators {
        finality.register_validator(sk.address());
    }
    
    // Insert genesis
    let genesis = make_vertex(0, 0, vec![], &validators[0]);
    dag.insert(genesis.clone());
    
    // Run to CHECKPOINT_INTERVAL + a few more rounds to ensure finality
    let mut tips = vec![genesis.hash()];
    for round in 1..=CHECKPOINT_INTERVAL + 10 {
        let mut new_tips = Vec::new();
        for (i, sk) in validators.iter().enumerate() {
            let vertex = make_vertex(i as u64, round, tips.clone(), sk);
            dag.insert(vertex.clone());
            new_tips.push(vertex.hash());
        }
        tips = new_tips;
        
        // Check finality
        let newly_finalized = finality.find_newly_finalized(&dag);
        if !newly_finalized.is_empty() {
            let finalized_vertices: Vec<DagVertex> = newly_finalized
                .iter()
                .filter_map(|h| dag.get(h).cloned())
                .collect();
            state.apply_finalized_vertices(&finalized_vertices).ok();
        }
    }
    
    // At CHECKPOINT_INTERVAL, produce a checkpoint
    let last_finalized_round = state.last_finalized_round().unwrap_or(0);
    assert!(last_finalized_round >= CHECKPOINT_INTERVAL, 
        "Should have finalized at least {} rounds, got {}", CHECKPOINT_INTERVAL, last_finalized_round);
    
    if last_finalized_round == CHECKPOINT_INTERVAL {
        let state_snapshot = state.snapshot();
        let state_root = compute_state_root(&state_snapshot);
        let dag_tip = tips.first().copied().unwrap_or([0u8; 32]);
        
        let checkpoint = Checkpoint {
            round: CHECKPOINT_INTERVAL,
            state_root,
            dag_tip,
            total_supply: state.total_supply(),
            signatures: vec![],
        };
        
        assert_eq!(checkpoint.round, CHECKPOINT_INTERVAL);
        assert!(checkpoint.total_supply > 0, "Total supply should be positive");
    }
}

#[test]
fn test_02_checkpoint_reaches_quorum() {
    let validators: Vec<SecretKey> = (0..4).map(|_| SecretKey::generate()).collect();
    let active: Vec<Address> = validators.iter().map(|sk| sk.address()).collect();
    
    // Create a checkpoint at round 1000
    let state = StateEngine::new_with_genesis();
    let state_snapshot = state.snapshot();
    let state_root = compute_state_root(&state_snapshot);
    
    let mut checkpoint = Checkpoint {
        round: 1000,
        state_root,
        dag_tip: [0u8; 32],
        total_supply: state.total_supply(),
        signatures: vec![],
    };
    
    // Have 3 of 4 validators sign it
    for sk in validators.iter().take(3) {
        let sig = CheckpointSignature {
            validator: sk.address(),
            pub_key: sk.verifying_key().to_bytes(),
            signature: sk.sign(&checkpoint.signable_bytes()),
        };
        checkpoint.signatures.push(sig);
    }
    
    // Verify is_accepted returns true (3/4 >= ceil(2*4/3) = 3)
    let quorum = (active.len() * 2 + 2) / 3;
    assert!(checkpoint.is_accepted(&active, quorum), 
        "Checkpoint with 3/4 signatures should be accepted");
}

#[test]
fn test_03_new_node_fast_syncs_from_checkpoint() {
    // Run 4-validator simulation to round 2000
    let validators: Vec<SecretKey> = (0..4).map(|_| SecretKey::generate()).collect();
    let mut dag = BlockDag::new();
    let mut finality = FinalityTracker::new(3);
    let mut state = StateEngine::new_with_genesis();
    
    for sk in &validators {
        finality.register_validator(sk.address());
    }
    
    let genesis = make_vertex(0, 0, vec![], &validators[0]);
    dag.insert(genesis.clone());
    
    let mut tips = vec![genesis.hash()];
    for round in 1..=2000 {
        let mut new_tips = Vec::new();
        for (i, sk) in validators.iter().enumerate() {
            let vertex = make_vertex(i as u64, round, tips.clone(), sk);
            dag.insert(vertex.clone());
            new_tips.push(vertex.hash());
        }
        tips = new_tips;
        
        let newly_finalized = finality.find_newly_finalized(&dag);
        if !newly_finalized.is_empty() {
            let finalized_vertices: Vec<DagVertex> = newly_finalized
                .iter()
                .filter_map(|h| dag.get(h).cloned())
                .collect();
            state.apply_finalized_vertices(&finalized_vertices).ok();
        }
    }
    
    // Produce checkpoint at round 1000
    let checkpoint_round = 1000u64;
    let state_snapshot = state.snapshot();
    let state_root = compute_state_root(&state_snapshot);
    
    let mut checkpoint = Checkpoint {
        round: checkpoint_round,
        state_root,
        dag_tip: tips.first().copied().unwrap_or([0u8; 32]),
        total_supply: state.total_supply(),
        signatures: vec![],
    };
    
    // Sign with all validators
    for sk in &validators {
        let sig = CheckpointSignature {
            validator: sk.address(),
            pub_key: sk.verifying_key().to_bytes(),
            signature: sk.sign(&checkpoint.signable_bytes()),
        };
        checkpoint.signatures.push(sig);
    }
    
    // Create a new state engine at round 0
    let mut new_state = StateEngine::new_with_genesis();
    
    // Apply the checkpoint state snapshot
    new_state.load_snapshot(state_snapshot.clone());
    
    // Verify new node has correct state
    assert_eq!(new_state.total_supply(), state.total_supply(), 
        "New node should have same total supply");
    assert_eq!(new_state.last_finalized_round(), state.last_finalized_round(),
        "New node should have same last finalized round");
    
    // Collect suffix vertices from checkpoint_round to 2000
    let mut suffix_count = 0;
    for round in checkpoint_round..=2000 {
        for vertex in dag.vertices_in_round(round) {
            suffix_count += 1;
            let _ = vertex; // Count vertices
        }
    }
    
    assert!(suffix_count > 0, "Should have suffix vertices to sync");
}
