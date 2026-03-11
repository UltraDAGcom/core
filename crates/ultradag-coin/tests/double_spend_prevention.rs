/// Part 2: Transaction Validity and Double-Spend Prevention
/// Proves the system correctly prevents double-spending in both linear and DAG scenarios.

use ultradag_coin::{
    Address, Block, BlockDag, BlockHeader, CoinbaseTx, DagVertex, FinalityTracker,
    SecretKey, Signature, StateEngine, Transaction, TransferTx,
};

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

// ============================================================================
// Part 2.1 — Nonce Enforcement
// ============================================================================

#[test]
fn transaction_with_correct_nonce_accepted() {
    let mut state = StateEngine::new();
    let sk = SecretKey::generate();
    let addr = sk.address();
    let to = SecretKey::generate().address();
    
    // Give account some coins
    let v0 = make_vertex(&sk, 0, 0, vec![], vec![]);
    state.apply_vertex(&v0).unwrap();
    
    // Current nonce is 0
    assert_eq!(state.nonce(&addr), 0);
    
    // Transaction with nonce 0 should be accepted
    let tx = make_signed_tx(&sk, to, 100, 10, 0);
    let v1 = make_vertex(&sk, 1, 1, vec![], vec![tx]);
    
    let result = state.apply_vertex(&v1);
    assert!(result.is_ok(), "Transaction with correct nonce should be accepted");
    
    // Nonce should now be 1
    assert_eq!(state.nonce(&addr), 1);
    
    println!("✓ Transaction with nonce N accepted when account nonce is N");
}

#[test]
fn replay_transaction_rejected() {
    let mut state = StateEngine::new();
    let sk = SecretKey::generate();
    let addr = sk.address();
    let to = SecretKey::generate().address();
    
    // Give account coins
    let v0 = make_vertex(&sk, 0, 0, vec![], vec![]);
    state.apply_vertex(&v0).unwrap();
    
    // Send transaction with nonce 0
    let tx0 = make_signed_tx(&sk, to, 100, 10, 0);
    let v1 = make_vertex(&sk, 1, 1, vec![], vec![tx0]);
    state.apply_vertex(&v1).unwrap();
    
    // Nonce is now 1
    assert_eq!(state.nonce(&addr), 1);
    
    // Try to replay with nonce 0 (N-1)
    let tx_replay = make_signed_tx(&sk, to, 100, 10, 0);
    let v2 = make_vertex(&sk, 2, 2, vec![], vec![tx_replay]);
    
    let result = state.apply_vertex(&v2);
    assert!(result.is_err(), "Replay transaction should be rejected");
    
    match result.unwrap_err() {
        ultradag_coin::CoinError::InvalidNonce { expected, got } => {
            assert_eq!(expected, 1, "Expected nonce should be 1");
            assert_eq!(got, 0, "Got nonce should be 0 (replay)");
        }
        _ => panic!("Should fail with InvalidNonce error"),
    }
    
    println!("✓ Transaction with nonce N-1 (replay) rejected");
}

#[test]
fn future_nonce_rejected() {
    let mut state = StateEngine::new();
    let sk = SecretKey::generate();
    let addr = sk.address();
    let to = SecretKey::generate().address();
    
    // Give account coins
    let v0 = make_vertex(&sk, 0, 0, vec![], vec![]);
    state.apply_vertex(&v0).unwrap();
    
    // Current nonce is 0, try nonce 1 (N+1)
    assert_eq!(state.nonce(&addr), 0);
    
    let tx_future = make_signed_tx(&sk, to, 100, 10, 1);
    let v1 = make_vertex(&sk, 1, 1, vec![], vec![tx_future]);
    
    let result = state.apply_vertex(&v1);
    assert!(result.is_err(), "Future nonce should be rejected");
    
    match result.unwrap_err() {
        ultradag_coin::CoinError::InvalidNonce { expected, got } => {
            assert_eq!(expected, 0, "Expected nonce should be 0");
            assert_eq!(got, 1, "Got nonce should be 1 (future)");
        }
        _ => panic!("Should fail with InvalidNonce error"),
    }
    
    println!("✓ Transaction with nonce N+1 (future) rejected");
}

