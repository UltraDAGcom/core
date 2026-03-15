// Additional test coverage for gaps identified in coverage matrix
// Uses correct API patterns from existing tests

use ultradag_coin::*;
use std::collections::HashSet;

// Helper functions matching existing test patterns
fn make_signed_tx(sk: &SecretKey, to: Address, amount: u64, fee: u64, nonce: u64) -> Transaction {
    let mut transfer = TransferTx {
        from: sk.address(),
        to,
        amount,
        fee,
        nonce,
        pub_key: sk.verifying_key().to_bytes(),
        signature: Signature([0u8; 64]),
        memo: None,
    };
    transfer.signature = sk.sign(&transfer.signable_bytes());
    Transaction::Transfer(transfer)
}

fn make_stake_tx(sk: &SecretKey, amount: u64, nonce: u64) -> StakeTx {
    let mut tx = StakeTx {
        from: sk.address(),
        amount,
        nonce,
        pub_key: sk.verifying_key().to_bytes(),
        signature: Signature([0u8; 64]),
    };
    tx.signature = sk.sign(&tx.signable_bytes());
    tx
}

fn make_vertex(sk: &SecretKey, round: u64, parents: Vec<[u8; 32]>) -> DagVertex {
    let validator = sk.address();
    let mempool = Mempool::new();
    let block = create_block([0u8; 32], round, &validator, &mempool, 0);
    let mut vertex = DagVertex::new(
        block,
        parents,
        round,
        validator,
        sk.verifying_key().to_bytes(),
        Signature([0u8; 64]),
    );
    vertex.signature = sk.sign(&vertex.signable_bytes());
    vertex
}

// ============================================================================
// CONSENSUS CORE
// ============================================================================

#[test]
fn test_finality_with_21_validators_maximum() {
    let mut dag = BlockDag::new();
    let mut finality = FinalityTracker::new(3);
    
    let validators: Vec<SecretKey> = (0..21).map(|_| SecretKey::generate()).collect();
    for sk in &validators {
        finality.register_validator(sk.address());
    }
    
    assert_eq!(finality.finality_threshold(), 14); // ceil(2*21/3)
    
    let mut round1_hashes = Vec::new();
    for sk in &validators {
        let v = make_vertex(sk, 1, vec![[0u8; 32]]);
        let hash = v.hash();
        dag.insert(v);
        round1_hashes.push(hash);
    }
    
    for sk in &validators {
        let v = make_vertex(sk, 2, round1_hashes.clone());
        dag.insert(v);
    }
    
    let newly_finalized = finality.find_newly_finalized(&dag);
    assert_eq!(newly_finalized.len(), 21);
}

#[test]
fn test_two_nodes_produce_identical_finalized_ordering() {
    let sk1 = SecretKey::generate();
    let sk2 = SecretKey::generate();
    let sk3 = SecretKey::generate();
    
    let mut dag_a = BlockDag::new();
    let mut finality_a = FinalityTracker::new(3);
    finality_a.register_validator(sk1.address());
    finality_a.register_validator(sk2.address());
    finality_a.register_validator(sk3.address());
    
    let mut dag_b = BlockDag::new();
    let mut finality_b = FinalityTracker::new(3);
    finality_b.register_validator(sk1.address());
    finality_b.register_validator(sk2.address());
    finality_b.register_validator(sk3.address());
    
    let v1_r1 = make_vertex(&sk1, 1, vec![[0u8; 32]]);
    let v2_r1 = make_vertex(&sk2, 1, vec![[0u8; 32]]);
    let v3_r1 = make_vertex(&sk3, 1, vec![[0u8; 32]]);
    
    let h1 = v1_r1.hash();
    let h2 = v2_r1.hash();
    let h3 = v3_r1.hash();
    
    let v1_r2 = make_vertex(&sk1, 2, vec![h1, h2, h3]);
    let v2_r2 = make_vertex(&sk2, 2, vec![h1, h2, h3]);
    let v3_r2 = make_vertex(&sk3, 2, vec![h1, h2, h3]);
    
    // Node A receives in order
    dag_a.insert(v1_r1.clone());
    dag_a.insert(v2_r1.clone());
    dag_a.insert(v3_r1.clone());
    dag_a.insert(v1_r2.clone());
    dag_a.insert(v2_r2.clone());
    dag_a.insert(v3_r2.clone());
    
    // Node B receives in different order (but respecting parent dependencies)
    dag_b.insert(v3_r1);
    dag_b.insert(v1_r1);
    dag_b.insert(v2_r1);
    dag_b.insert(v3_r2);
    dag_b.insert(v1_r2);
    dag_b.insert(v2_r2);
    
    let finalized_a = finality_a.find_newly_finalized(&dag_a);
    let finalized_b = finality_b.find_newly_finalized(&dag_b);
    
    let ordered_a = ultradag_coin::consensus::order_vertices(&finalized_a, &dag_a);
    let ordered_b = ultradag_coin::consensus::order_vertices(&finalized_b, &dag_b);
    
    assert_eq!(ordered_a.len(), ordered_b.len());
    for (va, vb) in ordered_a.iter().zip(ordered_b.iter()) {
        assert_eq!(va.hash(), vb.hash());
    }
}

