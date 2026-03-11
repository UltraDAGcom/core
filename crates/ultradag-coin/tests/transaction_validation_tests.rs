use ultradag_coin::{Transaction, TransferTx, SecretKey, Address, Signature};
use ultradag_coin::tx::{StakeTx, UnstakeTx};

#[test]
fn test_transfer_tx_signable_bytes() {
    let sk = SecretKey::generate();
    let tx = TransferTx {
        from: sk.address(),
        to: Address([2u8; 32]),
        amount: 1000,
        fee: 100,
        nonce: 0,
        pub_key: sk.verifying_key().to_bytes(),
        signature: Signature([0u8; 64]),
        memo: None,
    };
    
    let bytes = tx.signable_bytes();
    assert!(bytes.len() > 0);
    assert!(bytes.starts_with(b"ultradag-testnet"));
}

#[test]
fn test_transfer_tx_hash() {
    let sk = SecretKey::generate();
    let mut tx = TransferTx {
        from: sk.address(),
        to: Address([2u8; 32]),
        amount: 1000,
        fee: 100,
        nonce: 0,
        pub_key: sk.verifying_key().to_bytes(),
        signature: Signature([0u8; 64]),
        memo: None,
    };
    tx.signature = sk.sign(&tx.signable_bytes());
    
    let hash1 = tx.hash();
    let hash2 = tx.hash();
    assert_eq!(hash1, hash2);
}

#[test]
fn test_transfer_tx_verify_valid() {
    let sk = SecretKey::generate();
    let mut tx = TransferTx {
        from: sk.address(),
        to: Address([2u8; 32]),
        amount: 1000,
        fee: 100,
        nonce: 0,
        pub_key: sk.verifying_key().to_bytes(),
        signature: Signature([0u8; 64]),
        memo: None,
    };
    tx.signature = sk.sign(&tx.signable_bytes());
    
    assert!(tx.verify_signature());
}

#[test]
fn test_transfer_tx_verify_invalid() {
    let sk = SecretKey::generate();
    let tx = TransferTx {
        from: sk.address(),
        to: Address([2u8; 32]),
        amount: 1000,
        fee: 100,
        nonce: 0,
        pub_key: sk.verifying_key().to_bytes(),
        signature: Signature([0u8; 64]),
        memo: None,
    };
    
    assert!(!tx.verify_signature());
}

#[test]
fn test_transfer_tx_total_cost() {
    let sk = SecretKey::generate();
    let tx = TransferTx {
        from: sk.address(),
        to: Address([2u8; 32]),
        amount: 1000,
        fee: 100,
        nonce: 0,
        pub_key: sk.verifying_key().to_bytes(),
        signature: Signature([0u8; 64]),
        memo: None,
    };
    
    assert_eq!(tx.total_cost(), 1100);
}

#[test]
fn test_stake_tx_signable_bytes() {
    let sk = SecretKey::generate();
    let tx = StakeTx {
        from: sk.address(),
        amount: 10_000_000,
        nonce: 0,
        pub_key: sk.verifying_key().to_bytes(),
        signature: Signature([0u8; 64]),
    };
    
    let bytes = tx.signable_bytes();
    assert!(bytes.len() > 0);
}

#[test]
fn test_stake_tx_hash() {
    let sk = SecretKey::generate();
    let mut tx = StakeTx {
        from: sk.address(),
        amount: 10_000_000,
        nonce: 0,
        pub_key: sk.verifying_key().to_bytes(),
        signature: Signature([0u8; 64]),
    };
    tx.signature = sk.sign(&tx.signable_bytes());
    
    let hash1 = tx.hash();
    let hash2 = tx.hash();
    assert_eq!(hash1, hash2);
}

#[test]
fn test_stake_tx_verify() {
    let sk = SecretKey::generate();
    let mut tx = StakeTx {
        from: sk.address(),
        amount: 10_000_000,
        nonce: 0,
        pub_key: sk.verifying_key().to_bytes(),
        signature: Signature([0u8; 64]),
    };
    tx.signature = sk.sign(&tx.signable_bytes());
    
    assert!(tx.verify_signature());
}

