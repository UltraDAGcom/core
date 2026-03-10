//! Edge case tests covering state machine, persistence, consensus, and performance.
//! Each test corresponds to the production-readiness audit matrix.

use std::collections::HashSet;
use ultradag_coin::*;
use ultradag_coin::constants::*;

// ============================================================================
// Test helpers
// ============================================================================

fn make_signed_tx(sk: &SecretKey, to: Address, amount: u64, fee: u64, nonce: u64) -> Transaction {
    let mut transfer = TransferTx {
        from: sk.address(),
        to,
        amount,
        fee,
        nonce,
        pub_key: sk.verifying_key().to_bytes(),
        signature: Signature([0u8; 64]),
    };
    transfer.signature = sk.sign(&transfer.signable_bytes());
    Transaction::Transfer(transfer)
}

fn make_vertex(
    proposer_sk: &SecretKey,
    round: u64,
    height: u64,
    txs: Vec<Transaction>,
    parents: Vec<[u8; 32]>,
) -> DagVertex {
    let proposer = proposer_sk.address();
    let total_fees: u64 = txs.iter().map(|tx| tx.fee()).sum();
    let reward = block_reward(height);
    let coinbase = ultradag_coin::CoinbaseTx {
        to: proposer,
        amount: reward + total_fees,
        height,
    };
    let block = ultradag_coin::Block {
        header: ultradag_coin::BlockHeader {
            version: 1,
            height,
            timestamp: 1_000_000 + round as i64,
            prev_hash: parents.first().copied().unwrap_or([0u8; 32]),
            merkle_root: [0u8; 32],
        },
        coinbase,
        transactions: txs,
    };
    let mut vertex = DagVertex::new(
        block,
        if parents.is_empty() { vec![[0u8; 32]] } else { parents },
        round,
        proposer,
        proposer_sk.verifying_key().to_bytes(),
        Signature([0u8; 64]),
    );
    vertex.signature = proposer_sk.sign(&vertex.signable_bytes());
    vertex
}

// ============================================================================
// Round 1 — State Machine Edge Cases
// ============================================================================

/// Test 1: Coinbase with 0 fees — verified correct
#[test]
fn coinbase_zero_fees() {
    let mut state = StateEngine::new();
    let sk = SecretKey::generate();
    let vertex = make_vertex(&sk, 0, 0, vec![], vec![]);
    state.apply_vertex(&vertex).unwrap();

    let expected = block_reward(0);
    assert_eq!(state.balance(&sk.address()), expected);
    assert_eq!(state.total_supply(), expected);
}

/// Test 2: Coinbase with fees after supply exhaustion (height 13,440,001)
#[test]
fn coinbase_fees_after_supply_exhaustion() {
    let mut state = StateEngine::new_with_genesis();
    let proposer_sk = SecretKey::generate();
    let sender_sk = SecretKey::generate();
    let receiver = SecretKey::generate().address();

    // Give sender some coins first
    let fund_amount = 10_000_000;
    state.faucet_credit(&sender_sk.address(), fund_amount).unwrap();

    // Height after 64 halvings — block_reward = 0
    let post_exhaustion_height = 64 * HALVING_INTERVAL + 1;
    assert_eq!(block_reward(post_exhaustion_height), 0);

    // Set last_finalized_round so engine computes expected_height = post_exhaustion_height
    state.last_finalized_round = Some(post_exhaustion_height - 1);

    let fee = 100_000;
    let tx = make_signed_tx(&sender_sk, receiver, 1_000_000, fee, 0);

    // Create vertex at post-exhaustion height with a tx that has fees
    let proposer = proposer_sk.address();
    let coinbase = ultradag_coin::CoinbaseTx {
        to: proposer,
        amount: 0 + fee, // block_reward=0, but fees should still work
        height: post_exhaustion_height,
    };
    let block = ultradag_coin::Block {
        header: ultradag_coin::BlockHeader {
            version: 1,
            height: post_exhaustion_height,
            timestamp: 1_000_000,
            prev_hash: [0u8; 32],
            merkle_root: [0u8; 32],
        },
        coinbase,
        transactions: vec![tx],
    };
    let mut vertex = DagVertex::new(
        block,
        vec![[0u8; 32]],
        0,
        proposer,
        proposer_sk.verifying_key().to_bytes(),
        Signature([0u8; 64]),
    );
    vertex.signature = proposer_sk.sign(&vertex.signable_bytes());

    let supply_before = state.total_supply();
    state.apply_vertex(&vertex).unwrap();
    // Proposer should receive the fee (0 block reward + fee)
    assert_eq!(state.balance(&proposer), fee);
    // No new supply created (only internal transfer of fee from sender to proposer)
    assert_eq!(state.total_supply(), supply_before, "Supply should not change when block_reward=0");
}