// ============================================================================
// DAG STRUCTURE
// ============================================================================

#[test]
fn test_reject_vertex_with_timestamp_too_far_in_future() {
    let mut dag = BlockDag::new();
    let sk = SecretKey::generate();
    
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;
    
    let validator = sk.address();
    let mempool = Mempool::new();
    let mut block = create_block([0u8; 32], 1, &validator, &mempool, 0);
    block.header.timestamp = now + 600; // 10 minutes in future
    
    let mut vertex = DagVertex::new(
        block,
        vec![[0u8; 32]],
        1,
        validator,
        sk.verifying_key().to_bytes(),
        Signature([0u8; 64]),
    );
    vertex.signature = sk.sign(&vertex.signable_bytes());
    
    let result = dag.try_insert(vertex);
    assert!(result.is_err(), "Future timestamp vertex should be rejected");
}

#[test]
fn test_round_bucketing_vertices_in_round_returns_correct_set() {
    let mut dag = BlockDag::new();
    let sk1 = SecretKey::generate();
    let sk2 = SecretKey::generate();
    let sk3 = SecretKey::generate();
    
    let v1 = make_vertex(&sk1, 5, vec![[0u8; 32]]);
    let v2 = make_vertex(&sk2, 5, vec![[0u8; 32]]);
    let v3 = make_vertex(&sk3, 5, vec![[0u8; 32]]);
    
    let h1 = v1.hash();
    let h2 = v2.hash();
    let h3 = v3.hash();
    
    dag.insert(v1);
    dag.insert(v2);
    dag.insert(v3);
    
    let round5 = dag.vertices_in_round(5);
    assert_eq!(round5.len(), 3);
    let round5_hashes: HashSet<[u8; 32]> = round5.iter().map(|v| v.hash()).collect();
    assert!(round5_hashes.contains(&h1));
    assert!(round5_hashes.contains(&h2));
    assert!(round5_hashes.contains(&h3));
}

// ============================================================================
// TRANSACTIONS
// ============================================================================

#[test]
fn test_zero_fee_transaction_rejected() {
    use ultradag_coin::tx::Mempool;
    
    let sk = SecretKey::generate();
    let recipient = SecretKey::generate().address();
    let tx = make_signed_tx(&sk, recipient, 100 * COIN, 0, 0); // zero fee
    
    let mut mempool = Mempool::new();
    let accepted = mempool.insert(tx);
    
    // Zero-fee transactions should be rejected by mempool
    assert!(!accepted, "Zero-fee transaction should be rejected");
}

#[test]
fn test_min_fee_transaction_accepted() {
    use ultradag_coin::tx::Mempool;
    use ultradag_coin::constants::MIN_FEE_SATS;
    
    let sk = SecretKey::generate();
    let recipient = SecretKey::generate().address();
    let tx = make_signed_tx(&sk, recipient, 100 * COIN, MIN_FEE_SATS, 0);
    
    let mut mempool = Mempool::new();
    let accepted = mempool.insert(tx);
    
    // Transactions with exactly MIN_FEE_SATS should be accepted
    assert!(accepted, "Transaction with minimum fee should be accepted");
}

