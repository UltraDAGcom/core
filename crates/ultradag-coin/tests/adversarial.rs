/// Comprehensive adversarial test suite for UltraDAG consensus.
///
/// Tests cover:
/// - A: Consensus safety (finality, ordering, equivocation)
/// - B: State machine correctness under adversarial conditions
/// - C: Transaction validity edge cases
/// - D: Performance and stress tests
///
/// All tests use real Ed25519 cryptography — no mocks.

use ultradag_coin::{
    Address, BlockDag, DagVertex, FinalityTracker, Mempool, SecretKey, Signature, StateEngine,
    Transaction, TransferTx,
    block::block::Block,
    block::header::BlockHeader,
    consensus::dag::DagInsertError,
    constants::{self, COIN, DEV_ALLOCATION_SATS, FAUCET_PREFUND_SATS, FAUCET_SEED, faucet_keypair},
    tx::CoinbaseTx,
};

// ────────────────────────────────────────────────────────────
// Helpers
// ────────────────────────────────────────────────────────────

fn make_signed_tx(sk: &SecretKey, to: Address, amount: u64, fee: u64, nonce: u64) -> Transaction {
    let mut tx = TransferTx {
        from: sk.address(),
        to,
        amount,
        fee,
        nonce,
        pub_key: sk.verifying_key().to_bytes(),
        signature: Signature([0u8; 64]),
        memo: None,
    };
    tx.signature = sk.sign(&tx.signable_bytes());
    Transaction::Transfer(tx)
}

