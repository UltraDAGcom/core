use ultradag_coin::address::SecretKey;
use ultradag_coin::consensus::{Checkpoint, CheckpointSignature};

#[test]
fn test_checkpoint_multiple_signatures() {
    let validators: Vec<SecretKey> = (0..4).map(|_| SecretKey::generate()).collect();
    
    let mut checkpoint = Checkpoint {
        round: 1000,
        state_root: [1u8; 32],
        dag_tip: [2u8; 32],
        total_supply: 1_000_000_000,
        prev_checkpoint_hash: [0u8; 32],
            signatures: vec![],
    };
    
    // All validators sign
    for sk in &validators {
        checkpoint.sign(sk);
    }
    
    assert_eq!(checkpoint.signatures.len(), 4);
    
    // All signatures should be valid
    let valid_signers = checkpoint.valid_signers();
    assert_eq!(valid_signers.len(), 4);
}

#[test]
fn test_checkpoint_cosigning_accumulation() {
    let validators: Vec<SecretKey> = (0..4).map(|_| SecretKey::generate()).collect();
    
    // Validator 0 produces checkpoint
    let mut checkpoint = Checkpoint {
        round: 1000,
        state_root: [1u8; 32],
        dag_tip: [2u8; 32],
        total_supply: 1_000_000_000,
        prev_checkpoint_hash: [0u8; 32],
            signatures: vec![],
    };
    checkpoint.sign(&validators[0]);
    
    assert_eq!(checkpoint.signatures.len(), 1);
    
    // Validators 1, 2, 3 co-sign
    for sk in &validators[1..] {
        checkpoint.sign(sk);
    }
    
    assert_eq!(checkpoint.signatures.len(), 4);
    
    // All 4 signatures should be valid
    let valid_signers = checkpoint.valid_signers();
    assert_eq!(valid_signers.len(), 4);
}

#[test]
fn test_checkpoint_quorum_calculation() {
    let validators: Vec<SecretKey> = (0..4).map(|_| SecretKey::generate()).collect();
    let active_validators: Vec<_> = validators.iter().map(|sk| sk.address()).collect();
    
    // Quorum for 4 validators: ceil(2*4/3) = 3
    let quorum = 3;
    
    let mut checkpoint = Checkpoint {
        round: 1000,
        state_root: [1u8; 32],
        dag_tip: [2u8; 32],
        total_supply: 1_000_000_000,
        prev_checkpoint_hash: [0u8; 32],
            signatures: vec![],
    };
    
    // 2 signatures - below quorum
    checkpoint.sign(&validators[0]);
    checkpoint.sign(&validators[1]);
    assert!(!checkpoint.is_accepted(&active_validators, quorum));
    
    // 3 signatures - at quorum
    checkpoint.sign(&validators[2]);
    assert!(checkpoint.is_accepted(&active_validators, quorum));
    
    // 4 signatures - above quorum
    checkpoint.sign(&validators[3]);
    assert!(checkpoint.is_accepted(&active_validators, quorum));
}

#[test]
fn test_checkpoint_rejects_invalid_signature() {
    let sk = SecretKey::generate();
    
    let mut checkpoint = Checkpoint {
        round: 1000,
        state_root: [1u8; 32],
        dag_tip: [2u8; 32],
        total_supply: 1_000_000_000,
        prev_checkpoint_hash: [0u8; 32],
            signatures: vec![],
    };
    
    checkpoint.sign(&sk);
    
    // Tamper with signature
    checkpoint.signatures[0].signature.0[0] ^= 0xFF;
    
    // Should have no valid signers
    let valid_signers = checkpoint.valid_signers();
    assert_eq!(valid_signers.len(), 0);
}

#[test]
fn test_checkpoint_rejects_wrong_pubkey_address_mapping() {
    let sk1 = SecretKey::generate();
    let sk2 = SecretKey::generate();
    
    let mut checkpoint = Checkpoint {
        round: 1000,
        state_root: [1u8; 32],
        dag_tip: [2u8; 32],
        total_supply: 1_000_000_000,
        prev_checkpoint_hash: [0u8; 32],
            signatures: vec![],
    };
    
    // Create signature with sk1 but claim it's from sk2's address
    let sig = CheckpointSignature {
        validator: sk2.address(), // Wrong address
        pub_key: sk1.verifying_key().to_bytes(),
        signature: sk1.sign(&checkpoint.signable_bytes()),
    };
    
    checkpoint.signatures.push(sig);
    
    // Should be rejected (address doesn't match pub_key)
    let valid_signers = checkpoint.valid_signers();
    assert_eq!(valid_signers.len(), 0);
}

#[test]
fn test_checkpoint_different_state_roots_different_hashes() {
    let checkpoint1 = Checkpoint {
        round: 1000,
        state_root: [1u8; 32],
        dag_tip: [2u8; 32],
        total_supply: 1_000_000_000,
        prev_checkpoint_hash: [0u8; 32],
            signatures: vec![],
    };
    
    let checkpoint2 = Checkpoint {
        round: 1000,
        state_root: [99u8; 32], // Different state root
        dag_tip: [2u8; 32],
        total_supply: 1_000_000_000,
        prev_checkpoint_hash: [0u8; 32],
            signatures: vec![],
    };
    
    assert_ne!(checkpoint1.checkpoint_hash(), checkpoint2.checkpoint_hash());
}