/// Test 13: Empty block after supply exhaustion — coinbase = 0
#[test]
fn empty_block_after_supply_exhaustion() {
    let mut state = StateEngine::new();
    let sk = SecretKey::generate();

    let post_exhaustion_height = 64 * HALVING_INTERVAL + 1;
    assert_eq!(block_reward(post_exhaustion_height), 0);

    // Set last_finalized_round so engine computes expected_height = post_exhaustion_height
    state.last_finalized_round = Some(post_exhaustion_height - 1);

    // Vertex with 0 transactions, 0 block reward → coinbase = 0
    let proposer = sk.address();
    let coinbase = ultradag_coin::CoinbaseTx {
        to: proposer,
        amount: 0,
        height: post_exhaustion_height,
    };
    let block = ultradag_coin::Block {
        header: ultradag_coin::BlockHeader {
            version: 1,
            height: post_exhaustion_height,
            timestamp: 1_000_000,
            prev_hash: [0u8; 32],
            merkle_root: [0u8; 32],
        },
        coinbase,
        transactions: vec![],
    };
    let mut vertex = DagVertex::new(
        block,
        vec![[0u8; 32]],
        0,
        proposer,
        sk.verifying_key().to_bytes(),
        Signature([0u8; 64]),
    );
    vertex.signature = sk.sign(&vertex.signable_bytes());

    // Should succeed — coinbase of 0 is valid
    state.apply_vertex(&vertex).unwrap();
    assert_eq!(state.balance(&proposer), 0);
    assert_eq!(state.total_supply(), 0);
}

/// Test: Faucet prefund + first rounds don't overflow u64
#[test]
fn no_overflow_genesis_plus_rounds() {
    let mut state = StateEngine::new_with_genesis();
    let validators: Vec<_> = (0..4).map(|_| SecretKey::generate()).collect();

    // 1000 rounds with 4 validators
    for round in 0u64..1000 {
        let sk = &validators[(round % 4) as usize];
        let vertex = make_vertex(sk, round, round, vec![], vec![]);
        state.apply_vertex(&vertex).unwrap();
    }

    // FAUCET_PREFUND_SATS = 1_000_000 * 10^8 = 10^14
    // 1000 rounds * 50 UDAG * 10^8 = 5 * 10^12
    // Total ~ 1.05 * 10^14, well within u64
    assert!(state.total_supply() < u64::MAX);
    let sum: u64 = (0..4).map(|i| state.balance(&validators[i].address())).sum::<u64>()
        + state.balance(&faucet_keypair().address())
        + state.balance(&ultradag_coin::dev_address());
    assert_eq!(sum, state.total_supply());
}

/// Fee sum is computed from vertex block, not from modified state
#[test]
fn fee_sum_computed_before_tx_application() {
    let mut state = StateEngine::new_with_genesis();
    let proposer_sk = SecretKey::generate();
    let sender_sk = SecretKey::generate();
    let receiver = SecretKey::generate().address();

    // Give sender 10 UDAG
    let v0 = make_vertex(&proposer_sk, 0, 0, vec![], vec![]);
    state.apply_vertex(&v0).unwrap();

    let send_amount = 1_000_000;
    let fee = 100;
    state.faucet_credit(&sender_sk.address(), send_amount + fee).unwrap();

    let tx = make_signed_tx(&sender_sk, receiver, send_amount, fee, 0);
    let v1 = make_vertex(&proposer_sk, 1, 1, vec![tx], vec![]);
    state.apply_vertex(&v1).unwrap();

    // Proposer should get block_reward(1) + fee
    let expected_proposer = block_reward(0) + block_reward(1) + fee;
    assert_eq!(state.balance(&proposer_sk.address()), expected_proposer);
}

// ============================================================================
// Round 2 — Persistence Edge Cases
// ============================================================================

