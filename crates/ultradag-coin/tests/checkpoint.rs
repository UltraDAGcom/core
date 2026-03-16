use ultradag_coin::address::SecretKey;
use ultradag_coin::consensus::{Checkpoint, compute_state_root};
use ultradag_coin::state::persistence::StateSnapshot;

#[test]
fn test_01_checkpoint_signable_bytes_are_deterministic() {
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
        state_root: [1u8; 32],
        dag_tip: [2u8; 32],
        total_supply: 1_000_000_000,
        prev_checkpoint_hash: [0u8; 32],
            signatures: vec![],
    };

    assert_eq!(checkpoint1.signable_bytes(), checkpoint2.signable_bytes());
    assert_eq!(checkpoint1.checkpoint_hash(), checkpoint2.checkpoint_hash());
}

#[test]
fn test_02_checkpoint_signature_valid() {
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

    let valid_signers = checkpoint.valid_signers();
    assert_eq!(valid_signers.len(), 1);
    assert_eq!(valid_signers[0], sk.address());
}

#[test]
fn test_03_checkpoint_signature_wrong_key_rejected() {
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

    // Tamper with signature bytes
    checkpoint.signatures[0].signature.0[0] ^= 0xFF;

    let valid_signers = checkpoint.valid_signers();
    assert_eq!(valid_signers.len(), 0, "Tampered signature should be rejected");
}

#[test]
fn test_04_checkpoint_accepted_at_quorum() {
    let validators: Vec<SecretKey> = (0..4).map(|_| SecretKey::generate()).collect();
    let active_validators: Vec<_> = validators.iter().map(|sk| sk.address()).collect();
    let quorum = 3; // ceil(2*4/3) = 3

    // Sign with 3 validators
    let mut checkpoint = Checkpoint {
        round: 1000,
        state_root: [1u8; 32],
        dag_tip: [2u8; 32],
        total_supply: 1_000_000_000,
        prev_checkpoint_hash: [0u8; 32],
            signatures: vec![],
    };

    for sk in &validators[0..3] {
        checkpoint.sign(sk);
    }

    assert!(checkpoint.is_accepted(&active_validators, quorum), "3 signatures should meet quorum of 3");

    // Sign with only 2 validators
    let mut checkpoint2 = Checkpoint {
        round: 1000,
        state_root: [1u8; 32],
        dag_tip: [2u8; 32],
        total_supply: 1_000_000_000,
        prev_checkpoint_hash: [0u8; 32],
            signatures: vec![],
    };

    for sk in &validators[0..2] {
        checkpoint2.sign(sk);
    }

    assert!(!checkpoint2.is_accepted(&active_validators, quorum), "2 signatures should not meet quorum of 3");
}

#[test]
fn test_05_checkpoint_accepted_requires_active_validators() {
    let active_validators: Vec<SecretKey> = (0..4).map(|_| SecretKey::generate()).collect();
    let non_active = SecretKey::generate();
    let active_addrs: Vec<_> = active_validators.iter().map(|sk| sk.address()).collect();
    let quorum = 3;

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

    assert!(!checkpoint.is_accepted(&active_addrs, quorum), "Only 2 active signatures, should not meet quorum of 3");

    // Add one more active signature
    checkpoint.sign(&active_validators[2]);

    assert!(checkpoint.is_accepted(&active_addrs, quorum), "3 active signatures should meet quorum of 3");
}

#[test]
fn test_06_tampered_checkpoint_rejected() {
    let sk = SecretKey::generate();
    let active_validators = vec![sk.address()];
    let quorum = 1;

    let mut checkpoint = Checkpoint {
        round: 1000,
        state_root: [1u8; 32],
        dag_tip: [2u8; 32],
        total_supply: 1_000_000_000,
        prev_checkpoint_hash: [0u8; 32],
            signatures: vec![],
    };

    checkpoint.sign(&sk);
    assert!(checkpoint.is_accepted(&active_validators, quorum), "Valid checkpoint should be accepted");

    // Tamper with state_root
    checkpoint.state_root[0] ^= 0xFF;

    assert!(!checkpoint.is_accepted(&active_validators, quorum), "Tampered checkpoint should be rejected");
}

#[test]
fn test_07_state_root_is_deterministic() {
    let snapshot1 = StateSnapshot {
        accounts: vec![],
        stake_accounts: vec![],
        active_validator_set: vec![],
        current_epoch: 0,
        total_supply: 1_000_000,
        last_finalized_round: Some(100),
        proposals: vec![],
        votes: vec![],
        next_proposal_id: 0,
        governance_params: Default::default(),
        council_members: vec![],
        treasury_balance: 0,
        delegation_accounts: vec![],
        configured_validator_count: None,
    };

    let snapshot2 = StateSnapshot {
        accounts: vec![],
        stake_accounts: vec![],
        active_validator_set: vec![],
        current_epoch: 0,
        total_supply: 1_000_000,
        last_finalized_round: Some(100),
        proposals: vec![],
        votes: vec![],
        next_proposal_id: 0,
        governance_params: Default::default(),
        council_members: vec![],
        treasury_balance: 0,
        delegation_accounts: vec![],
        configured_validator_count: None,
    };

    let root1 = compute_state_root(&snapshot1);
    let root2 = compute_state_root(&snapshot2);

    assert_eq!(root1, root2, "Identical snapshots should produce identical state roots");
}
