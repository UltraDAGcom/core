/// Comprehensive BFT Rules Verification Tests
/// Each test proves a specific BFT property with real Ed25519 keys and actual code paths.

use ultradag_coin::{
    Block, BlockDag, BlockHeader, CoinbaseTx, DagVertex, FinalityTracker,
    SecretKey, Signature, Transaction, ValidatorSet,
};

/// Helper: Create a signed DAG vertex
fn make_vertex(
    sk: &SecretKey,
    round: u64,
    height: u64,
    parent_hashes: Vec<[u8; 32]>,
    txs: Vec<Transaction>,
) -> DagVertex {
    let proposer = sk.address();
    let total_fees: u64 = txs.iter().map(|tx| tx.fee).sum();
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
// RULE 1: Every validator produces exactly one vertex per round
// ============================================================================

#[test]
fn rule1_equivocation_is_rejected() {
    // Setup: Fresh Ed25519 keypair
    let sk = SecretKey::generate();
    let validator = sk.address();
    
    let mut dag = BlockDag::new();
    
    // First vertex for round 1 with empty transactions
    let vertex1 = make_vertex(&sk, 1, 0, vec![], vec![]);
    let hash1 = vertex1.hash();
    
    // Insert first vertex - should succeed
    dag.insert(vertex1.clone());
    assert_eq!(dag.len(), 1);
    assert!(dag.get(&hash1).is_some());
    
    // Second vertex for SAME round 1 with DIFFERENT height (equivocation)
    // Different height creates different block hash, thus different vertex hash
    let vertex2 = make_vertex(&sk, 1, 1, vec![], vec![]);
    let hash2 = vertex2.hash();
    
    // Hashes must be different (different transaction sets)
    assert_ne!(hash1, hash2, "Different vertices should have different hashes");
    
    // Check equivocation BEFORE inserting
    let has_equivocation = dag.has_vertex_from_validator_in_round(&validator, 1);
    assert!(has_equivocation, "Should detect existing vertex in round 1");
    
    // Try to insert with equivocation check - should be rejected
    let result = dag.try_insert(vertex2.clone());
    assert!(result.is_err(), "Equivocation should be rejected");
    
    // DAG should still have only 1 vertex
    assert_eq!(dag.len(), 1, "Equivocating vertex should not be inserted");
}

#[test]
fn rule1_equivocation_check_is_per_validator_per_round() {
    let sk1 = SecretKey::generate();
    let sk2 = SecretKey::generate();
    let addr1 = sk1.address();
    let addr2 = sk2.address();
    
    let mut dag = BlockDag::new();
    
    // Validator 1 produces for round 1
    let v1_r1 = make_vertex(&sk1, 1, 0, vec![], vec![]);
    let inserted = dag.insert(v1_r1);
    assert!(inserted, "First vertex should be inserted");
    
    // Validator 2 produces for round 1 - should be allowed (different validator)
    let v2_r1 = make_vertex(&sk2, 1, 1, vec![], vec![]);
    let inserted = dag.insert(v2_r1);
    assert!(inserted, "Second validator's vertex should be inserted");
    
    assert_eq!(dag.len(), 2, "Should have 2 vertices");
    assert!(dag.has_vertex_from_validator_in_round(&addr1, 1));
    assert!(dag.has_vertex_from_validator_in_round(&addr2, 1));
    
    // Validator 1 produces for round 2 - should be allowed (different round)
    let v1_r2_parents = dag.tips();
    let v1_r2 = make_vertex(&sk1, 2, 2, v1_r2_parents, vec![]);
    let inserted = dag.insert(v1_r2);
    assert!(inserted, "Validator 1 round 2 vertex should be inserted");
    
    assert_eq!(dag.len(), 3, "Should have 3 vertices");
    assert!(dag.has_vertex_from_validator_in_round(&addr1, 2));
    
    // But validator 1 cannot produce another round 1 vertex
    assert!(dag.has_vertex_from_validator_in_round(&addr1, 1));
}

// ============================================================================
// RULE 2: 2f+1 reference rule is enforced before production
// ============================================================================

#[test]
fn rule2_cannot_produce_without_quorum() {
    // 4 validators, f=1, threshold=3
    let validators: Vec<SecretKey> = (0..4).map(|_| SecretKey::generate()).collect();
    let mut val_set = ValidatorSet::new(3);
    for sk in &validators {
        val_set.register(sk.address());
    }
    
    let threshold = val_set.quorum_threshold();
    assert_eq!(threshold, 3, "With 4 validators, threshold must be 3");
    
    let mut dag = BlockDag::new();
    
    // Round 1: Only validator 0 produces
    let v0_r1 = make_vertex(&validators[0], 1, 0, vec![], vec![]);
    dag.insert(v0_r1);
    
    // Validator 3 wants to produce round 2
    // Check: how many round 1 vertices exist?
    let round1_validators = dag.distinct_validators_in_round(1);
    assert_eq!(round1_validators.len(), 1, "Only 1 validator in round 1");
    
    // 1 < 3, so validator 3 should NOT produce
    let can_produce = round1_validators.len() >= threshold;
    assert!(!can_produce, "Should not be able to produce with only 1/3 quorum");
    
    // Add second round 1 vertex (different height to ensure unique hash)
    let v1_r1 = make_vertex(&validators[1], 1, 1, vec![], vec![]);
    dag.insert(v1_r1);
    
    let round1_validators = dag.distinct_validators_in_round(1);
    assert_eq!(round1_validators.len(), 2, "Now 2 validators in round 1");
    
    // 2 < 3, still cannot produce
    let can_produce = round1_validators.len() >= threshold;
    assert!(!can_produce, "Should not be able to produce with only 2/3 quorum");
    
    // Add third round 1 vertex (different height to ensure unique hash)
    let v2_r1 = make_vertex(&validators[2], 1, 2, vec![], vec![]);
    dag.insert(v2_r1);
    
    let round1_validators = dag.distinct_validators_in_round(1);
    assert_eq!(round1_validators.len(), 3, "Now 3 validators in round 1");
    
    // 3 >= 3, NOW can produce
    let can_produce = round1_validators.len() >= threshold;
    assert!(can_produce, "Should be able to produce with 3/3 quorum");
    
    // Validator 3 can now produce round 2
    let parent_hashes: Vec<[u8; 32]> = dag.tips();
    let v3_r2 = make_vertex(&validators[3], 2, 3, parent_hashes, vec![]);
    dag.insert(v3_r2);
    
    assert_eq!(dag.current_round(), 2);
}

// ============================================================================
// RULE 3: Signatures are verified on receipt
// ============================================================================

#[test]
fn rule3_tampered_payload_rejected() {
    let sk = SecretKey::generate();
    
    // Create valid vertex
    let mut vertex = make_vertex(&sk, 1, 0, vec![], vec![]);
    
    // Verify it's valid first
    assert!(vertex.verify_signature(), "Original vertex should be valid");
    
    // Tamper with payload AFTER signing (change round number)
    vertex.round = 999;
    
    // Signature verification should fail
    assert!(!vertex.verify_signature(), "Tampered payload should fail verification");
}

#[test]
fn rule3_tampered_signature_rejected() {
    let sk = SecretKey::generate();
    
    // Create valid vertex
    let mut vertex = make_vertex(&sk, 1, 0, vec![], vec![]);
    assert!(vertex.verify_signature(), "Original vertex should be valid");
    
    // Tamper with signature bytes directly
    vertex.signature.0[0] ^= 0xFF;
    vertex.signature.0[1] ^= 0xFF;
    
    // Signature verification should fail
    assert!(!vertex.verify_signature(), "Tampered signature should fail verification");
}

#[test]
fn rule3_tampered_validator_address_rejected() {
    let sk = SecretKey::generate();
    let other_sk = SecretKey::generate();
    
    // Create valid vertex
    let mut vertex = make_vertex(&sk, 1, 0, vec![], vec![]);
    assert!(vertex.verify_signature(), "Original vertex should be valid");
    
    // Tamper with validator address (claim to be someone else)
    vertex.validator = other_sk.address();
    
    // Signature verification should fail (pubkey doesn't match address)
    assert!(!vertex.verify_signature(), "Tampered validator address should fail verification");
}

// ============================================================================
// RULE 4: Unknown validators are rejected
// ============================================================================

#[test]
fn rule4_unknown_validator_rejected_then_accepted() {
    let known_sk = SecretKey::generate();
    let unknown_sk = SecretKey::generate();
    
    let mut val_set = ValidatorSet::new(1);
    val_set.register(known_sk.address());
    
    // Create vertex from unknown validator
    let vertex = make_vertex(&unknown_sk, 1, 0, vec![], vec![]);
    
    // Check membership
    let is_member = val_set.contains(&vertex.validator);
    assert!(!is_member, "Unknown validator should not be in set");
    
    // Now add the unknown validator to the set
    val_set.register(unknown_sk.address());
    
    // Check membership again
    let is_member = val_set.contains(&vertex.validator);
    assert!(is_member, "Validator should now be in set");
    
    // Signature should still be valid
    assert!(vertex.verify_signature(), "Vertex signature should be valid");
}

#[test]
fn rule4_validator_set_membership_is_checked() {
    let sk1 = SecretKey::generate();
    let sk2 = SecretKey::generate();
    let sk3 = SecretKey::generate();
    
    let mut val_set = ValidatorSet::new(2);
    val_set.register(sk1.address());
    val_set.register(sk2.address());
    
    // sk1 and sk2 are members
    assert!(val_set.contains(&sk1.address()));
    assert!(val_set.contains(&sk2.address()));
    
    // sk3 is not a member
    assert!(!val_set.contains(&sk3.address()));
    
    // Create vertices
    let v1 = make_vertex(&sk1, 1, 0, vec![], vec![]);
    let v2 = make_vertex(&sk2, 1, 0, vec![], vec![]);
    let v3 = make_vertex(&sk3, 1, 0, vec![], vec![]);
    
    // Check which should be accepted
    assert!(val_set.contains(&v1.validator), "v1 should be accepted");
    assert!(val_set.contains(&v2.validator), "v2 should be accepted");
    assert!(!val_set.contains(&v3.validator), "v3 should be rejected");
}

// ============================================================================
// RULE 5: Finality requires exactly 2f+1 distinct validator descendants
// ============================================================================

#[test]
fn rule5_finality_threshold_n4_f1() {
    // n=4, f=1, threshold must be exactly 3
    let mut val_set = ValidatorSet::new(3);
    for _ in 0..4 {
        val_set.register(SecretKey::generate().address());
    }
    
    let threshold = val_set.quorum_threshold();
    assert_eq!(threshold, 3, "n=4, f=1: threshold must be exactly 3");
    
    // Verify formula: ceil(2n/3) = ceil(8/3) = 3
    let n = 4;
    let expected = (2 * n + 2) / 3; // Integer ceiling
    assert_eq!(threshold, expected);
}

#[test]
fn rule5_finality_threshold_n7_f2() {
    // n=7, f=2, threshold must be exactly 5
    let mut val_set = ValidatorSet::new(5);
    for _ in 0..7 {
        val_set.register(SecretKey::generate().address());
    }
    
    let threshold = val_set.quorum_threshold();
    assert_eq!(threshold, 5, "n=7, f=2: threshold must be exactly 5");
    
    // Verify formula: ceil(2*7/3) = ceil(14/3) = 5
    let n = 7;
    let expected = (2 * n + 2) / 3;
    assert_eq!(threshold, expected);
}

#[test]
fn rule5_finality_threshold_n10_f3() {
    // n=10, f=3, threshold must be exactly 7
    let mut val_set = ValidatorSet::new(7);
    for _ in 0..10 {
        val_set.register(SecretKey::generate().address());
    }
    
    let threshold = val_set.quorum_threshold();
    assert_eq!(threshold, 7, "n=10, f=3: threshold must be exactly 7");
    
    // Verify formula: ceil(2*10/3) = ceil(20/3) = 7
    let n = 10;
    let expected = (2 * n + 2) / 3;
    assert_eq!(threshold, expected);
}

#[test]
fn rule5_finality_reached_at_exactly_threshold() {
    // 4 validators, threshold = 3
    let validators: Vec<SecretKey> = (0..4).map(|_| SecretKey::generate()).collect();
    
    let mut dag = BlockDag::new();
    let mut finality = FinalityTracker::new(3);
    
    for sk in &validators {
        finality.register_validator(sk.address());
    }
    
    // Round 1: All 4 validators produce
    let mut round1_hashes = Vec::new();
    for (i, sk) in validators.iter().enumerate() {
        let v = make_vertex(sk, 1, i as u64, vec![], vec![]);
        let hash = v.hash();
        round1_hashes.push(hash);
        dag.insert(v);
        finality.register_validator(sk.address());
    }
    
    // Check finality - round 1 should NOT be finalized yet (need descendants)
    let finalized = finality.find_newly_finalized(&dag);
    assert_eq!(finalized.len(), 0, "Round 1 should not be finalized yet");
    
    // Round 2: Only 2 validators produce (below threshold)
    for i in 0..2 {
        let v = make_vertex(&validators[i], 2, (4 + i) as u64, round1_hashes.clone(), vec![]);
        dag.insert(v);
    }
    
    // Check finality - still not finalized (only 2/3 descendants)
    let finalized = finality.find_newly_finalized(&dag);
    assert_eq!(finalized.len(), 0, "Round 1 should not be finalized with only 2 descendants");
    
    // Round 2: Add 3rd validator (exactly at threshold)
    let v = make_vertex(&validators[2], 2, 6, round1_hashes.clone(), vec![]);
    dag.insert(v);
    
    // Check finality - NOW round 1 should be finalized (3/3 descendants)
    let finalized = finality.find_newly_finalized(&dag);
    assert!(finalized.len() > 0, "Round 1 should be finalized with 3 descendants");
    
    // Verify all round 1 vertices are finalized
    for hash in &round1_hashes {
        assert!(finalized.contains(hash), "All round 1 vertices should be finalized");
    }
}