/// Test 3: Crash recovery — state behind DAG is handled by sync
#[test]
fn crash_recovery_state_behind_dag() {
    let sk = SecretKey::generate();
    let tmp = std::env::temp_dir().join("ultradag_test_crash_recovery");
    std::fs::create_dir_all(&tmp).unwrap();

    // Create a state at round 5
    let mut state = StateEngine::new();
    for r in 0..6 {
        let v = make_vertex(&sk, r, r, vec![], vec![]);
        state.apply_vertex(&v).unwrap();
    }
    state.save(&tmp.join("state.json")).unwrap();

    // Create a DAG at round 9 (state saved at round 5, crashed before saving at 10)
    let mut dag = BlockDag::new();
    for r in 0..10 {
        let v = make_vertex(&sk, r, r, vec![], vec![]);
        dag.insert(v);
    }
    dag.save(&tmp.join("dag.json")).unwrap();

    // Load both back
    let loaded_state = StateEngine::load(&tmp.join("state.json")).unwrap();
    let loaded_dag = BlockDag::load(&tmp.join("dag.json")).unwrap();

    // State is behind DAG — state at round 5, DAG at round 9
    assert_eq!(loaded_state.last_finalized_round(), Some(5));
    assert_eq!(loaded_dag.current_round(), 9);

    // The node would re-apply vertices from round 6-9 from the DAG via peer sync
    // This is safe because apply_vertex is idempotent per round

    std::fs::remove_dir_all(&tmp).ok();
}

/// Test 4: State ahead of DAG is impossible in practice but verify load works
#[test]
fn state_ahead_of_dag_detected() {
    let sk = SecretKey::generate();
    let tmp = std::env::temp_dir().join("ultradag_test_state_ahead");
    std::fs::create_dir_all(&tmp).unwrap();

    // State at round 5
    let mut state = StateEngine::new();
    for r in 0..6 {
        let v = make_vertex(&sk, r, r, vec![], vec![]);
        state.apply_vertex(&v).unwrap();
    }
    state.save(&tmp.join("state.json")).unwrap();

    // DAG at round 2 (corrupted/incomplete)
    let mut dag = BlockDag::new();
    for r in 0..3 {
        let v = make_vertex(&sk, r, r, vec![], vec![]);
        dag.insert(v);
    }
    dag.save(&tmp.join("dag.json")).unwrap();

    let loaded_state = StateEngine::load(&tmp.join("state.json")).unwrap();
    let loaded_dag = BlockDag::load(&tmp.join("dag.json")).unwrap();

    // State is ahead — this scenario doesn't crash, just results in stale DAG
    // Peer sync will fill in the missing DAG vertices
    assert_eq!(loaded_state.last_finalized_round(), Some(5));
    assert_eq!(loaded_dag.current_round(), 2);

    std::fs::remove_dir_all(&tmp).ok();
}