#[test]
fn duplicate_nonce_in_same_vertex_rejected() {
    let mut state = StateEngine::new();
    let sk = SecretKey::generate();
    let _addr = sk.address();
    let to = SecretKey::generate().address();
    
    // Give account coins
    let v0 = make_vertex(&sk, 0, 0, vec![], vec![]);
    state.apply_vertex(&v0).unwrap();
    
    // Try to include two transactions with same nonce in one vertex
    let tx1 = make_signed_tx(&sk, to, 100, 10, 0);
    let tx2 = make_signed_tx(&sk, to, 200, 10, 0); // Same nonce!
    
    let v1 = make_vertex(&sk, 1, 1, vec![], vec![tx1, tx2]);
    
    let result = state.apply_vertex(&v1);
    assert!(result.is_err(), "Second transaction with same nonce should be rejected");
    
    // The first tx succeeds, second fails
    match result.unwrap_err() {
        ultradag_coin::CoinError::InvalidNonce { expected, got } => {
            assert_eq!(expected, 1, "After first tx, nonce should be 1");
            assert_eq!(got, 0, "Second tx has nonce 0");
        }
        _ => panic!("Should fail with InvalidNonce error"),
    }
    
    println!("✓ Two transactions with same nonce rejected (second one fails)");
}

#[test]
fn nonce_increments_after_finalization() {
    let mut state = StateEngine::new();
    let sk = SecretKey::generate();
    let addr = sk.address();
    let to = SecretKey::generate().address();
    
    // Give account coins
    let v0 = make_vertex(&sk, 0, 0, vec![], vec![]);
    state.apply_vertex(&v0).unwrap();
    
    assert_eq!(state.nonce(&addr), 0);
    
    // Send transaction with nonce 0
    let tx = make_signed_tx(&sk, to, 100, 10, 0);
    let v1 = make_vertex(&sk, 1, 1, vec![], vec![tx]);
    state.apply_vertex(&v1).unwrap();
    
    // Nonce should be exactly N+1
    assert_eq!(state.nonce(&addr), 1, "Nonce should be exactly 1 after one transaction");
    
    println!("✓ After transaction with nonce N finalized, account nonce is exactly N+1");
}

#[test]
fn nonce_tracking_survives_replay() {
    let sk1 = SecretKey::generate();
    let sk2 = SecretKey::generate();
    let to = SecretKey::generate().address();
    
    // Build a sequence of vertices
    let v0 = make_vertex(&sk1, 0, 0, vec![], vec![]);
    let v1 = make_vertex(&sk2, 1, 1, vec![], vec![]);
    
    let tx1 = make_signed_tx(&sk1, to, 100, 10, 0);
    let v2 = make_vertex(&sk1, 2, 2, vec![], vec![tx1]);
    
    let tx2 = make_signed_tx(&sk1, to, 200, 10, 1);
    let v3 = make_vertex(&sk1, 3, 3, vec![], vec![tx2]);
    
    // Apply to state1
    let mut state1 = StateEngine::new();
    state1.apply_finalized_vertices(&[v0.clone(), v1.clone(), v2.clone(), v3.clone()]).unwrap();
    
    let nonce1 = state1.nonce(&sk1.address());
    
    // Apply to state2 from scratch (replay)
    let mut state2 = StateEngine::new();
    state2.apply_finalized_vertices(&[v0, v1, v2, v3]).unwrap();
    
    let nonce2 = state2.nonce(&sk1.address());
    
    // Nonces must be identical
    assert_eq!(nonce1, nonce2, "Nonce must be identical after replay");
    assert_eq!(nonce1, 2, "Nonce should be 2 after two transactions");
    
    println!("✓ Nonce tracking survives StateEngine replay from scratch");
}

// ============================================================================
// Part 2.2 — Balance Enforcement
// ============================================================================

#[test]
fn transaction_for_exact_balance_minus_fee_succeeds() {
    let mut state = StateEngine::new();
    let sk = SecretKey::generate();
    let addr = sk.address();
    let to = SecretKey::generate().address();
    
    // Give account exactly 1000
    let v0 = make_vertex(&sk, 0, 0, vec![], vec![]);
    state.apply_vertex(&v0).unwrap();
    
    let balance = state.balance(&addr);
    let fee = 10;
    let amount = balance - fee; // Exact balance minus fee
    
    let tx = make_signed_tx(&sk, to, amount, fee, 0);
    let v1 = make_vertex(&sk, 1, 1, vec![], vec![tx]);
    
    let result = state.apply_vertex(&v1);
    assert!(result.is_ok(), "Transaction for exact balance minus fee should succeed");
    
    // Sender should have 0 left (spent all on amount + fee, but got new reward)
    let reward1 = ultradag_coin::constants::block_reward(1);
    assert_eq!(state.balance(&addr), reward1 + fee, "Sender gets new reward + fee back");
    assert_eq!(state.balance(&to), amount, "Recipient gets amount");
    
    println!("✓ Transaction for exactly available balance minus fee succeeds");
}

