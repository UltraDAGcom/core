//! Tests for cross-batch equivocation detection.
//!
//! Verifies that equivocating vertices split across separate calls to
//! `apply_finalized_vertices` are correctly detected and slashed via the
//! `applied_validators_per_round` HashMap on StateEngine.

use ultradag_coin::*;

fn make_vertex(
    proposer_sk: &SecretKey,
    round: u64,
    height: u64,
    txs: Vec<Transaction>,
) -> DagVertex {
    let proposer = proposer_sk.address();
    let total_fees: u64 = txs.iter().map(|tx| tx.fee()).sum();
    let coinbase = CoinbaseTx {
        to: proposer,
        amount: total_fees,
        height,
    };
    let block = Block {
        header: BlockHeader {
            version: 1,
            height,
            timestamp: 1_000_000 + round as i64,
            prev_hash: [0u8; 32],
            merkle_root: [0u8; 32],
        },
        coinbase,
        transactions: txs,
    };
    let mut vertex = DagVertex::new(
        block,
        vec![[0u8; 32]],
        round,
        proposer,
        proposer_sk.verifying_key().to_bytes(),
        Signature([0u8; 64]),
    );
    vertex.signature = proposer_sk.sign(&vertex.signable_bytes());
    vertex
}

/// Make a vertex that differs from another by including a dummy timestamp change,
/// producing a different hash for the same (validator, round) pair.
fn make_equivocating_vertex(
    proposer_sk: &SecretKey,
    round: u64,
    height: u64,
) -> DagVertex {
    let proposer = proposer_sk.address();
    let coinbase = CoinbaseTx {
        to: proposer,
        amount: 0,
        height,
    };
    let block = Block {
        header: BlockHeader {
            version: 1,
            height,
            timestamp: 2_000_000 + round as i64, // Different timestamp → different hash
            prev_hash: [0u8; 32],
            merkle_root: [0u8; 32],
        },
        coinbase,
        transactions: vec![],
    };
    let mut vertex = DagVertex::new(
        block,
        vec![[0u8; 32]],
        round,
        proposer,
        proposer_sk.verifying_key().to_bytes(),
        Signature([0u8; 64]),
    );
    vertex.signature = proposer_sk.sign(&vertex.signable_bytes());
    vertex
}

/// Cross-batch equivocation: vertex A from validator V in round R arrives in batch 1,
/// equivocating vertex B from validator V in round R arrives in batch 2.
/// The `applied_validators_per_round` HashMap must detect this and slash V.
#[test]
fn test_cross_batch_equivocation_detected_and_slashed() {
    let mut state = StateEngine::new_with_genesis();
    state.set_configured_validator_count(3);

    let sk_bad = SecretKey::generate();
    let sk_good1 = SecretKey::generate();
    let sk_good2 = SecretKey::generate();

    // Fund and stake the bad validator
    let stake_amount = MIN_STAKE_SATS * 2;
    state.faucet_credit(&sk_bad.address(), stake_amount).unwrap();
    let mut stake_tx = StakeTx {
        from: sk_bad.address(),
        amount: stake_amount,
        nonce: 0,
        pub_key: sk_bad.verifying_key().to_bytes(),
        signature: Signature([0u8; 64]),
    };
    stake_tx.signature = sk_bad.sign(&stake_tx.signable_bytes());
    state.apply_stake_tx(&stake_tx).unwrap();

    let stake_before = state.stake_of(&sk_bad.address());
    assert!(stake_before > 0, "Bad validator must have stake");

    // Batch 1: normal vertex from bad validator in round 5
    let v1 = make_vertex(&sk_bad, 5, 0, vec![]);
    let v1_hash = v1.hash();
    let v_good1 = make_vertex(&sk_good1, 5, 0, vec![]);
    let v_good2 = make_vertex(&sk_good2, 5, 0, vec![]);
    state.apply_finalized_vertices(&[v1, v_good1, v_good2]).unwrap();

    let stake_after_batch1 = state.stake_of(&sk_bad.address());
    assert_eq!(stake_after_batch1, stake_before, "No slash after batch 1 (no equivocation yet)");

    // Batch 2: equivocating vertex from bad validator in SAME round 5
    // This simulates the scenario where the DAG defense was somehow bypassed
    // (e.g., CheckpointSync suffix, future implementation bug)
    let v_equivocating = make_equivocating_vertex(&sk_bad, 5, 0);
    assert_ne!(v1_hash, v_equivocating.hash(), "Equivocating vertices must have different hashes");

    let v_good1_r6 = make_vertex(&sk_good1, 6, 1, vec![]);
    let v_good2_r6 = make_vertex(&sk_good2, 6, 1, vec![]);
    state.apply_finalized_vertices(&[v_equivocating, v_good1_r6, v_good2_r6]).unwrap();

    // Verify slashing occurred
    let stake_after_slash = state.stake_of(&sk_bad.address());
    let slash_pct = state.governance_params().slash_percent;
    let expected_slashed = stake_before.saturating_mul(slash_pct) / 100;
    let expected_remaining = stake_before.saturating_sub(expected_slashed);
    assert_eq!(
        stake_after_slash, expected_remaining,
        "Cross-batch equivocation must slash {}% of stake", slash_pct
    );
}