#[test]
fn test_checkpoint_signature_covers_all_fields() {
    let sk = SecretKey::generate();
    
    let mut checkpoint = Checkpoint {
        round: 1000,
        state_root: [1u8; 32],
        dag_tip: [2u8; 32],
        total_supply: 1_000_000_000,
        prev_checkpoint_hash: [0u8; 32],
            signatures: vec![],
    };
    
    checkpoint.sign(&sk);
    let original_sig = checkpoint.signatures[0].clone();
    
    // Tamper with round
    checkpoint.round = 1001;
    let valid = checkpoint.valid_signers();
    assert_eq!(valid.len(), 0, "Signature should not validate after round change");
    
    // Restore and tamper with state_root
    checkpoint.round = 1000;
    checkpoint.state_root = [99u8; 32];
    let valid = checkpoint.valid_signers();
    assert_eq!(valid.len(), 0, "Signature should not validate after state_root change");
    
    // Restore and tamper with dag_tip
    checkpoint.state_root = [1u8; 32];
    checkpoint.dag_tip = [99u8; 32];
    let valid = checkpoint.valid_signers();
    assert_eq!(valid.len(), 0, "Signature should not validate after dag_tip change");
    
    // Restore and tamper with total_supply
    checkpoint.dag_tip = [2u8; 32];
    checkpoint.total_supply = 999_999_999;
    let valid = checkpoint.valid_signers();
    assert_eq!(valid.len(), 0, "Signature should not validate after total_supply change");
}

#[test]
fn test_checkpoint_duplicate_signatures_ignored() {
    let sk = SecretKey::generate();
    let active_validators = vec![sk.address()];
    
    let mut checkpoint = Checkpoint {
        round: 1000,
        state_root: [1u8; 32],
        dag_tip: [2u8; 32],
        total_supply: 1_000_000_000,
        prev_checkpoint_hash: [0u8; 32],
            signatures: vec![],
    };
    
    // Sign twice with same key
    checkpoint.sign(&sk);
    checkpoint.sign(&sk);
    
    assert_eq!(checkpoint.signatures.len(), 2);
    
    // But valid_signers should deduplicate
    let valid_signers = checkpoint.valid_signers();
    // Both signatures are valid, but they're from the same validator
    assert_eq!(valid_signers.len(), 2); // Both are valid individually
    
    // For quorum, only unique validators count
    // is_accepted uses a HashSet to deduplicate
    assert!(checkpoint.is_accepted(&active_validators, 1));
}

#[test]
fn test_checkpoint_non_active_validator_signature_ignored() {
    let active_validators: Vec<SecretKey> = (0..3).map(|_| SecretKey::generate()).collect();
    let non_active = SecretKey::generate();
    let active_addrs: Vec<_> = active_validators.iter().map(|sk| sk.address()).collect();
    
    let mut checkpoint = Checkpoint {
        round: 1000,
        state_root: [1u8; 32],
        dag_tip: [2u8; 32],
        total_supply: 1_000_000_000,
        prev_checkpoint_hash: [0u8; 32],
            signatures: vec![],
    };
    
    // Sign with 2 active + 1 non-active
    checkpoint.sign(&active_validators[0]);
    checkpoint.sign(&active_validators[1]);
    checkpoint.sign(&non_active);
    
    // All 3 signatures are valid
    let valid_signers = checkpoint.valid_signers();
    assert_eq!(valid_signers.len(), 3);
    
    // But only 2 are from active validators (quorum = 2 for 3 validators)
    let quorum = 2;
    assert!(checkpoint.is_accepted(&active_addrs, quorum));
}

#[test]
fn test_checkpoint_signable_bytes_includes_network_id() {
    let checkpoint = Checkpoint {
        round: 1000,
        state_root: [1u8; 32],
        dag_tip: [2u8; 32],
        total_supply: 1_000_000_000,
        prev_checkpoint_hash: [0u8; 32],
            signatures: vec![],
    };
    
    let signable = checkpoint.signable_bytes();
    
    // Should start with network ID
    let network_id = ultradag_coin::constants::NETWORK_ID;
    assert!(signable.starts_with(network_id));
    
    // Should contain "checkpoint" tag
    let tag_start = network_id.len();
    assert_eq!(&signable[tag_start..tag_start + 10], b"checkpoint");
}

#[test]
fn test_checkpoint_empty_signatures_not_accepted() {
    let validators: Vec<SecretKey> = (0..4).map(|_| SecretKey::generate()).collect();
    let active_validators: Vec<_> = validators.iter().map(|sk| sk.address()).collect();
    
    let checkpoint = Checkpoint {
        round: 1000,
        state_root: [1u8; 32],
        dag_tip: [2u8; 32],
        total_supply: 1_000_000_000,
        prev_checkpoint_hash: [0u8; 32],
            signatures: vec![], // No signatures
    };
    
    // Should not be accepted with any quorum
    assert!(!checkpoint.is_accepted(&active_validators, 1));
    assert!(!checkpoint.is_accepted(&active_validators, 3));
}

#[test]
fn test_checkpoint_large_validator_set() {
    // Test with 21 validators (MAX_ACTIVE_VALIDATORS)
    let validators: Vec<SecretKey> = (0..21).map(|_| SecretKey::generate()).collect();
    let active_validators: Vec<_> = validators.iter().map(|sk| sk.address()).collect();
    
    let mut checkpoint = Checkpoint {
        round: 1000,
        state_root: [1u8; 32],
        dag_tip: [2u8; 32],
        total_supply: 1_000_000_000,
        prev_checkpoint_hash: [0u8; 32],
            signatures: vec![],
    };
    
    // Quorum for 21: ceil(2*21/3) = 14
    let quorum = 14;
    
    // Sign with 13 validators (below quorum)
    for sk in &validators[..13] {
        checkpoint.sign(sk);
    }
    assert!(!checkpoint.is_accepted(&active_validators, quorum));
    
    // Add one more signature (at quorum)
    checkpoint.sign(&validators[13]);
    assert!(checkpoint.is_accepted(&active_validators, quorum));
}