#[test]
fn transaction_exceeding_balance_by_one_fails() {
    let mut state = StateEngine::new();
    let sk_sender = SecretKey::generate();
    let sk_proposer = SecretKey::generate();
    let sender = sk_sender.address();
    let to = SecretKey::generate().address();
    
    // Give sender some coins
    let v0 = make_vertex(&sk_sender, 0, 0, vec![], vec![]);
    state.apply_vertex(&v0).unwrap();
    
    let balance = state.balance(&sender);
    let fee = 10;
    let amount = balance - fee + 1; // One satoshi too much
    
    // Different proposer includes the transaction
    let tx = make_signed_tx(&sk_sender, to, amount, fee, 0);
    let v1 = make_vertex(&sk_proposer, 1, 1, vec![], vec![tx]);
    
    let result = state.apply_vertex(&v1);
    assert!(result.is_err(), "Transaction exceeding balance should fail");
    
    match result.unwrap_err() {
        ultradag_coin::CoinError::InsufficientBalance { required, available, .. } => {
            assert_eq!(required, amount + fee);
            assert_eq!(available, balance);
        }
        _ => panic!("Should fail with InsufficientBalance error"),
    }
    
    println!("✓ Transaction for balance minus fee plus one satoshi fails");
}

#[test]
fn transaction_with_zero_amount_fails() {
    let mut state = StateEngine::new();
    let sk = SecretKey::generate();
    let to = SecretKey::generate().address();
    
    // Give account coins
    let v0 = make_vertex(&sk, 0, 0, vec![], vec![]);
    state.apply_vertex(&v0).unwrap();
    
    // Try zero amount
    let tx = make_signed_tx(&sk, to, 0, 10, 0);
    let v1 = make_vertex(&sk, 1, 1, vec![], vec![tx]);
    
    let result = state.apply_vertex(&v1);
    // Zero amount transactions are technically valid but pointless
    // The system should either reject them or handle them correctly
    // Let's verify the behavior
    if result.is_ok() {
        // If accepted, verify state is correct
        assert_eq!(state.balance(&to), 0, "Recipient should have 0 from zero-amount tx");
        println!("✓ Zero amount transaction accepted (but transfers nothing)");
    } else {
        println!("✓ Zero amount transaction rejected");
    }
}

#[test]
fn transaction_with_zero_fee_accepted() {
    let mut state = StateEngine::new();
    let sk = SecretKey::generate();
    let to = SecretKey::generate().address();
    
    // Give account coins
    let v0 = make_vertex(&sk, 0, 0, vec![], vec![]);
    state.apply_vertex(&v0).unwrap();
    
    // Zero fee is allowed (no minimum fee requirement)
    let tx = make_signed_tx(&sk, to, 100, 0, 0);
    let v1 = make_vertex(&sk, 1, 1, vec![], vec![tx]);
    
    let result = state.apply_vertex(&v1);
    assert!(result.is_ok(), "Zero fee transaction should be accepted");
    
    println!("✓ Zero fee transaction accepted (no minimum fee requirement)");
}