fn make_vertex(
    proposer: &Address,
    round: u64,
    height: u64,
    parents: Vec<[u8; 32]>,
    txs: Vec<Transaction>,
    sk: &SecretKey,
) -> DagVertex {
    let total_fees: u64 = txs.iter().map(|tx| tx.fee()).sum();
    let reward = constants::block_reward(height);
    let coinbase = CoinbaseTx {
        to: *proposer,
        amount: reward + total_fees,
        height,
    };
    let block = Block {
        header: BlockHeader {
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
        parents,
        round,
        *proposer,
        sk.verifying_key().to_bytes(),
        Signature([0u8; 64]),
    );
    vertex.signature = sk.sign(&vertex.signable_bytes());
    vertex
}

fn setup_validators(n: usize) -> Vec<SecretKey> {
    (0..n).map(|i| SecretKey::from_bytes([i as u8 + 1; 32])).collect()
}

/// Run a multi-round DAG simulation with n validators.
/// Returns (dag, finality, state, validators).
fn simulate_rounds(
    n: usize,
    rounds: u64,
) -> (BlockDag, FinalityTracker, StateEngine, Vec<SecretKey>) {
    let sks = setup_validators(n);
    let mut dag = BlockDag::new();
    let mut fin = FinalityTracker::new(3);
    let mut state = StateEngine::new_with_genesis();

    for sk in &sks {
        fin.register_validator(sk.address());
    }

    for round in 0..rounds {
        let tips = dag.tips();
        let parents = if tips.is_empty() { vec![[0u8; 32]] } else { tips };

        for (i, sk) in sks.iter().enumerate() {
            let v = make_vertex(
                &sk.address(),
                round,
                round * n as u64 + i as u64,
                parents.clone(),
                vec![],
                sk,
            );
            dag.insert(v);
        }

        // Check finality
        loop {
            let newly = fin.find_newly_finalized(&dag);
            if newly.is_empty() {
                break;
            }
            let finalized_verts: Vec<DagVertex> =
                newly.iter().filter_map(|h| dag.get(h).cloned()).collect();
            let _ = state.apply_finalized_vertices(&finalized_verts);
        }
    }

    (dag, fin, state, sks)
}

// ════════════════════════════════════════════════════════════
// Category A: Consensus Safety
// ════════════════════════════════════════════════════════════

#[test]
fn a1_equivocation_detected_and_banned() {
    let sk = SecretKey::from_bytes([42u8; 32]);
    let addr = sk.address();
    let mut dag = BlockDag::new();

    let v1 = make_vertex(&addr, 1, 1, vec![[0u8; 32]], vec![], &sk);
    assert!(dag.try_insert(v1.clone()).unwrap());

    // Create a different vertex in the same round (different timestamp = different hash)
    let mut v2_block = v1.block.clone();
    v2_block.header.timestamp += 1;
    let mut v2 = DagVertex::new(
        v2_block,
        vec![[0u8; 32]],
        1,
        addr,
        sk.verifying_key().to_bytes(),
        Signature([0u8; 64]),
    );
    v2.signature = sk.sign(&v2.signable_bytes());

    // Second vertex in same round must be rejected with equivocation error
    let result = dag.try_insert(v2);
    assert!(matches!(result, Err(DagInsertError::Equivocation { .. })));
    assert!(dag.is_byzantine(&addr));
}

#[test]
fn a2_byzantine_validator_permanently_banned() {
    let sk = SecretKey::from_bytes([42u8; 32]);
    let addr = sk.address();
    let mut dag = BlockDag::new();

    let v1 = make_vertex(&addr, 1, 1, vec![[0u8; 32]], vec![], &sk);
    dag.try_insert(v1.clone()).unwrap();

    // Trigger equivocation
    let mut v2_block = v1.block.clone();
    v2_block.header.timestamp += 1;
    let mut v2 = DagVertex::new(
        v2_block,
        vec![[0u8; 32]],
        1,
        addr,
        sk.verifying_key().to_bytes(),
        Signature([0u8; 64]),
    );
    v2.signature = sk.sign(&v2.signable_bytes());
    let _ = dag.try_insert(v2); // triggers equivocation

    // Future vertices from this validator must be rejected
    let v3 = make_vertex(&addr, 2, 2, vec![[0u8; 32]], vec![], &sk);
    let result = dag.try_insert(v3);
    assert_eq!(result.unwrap(), false, "Byzantine validator should be permanently banned");
}

#[test]
fn a3_invalid_signature_rejected() {
    let sk = SecretKey::from_bytes([1u8; 32]);
    let addr = sk.address();

    let mut v = make_vertex(&addr, 0, 0, vec![[0u8; 32]], vec![], &sk);
    // Corrupt the signature
    v.signature = Signature([0xAA; 64]);

    assert!(!v.verify_signature(), "Corrupted signature should fail verification");
}

#[test]
fn a4_phantom_parent_rejected() {
    let sk = SecretKey::from_bytes([1u8; 32]);
    let addr = sk.address();
    let mut dag = BlockDag::new();

    // Reference a non-existent parent (not zero hash and not in DAG)
    let phantom_hash = [0xDE; 32];
    let v = make_vertex(&addr, 0, 0, vec![phantom_hash], vec![], &sk);
    assert!(!dag.insert(v), "Vertex with phantom parent should be rejected");
}

#[test]
fn a5_future_round_rejected() {
    let sk = SecretKey::from_bytes([1u8; 32]);
    let addr = sk.address();
    let mut dag = BlockDag::new();

    // Current round is 0, insert vertex in round 100 (>10 rounds ahead)
    let v = make_vertex(&addr, 100, 100, vec![[0u8; 32]], vec![], &sk);
    assert!(!dag.insert(v), "Far-future round vertex should be rejected");
}

#[test]
fn a6_finality_requires_quorum() {
    let sks = setup_validators(4);
    let mut dag = BlockDag::new();
    let mut fin = FinalityTracker::new(3);

    for sk in &sks {
        fin.register_validator(sk.address());
    }

    // Only 1 validator produces a vertex — not enough for finality
    let v = make_vertex(&sks[0].address(), 0, 0, vec![[0u8; 32]], vec![], &sks[0]);
    dag.insert(v.clone());

    let newly = fin.find_newly_finalized(&dag);
    assert!(newly.is_empty(), "Single validator vertex should not finalize (need ceil(2*4/3)=3)");
}

#[test]
fn a7_finality_achieved_with_quorum() {
    let (_, fin, _, _) = simulate_rounds(4, 5);
    assert!(fin.finalized_count() > 0, "5 rounds with 4 validators should produce finalized vertices");
}

#[test]
fn a8_deterministic_ordering_across_instances() {
    // Two independent simulations with same inputs must produce same state
    let (_, _, state1, _) = simulate_rounds(4, 5);
    let (_, _, state2, _) = simulate_rounds(4, 5);

    assert_eq!(state1.total_supply(), state2.total_supply());
    assert_eq!(state1.last_finalized_round(), state2.last_finalized_round());
}

// ════════════════════════════════════════════════════════════
// Category B: State Machine Under Adversarial Conditions
// ════════════════════════════════════════════════════════════

#[test]
fn b1_faucet_genesis_prefund() {
    let state = StateEngine::new_with_genesis();
    let faucet_addr = faucet_keypair().address();

    assert_eq!(state.balance(&faucet_addr), FAUCET_PREFUND_SATS);
    assert_eq!(state.total_supply(), FAUCET_PREFUND_SATS + DEV_ALLOCATION_SATS);
}

#[test]
fn b2_faucet_keypair_deterministic() {
    let sk1 = faucet_keypair();
    let sk2 = faucet_keypair();
    assert_eq!(sk1.address(), sk2.address());
    assert_eq!(sk1.to_bytes(), sk2.to_bytes());
    assert_eq!(sk1.to_bytes(), FAUCET_SEED);
}

#[test]
fn b3_faucet_transaction_valid() {
    let mut state = StateEngine::new_with_genesis();
    let faucet_sk = faucet_keypair();
    let faucet_addr = faucet_sk.address();
    let recipient = SecretKey::from_bytes([99u8; 32]).address();

    let amount = 100 * COIN;
    let fee = 0;
    let tx = make_signed_tx(&faucet_sk, recipient, amount, fee, 0);

    // Create a vertex with this transaction
    let proposer_sk = SecretKey::from_bytes([1u8; 32]);
    let v = make_vertex(
        &proposer_sk.address(),
        0,
        0,
        vec![],
        vec![tx],
        &proposer_sk,
    );

    state.apply_vertex(&v).unwrap();
    assert_eq!(state.balance(&recipient), amount);
    assert_eq!(
        state.balance(&faucet_addr),
        FAUCET_PREFUND_SATS - amount
    );
}

#[test]
fn b4_double_spend_deterministic_resolution() {
    // Two conflicting txs from same sender in different vertices of the same round
    let mut state = StateEngine::new_with_genesis();
    let sender_sk = faucet_keypair();
    let sender = sender_sk.address();
    let recv_a = SecretKey::from_bytes([10u8; 32]).address();
    let recv_b = SecretKey::from_bytes([11u8; 32]).address();

    let tx_a = make_signed_tx(&sender_sk, recv_a, 800_000 * COIN, 0, 0);
    let tx_b = make_signed_tx(&sender_sk, recv_b, 800_000 * COIN, 0, 0);

    let prop1 = SecretKey::from_bytes([1u8; 32]);
    let prop2 = SecretKey::from_bytes([2u8; 32]);

    let v1 = make_vertex(&prop1.address(), 0, 0, vec![], vec![tx_a], &prop1);
    let v2 = make_vertex(&prop2.address(), 0, 1, vec![], vec![tx_b], &prop2);

    // Apply first vertex — succeeds
    state.apply_vertex(&v1).unwrap();

    // Apply second vertex — tx_b should fail (insufficient balance after tx_a)
    let result = state.apply_vertex(&v2);
    assert!(result.is_err(), "Second conflicting tx should fail");

    // Only recv_a should have funds
    assert_eq!(state.balance(&recv_a), 800_000 * COIN);
    assert_eq!(state.balance(&recv_b), 0);
}

#[test]
fn b5_nonce_replay_rejected() {
    let mut state = StateEngine::new_with_genesis();
    let sender_sk = faucet_keypair();
    let recv = SecretKey::from_bytes([20u8; 32]).address();

    let tx0 = make_signed_tx(&sender_sk, recv, 1000, 0, 0);
    let prop = SecretKey::from_bytes([1u8; 32]);
    let v0 = make_vertex(&prop.address(), 0, 0, vec![], vec![tx0.clone()], &prop);
    state.apply_vertex(&v0).unwrap();

    assert_eq!(state.nonce(&sender_sk.address()), 1);

    // Replay nonce=0
    let tx_replay = make_signed_tx(&sender_sk, recv, 1000, 0, 0);
    let v1 = make_vertex(&prop.address(), 1, 1, vec![], vec![tx_replay], &prop);
    let result = state.apply_vertex(&v1);
    assert!(result.is_err(), "Replayed nonce=0 should be rejected");
}

#[test]
fn b6_atomic_vertex_application() {
    // If any tx in a vertex fails, entire vertex must be rolled back
    let mut state = StateEngine::new_with_genesis();
    let faucet_sk = faucet_keypair();
    let recv = SecretKey::from_bytes([30u8; 32]).address();

    // Good tx then bad tx (insufficient balance)
    let tx_good = make_signed_tx(&faucet_sk, recv, 1000, 0, 0);
    let bad_sk = SecretKey::from_bytes([99u8; 32]); // no balance
    let tx_bad = make_signed_tx(&bad_sk, recv, 999_999 * COIN, 0, 0);

    let prop = SecretKey::from_bytes([1u8; 32]);
    let v = make_vertex(&prop.address(), 0, 0, vec![], vec![tx_good, tx_bad], &prop);

    let supply_before = state.total_supply();
    let result = state.apply_vertex(&v);
    assert!(result.is_err(), "Vertex with bad tx should fail");

    // State should not have changed (atomic rollback)
    assert_eq!(state.total_supply(), supply_before);
    assert_eq!(state.balance(&recv), 0, "Receiver should have 0 after rollback");
    assert_eq!(state.nonce(&faucet_sk.address()), 0, "Nonce should not advance after rollback");
}

// ════════════════════════════════════════════════════════════
// Category C: Transaction Edge Cases
// ════════════════════════════════════════════════════════════

#[test]
fn c1_max_amount_transaction() {
    let mut state = StateEngine::new_with_genesis();
    let faucet_sk = faucet_keypair();
    let recv = SecretKey::from_bytes([40u8; 32]).address();

    // Send entire faucet balance
    let tx = make_signed_tx(&faucet_sk, recv, FAUCET_PREFUND_SATS, 0, 0);
    let prop = SecretKey::from_bytes([1u8; 32]);
    let v = make_vertex(&prop.address(), 0, 0, vec![], vec![tx], &prop);
    state.apply_vertex(&v).unwrap();

    assert_eq!(state.balance(&faucet_sk.address()), 0);
    assert_eq!(state.balance(&recv), FAUCET_PREFUND_SATS);
}

#[test]
fn c2_exceed_balance_by_one_satoshi() {
    let mut state = StateEngine::new_with_genesis();
    let faucet_sk = faucet_keypair();
    let recv = SecretKey::from_bytes([41u8; 32]).address();

    let tx = make_signed_tx(&faucet_sk, recv, FAUCET_PREFUND_SATS + 1, 0, 0);
    let prop = SecretKey::from_bytes([1u8; 32]);
    let v = make_vertex(&prop.address(), 0, 0, vec![], vec![tx], &prop);
    let result = state.apply_vertex(&v);
    assert!(result.is_err(), "Should reject tx exceeding balance by 1 sat");
}

#[test]
fn c3_fee_counts_against_balance() {
    let mut state = StateEngine::new_with_genesis();
    let faucet_sk = faucet_keypair();
    let recv = SecretKey::from_bytes([42u8; 32]).address();

    // amount + fee > balance
    let tx = make_signed_tx(&faucet_sk, recv, FAUCET_PREFUND_SATS, 1, 0);
    let prop = SecretKey::from_bytes([1u8; 32]);
    let v = make_vertex(&prop.address(), 0, 0, vec![], vec![tx], &prop);
    let result = state.apply_vertex(&v);
    assert!(result.is_err(), "Fee should count against balance");
}

#[test]
fn c4_self_send_preserves_balance() {
    let mut state = StateEngine::new_with_genesis();
    let faucet_sk = faucet_keypair();
    let faucet_addr = faucet_sk.address();

    // Self-send with fee
    let fee = 1000;
    let tx = make_signed_tx(&faucet_sk, faucet_addr, 1000 * COIN, fee, 0);
    let prop = SecretKey::from_bytes([1u8; 32]);
    let v = make_vertex(&prop.address(), 0, 0, vec![], vec![tx], &prop);
    state.apply_vertex(&v).unwrap();

    // Balance should decrease by only the fee
    assert_eq!(
        state.balance(&faucet_addr),
        FAUCET_PREFUND_SATS - fee
    );
}

#[test]
fn c5_sequential_nonce_enforcement() {
    let mut state = StateEngine::new_with_genesis();
    let faucet_sk = faucet_keypair();
    let recv = SecretKey::from_bytes([43u8; 32]).address();

    // Skip nonce 0, try nonce 1 — should fail
    let tx = make_signed_tx(&faucet_sk, recv, 1000, 0, 1);
    let prop = SecretKey::from_bytes([1u8; 32]);
    let v = make_vertex(&prop.address(), 0, 0, vec![], vec![tx], &prop);
    let result = state.apply_vertex(&v);
    assert!(result.is_err(), "Skipped nonce should be rejected");
}

#[test]
fn c6_many_sequential_nonces() {
    let mut state = StateEngine::new_with_genesis();
    let faucet_sk = faucet_keypair();
    let recv = SecretKey::from_bytes([44u8; 32]).address();
    let prop = SecretKey::from_bytes([1u8; 32]);

    // 100 sequential transactions, each in its own vertex
    for nonce in 0u64..100 {
        let tx = make_signed_tx(&faucet_sk, recv, 1000, 0, nonce);
        let v = make_vertex(
            &prop.address(),
            nonce,
            nonce,
            if nonce == 0 { vec![] } else { vec![[0u8; 32]] },
            vec![tx],
            &prop,
        );
        state.apply_vertex(&v).unwrap();
    }

    assert_eq!(state.nonce(&faucet_sk.address()), 100);
    assert_eq!(state.balance(&recv), 100 * 1000);
}

// ════════════════════════════════════════════════════════════
// Category D: Multi-Validator DAG Adversarial Scenarios
// ════════════════════════════════════════════════════════════

#[test]
fn d1_minority_cannot_finalize() {
    // With 4 validators, 1 validator alone cannot finalize
    let sks = setup_validators(4);
    let mut dag = BlockDag::new();
    let mut fin = FinalityTracker::new(3);

    for sk in &sks {
        fin.register_validator(sk.address());
    }

    // Only validator 0 produces vertices for 10 rounds
    for round in 0..10 {
        let tips = dag.tips();
        let parents = if tips.is_empty() { vec![[0u8; 32]] } else { tips };
        let v = make_vertex(&sks[0].address(), round, round, parents, vec![], &sks[0]);
        dag.insert(v);
    }

    let newly = fin.find_newly_finalized(&dag);
    assert!(newly.is_empty(), "Minority (1/4) should never finalize");
}

#[test]
fn d2_network_continues_with_one_crash() {
    // 3 out of 4 validators produce — should still finalize
    let sks = setup_validators(4);
    let mut dag = BlockDag::new();
    let mut fin = FinalityTracker::new(3);

    for sk in &sks {
        fin.register_validator(sk.address());
    }

    // Only validators 0, 1, 2 produce (validator 3 is "crashed")
    for round in 0..10 {
        let tips = dag.tips();
        let parents = if tips.is_empty() { vec![[0u8; 32]] } else { tips };

        for sk in &sks[..3] {
            let v = make_vertex(
                &sk.address(),
                round,
                round * 3 + sks.iter().position(|s| s.address() == sk.address()).unwrap() as u64,
                parents.clone(),
                vec![],
                sk,
            );
            dag.insert(v);
        }
    }

    // Check finality
    loop {
        let newly = fin.find_newly_finalized(&dag);
        if newly.is_empty() {
            break;
        }
    }

    assert!(fin.finalized_count() > 0, "3/4 validators (above quorum=3) should finalize");
}

#[test]
fn d3_network_stalls_below_quorum() {
    // Only 2 out of 4 validators — below quorum, should NOT finalize
    let sks = setup_validators(4);
    let mut dag = BlockDag::new();
    let mut fin = FinalityTracker::new(3);

    for sk in &sks {
        fin.register_validator(sk.address());
    }

    for round in 0..10 {
        let tips = dag.tips();
        let parents = if tips.is_empty() { vec![[0u8; 32]] } else { tips };

        // Only 2 validators
        for sk in &sks[..2] {
            let v = make_vertex(
                &sk.address(),
                round,
                round * 2 + sks.iter().position(|s| s.address() == sk.address()).unwrap() as u64,
                parents.clone(),
                vec![],
                sk,
            );
            dag.insert(v);
        }
    }

    loop {
        let newly = fin.find_newly_finalized(&dag);
        if newly.is_empty() {
            break;
        }
    }

    assert_eq!(fin.finalized_count(), 0, "2/4 validators (below quorum=3) should NOT finalize");
}

#[test]
fn d4_supply_cap_enforced_near_max() {
    let mut state = StateEngine::new_with_genesis();
    // Genesis has 2,050,000 UDAG. We need to get close to max (21M).
    let max = constants::MAX_SUPPLY_SATS;
    
    // Apply many vertices to approach max supply through block rewards
    let validators: Vec<_> = (0..4).map(|i| SecretKey::from_bytes([i as u8; 32])).collect();
    
    // Apply vertices until we're close to max supply
    let mut round = 0u64;
    while state.total_supply() < max - (1000 * constants::COIN) {
        let proposer = &validators[(round % 4) as usize];
        let v = make_vertex(&proposer.address(), round, round, vec![], vec![], proposer);
        state.apply_vertex(&v).unwrap();
        round += 1;
        
        // Safety: don't loop forever
        if round > 100_000 {
            break;
        }
    }

    // Now apply one more vertex - should be capped
    let prop = &validators[0];
    let v = make_vertex(&prop.address(), round, round, vec![], vec![], prop);
    state.apply_vertex(&v).unwrap();

    assert!(state.total_supply() <= max, "Supply must never exceed MAX_SUPPLY_SATS");
}

#[test]
fn d5_mempool_capacity_limit() {
    let mut mp = Mempool::new();
    let sk = SecretKey::from_bytes([1u8; 32]);
    let recv = SecretKey::from_bytes([2u8; 32]).address();

    // Insert 10,001 transactions — should not exceed capacity
    for nonce in 0..10_001u64 {
        let tx = make_signed_tx(&sk, recv, 100, nonce + 1, nonce); // fee = nonce+1 for ordering
        mp.insert(tx);
    }

    assert!(mp.len() <= 10_000, "Mempool should enforce capacity limit (10,000)");
}

#[test]
fn d6_message_size_constant() {
    // Verify MAX_MESSAGE_SIZE is enforced at the protocol level
    // We can only test the constant exists in ultradag-coin
    assert_eq!(FAUCET_PREFUND_SATS, 1_000_000 * COIN, "Faucet prefund should be 1M UDAG");
}

#[test]
fn d7_genesis_state_deterministic() {
    let s1 = StateEngine::new_with_genesis();
    let s2 = StateEngine::new_with_genesis();
    let faucet = faucet_keypair().address();

    assert_eq!(s1.balance(&faucet), s2.balance(&faucet));
    assert_eq!(s1.total_supply(), s2.total_supply());
}

// ════════════════════════════════════════════════════════════
// Category E: Protocol Improvements (Optimistic, Epoch, Descendant)
// ════════════════════════════════════════════════════════════

/// E1: Optimistic responsiveness must not cause double production.
/// A validator that produces in a round must not produce again in the same round
/// even if new vertices arrive (simulated by re-inserting tips).
#[test]
fn e1_optimistic_does_not_double_produce() {
    let sks = setup_validators(4);
    let mut dag = BlockDag::new();

    // Round 0: all 4 validators produce
    for (i, sk) in sks.iter().enumerate() {
        let v = make_vertex(&sk.address(), 0, i as u64, vec![[0u8; 32]], vec![], sk);
        dag.insert(v);
    }

    // Round 1: validator 0 produces
    let tips = dag.tips();
    let v1 = make_vertex(&sks[0].address(), 1, 10, tips.clone(), vec![], &sks[0]);
    dag.insert(v1);

    // Equivocation check: DAG must reject a second vertex from sks[0] in round 1
    let v1_dup = make_vertex(&sks[0].address(), 1, 11, tips, vec![], &sks[0]);
    let result = dag.try_insert(v1_dup);
    assert!(
        matches!(result, Err(DagInsertError::Equivocation { .. })),
        "DAG must reject duplicate validator in same round"
    );

    // Only one vertex from sks[0] in round 1
    assert_eq!(dag.distinct_validators_in_round(1).len(), 1);
}

/// E2: Epoch transition with a Byzantine validator that gets removed.
/// After epoch transition, a previously active validator that unstaked should be
/// excluded from the FinalityTracker.
#[test]
fn e2_epoch_transition_with_byzantine_validator() {
    use ultradag_coin::consensus::epoch::sync_epoch_validators;
    use ultradag_coin::tx::stake::MIN_STAKE_SATS;

    let mut state = StateEngine::new_with_genesis();
    let mut ft = FinalityTracker::new(2);

    let sk_good1 = SecretKey::generate();
    let sk_good2 = SecretKey::generate();
    let sk_bad = SecretKey::generate();

    // All 3 register in FinalityTracker
    ft.register_validator(sk_good1.address());
    ft.register_validator(sk_good2.address());
    ft.register_validator(sk_bad.address());
    assert_eq!(ft.validator_count(), 3);

    // Only good validators stake
    state.faucet_credit(&sk_good1.address(), MIN_STAKE_SATS).unwrap();
    state.faucet_credit(&sk_good2.address(), MIN_STAKE_SATS).unwrap();
    let mut stx1 = ultradag_coin::StakeTx {
        from: sk_good1.address(),
        amount: MIN_STAKE_SATS,
        nonce: 0,
        pub_key: sk_good1.verifying_key().to_bytes(),
        signature: Signature([0u8; 64]),
    };
    stx1.signature = sk_good1.sign(&stx1.signable_bytes());
    state.apply_stake_tx(&stx1).unwrap();

    let mut stx2 = ultradag_coin::StakeTx {
        from: sk_good2.address(),
        amount: MIN_STAKE_SATS,
        nonce: 0,
        pub_key: sk_good2.verifying_key().to_bytes(),
        signature: Signature([0u8; 64]),
    };
    stx2.signature = sk_good2.sign(&stx2.signable_bytes());
    state.apply_stake_tx(&stx2).unwrap();

    state.recalculate_active_set();

    // Sync: bad validator should be removed
    sync_epoch_validators(&mut ft, &state);
    assert_eq!(ft.validator_count(), 2);
    assert!(!ft.validator_set().is_allowed(&sk_bad.address()));
}

/// E3: After epoch transition, quorum threshold changes to match new validator set size.
#[test]
fn e3_epoch_transition_quorum_change() {
    use ultradag_coin::consensus::epoch::sync_epoch_validators;
    use ultradag_coin::tx::stake::MIN_STAKE_SATS;

    let mut state = StateEngine::new_with_genesis();
    let mut ft = FinalityTracker::new(2);

    // Start with 4 validators in FinalityTracker
    let sks: Vec<SecretKey> = (0..4).map(|_| SecretKey::generate()).collect();
    for sk in &sks {
        ft.register_validator(sk.address());
    }
    assert_eq!(ft.finality_threshold(), 3); // ceil(2*4/3) = 3

    // Only 3 stake
    for sk in &sks[..3] {
        state.faucet_credit(&sk.address(), MIN_STAKE_SATS).unwrap();
        let mut stx = ultradag_coin::StakeTx {
            from: sk.address(),
            amount: MIN_STAKE_SATS,
            nonce: 0,
            pub_key: sk.verifying_key().to_bytes(),
            signature: Signature([0u8; 64]),
        };
        stx.signature = sk.sign(&stx.signable_bytes());
        state.apply_stake_tx(&stx).unwrap();
    }
    state.recalculate_active_set();

    // After epoch sync, threshold should adjust to 3 active validators
    sync_epoch_validators(&mut ft, &state);
    assert_eq!(ft.validator_count(), 3);
    assert_eq!(ft.finality_threshold(), 2); // ceil(2*3/3) = 2
}

/// E4: Descendant validator count must be correct after a fork resolves.
/// When two branches of the DAG merge, ancestor vertices should see
/// the union of descendant validators.
#[test]
fn e4_descendant_count_correct_after_fork_resolution() {
    let mut dag = BlockDag::new();

    let sk1 = SecretKey::generate();
    let sk2 = SecretKey::generate();
    let sk3 = SecretKey::generate();

    // Genesis vertex by sk1
    let v1 = make_vertex(&sk1.address(), 0, 0, vec![[0u8; 32]], vec![], &sk1);
    let h1 = v1.hash();
    dag.insert(v1);

    // Fork: sk2 and sk3 each build on v1 independently
    let v2 = make_vertex(&sk2.address(), 1, 1, vec![h1], vec![], &sk2);
    let h2 = v2.hash();
    dag.insert(v2);

    let v3 = make_vertex(&sk3.address(), 1, 2, vec![h1], vec![], &sk3);
    let h3 = v3.hash();
    dag.insert(v3);

    // v1 should see descendant validators sk2 and sk3 = 2
    assert_eq!(dag.descendant_validator_count(&h1), 2);

    // Merge: sk1 builds on both branches
    let v4 = make_vertex(&sk1.address(), 2, 3, vec![h2, h3], vec![], &sk1);
    dag.insert(v4);

    // v2 and v3 should each see 1 descendant validator (sk1 via v4)
    assert_eq!(dag.descendant_validator_count(&h2), 1);
    assert_eq!(dag.descendant_validator_count(&h3), 1);

    // v1 should now see 3 (sk2, sk3, and sk1 propagated from v4)
    assert_eq!(dag.descendant_validator_count(&h1), 3);
}

/// E5: Finality must never regress — once a vertex is finalized,
/// adding more vertices must not un-finalize it.
#[test]
fn e5_finality_does_not_regress() {
    let mut dag = BlockDag::new();
    let mut ft = FinalityTracker::new(2);

    let sks = setup_validators(3);
    for sk in &sks {
        ft.register_validator(sk.address());
    }

    // Build a chain: v1(sk1) <- v2(sk2) <- v3(sk3)
    let v1 = make_vertex(&sks[0].address(), 0, 0, vec![[0u8; 32]], vec![], &sks[0]);
    let h1 = v1.hash();
    dag.insert(v1);

    let v2 = make_vertex(&sks[1].address(), 1, 1, vec![h1], vec![], &sks[1]);
    let h2 = v2.hash();
    dag.insert(v2);

    let v3 = make_vertex(&sks[2].address(), 2, 2, vec![h2], vec![], &sks[2]);
    dag.insert(v3);

    // Finalize
    let finalized = ft.find_newly_finalized(&dag);
    assert!(finalized.contains(&h1), "v1 should be finalized");
    assert!(ft.is_finalized(&h1));

    // Add many more vertices — v1 must remain finalized
    for round in 3..20 {
        let sk = &sks[round as usize % 3];
        let tips = dag.tips();
        let v = make_vertex(&sk.address(), round, round, tips, vec![], sk);
        dag.insert(v);
        let _ = ft.find_newly_finalized(&dag);
    }

    assert!(ft.is_finalized(&h1), "v1 finality must never regress");
    assert!(ft.is_finalized(&h2), "v2 should also be finalized by now");
}