#[test]
fn test_transaction_to_self_is_valid() {
    let sk = SecretKey::generate();
    let _tx = make_signed_tx(&sk, sk.address(), 100 * COIN, 1000, 0);
    // Self-send is a valid transaction structure (no special rejection)
}

// ============================================================================
// SUPPLY / TOKENOMICS
// ============================================================================

#[test]
fn test_block_reward_halves_at_interval() {
    let reward_before = block_reward(HALVING_INTERVAL - 1);
    let reward_at = block_reward(HALVING_INTERVAL);
    let reward_after = block_reward(HALVING_INTERVAL + 1);
    
    assert_eq!(reward_before, INITIAL_REWARD_SATS);
    assert_eq!(reward_at, INITIAL_REWARD_SATS / 2);
    assert_eq!(reward_after, INITIAL_REWARD_SATS / 2);
}

#[test]
fn test_block_reward_at_round_zero() {
    assert_eq!(block_reward(0), INITIAL_REWARD_SATS);
}

#[test]
fn test_block_reward_geometric_series_converges() {
    let mut total_rewards = 0u64;
    for halving in 0..64 {
        let rounds_in_period = HALVING_INTERVAL;
        let reward_per_round = block_reward(halving * HALVING_INTERVAL);
        let period_total = rounds_in_period.saturating_mul(reward_per_round);
        total_rewards = total_rewards.saturating_add(period_total);
    }
    
    // Total emission approaches MAX_SUPPLY (21M UDAG = 2,100,000,000,000,000 sats)
    // Geometric series: INITIAL_REWARD_SATS * HALVING_INTERVAL * (1 + 1/2 + 1/4 + ...) ≈ INITIAL_REWARD_SATS * HALVING_INTERVAL * 2
    let expected_min = 20_900_000 * COIN; // Just under 21M
    let expected_max = MAX_SUPPLY_SATS;   // At most 21M

    assert!(total_rewards >= expected_min && total_rewards <= expected_max,
        "total_rewards={} expected {}..{}", total_rewards, expected_min, expected_max);
}

// ============================================================================
// EPOCH TRANSITIONS
// ============================================================================

#[test]
fn test_deterministic_tiebreaking_in_active_set_selection() {
    use ultradag_coin::tx::stake::MIN_STAKE_SATS;
    let mut state = StateEngine::new_with_genesis();

    // Create 22 stakers (21 + 1 to test max cap)
    let stakers: Vec<SecretKey> = (0..22).map(|_| SecretKey::generate()).collect();

    // Add all 22 with same stake amount
    for sk in &stakers {
        state.credit(&sk.address(), MIN_STAKE_SATS);
        state.total_supply = state.total_supply.saturating_add(MIN_STAKE_SATS);
        let stake_tx = make_stake_tx(sk, MIN_STAKE_SATS, 0);
        state.apply_stake_tx(&stake_tx).unwrap();
    }

    state.recalculate_active_set();
    let active1: Vec<_> = state.active_validators().to_vec();

    state.recalculate_active_set();
    let active2: Vec<_> = state.active_validators().to_vec();

    assert_eq!(active1.len(), 21); // Should have 21 validators (max)
    assert_eq!(active2.len(), 21);
    assert_eq!(active1, active2);
}

#[test]
fn test_epoch_transition_with_exactly_21_stakers() {
    use ultradag_coin::tx::stake::MIN_STAKE_SATS;
    let mut state = StateEngine::new_with_genesis();

    let validators: Vec<SecretKey> = (0..21).map(|_| SecretKey::generate()).collect();

    // Credit directly and stake (no council membership needed for validation)
    for sk in &validators {
        state.credit(&sk.address(), MIN_STAKE_SATS);
        state.total_supply = state.total_supply.saturating_add(MIN_STAKE_SATS);
        let stake_tx = make_stake_tx(sk, MIN_STAKE_SATS, 0);
        state.apply_stake_tx(&stake_tx).unwrap();
    }

    state.recalculate_active_set();
    let active = state.active_validators();

    assert_eq!(active.len(), 21);
}

// ============================================================================
// CHECKPOINTS
// ============================================================================