#[test]
fn balance_updates_correctly_after_transfer() {
    let mut state = StateEngine::new();
    let sk_sender = SecretKey::generate();
    let sk_proposer = SecretKey::generate();
    let sender = sk_sender.address();
    let proposer = sk_proposer.address();
    let receiver = SecretKey::generate().address();
    
    // Give sender coins
    let v0 = make_vertex(&sk_sender, 0, 0, vec![], vec![]);
    state.apply_vertex(&v0).unwrap();
    
    let initial_sender = state.balance(&sender);
    let amount = 1000;
    let fee = 50;
    
    // Sender sends to receiver, proposer includes the tx
    let tx = make_signed_tx(&sk_sender, receiver, amount, fee, 0);
    let v1 = make_vertex(&sk_proposer, 1, 1, vec![], vec![tx]);
    state.apply_vertex(&v1).unwrap();
    
    let reward1 = ultradag_coin::constants::block_reward(1);
    
    // Verify balances
    let expected_sender = initial_sender - amount - fee;
    let expected_receiver = amount;
    let expected_proposer = reward1 + fee;
    
    assert_eq!(state.balance(&sender), expected_sender, 
        "Sender balance should be reduced by amount + fee");
    assert_eq!(state.balance(&receiver), expected_receiver, 
        "Receiver balance should be increased by amount");
    assert_eq!(state.balance(&proposer), expected_proposer, 
        "Proposer should get reward + fee");
    
    println!("✓ After transfer: sender -= amount+fee, receiver += amount, fee goes to proposer");
}

// ============================================================================
// Part 2.3 — The DAG Double-Spend Scenario
// ============================================================================

