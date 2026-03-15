use ultradag_coin::{Checkpoint, SecretKey};
use ultradag_coin::consensus::checkpoint::compute_state_root;
use ultradag_coin::state::persistence::StateSnapshot;

#[test]
fn test_checkpoint_signable_bytes() {
    let checkpoint = Checkpoint {
        round: 100,
        state_root: [1u8; 32],
        dag_tip: [2u8; 32],
        total_supply: 1_000_000,
        prev_checkpoint_hash: [0u8; 32],
            signatures: vec![],
    };
    
    let bytes = checkpoint.signable_bytes();
    assert!(!bytes.is_empty());
    assert!(bytes.starts_with(b"ultradag-testnet"));
}

#[test]
fn test_checkpoint_hash() {
    let checkpoint = Checkpoint {
        round: 100,
        state_root: [1u8; 32],
        dag_tip: [2u8; 32],
        total_supply: 1_000_000,
        prev_checkpoint_hash: [0u8; 32],
            signatures: vec![],
    };
    
    let hash1 = checkpoint.checkpoint_hash();
    let hash2 = checkpoint.checkpoint_hash();
    assert_eq!(hash1, hash2);
}

#[test]
fn test_checkpoint_sign() {
    let sk = SecretKey::generate();
    let mut checkpoint = Checkpoint {
        round: 100,
        state_root: [1u8; 32],
        dag_tip: [2u8; 32],
        total_supply: 1_000_000,
        prev_checkpoint_hash: [0u8; 32],
            signatures: vec![],
    };
    
    checkpoint.sign(&sk);
    assert_eq!(checkpoint.signatures.len(), 1);
    assert_eq!(checkpoint.signatures[0].validator, sk.address());
}

#[test]
fn test_checkpoint_valid_signers() {
    let sk1 = SecretKey::generate();
    let sk2 = SecretKey::generate();
    
    let mut checkpoint = Checkpoint {
        round: 100,
        state_root: [1u8; 32],
        dag_tip: [2u8; 32],
        total_supply: 1_000_000,
        prev_checkpoint_hash: [0u8; 32],
            signatures: vec![],
    };
    
    checkpoint.sign(&sk1);
    checkpoint.sign(&sk2);
    
    let valid = checkpoint.valid_signers();
    assert_eq!(valid.len(), 2);
    assert!(valid.contains(&sk1.address()));
    assert!(valid.contains(&sk2.address()));
}

#[test]
fn test_checkpoint_invalid_signature() {
    let sk = SecretKey::generate();
    let mut checkpoint = Checkpoint {
        round: 100,
        state_root: [1u8; 32],
        dag_tip: [2u8; 32],
        total_supply: 1_000_000,
        prev_checkpoint_hash: [0u8; 32],
            signatures: vec![],
    };
    
    checkpoint.sign(&sk);
    
    checkpoint.round = 101;
    
    let valid = checkpoint.valid_signers();
    assert_eq!(valid.len(), 0);
}

#[test]
fn test_checkpoint_is_accepted_with_quorum() {
    let sk1 = SecretKey::generate();
    let sk2 = SecretKey::generate();
    let sk3 = SecretKey::generate();
    
    let mut checkpoint = Checkpoint {
        round: 100,
        state_root: [1u8; 32],
        dag_tip: [2u8; 32],
        total_supply: 1_000_000,
        prev_checkpoint_hash: [0u8; 32],
            signatures: vec![],
    };
    
    checkpoint.sign(&sk1);
    checkpoint.sign(&sk2);
    checkpoint.sign(&sk3);
    
    let active = vec![sk1.address(), sk2.address(), sk3.address()];
    
    assert!(checkpoint.is_accepted(&active, 2));
    assert!(checkpoint.is_accepted(&active, 3));
    assert!(!checkpoint.is_accepted(&active, 4));
}

#[test]
fn test_checkpoint_is_accepted_without_quorum() {
    let sk1 = SecretKey::generate();
    let sk2 = SecretKey::generate();
    
    let mut checkpoint = Checkpoint {
        round: 100,
        state_root: [1u8; 32],
        dag_tip: [2u8; 32],
        total_supply: 1_000_000,
        prev_checkpoint_hash: [0u8; 32],
            signatures: vec![],
    };
    
    checkpoint.sign(&sk1);
    
    let active = vec![sk1.address(), sk2.address()];
    
    assert!(checkpoint.is_accepted(&active, 1));
    assert!(!checkpoint.is_accepted(&active, 2));
}

#[test]
fn test_checkpoint_non_validator_signature_ignored() {
    let sk1 = SecretKey::generate();
    let sk2 = SecretKey::generate();
    let sk_outsider = SecretKey::generate();
    
    let mut checkpoint = Checkpoint {
        round: 100,
        state_root: [1u8; 32],
        dag_tip: [2u8; 32],
        total_supply: 1_000_000,
        prev_checkpoint_hash: [0u8; 32],
            signatures: vec![],
    };
    
    checkpoint.sign(&sk1);
    checkpoint.sign(&sk_outsider);
    
    let active = vec![sk1.address(), sk2.address()];
    
    assert!(!checkpoint.is_accepted(&active, 2));
}

#[test]
fn test_compute_state_root_deterministic() {
    let snapshot = StateSnapshot {
        accounts: vec![],
        stake_accounts: vec![],
        active_validator_set: vec![],
        current_epoch: 0,
        total_supply: 0,
        last_finalized_round: Some(0),
        proposals: vec![],
        votes: vec![],
        next_proposal_id: 0,
        governance_params: Default::default(),
        council_members: vec![],
        treasury_balance: 0,
        delegation_accounts: vec![],
    };

    let root1 = compute_state_root(&snapshot);
    let root2 = compute_state_root(&snapshot);
    
    assert_eq!(root1, root2);
}

#[test]
fn test_compute_state_root_different_for_different_state() {
    let snapshot1 = StateSnapshot {
        accounts: vec![],
        stake_accounts: vec![],
        active_validator_set: vec![],
        current_epoch: 0,
        total_supply: 0,
        last_finalized_round: Some(0),
        proposals: vec![],
        votes: vec![],
        next_proposal_id: 0,
        governance_params: Default::default(),
        council_members: vec![],
        treasury_balance: 0,
        delegation_accounts: vec![],
    };

    let snapshot2 = StateSnapshot {
        accounts: vec![],
        stake_accounts: vec![],
        active_validator_set: vec![],
        current_epoch: 1,
        total_supply: 0,
        last_finalized_round: Some(0),
        proposals: vec![],
        votes: vec![],
        next_proposal_id: 0,
        governance_params: Default::default(),
        council_members: vec![],
        treasury_balance: 0,
        delegation_accounts: vec![],
    };

    let root1 = compute_state_root(&snapshot1);
    let root2 = compute_state_root(&snapshot2);
    
    assert_ne!(root1, root2);
}

#[test]
fn test_checkpoint_multiple_signatures_from_same_validator() {
    let sk = SecretKey::generate();
    
    let mut checkpoint = Checkpoint {
        round: 100,
        state_root: [1u8; 32],
        dag_tip: [2u8; 32],
        total_supply: 1_000_000,
        prev_checkpoint_hash: [0u8; 32],
            signatures: vec![],
    };
    
    checkpoint.sign(&sk);
    checkpoint.sign(&sk);
    
    assert_eq!(checkpoint.signatures.len(), 2);
    
    let active = vec![sk.address()];
    assert!(checkpoint.is_accepted(&active, 1));
}
