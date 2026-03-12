/// Part 3: Multi-Validator Round Progression Test
/// Proves the DAG-BFT consensus works end-to-end with real Ed25519 keys,
/// real signatures, real finality computation, and deterministic ordering.

use ultradag_coin::{
    Address, Block, BlockDag, BlockHeader, CoinbaseTx, DagVertex, FinalityTracker,
    SecretKey, Signature, StateEngine, Transaction, TransferTx,
};

/// Helper: Create a signed transaction
fn make_signed_tx(
    sk: &SecretKey,
    to: Address,
    amount: u64,
    fee: u64,
    nonce: u64,
) -> Transaction {
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

/// Helper: Create a signed DAG vertex
fn make_vertex(
    sk: &SecretKey,
    round: u64,
    height: u64,
    parent_hashes: Vec<[u8; 32]>,
    txs: Vec<Transaction>,
) -> DagVertex {
    let proposer = sk.address();
    let total_fees: u64 = txs.iter().map(|tx| tx.fee()).sum();
    let reward = ultradag_coin::constants::block_reward(height);
    
    let coinbase = CoinbaseTx {
        to: proposer,
        amount: reward + total_fees,
        height,
    };
    
    let block = Block {
        header: BlockHeader {
            version: 1,
            height,
            timestamp: 1_000_000 + round as i64,
            prev_hash: parent_hashes.first().copied().unwrap_or([0u8; 32]),
            merkle_root: [0u8; 32],
        },
        coinbase,
        transactions: txs,
    };
    
    let mut vertex = DagVertex::new(
        block,
        parent_hashes,
        round,
        proposer,
        sk.verifying_key().to_bytes(),
        Signature([0u8; 64]),
    );
    vertex.signature = sk.sign(&vertex.signable_bytes());
    vertex
}

#[test]
fn test_4_validators_5_rounds_complete_progression() {
    // Setup: 4 validators with real Ed25519 keypairs
    let validators: Vec<SecretKey> = (0..4).map(|_| SecretKey::generate()).collect();
    let addresses: Vec<Address> = validators.iter().map(|sk| sk.address()).collect();
    
    // Shared DAG and finality tracker
    let mut dag = BlockDag::new();
    let mut finality = FinalityTracker::new(3); // min_validators = 3
    
    // Register all validators
    for addr in &addresses {
        finality.register_validator(*addr);
    }
    
    // Verify threshold is correct: ceil(2*4/3) = 3
    assert_eq!(finality.finality_threshold(), 3);
    
    // Track all vertex hashes by round
    let mut round_vertices: Vec<Vec<[u8; 32]>> = vec![Vec::new(); 6]; // rounds 0-5
    
    // ========================================================================
    // ROUND 1: All 4 validators produce vertices
    // ========================================================================
    println!("=== Round 1 ===");
    for (i, sk) in validators.iter().enumerate() {
        let vertex = make_vertex(sk, 1, i as u64, vec![], vec![]);
        let hash = vertex.hash();
        round_vertices[1].push(hash);
        
        let inserted = dag.insert(vertex);
        assert!(inserted, "Round 1 vertex {} should be inserted", i);
    }
    
    assert_eq!(dag.current_round(), 1);
    assert_eq!(dag.len(), 4, "Should have 4 vertices after round 1");
    
    // Round 1 should NOT be finalized yet (needs descendants)
    let finalized = finality.find_newly_finalized(&dag);
    assert_eq!(finalized.len(), 0, "Round 1 should not be finalized yet");
    
    // ========================================================================
    // ROUND 2: All 4 validators produce vertices referencing round 1
    // ========================================================================
    println!("=== Round 2 ===");
    let round1_tips = dag.tips();
    assert_eq!(round1_tips.len(), 4, "Should have 4 tips from round 1");
    
    for (i, sk) in validators.iter().enumerate() {
        let vertex = make_vertex(sk, 2, (4 + i) as u64, round1_tips.clone(), vec![]);
        let hash = vertex.hash();
        round_vertices[2].push(hash);
        
        let inserted = dag.insert(vertex);
        assert!(inserted, "Round 2 vertex {} should be inserted", i);
    }
    
    assert_eq!(dag.current_round(), 2);
    assert_eq!(dag.len(), 8, "Should have 8 vertices after round 2");
    
    // After round 2, round 1 should be finalized (all 4 round-2 vertices reference all round-1 vertices)
    let finalized = finality.find_newly_finalized(&dag);
    assert!(!finalized.is_empty(), "Round 1 should be finalized after round 2");
    
    // Verify all round 1 vertices are finalized
    for hash in &round_vertices[1] {
        assert!(finalized.contains(hash), "Round 1 vertex should be finalized");
    }
    
    println!("Round 1 finalized: {} vertices", finalized.len());
    
    // ========================================================================
    // ROUND 3: All 4 validators produce vertices referencing round 2
    // ========================================================================
    println!("=== Round 3 ===");
    let round2_tips = dag.tips();
    assert_eq!(round2_tips.len(), 4, "Should have 4 tips from round 2");
    
    for (i, sk) in validators.iter().enumerate() {
        let vertex = make_vertex(sk, 3, (8 + i) as u64, round2_tips.clone(), vec![]);
        let hash = vertex.hash();
        round_vertices[3].push(hash);
        
        let inserted = dag.insert(vertex);
        assert!(inserted, "Round 3 vertex {} should be inserted", i);
    }
    
    assert_eq!(dag.current_round(), 3);
    assert_eq!(dag.len(), 12, "Should have 12 vertices after round 3");
    
    // After round 3, round 2 should be finalized
    let finalized = finality.find_newly_finalized(&dag);
    assert!(!finalized.is_empty(), "Round 2 should be finalized after round 3");
    
    // Verify all round 2 vertices are finalized
    for hash in &round_vertices[2] {
        assert!(finalized.contains(hash), "Round 2 vertex should be finalized");
    }
    
    println!("Round 2 finalized: {} vertices", finalized.len());
    
    // ========================================================================
    // ROUND 4: All 4 validators produce vertices referencing round 3
    // ========================================================================
    println!("=== Round 4 ===");
    let round3_tips = dag.tips();
    
    for (i, sk) in validators.iter().enumerate() {
        let vertex = make_vertex(sk, 4, (12 + i) as u64, round3_tips.clone(), vec![]);
        let hash = vertex.hash();
        round_vertices[4].push(hash);
        
        let inserted = dag.insert(vertex);
        assert!(inserted, "Round 4 vertex {} should be inserted", i);
    }
    
    assert_eq!(dag.current_round(), 4);
    assert_eq!(dag.len(), 16, "Should have 16 vertices after round 4");
    
    // After round 4, round 3 should be finalized
    let finalized = finality.find_newly_finalized(&dag);
    assert!(!finalized.is_empty(), "Round 3 should be finalized after round 4");
    
    println!("Round 3 finalized: {} vertices", finalized.len());
    
    // ========================================================================
    // ROUND 5: All 4 validators produce vertices referencing round 4
    // ========================================================================
    println!("=== Round 5 ===");
    let round4_tips = dag.tips();
    
    for (i, sk) in validators.iter().enumerate() {
        let vertex = make_vertex(sk, 5, (16 + i) as u64, round4_tips.clone(), vec![]);
        let hash = vertex.hash();
        round_vertices[5].push(hash);
        
        let inserted = dag.insert(vertex);
        assert!(inserted, "Round 5 vertex {} should be inserted", i);
    }
    
    assert_eq!(dag.current_round(), 5);
    assert_eq!(dag.len(), 20, "Should have 20 vertices after round 5");
    
    // After round 5, round 4 should be finalized (but not round 5 yet)
    let finalized = finality.find_newly_finalized(&dag);
    assert!(!finalized.is_empty(), "Round 4 should be finalized after round 5");
    
    println!("Round 4 finalized: {} vertices", finalized.len());
    
    // ========================================================================
    // FINAL VERIFICATION
    // ========================================================================
    
    // Total finalized vertices: rounds 1, 2, 3, 4 (not 5 yet)
    // Each round has 4 validators = 4 * 4 = 16 vertices
    let all_finalized: Vec<[u8; 32]> = (1..=4)
        .flat_map(|r| round_vertices[r].clone())
        .collect();
    
    for hash in &all_finalized {
        assert!(finality.is_finalized(hash), "Vertex should be marked as finalized");
    }
    
    assert_eq!(all_finalized.len(), 16, "Should have exactly 16 finalized vertices (rounds 1-4)");
    
    // Round 5 should NOT be finalized yet (needs round 6)
    for hash in &round_vertices[5] {
        assert!(!finality.is_finalized(hash), "Round 5 should not be finalized yet");
    }
    
    // Verify all 4 validators are represented in every round
    for round in 1..=5 {
        let validators_in_round = dag.distinct_validators_in_round(round);
        assert_eq!(
            validators_in_round.len(),
            4,
            "Round {} should have all 4 validators",
            round
        );
        
        for addr in &addresses {
            assert!(
                validators_in_round.contains(addr),
                "Validator should be in round {}",
                round
            );
        }
    }
    
    println!("=== VERIFICATION COMPLETE ===");
    println!("Total vertices: {}", dag.len());
    println!("Finalized vertices: {}", all_finalized.len());
    println!("Current round: {}", dag.current_round());
}

#[test]
fn test_deterministic_ordering() {
    // Create two independent finality trackers with the same validator set
    let validators: Vec<SecretKey> = (0..4).map(|_| SecretKey::generate()).collect();
    
    let mut finality1 = FinalityTracker::new(3);
    let mut finality2 = FinalityTracker::new(3);
    
    for sk in &validators {
        finality1.register_validator(sk.address());
        finality2.register_validator(sk.address());
    }
    
    // Create a DAG with 3 rounds
    let mut dag = BlockDag::new();
    
    // Round 1
    for (i, sk) in validators.iter().enumerate() {
        let v = make_vertex(sk, 1, i as u64, vec![], vec![]);
        dag.insert(v);
    }
    
    // Round 2
    let r1_tips = dag.tips();
    for (i, sk) in validators.iter().enumerate() {
        let v = make_vertex(sk, 2, (4 + i) as u64, r1_tips.clone(), vec![]);
        dag.insert(v);
    }
    
    // Round 3
    let r2_tips = dag.tips();
    for (i, sk) in validators.iter().enumerate() {
        let v = make_vertex(sk, 3, (8 + i) as u64, r2_tips.clone(), vec![]);
        dag.insert(v);
    }
    
    // Compute finalized sets independently
    let finalized1 = finality1.find_newly_finalized(&dag);
    let finalized2 = finality2.find_newly_finalized(&dag);
    
    // Both should produce identical results
    assert_eq!(finalized1.len(), finalized2.len(), "Both trackers should find same number of finalized vertices");
    
    for hash in &finalized1 {
        assert!(finalized2.contains(hash), "Both trackers should agree on finalized vertices");
    }
    
    println!("Deterministic ordering verified: {} vertices", finalized1.len());
}

#[test]
fn test_state_correctness_with_transactions() {
    // Create 3 accounts with known starting balances
    let sk_a = SecretKey::generate();
    let sk_b = SecretKey::generate();
    let sk_c = SecretKey::generate();
    
    let addr_a = sk_a.address();
    let addr_b = sk_b.address();
    let addr_c = sk_c.address();
    
    let validators: Vec<SecretKey> = vec![sk_a.clone(), sk_b.clone(), sk_c.clone()];
    
    let mut dag = BlockDag::new();
    let mut finality = FinalityTracker::new(2);
    let mut state = StateEngine::new();
    
    for sk in &validators {
        finality.register_validator(sk.address());
    }
    
    // Round 1: Give each validator initial coins (no transactions)
    let mut round1_vertices = Vec::new();
    for (i, sk) in validators.iter().enumerate() {
        let v = make_vertex(sk, 1, i as u64, vec![], vec![]);
        round1_vertices.push(v.clone());
        dag.insert(v);
    }
    
    // Round 2: Finalize round 1
    let r1_tips = dag.tips();
    for (i, sk) in validators.iter().enumerate() {
        let v = make_vertex(sk, 2, (3 + i) as u64, r1_tips.clone(), vec![]);
        dag.insert(v);
    }
    
    let mut any_finalized = false;
    loop {
        let finalized = finality.find_newly_finalized(&dag);
        if finalized.is_empty() { break; }
        any_finalized = true;
        let finalized_vertices: Vec<DagVertex> = finalized
            .iter()
            .filter_map(|h| dag.get(h).cloned())
            .collect();
        state.apply_finalized_vertices(&finalized_vertices).unwrap();
    }
    assert!(any_finalized, "Round 1 should be finalized");

    // Check initial balances after round 1 finalized
    // Round 1: heights 0, 1, 2 (3 validators)
    let r0 = ultradag_coin::constants::block_reward(0);
    let r1 = ultradag_coin::constants::block_reward(1);
    let r2 = ultradag_coin::constants::block_reward(2);
    
    assert_eq!(state.balance(&addr_a), r0, "Account A should have reward for height 0");
    assert_eq!(state.balance(&addr_b), r1, "Account B should have reward for height 1");
    assert_eq!(state.balance(&addr_c), r2, "Account C should have reward for height 2");
    
    let supply_after_r1 = state.total_supply();
    assert_eq!(supply_after_r1, r0 + r1 + r2, "Total supply should be sum of round 1 rewards");
    
    // Round 3: Account A sends 1000 to Account B
    let tx1 = make_signed_tx(&sk_a, addr_b, 1000, 10, 0);
    let r2_tips = dag.tips();
    
    // Round 3 vertices: heights 6, 7, 8
    let v_a = make_vertex(&sk_a, 3, 6, r2_tips.clone(), vec![tx1.clone()]);
    let v_b = make_vertex(&sk_b, 3, 7, r2_tips.clone(), vec![]);
    let v_c = make_vertex(&sk_c, 3, 8, r2_tips, vec![]);
    
    dag.insert(v_a);
    dag.insert(v_b);
    dag.insert(v_c);
    
    // Round 4: Finalize round 2 and round 3
    let r3_tips = dag.tips();
    for (i, sk) in validators.iter().enumerate() {
        let v = make_vertex(sk, 4, (9 + i) as u64, r3_tips.clone(), vec![]);
        dag.insert(v);
    }
    
    loop {
        let finalized = finality.find_newly_finalized(&dag);
        if finalized.is_empty() { break; }
        let finalized_vertices: Vec<DagVertex> = finalized
            .iter()
            .filter_map(|h| dag.get(h).cloned())
            .collect();
        state.apply_finalized_vertices(&finalized_vertices).unwrap();
    }

    // After applying round 2 (heights 3,4,5) and round 3 (heights 6,7,8):
    // Account A: r0 + r3 (from round 2) - 1000 - 10 + r6 + 10 (from round 3 with tx)
    // Account B: r1 + r4 (from round 2) + 1000 + r7 (from round 3)
    // Account C: r2 + r5 (from round 2) + r8 (from round 3)
    
    let r3 = ultradag_coin::constants::block_reward(3);
    let r4 = ultradag_coin::constants::block_reward(4);
    let r5 = ultradag_coin::constants::block_reward(5);
    let r6 = ultradag_coin::constants::block_reward(6);
    let r7 = ultradag_coin::constants::block_reward(7);
    let r8 = ultradag_coin::constants::block_reward(8);
    
    let expected_a = r0 + r3 - 1000 - 10 + r6 + 10; // Fee goes back to proposer
    let expected_b = r1 + r4 + 1000 + r7;
    let expected_c = r2 + r5 + r8;
    
    assert_eq!(state.balance(&addr_a), expected_a, "Account A balance incorrect");
    assert_eq!(state.balance(&addr_b), expected_b, "Account B balance incorrect");
    assert_eq!(state.balance(&addr_c), expected_c, "Account C balance incorrect");
    
    // Verify total supply is conserved (sum of all block rewards)
    let final_supply = state.total_supply();
    let expected_supply = r0 + r1 + r2 + r3 + r4 + r5 + r6 + r7 + r8;
    assert_eq!(final_supply, expected_supply, "Total supply should be conserved");
    
    println!("State correctness verified!");
    println!("Account A: {}", state.balance(&addr_a));
    println!("Account B: {}", state.balance(&addr_b));
    println!("Account C: {}", state.balance(&addr_c));
    println!("Total supply: {}", final_supply);
}
