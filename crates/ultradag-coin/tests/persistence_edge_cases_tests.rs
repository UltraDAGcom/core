use ultradag_coin::persistence::{save_dag, load_dag, dag_exists};
use ultradag_coin::{BlockDag, SecretKey, Signature, Address};
use ultradag_coin::block::block::Block;
use ultradag_coin::block::header::BlockHeader;
use ultradag_coin::tx::CoinbaseTx;
use ultradag_coin::consensus::DagVertex;
use std::path::PathBuf;

#[test]
fn test_dag_save_and_load() {
    let temp_dir = std::env::temp_dir();
    let path = temp_dir.join("test_dag.json");
    
    let mut dag = BlockDag::new();
    let sk = SecretKey::generate();
    
    let coinbase = CoinbaseTx {
        to: sk.address(),
        amount: 5_000_000_000,
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
    
    dag.insert(vertex);
    
    save_dag(&dag, &path).unwrap();
    
    let loaded = load_dag(&path).unwrap();
    assert_eq!(loaded.vertex_count(), dag.vertex_count());
    
    std::fs::remove_file(&path).ok();
}

#[test]
fn test_dag_exists() {
    let temp_dir = std::env::temp_dir();
    let path = temp_dir.join("test_exists_dag.json");
    
    assert!(!dag_exists(&path));
    
    let dag = BlockDag::new();
    save_dag(&dag, &path).unwrap();
    
    assert!(dag_exists(&path));
    
    std::fs::remove_file(&path).ok();
}

#[test]
fn test_dag_load_nonexistent() {
    let path = PathBuf::from("/nonexistent/path/dag.json");
    let result = load_dag(&path);
    assert!(result.is_err());
}

#[test]
fn test_dag_save_empty() {
    let temp_dir = std::env::temp_dir();
    let path = temp_dir.join("test_empty_dag.json");
    
    let dag = BlockDag::new();
    save_dag(&dag, &path).unwrap();
    
    let loaded = load_dag(&path).unwrap();
    assert_eq!(loaded.vertex_count(), 0);
    
    std::fs::remove_file(&path).ok();
}

#[test]
fn test_dag_save_multiple_vertices() {
    let temp_dir = std::env::temp_dir();
    let path = temp_dir.join("test_multi_dag.json");
    
    let mut dag = BlockDag::new();
    let sk = SecretKey::generate();
    
    for i in 0..5 {
        let coinbase = CoinbaseTx {
            to: sk.address(),
            amount: 5_000_000_000,
            height: i,
        };
        let header = BlockHeader {
            version: 1,
            height: i,
            timestamp: 1_000_000 + i as i64,
            prev_hash: [i as u8; 32],
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
            i,
            sk.address(),
            sk.verifying_key().to_bytes(),
            Signature([0u8; 64]),
        );
        vertex.signature = sk.sign(&vertex.signable_bytes());
        dag.insert(vertex);
    }
    
    save_dag(&dag, &path).unwrap();
    let loaded = load_dag(&path).unwrap();
    assert_eq!(loaded.vertex_count(), 5);
    
    std::fs::remove_file(&path).ok();
}

#[test]
fn test_checkpoint_persistence() {
    use ultradag_coin::consensus::persistence::{save_checkpoint, load_checkpoint, checkpoint_exists};
    use ultradag_coin::Checkpoint;
    
    let temp_dir = std::env::temp_dir();
    let path = temp_dir.join("test_checkpoint.json");
    
    let checkpoint = Checkpoint {
        round: 100,
        state_root: [1u8; 32],
        dag_tip: [2u8; 32],
        total_supply: 1_000_000,
        signatures: vec![],
    };
    
    save_checkpoint(&checkpoint, &path).unwrap();
    assert!(checkpoint_exists(&path));
    
    let loaded = load_checkpoint(&path).unwrap();
    assert_eq!(loaded.round, 100);
    assert_eq!(loaded.total_supply, 1_000_000);
    
    std::fs::remove_file(&path).ok();
}

#[test]
fn test_checkpoint_load_nonexistent() {
    use ultradag_coin::consensus::persistence::load_checkpoint;
    
    let path = PathBuf::from("/nonexistent/checkpoint.json");
    let result = load_checkpoint(&path);
    assert!(result.is_err());
}

#[test]
fn test_mempool_persistence() {
    use ultradag_coin::tx::persistence::{save_mempool, load_mempool};
    use ultradag_coin::{Mempool, Transaction, TransferTx};
    
    let temp_dir = std::env::temp_dir();
    let path = temp_dir.join("test_mempool.json");
    
    let mut mempool = Mempool::new();
    let sk = SecretKey::generate();
    
    let mut tx = TransferTx {
        from: sk.address(),
        to: Address([2u8; 32]),
        amount: 1000,
        fee: 100,
        nonce: 0,
        pub_key: sk.verifying_key().to_bytes(),
        signature: Signature([0u8; 64]),
    };
    tx.signature = sk.sign(&tx.signable_bytes());
    
    mempool.insert(Transaction::Transfer(tx));
    
    save_mempool(&mempool, &path).unwrap();
    let loaded = load_mempool(&path).unwrap();
    
    assert_eq!(loaded.pending_count(), mempool.pending_count());
    
    std::fs::remove_file(&path).ok();
}

#[test]
fn test_mempool_load_nonexistent() {
    use ultradag_coin::tx::persistence::load_mempool;
    
    let path = PathBuf::from("/nonexistent/mempool.json");
    let result = load_mempool(&path);
    assert!(result.is_err());
}

#[test]
fn test_persistence_roundtrip_consistency() {
    let temp_dir = std::env::temp_dir();
    let path = temp_dir.join("test_roundtrip.json");
    
    let mut dag = BlockDag::new();
    let sk = SecretKey::generate();
    
    let coinbase = CoinbaseTx {
        to: sk.address(),
        amount: 5_000_000_000,
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
    let hash = vertex.hash();
    
    dag.insert(vertex);
    
    save_dag(&dag, &path).unwrap();
    let loaded = load_dag(&path).unwrap();
    
    assert!(loaded.get(&hash).is_some());
    
    std::fs::remove_file(&path).ok();
}
