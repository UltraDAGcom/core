use ultradag_coin::block::block::Block;
use ultradag_coin::block::header::BlockHeader;
use ultradag_coin::tx::CoinbaseTx;
use ultradag_coin::{SecretKey, Transaction, TransferTx, Signature};

#[test]
fn test_block_hash_deterministic() {
    let sk = SecretKey::generate();
    
    let coinbase = CoinbaseTx {
        to: sk.address(),
        amount: 0,
        height: 0,
    };
    let header = BlockHeader {
        version: 1,
        height: 0,
        timestamp: 1_000_000,
        prev_hash: [0u8; 32],
        merkle_root: [0u8; 32],
    };
    let block = Block {
        header,
        coinbase,
        transactions: vec![],
    };
    
    let hash1 = block.hash();
    let hash2 = block.hash();
    
    assert_eq!(hash1, hash2);
}

#[test]
fn test_block_with_transactions() {
    let sk = SecretKey::generate();
    let to = SecretKey::generate();
    
    let mut tx = TransferTx {
        from: sk.address(),
        to: to.address(),
        amount: 1000,
        fee: 100,
        nonce: 0,
        pub_key: sk.verifying_key().to_bytes(),
        signature: Signature([0u8; 64]),
        memo: None,
    };
    tx.signature = sk.sign(&tx.signable_bytes());
    
    let coinbase = CoinbaseTx {
        to: sk.address(),
        amount: 0,
        height: 0,
    };
    let header = BlockHeader {
        version: 1,
        height: 0,
        timestamp: 1_000_000,
        prev_hash: [0u8; 32],
        merkle_root: [0u8; 32],
    };
    let block = Block {
        header,
        coinbase,
        transactions: vec![Transaction::Transfer(tx)],
    };
    
    assert_eq!(block.transactions.len(), 1);
}

#[test]
fn test_block_header_fields() {
    let header = BlockHeader {
        version: 1,
        height: 42,
        timestamp: 1_234_567,
        prev_hash: [1u8; 32],
        merkle_root: [2u8; 32],
    };
    
    assert_eq!(header.version, 1);
    assert_eq!(header.height, 42);
    assert_eq!(header.timestamp, 1_234_567);
    assert_eq!(header.prev_hash, [1u8; 32]);
    assert_eq!(header.merkle_root, [2u8; 32]);
}

#[test]
fn test_coinbase_tx() {
    let sk = SecretKey::generate();
    
    let coinbase = CoinbaseTx {
        to: sk.address(),
        amount: 0,
        height: 100,
    };
    
    assert_eq!(coinbase.to, sk.address());
    assert_eq!(coinbase.amount, 0);
    assert_eq!(coinbase.height, 100);
}

#[test]
fn test_block_different_heights() {
    let sk = SecretKey::generate();
    
    for height in 0..10 {
        let coinbase = CoinbaseTx {
            to: sk.address(),
            amount: 0,
            height,
        };
        let header = BlockHeader {
            version: 1,
            height,
            timestamp: 1_000_000 + height as i64,
            prev_hash: [0u8; 32],
            merkle_root: [0u8; 32],
        };
        let block = Block {
            header,
            coinbase,
            transactions: vec![],
        };
        
        assert_eq!(block.header.height, height);
        assert_eq!(block.coinbase.height, height);
    }
}

#[test]
fn test_block_hash_changes_with_content() {
    let sk = SecretKey::generate();
    
    let coinbase1 = CoinbaseTx {
        to: sk.address(),
        amount: 0,
        height: 0,
    };
    let header1 = BlockHeader {
        version: 1,
        height: 0,
        timestamp: 1_000_000,
        prev_hash: [0u8; 32],
        merkle_root: [0u8; 32],
    };
    let block1 = Block {
        header: header1,
        coinbase: coinbase1,
        transactions: vec![],
    };
    
    let coinbase2 = CoinbaseTx {
        to: sk.address(),
        amount: 0,
        height: 1,
    };
    let header2 = BlockHeader {
        version: 1,
        height: 1,
        timestamp: 1_000_001,
        prev_hash: [0u8; 32],
        merkle_root: [0u8; 32],
    };
    let block2 = Block {
        header: header2,
        coinbase: coinbase2,
        transactions: vec![],
    };
    
    assert_ne!(block1.hash(), block2.hash());
}

#[test]
fn test_block_with_multiple_transactions() {
    let sk1 = SecretKey::generate();
    let sk2 = SecretKey::generate();
    let to = SecretKey::generate();
    
    let mut tx1 = TransferTx {
        from: sk1.address(),
        to: to.address(),
        amount: 1000,
        fee: 100,
        nonce: 0,
        pub_key: sk1.verifying_key().to_bytes(),
        signature: Signature([0u8; 64]),
        memo: None,
    };
    tx1.signature = sk1.sign(&tx1.signable_bytes());
    
    let mut tx2 = TransferTx {
        from: sk2.address(),
        to: to.address(),
        amount: 2000,
        fee: 200,
        nonce: 0,
        pub_key: sk2.verifying_key().to_bytes(),
        signature: Signature([0u8; 64]),
        memo: None,
    };
    tx2.signature = sk2.sign(&tx2.signable_bytes());
    
    let coinbase = CoinbaseTx {
        to: sk1.address(),
        amount: 0,
        height: 0,
    };
    let header = BlockHeader {
        version: 1,
        height: 0,
        timestamp: 1_000_000,
        prev_hash: [0u8; 32],
        merkle_root: [0u8; 32],
    };
    let block = Block {
        header,
        coinbase,
        transactions: vec![
            Transaction::Transfer(tx1),
            Transaction::Transfer(tx2),
        ],
    };
    
    assert_eq!(block.transactions.len(), 2);
}

#[test]
fn test_genesis_block() {
    use ultradag_coin::block::genesis::genesis_block;

    let block = genesis_block();

    assert_eq!(block.header.height, 0);
    assert_eq!(block.header.version, 1);
    assert_eq!(block.transactions.len(), 0);
}