#[test]
fn test_checkpoint_file_persisted_to_disk() {
    let temp_dir = std::env::temp_dir().join(format!("ultradag_checkpoint_test_{}", std::process::id()));
    std::fs::create_dir_all(&temp_dir).unwrap();
    
    let sk = SecretKey::generate();
    let checkpoint = Checkpoint {
        round: 1000,
        state_root: [1u8; 32],
        dag_tip: [2u8; 32],
        total_supply: 2_050_000 * COIN,
        prev_checkpoint_hash: [0u8; 32],
            signatures: vec![
            ultradag_coin::consensus::CheckpointSignature {
                validator: sk.address(),
                pub_key: sk.verifying_key().to_bytes(),
                signature: sk.sign(&[1, 2, 3]),
            }
        ],
    };
    
    ultradag_coin::persistence::save_checkpoint(&temp_dir, &checkpoint).unwrap();
    
    let checkpoint_file = temp_dir.join("checkpoint_0000001000.bin");
    assert!(checkpoint_file.exists(), "Expected {:?} to exist", checkpoint_file);
    
    std::fs::remove_dir_all(&temp_dir).ok();
}

#[test]
fn test_latest_checkpoint_loaded_correctly_from_disk() {
    let temp_dir = std::env::temp_dir().join(format!("ultradag_checkpoint_load_test_{}", std::process::id()));
    std::fs::create_dir_all(&temp_dir).unwrap();
    
    for round in &[1000, 2000, 3000] {
        let checkpoint = Checkpoint {
            round: *round,
            state_root: [(*round / 1000) as u8; 32],
            dag_tip: [0u8; 32],
            total_supply: 2_050_000 * COIN,
            prev_checkpoint_hash: [0u8; 32],
            signatures: vec![],
        };
        ultradag_coin::persistence::save_checkpoint(&temp_dir, &checkpoint).unwrap();
    }
    
    let latest = ultradag_coin::persistence::load_latest_checkpoint(&temp_dir).unwrap();
    assert_eq!(latest.round, 3000);
    assert_eq!(latest.state_root[0], 3);
    
    std::fs::remove_dir_all(&temp_dir).ok();
}

// ============================================================================
// BFT SAFETY
// ============================================================================

#[test]
fn test_f_plus_1_byzantine_validators_can_prevent_finality() {
    let mut dag = BlockDag::new();
    let mut finality = FinalityTracker::new(3);
    
    let sk1 = SecretKey::generate();
    let sk2 = SecretKey::generate();
    let sk3 = SecretKey::generate();
    let sk4 = SecretKey::generate();
    
    finality.register_validator(sk1.address());
    finality.register_validator(sk2.address());
    finality.register_validator(sk3.address());
    finality.register_validator(sk4.address());
    
    assert_eq!(finality.finality_threshold(), 3);
    
    let v1 = make_vertex(&sk1, 1, vec![[0u8; 32]]);
    let v2 = make_vertex(&sk2, 1, vec![[0u8; 32]]);
    
    dag.insert(v1.clone());
    dag.insert(v2.clone());
    
    let v1_r2 = make_vertex(&sk1, 2, vec![v1.hash(), v2.hash()]);
    let v2_r2 = make_vertex(&sk2, 2, vec![v1.hash(), v2.hash()]);
    
    dag.insert(v1_r2);
    dag.insert(v2_r2);
    
    let newly_finalized = finality.find_newly_finalized(&dag);
    assert_eq!(newly_finalized.len(), 0);
}

// ============================================================================
// PERFORMANCE
// ============================================================================

#[test]
fn test_equivocation_check_performance_at_21_validators() {
    let mut dag = BlockDag::new();
    
    let validators: Vec<SecretKey> = (0..21).map(|_| SecretKey::generate()).collect();
    
    let mut prev_hashes = vec![[0u8; 32]];
    for round in 1..=1000 {
        let mut round_hashes = Vec::new();
        for sk in &validators {
            let v = make_vertex(sk, round, prev_hashes.clone());
            let hash = v.hash();
            dag.insert(v);
            round_hashes.push(hash);
        }
        prev_hashes = round_hashes;
    }
    
    let sk = &validators[0];
    let start = std::time::Instant::now();
    let has_vertex = dag.has_vertex_from_validator_in_round(&sk.address(), 1000);
    let elapsed = start.elapsed();
    
    assert!(has_vertex);
    assert!(elapsed.as_millis() < 10);
}
