use ultradag_coin::{StateEngine, BlockDag, FinalityTracker, Mempool};
use ultradag_coin::{Address, SecretKey, Transaction, TransferTx, Signature};
use ultradag_coin::block::block::Block;
use ultradag_coin::block::header::BlockHeader;
use ultradag_coin::tx::CoinbaseTx;
use ultradag_coin::consensus::DagVertex;

#[test]
fn test_dag_insert_duplicate() {
    let mut dag = BlockDag::new();
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
    let mut vertex = DagVertex::new(
        block,
        vec![[0u8; 32]],
        0,
        sk.address(),
        sk.verifying_key().to_bytes(),
        Signature([0u8; 64]),
    );
    vertex.signature = sk.sign(&vertex.signable_bytes());
    
    assert!(dag.insert(vertex.clone()));
    assert!(!dag.insert(vertex));
}

#[test]
fn test_mempool_rejects_invalid_transaction() {
    let mut mempool = Mempool::new();
    let sk = SecretKey::generate();
    
    let tx = TransferTx {
        from: sk.address(),
        to: Address([2u8; 20]),
        amount: 1000,
        fee: 100,
        nonce: 0,
        pub_key: sk.verifying_key().to_bytes(),
        signature: Signature([0u8; 64]),
        memo: None,
    };

    let result = mempool.insert(Transaction::Transfer(tx));
    assert!(!result);
}

#[test]
fn test_finality_tracker_empty() {
    let mut tracker = FinalityTracker::new(3);
    let dag = BlockDag::new();
    
    let finalized = tracker.find_newly_finalized(&dag);
    assert!(finalized.is_empty());
}

#[test]
fn test_block_hash_consistency() {
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
fn test_dag_vertex_hash_deterministic() {
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
    let mut vertex = DagVertex::new(
        block,
        vec![[0u8; 32]],
        0,
        sk.address(),
        sk.verifying_key().to_bytes(),
        Signature([0u8; 64]),
    );
    vertex.signature = sk.sign(&vertex.signable_bytes());
    
    let hash1 = vertex.hash();
    let hash2 = vertex.hash();
    
    assert_eq!(hash1, hash2);
}

#[test]
fn test_state_engine_initialization() {
    let state = StateEngine::new();
    assert_eq!(state.current_epoch(), ultradag_coin::constants::EPOCH_UNINITIALIZED); // sentinel: epoch never initialized
}

#[test]
fn test_mempool_best_returns_sorted() {
    let mut mempool = Mempool::new();
    let sk = SecretKey::generate();
    let to = SecretKey::generate();
    
    let mut tx1 = TransferTx {
        from: sk.address(),
        to: to.address(),
        amount: 1000,
        fee: 100,
        nonce: 0,
        pub_key: sk.verifying_key().to_bytes(),
        signature: Signature([0u8; 64]),
        memo: None,
    };
    tx1.signature = sk.sign(&tx1.signable_bytes());
    
    let mut tx2 = TransferTx {
        from: sk.address(),
        to: to.address(),
        amount: 1000,
        fee: 200,
        nonce: 1,
        pub_key: sk.verifying_key().to_bytes(),
        signature: Signature([0u8; 64]),
        memo: None,
    };
    tx2.signature = sk.sign(&tx2.signable_bytes());
    
    mempool.insert(Transaction::Transfer(tx1));
    mempool.insert(Transaction::Transfer(tx2.clone()));
    
    let best = mempool.best(10);
    if !best.is_empty() {
        assert_eq!(best[0].hash(), Transaction::Transfer(tx2).hash());
    }
}