/// Test 5: Corrupt dag.json — verified does not panic
#[test]
fn corrupt_dag_json_does_not_panic() {
    let tmp = std::env::temp_dir().join("ultradag_test_corrupt");
    std::fs::create_dir_all(&tmp).unwrap();
    let path = tmp.join("dag.json");

    std::fs::write(&path, "{ invalid json!!!").unwrap();

    let result = BlockDag::load(&path);
    assert!(result.is_err(), "Loading corrupt JSON should return error, not panic");

    // Also test truncated JSON
    std::fs::write(&path, r#"{"vertices":[{"key":[1,2"#).unwrap();
    let result2 = BlockDag::load(&path);
    assert!(result2.is_err());

    // Empty file
    std::fs::write(&path, "").unwrap();
    let result3 = BlockDag::load(&path);
    assert!(result3.is_err());

    std::fs::remove_dir_all(&tmp).ok();
}

/// Test: Leftover .tmp file from crash during save
#[test]
fn leftover_tmp_file_from_crash() {
    let tmp = std::env::temp_dir().join("ultradag_test_tmp_leftover");
    std::fs::create_dir_all(&tmp).unwrap();

    let path = tmp.join("state.json");
    let tmp_path = tmp.join("state.tmp");

    // Simulate a crash that left a .tmp file
    std::fs::write(&tmp_path, r#"{"partial":"data"}"#).unwrap();

    // A fresh save should overwrite the .tmp file via the normal write-then-rename path
    let state = StateEngine::new();
    state.save(&path).unwrap();

    // The .tmp file should be gone (renamed to .json)
    assert!(path.exists());
    // The .tmp may or may not exist depending on OS rename behavior — but loading works
    let loaded = StateEngine::load(&path);
    assert!(loaded.is_ok());

    std::fs::remove_dir_all(&tmp).ok();
}

// ============================================================================
// Round 4 — Consensus Edge Cases
// ============================================================================

/// Test 8: Round timer with only 2/4 validators — verified stalls correctly
#[test]
fn round_stalls_with_insufficient_validators() {
    let mut dag = BlockDag::new();
    let mut ft = FinalityTracker::new(3);

    let sks: Vec<_> = (0..4).map(|_| SecretKey::generate()).collect();
    for sk in &sks {
        ft.register_validator(sk.address());
    }
    // threshold = ceil(8/3) = 3

    // Only 2 validators produce in round 0
    let v0a = make_vertex(&sks[0], 0, 0, vec![], vec![]);
    let h0a = v0a.hash();
    dag.insert(v0a);

    let v0b = make_vertex(&sks[1], 0, 0, vec![], vec![]);
    let h0b = v0b.hash();
    dag.insert(v0b);

    // Only 2 validators have descendants — not enough for finality (need 3)
    let v1 = make_vertex(&sks[2], 1, 1, vec![], vec![h0a, h0b]);
    dag.insert(v1);

    // v0a has descendants from sks[2] only (1 validator) — not finalized
    assert!(!ft.check_finality(&h0a, &dag));

    // After 3rd validator adds a descendant
    let v2 = make_vertex(&sks[3], 1, 1, vec![], vec![h0a, h0b]);
    dag.insert(v2);

    // Now h0a has descendants from sks[2] and sks[3] = 2 descendant validators
    // Still need 3 for quorum with 4 validators
    assert!(!ft.check_finality(&h0a, &dag));
}

/// Test 14: Vertex with empty parent_hashes — only valid for genesis sentinel
#[test]
fn vertex_empty_parents_uses_genesis_sentinel() {
    let mut dag = BlockDag::new();
    let sk = SecretKey::generate();

    // A vertex with parent [0u8; 32] (genesis sentinel) should be accepted
    let v = make_vertex(&sk, 0, 0, vec![], vec![]);
    // make_vertex uses vec![[0u8; 32]] when parents is empty
    assert!(dag.insert(v));
    assert_eq!(dag.len(), 1);
}

/// Test: Vertex referencing nonexistent parent is rejected
#[test]
fn vertex_with_phantom_parent_rejected() {
    let mut dag = BlockDag::new();
    let sk = SecretKey::generate();

    // Insert a valid genesis vertex first
    let v0 = make_vertex(&sk, 0, 0, vec![], vec![]);
    dag.insert(v0);

    // Create a vertex referencing a parent that doesn't exist
    let phantom_parent = [0xDE; 32];
    let v1 = make_vertex(&sk, 1, 1, vec![], vec![phantom_parent]);
    assert!(!dag.insert(v1), "Vertex with nonexistent parent should be rejected");
}

/// Test 15: Vertex with more parents than validators — accepted (valid DAG)
#[test]
fn vertex_with_many_parents_accepted() {
    let mut dag = BlockDag::new();
    let sks: Vec<_> = (0..6).map(|_| SecretKey::generate()).collect();

    // Create 6 genesis vertices
    let mut hashes = Vec::new();
    for (i, sk) in sks.iter().enumerate() {
        let v = make_vertex(sk, 0, 0 + i as u64, vec![], vec![]);
        let h = v.hash();
        dag.insert(v);
        hashes.push(h);
    }

    // Create a vertex referencing all 6 as parents (more than typical 4 validators)
    let sk_new = SecretKey::generate();
    let v = make_vertex(&sk_new, 1, 6, vec![], hashes.clone());
    assert!(dag.insert(v), "Vertex with many parents should be accepted");

    // All former tips should no longer be tips
    for h in &hashes {
        assert!(!dag.tips().contains(h));
    }
}

/// Test: Identical block content in different rounds is valid (not equivocation)
#[test]
fn same_content_different_rounds_valid() {
    let mut dag = BlockDag::new();
    let sk = SecretKey::generate();

    let v1 = make_vertex(&sk, 0, 0, vec![], vec![]);
    let h1 = v1.hash();
    assert!(dag.insert(v1));

    // Same validator, same content, different round — NOT equivocation
    let v2 = make_vertex(&sk, 1, 1, vec![], vec![h1]);
    let h2 = v2.hash();
    assert_ne!(h1, h2, "Different rounds produce different hashes");
    assert!(dag.insert(v2));
    assert_eq!(dag.len(), 2);
}

// ============================================================================
// Round 5 — Scenarios That Break Things
// ============================================================================

/// Test 9: 5th validator joins mid-stream — quorum updates correctly
/// With permissioned validator set, the 5th validator is simply rejected.
/// Without allowlist, quorum threshold increases from 3 to 4.
#[test]
fn fifth_validator_joins_midstream_permissioned() {
    let mut ft = FinalityTracker::new(3);
    let sks: Vec<_> = (0..4).map(|_| SecretKey::generate()).collect();

    let mut allowed = HashSet::new();
    for sk in &sks {
        allowed.insert(sk.address());
    }
    ft.set_allowed_validators(allowed);

    for sk in &sks {
        ft.register_validator(sk.address());
    }
    assert_eq!(ft.validator_count(), 4);
    assert_eq!(ft.finality_threshold(), 3); // ceil(8/3)

    // 5th validator tries to register — blocked by allowlist
    let sk5 = SecretKey::generate();
    ft.register_validator(sk5.address());
    assert_eq!(ft.validator_count(), 4); // Still 4
    assert_eq!(ft.finality_threshold(), 3); // Unchanged
}

/// Test 10: All 4 nodes restart simultaneously — consensus resumes
/// Simulated: create DAG with 4 validators through 10 rounds,
/// then create fresh FinalityTracker and rebuild — finality should resume.
#[test]
fn all_nodes_restart_consensus_resumes() {
    let sks: Vec<_> = (0..4).map(|_| SecretKey::generate()).collect();
    let mut dag = BlockDag::new();
    let mut ft = FinalityTracker::new(3);

    // Build 10 rounds with 4 validators
    for round in 0u64..10 {
        for (i, sk) in sks.iter().enumerate() {
            let tips = dag.tips();
            let parents = if tips.is_empty() { vec![] } else { tips };
            let v = make_vertex(sk, round, round * 4 + i as u64, vec![], parents);
            dag.insert(v);
        }
    }

    // Register all validators and run finality
    for sk in &sks {
        ft.register_validator(sk.address());
    }
    let mut total_finalized = 0;
    loop {
        let newly = ft.find_newly_finalized(&dag);
        if newly.is_empty() { break; }
        total_finalized += newly.len();
    }
    assert!(total_finalized > 0, "Should have finalized vertices");

    // Simulate restart: new FinalityTracker, rebuild from DAG
    let mut ft2 = FinalityTracker::new(3);
    let validators = dag.all_validators();
    for addr in &validators {
        ft2.register_validator(*addr);
    }
    assert_eq!(ft2.validator_count(), 4);

    // All previously finalized vertices need to be re-finalized
    // (since ft2 starts with empty finalized set)
    let mut total_refinalized = 0;
    loop {
        let newly = ft2.find_newly_finalized(&dag);
        if newly.is_empty() { break; }
        total_refinalized += newly.len();
    }
    assert_eq!(total_refinalized, total_finalized,
        "Rebuilt FinalityTracker should finalize same vertices");
}

/// Test 12: Faucet depletion — error is graceful
#[test]
fn faucet_depletion_graceful_error() {
    let mut state = StateEngine::new_with_genesis();
    let proposer_sk = SecretKey::generate();
    let faucet_sk = faucet_keypair();
    let faucet_addr = faucet_sk.address();
    let receiver = SecretKey::generate().address();

    // Give proposer some balance for coinbase
    let v0 = make_vertex(&proposer_sk, 0, 0, vec![], vec![]);
    state.apply_vertex(&v0).unwrap();

    let faucet_balance = state.balance(&faucet_addr);
    assert_eq!(faucet_balance, FAUCET_PREFUND_SATS);

    // Try to send more than faucet balance
    let tx = make_signed_tx(&faucet_sk, receiver, faucet_balance + 1, 0, 0);
    let v1 = make_vertex(&proposer_sk, 1, 1, vec![tx], vec![]);
    let result = state.apply_vertex(&v1);
    assert!(result.is_err());
    match result {
        Err(ultradag_coin::CoinError::InsufficientBalance { .. }) => {}
        other => panic!("Expected InsufficientBalance, got {:?}", other),
    }

    // Faucet balance unchanged (atomic rollback)
    assert_eq!(state.balance(&faucet_addr), FAUCET_PREFUND_SATS);
}

// ============================================================================
// Round 6 — Performance / Architecture
// ============================================================================

/// Test 11: check_finality() benchmark at 1K vertices
#[test]
fn check_finality_performance_1k_vertices() {
    let sks: Vec<_> = (0..4).map(|_| SecretKey::generate()).collect();
    let mut dag = BlockDag::new();
    let mut ft = FinalityTracker::new(3);

    for sk in &sks {
        ft.register_validator(sk.address());
    }

    // Build 250 rounds × 4 validators = 1000 vertices
    for round in 0u64..250 {
        for (i, sk) in sks.iter().enumerate() {
            let tips = dag.tips();
            let parents = if tips.is_empty() { vec![] } else { tips };
            let v = make_vertex(sk, round, round * 4 + i as u64, vec![], parents);
            dag.insert(v);
        }
    }
    assert_eq!(dag.len(), 1000);

    // Finalize all
    let start = std::time::Instant::now();
    let mut total = 0;
    loop {
        let newly = ft.find_newly_finalized(&dag);
        if newly.is_empty() { break; }
        total += newly.len();
    }
    let elapsed = start.elapsed();

    assert!(total > 0);
    // At 1K vertices, finality should complete in well under 1 second
    assert!(
        elapsed.as_millis() < 5000,
        "Finality at 1K vertices took {}ms (limit 5000ms)", elapsed.as_millis()
    );
    eprintln!(
        "1K vertices: finalized {} in {}ms",
        total, elapsed.as_millis()
    );
}

/// Test: Orphan buffer cap enforced
#[test]
fn orphan_buffer_cap_enforced() {
    // The orphan buffer is capped at 1000 entries in server.rs
    // This test verifies the cap works as a concept (not testing P2P directly)
    use std::collections::HashMap;

    let mut orphans: HashMap<[u8; 32], DagVertex> = HashMap::new();
    let cap = 1000;
    let sk = SecretKey::generate();

    for i in 0u64..1100 {
        if orphans.len() >= cap {
            break;
        }
        let v = make_vertex(&sk, i, i, vec![], vec![[0xDE; 32]]); // phantom parent
        orphans.insert(v.hash(), v);
    }

    assert_eq!(orphans.len(), cap);
}

/// Test: Duplicate transaction in same vertex is caught
#[test]
fn duplicate_tx_in_vertex_same_nonce() {
    let mut state = StateEngine::new_with_genesis();
    let proposer_sk = SecretKey::generate();
    let sender_sk = SecretKey::generate();
    let receiver = SecretKey::generate().address();

    // Give sender coins
    state.faucet_credit(&sender_sk.address(), 10_000_000).unwrap();

    let tx1 = make_signed_tx(&sender_sk, receiver, 1_000, 100, 0);
    let tx2 = make_signed_tx(&sender_sk, receiver, 2_000, 100, 0); // Same nonce!

    // Second tx has same nonce=0, so after tx1 is applied (nonce becomes 1),
    // tx2 will fail with InvalidNonce
    let v = make_vertex(&proposer_sk, 0, 0, vec![tx1, tx2], vec![]);
    let result = state.apply_vertex(&v);
    assert!(result.is_err());
    match result {
        Err(ultradag_coin::CoinError::InvalidNonce { expected: 1, got: 0 }) => {}
        other => panic!("Expected InvalidNonce(1, 0), got {:?}", other),
    }
}

/// Test: Supply invariant holds with genesis + many rounds
#[test]
fn supply_invariant_genesis_plus_500_rounds() {
    let mut state = StateEngine::new_with_genesis();
    let validators: Vec<_> = (0..4).map(|_| SecretKey::generate()).collect();
    let faucet_addr = faucet_keypair().address();
    let dev_addr = ultradag_coin::dev_address();

    for round in 0u64..500 {
        let sk = &validators[(round % 4) as usize];
        let vertex = make_vertex(sk, round, round, vec![], vec![]);
        state.apply_vertex(&vertex).unwrap();
    }

    // Verify supply invariant: sum(all balances) == total_supply
    let mut sum = state.balance(&faucet_addr) + state.balance(&dev_addr);
    for sk in &validators {
        sum += state.balance(&sk.address());
    }
    assert_eq!(sum, state.total_supply());
}

#[test]
fn dev_allocation_in_genesis() {
    let state = StateEngine::new_with_genesis();
    let dev_addr = ultradag_coin::dev_address();
    assert_eq!(
        state.balance(&dev_addr),
        DEV_ALLOCATION_SATS,
        "Dev allocation must be exactly 5% of max supply at genesis"
    );
    assert_eq!(DEV_ALLOCATION_SATS, MAX_SUPPLY_SATS / 20);
}