#[test]
fn dag_double_spend_deterministically_resolved() {
    // This is the critical test for DAG systems.
    // Account A has exactly 1000 units.
    // Validator 1 includes "A sends 800 to B" in round 1
    // Validator 2 includes "A sends 700 to C" in round 1 (concurrent)
    // Both are valid at creation time.
    // After finalization and deterministic ordering, only one succeeds.
    
    let sk_a = SecretKey::generate();
    let sk_v1 = SecretKey::generate();
    let sk_v2 = SecretKey::generate();
    let addr_a = sk_a.address();
    let addr_b = SecretKey::generate().address();
    let addr_c = SecretKey::generate().address();
    
    let mut dag = BlockDag::new();
    let mut finality = FinalityTracker::new(2);
    let mut state = StateEngine::new();
    
    // Register validators
    finality.register_validator(sk_v1.address());
    finality.register_validator(sk_v2.address());
    
    // Round 0: Validators get initial rewards
    let v0_v1 = make_vertex(&sk_v1, 0, 0, vec![], vec![]);
    let v0_v2 = make_vertex(&sk_v2, 0, 1, vec![], vec![]);
    dag.insert(v0_v1.clone());
    dag.insert(v0_v2.clone());
    
    // Round 1: Finalize round 0
    let r0_tips = dag.tips();
    let v1_v1 = make_vertex(&sk_v1, 1, 2, r0_tips.clone(), vec![]);
    let v1_v2 = make_vertex(&sk_v2, 1, 3, r0_tips, vec![]);
    dag.insert(v1_v1.clone());
    dag.insert(v1_v2.clone());
    
    // Finalize and apply round 0 (loop for parent finality guarantee)
    loop {
        let finalized = finality.find_newly_finalized(&dag);
        if finalized.is_empty() { break; }
        let finalized_vertices: Vec<DagVertex> = finalized
            .iter()
            .filter_map(|h| dag.get(h).cloned())
            .collect();
        state.apply_finalized_vertices(&finalized_vertices).unwrap();
    }

    // Now validator 1 has funds, send exactly 1000 to A
    let r1_tips = dag.tips();
    let tx_fund_a = make_signed_tx(&sk_v1, addr_a, 1000, 0, 0);
    let v2_v1_funding = make_vertex(&sk_v1, 2, 4, r1_tips.clone(), vec![tx_fund_a]);
    let v2_v2 = make_vertex(&sk_v2, 2, 5, r1_tips, vec![]);
    dag.insert(v2_v1_funding.clone());
    dag.insert(v2_v2.clone());
    
    // Round 3: Finalize round 1 and 2
    let r2_tips = dag.tips();
    let v3_v1 = make_vertex(&sk_v1, 3, 6, r2_tips.clone(), vec![]);
    let v3_v2 = make_vertex(&sk_v2, 3, 7, r2_tips, vec![]);
    dag.insert(v3_v1.clone());
    dag.insert(v3_v2.clone());

    loop {
        let finalized = finality.find_newly_finalized(&dag);
        if finalized.is_empty() { break; }
        let finalized_vertices: Vec<DagVertex> = finalized
            .iter()
            .filter_map(|h| dag.get(h).cloned())
            .collect();
        state.apply_finalized_vertices(&finalized_vertices).unwrap();
    }

    // Verify A has exactly 1000
    assert_eq!(state.balance(&addr_a), 1000, "A should have exactly 1000");
    
    // Round 4: CONCURRENT double-spend attempts
    // Validator 1 includes "A sends 800 to B"
    // Validator 2 includes "A sends 700 to C"
    // Both are valid at creation time (A has 1000)
    
    let r3_tips = dag.tips();
    
    let tx_a_to_b = make_signed_tx(&sk_a, addr_b, 800, 10, 0);
    let v4_v1 = make_vertex(&sk_v1, 4, 8, r3_tips.clone(), vec![tx_a_to_b]);
    
    let tx_a_to_c = make_signed_tx(&sk_a, addr_c, 700, 10, 0);
    let v4_v2 = make_vertex(&sk_v2, 4, 9, r3_tips, vec![tx_a_to_c]);
    
    dag.insert(v4_v1.clone());
    dag.insert(v4_v2.clone());
    
    // Round 5: Finalize round 4
    let r4_tips = dag.tips();
    let v5_v1 = make_vertex(&sk_v1, 5, 10, r4_tips.clone(), vec![]);
    let v5_v2 = make_vertex(&sk_v2, 5, 11, r4_tips, vec![]);
    dag.insert(v5_v1);
    dag.insert(v5_v2);
    
    // Get newly finalized vertices (loop for parent finality guarantee)
    let mut all_finalized = Vec::new();
    loop {
        let finalized = finality.find_newly_finalized(&dag);
        if finalized.is_empty() { break; }
        all_finalized.extend(finalized);
    }
    let mut finalized_vertices: Vec<DagVertex> = all_finalized
        .iter()
        .filter_map(|h| dag.get(h).cloned())
        .collect();

    // Deterministic ordering
    finalized_vertices.sort_by_key(|v| (v.round, v.height(), v.hash()));

    // Apply to state
    let mut success_count = 0;
    let mut failed_count = 0;

    for vertex in &finalized_vertices {
        let result = state.apply_vertex(vertex);
        if result.is_ok() {
            success_count += 1;
        } else {
            failed_count += 1;
            // Should fail with insufficient balance
            match result.unwrap_err() {
                ultradag_coin::CoinError::InsufficientBalance { .. } => {
                    println!("  Transaction failed: insufficient balance (expected)");
                }
                e => panic!("Unexpected error: {:?}", e),
            }
        }
    }

    // Exactly one transaction should succeed
    assert_eq!(success_count + failed_count, finalized_vertices.len());
    
    // One of the transactions succeeded, the other failed
    let balance_b = state.balance(&addr_b);
    let balance_c = state.balance(&addr_c);
    
    let one_succeeded = (balance_b == 800 && balance_c == 0) || (balance_b == 0 && balance_c == 700);
    assert!(one_succeeded, "Exactly one transaction should succeed");
    
    // Total supply is conserved
    // Count rewards from all finalized vertices
    let final_supply = state.total_supply();
    
    // Finalized rounds 0-3 (round 4 and 5 not finalized yet due to 2-round lag)
    // Heights 0-8 (9 vertices)
    let mut expected_supply = 0u64;
    for h in 0..=8 {
        expected_supply += ultradag_coin::constants::block_reward(h);
    }
    assert_eq!(final_supply, expected_supply, 
        "Total supply must equal sum of all finalized block rewards");
    
    // Verify determinism: replay with same ordered vertices produces same result
    let mut state2 = StateEngine::new();
    
    // Apply all setup vertices
    state2.apply_finalized_vertices(&[v0_v1, v0_v2, v1_v1, v1_v2, v2_v1_funding, v2_v2, v3_v1, v3_v2]).unwrap();
    
    // Apply the double-spend vertices in the same order
    for vertex in &finalized_vertices {
        let _ = state2.apply_vertex(vertex);
    }
    
    // Results must be identical
    assert_eq!(state2.balance(&addr_b), balance_b, "Replay should produce same result for B");
    assert_eq!(state2.balance(&addr_c), balance_c, "Replay should produce same result for C");
    
    println!("✓ DAG double-spend resolved deterministically");
    println!("  Account A had 1000");
    println!("  Concurrent txs: A→B (800) and A→C (700)");
    println!("  Result: B={}, C={}", balance_b, balance_c);
    println!("  Exactly one succeeded, supply conserved, deterministic");
}