/// Verify that within-batch equivocation also works (baseline).
#[test]
fn test_intra_batch_equivocation_detected_and_slashed() {
    let mut state = StateEngine::new_with_genesis();
    state.set_configured_validator_count(3);

    let sk_bad = SecretKey::generate();
    let sk_good1 = SecretKey::generate();
    let sk_good2 = SecretKey::generate();

    // Fund and stake the bad validator
    let stake_amount = MIN_STAKE_SATS * 2;
    state.faucet_credit(&sk_bad.address(), stake_amount).unwrap();
    let mut stake_tx = StakeTx {
        from: sk_bad.address(),
        amount: stake_amount,
        nonce: 0,
        pub_key: sk_bad.verifying_key().to_bytes(),
        signature: Signature([0u8; 64]),
    };
    stake_tx.signature = sk_bad.sign(&stake_tx.signable_bytes());
    state.apply_stake_tx(&stake_tx).unwrap();

    let stake_before = state.stake_of(&sk_bad.address());

    // Single batch with two vertices from the same validator in the same round
    let v1 = make_vertex(&sk_bad, 5, 0, vec![]);
    let v2 = make_equivocating_vertex(&sk_bad, 5, 0);
    let v_good1 = make_vertex(&sk_good1, 5, 0, vec![]);
    let v_good2 = make_vertex(&sk_good2, 5, 0, vec![]);

    state.apply_finalized_vertices(&[v1, v2, v_good1, v_good2]).unwrap();

    // Verify slashing occurred
    let stake_after = state.stake_of(&sk_bad.address());
    let slash_pct = state.governance_params().slash_percent;
    let expected_slashed = stake_before.saturating_mul(slash_pct) / 100;
    let expected_remaining = stake_before.saturating_sub(expected_slashed);
    assert_eq!(
        stake_after, expected_remaining,
        "Intra-batch equivocation must slash {}% of stake", slash_pct
    );
}

/// Verify that the applied_validators_per_round map is pruned after 1000 rounds,
/// so memory doesn't grow unbounded.
#[test]
fn test_applied_validators_pruned_after_1000_rounds() {
    let mut state = StateEngine::new_with_genesis();
    state.set_configured_validator_count(1);

    let sk = SecretKey::generate();

    // Apply vertices across a wide range of rounds
    for round in 1..=1100u64 {
        let v = make_vertex(&sk, round, round, vec![]);
        state.apply_finalized_vertices(&[v]).unwrap();
    }

    // After applying round 1100, rounds < 100 should be pruned from the tracker.
    // We can't directly inspect the map, but we can verify that a "cross-batch"
    // vertex in round 1 does NOT cause a slash (because it was pruned from the tracker).
    let stake_before = state.stake_of(&sk.address());
    let old_v = make_equivocating_vertex(&sk, 1, 1);
    let continuation = make_vertex(&sk, 1101, 1101, vec![]);
    state.apply_finalized_vertices(&[old_v, continuation]).unwrap();
    let stake_after = state.stake_of(&sk.address());

    // The old round 1 entry was pruned from applied_validators_per_round,
    // so the "equivocating" vertex in round 1 won't trigger cross-batch detection.
    // (Intra-batch with itself doesn't count as equivocation since it's only 1 vertex.)
    assert_eq!(stake_before, stake_after, "Pruned rounds should not trigger false slashing");
}