#[test]
fn test_unstake_tx_signable_bytes() {
    let sk = SecretKey::generate();
    let tx = UnstakeTx {
        from: sk.address(),
        nonce: 0,
        pub_key: sk.verifying_key().to_bytes(),
        signature: Signature([0u8; 64]),
    };
    
    let bytes = tx.signable_bytes();
    assert!(bytes.len() > 0);
}

#[test]
fn test_unstake_tx_hash() {
    let sk = SecretKey::generate();
    let mut tx = UnstakeTx {
        from: sk.address(),
        nonce: 0,
        pub_key: sk.verifying_key().to_bytes(),
        signature: Signature([0u8; 64]),
    };
    tx.signature = sk.sign(&tx.signable_bytes());
    
    let hash1 = tx.hash();
    let hash2 = tx.hash();
    assert_eq!(hash1, hash2);
}

#[test]
fn test_unstake_tx_verify() {
    let sk = SecretKey::generate();
    let mut tx = UnstakeTx {
        from: sk.address(),
        nonce: 0,
        pub_key: sk.verifying_key().to_bytes(),
        signature: Signature([0u8; 64]),
    };
    tx.signature = sk.sign(&tx.signable_bytes());
    
    assert!(tx.verify_signature());
}

#[test]
fn test_transaction_enum_hash() {
    let sk = SecretKey::generate();
    let mut tx = TransferTx {
        from: sk.address(),
        to: Address([2u8; 32]),
        amount: 1000,
        fee: 100,
        nonce: 0,
        pub_key: sk.verifying_key().to_bytes(),
        signature: Signature([0u8; 64]),
        memo: None,
    };
    tx.signature = sk.sign(&tx.signable_bytes());

    let transaction = Transaction::Transfer(tx);
    let hash1 = transaction.hash();
    let hash2 = transaction.hash();
    assert_eq!(hash1, hash2);
}

#[test]
fn test_stake_transaction_enum() {
    let sk = SecretKey::generate();
    let mut tx = StakeTx {
        from: sk.address(),
        amount: 10_000_000,
        nonce: 0,
        pub_key: sk.verifying_key().to_bytes(),
        signature: Signature([0u8; 64]),
    };
    tx.signature = sk.sign(&tx.signable_bytes());
    
    let transaction = Transaction::Stake(tx);
    let hash = transaction.hash();
    assert_ne!(hash, [0u8; 32]);
}

#[test]
fn test_unstake_transaction_enum() {
    let sk = SecretKey::generate();
    let mut tx = UnstakeTx {
        from: sk.address(),
        nonce: 0,
        pub_key: sk.verifying_key().to_bytes(),
        signature: Signature([0u8; 64]),
    };
    tx.signature = sk.sign(&tx.signable_bytes());
    
    let transaction = Transaction::Unstake(tx);
    let hash = transaction.hash();
    assert_ne!(hash, [0u8; 32]);
}

#[test]
fn test_transaction_different_types() {
    let sk = SecretKey::generate();
    
    let mut transfer = TransferTx {
        from: sk.address(),
        to: Address([2u8; 32]),
        amount: 1000,
        fee: 100,
        nonce: 0,
        pub_key: sk.verifying_key().to_bytes(),
        signature: Signature([0u8; 64]),
        memo: None,
    };
    transfer.signature = sk.sign(&transfer.signable_bytes());

    let mut stake = StakeTx {
        from: sk.address(),
        amount: 10_000_000,
        nonce: 0,
        pub_key: sk.verifying_key().to_bytes(),
        signature: Signature([0u8; 64]),
    };
    stake.signature = sk.sign(&stake.signable_bytes());
    
    let tx1 = Transaction::Transfer(transfer);
    let tx2 = Transaction::Stake(stake);
    
    assert_ne!(tx1.hash(), tx2.hash());
}
