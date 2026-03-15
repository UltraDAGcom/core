use ultradag_coin::{BlockDag, SecretKey, Signature};
use ultradag_coin::block::block::Block;
use ultradag_coin::block::header::BlockHeader;
use ultradag_coin::tx::CoinbaseTx;
use ultradag_coin::consensus::DagVertex;
use ultradag_coin::persistence;
use std::path::PathBuf;

fn make_test_vertex(sk: &SecretKey, round: u64, nonce: u64) -> DagVertex {
    let coinbase = CoinbaseTx {
        to: sk.address(),
        amount: 0,
        height: round,
    };
    let header = BlockHeader {
        version: 1,
        height: round,
        timestamp: 1_000_000 + nonce as i64,
        prev_hash: [round as u8; 32],
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
        round,
        sk.address(),
        sk.verifying_key().to_bytes(),
        Signature([0u8; 64]),
    );
    vertex.signature = sk.sign(&vertex.signable_bytes());
    vertex
}

#[test]
fn test_dag_save_and_load() {
    let temp_dir = std::env::temp_dir();
    let path = temp_dir.join("test_dag_pe.json");

    let mut dag = BlockDag::new();
    let sk = SecretKey::generate();
    let vertex = make_test_vertex(&sk, 0, 1);
    dag.insert(vertex);

    dag.save(&path).unwrap();
    let loaded = BlockDag::load(&path).unwrap();
    assert_eq!(loaded.all_vertices().count(), dag.all_vertices().count());

    std::fs::remove_file(&path).ok();
}

#[test]
fn test_dag_exists() {
    let temp_dir = std::env::temp_dir();
    let path = temp_dir.join("test_exists_dag_pe.json");
    let _ = std::fs::remove_file(&path);

    assert!(!BlockDag::exists(&path));

    let dag = BlockDag::new();
    dag.save(&path).unwrap();
    assert!(BlockDag::exists(&path));

    std::fs::remove_file(&path).ok();
}

#[test]
fn test_dag_load_nonexistent() {
    let path = PathBuf::from("/nonexistent/path/dag_pe.json");
    let result = BlockDag::load(&path);
    assert!(result.is_err());
}

#[test]
fn test_dag_save_empty() {
    let temp_dir = std::env::temp_dir();
    let path = temp_dir.join("test_empty_dag_pe.json");

    let dag = BlockDag::new();
    dag.save(&path).unwrap();

    let loaded = BlockDag::load(&path).unwrap();
    assert_eq!(loaded.all_vertices().count(), 0);

    std::fs::remove_file(&path).ok();
}

#[test]
fn test_dag_save_multiple_vertices() {
    let temp_dir = std::env::temp_dir();
    let path = temp_dir.join("test_multi_dag_pe.json");

    let mut dag = BlockDag::new();
    for i in 0..5u64 {
        let sk = SecretKey::generate();
        let vertex = make_test_vertex(&sk, i, i);
        dag.insert(vertex);
    }

    dag.save(&path).unwrap();
    let loaded = BlockDag::load(&path).unwrap();
    assert_eq!(loaded.all_vertices().count(), 5);

    std::fs::remove_file(&path).ok();
}

#[test]
fn test_checkpoint_persistence() {
    use ultradag_coin::Checkpoint;

    let temp_dir = std::env::temp_dir();
    let path = temp_dir.join("test_checkpoint_pe.json");

    let checkpoint = Checkpoint {
        round: 100,
        state_root: [1u8; 32],
        dag_tip: [2u8; 32],
        total_supply: 1_000_000,
        prev_checkpoint_hash: [0u8; 32],
        signatures: vec![],
    };

    persistence::save(&checkpoint, &path).unwrap();
    assert!(persistence::exists(&path));

    let loaded: Checkpoint = persistence::load(&path).unwrap();
    assert_eq!(loaded.round, 100);
    assert_eq!(loaded.total_supply, 1_000_000);

    std::fs::remove_file(&path).ok();
}

#[test]
fn test_checkpoint_load_nonexistent() {
    use ultradag_coin::Checkpoint;

    let path = PathBuf::from("/nonexistent/checkpoint_pe.json");
    let result: Result<Checkpoint, _> = persistence::load(&path);
    assert!(result.is_err());
}

#[test]
fn test_mempool_persistence() {
    use ultradag_coin::{Mempool, Transaction, TransferTx, Address};

    let temp_dir = std::env::temp_dir();
    let path = temp_dir.join("test_mempool_pe.json");

    let mut mempool = Mempool::new();
    let sk = SecretKey::generate();

    let mut tx = TransferTx {
        from: sk.address(),
        to: Address([2u8; 32]),
        amount: 1000,
        fee: 100_000,
        nonce: 0,
        pub_key: sk.verifying_key().to_bytes(),
        signature: Signature([0u8; 64]),
        memo: None,
    };
    tx.signature = sk.sign(&tx.signable_bytes());

    mempool.insert(Transaction::Transfer(tx));

    mempool.save(&path).unwrap();
    let loaded = Mempool::load(&path).unwrap();

    assert_eq!(loaded.len(), mempool.len());

    std::fs::remove_file(&path).ok();
}

#[test]
fn test_mempool_load_nonexistent() {
    use ultradag_coin::Mempool;

    let path = PathBuf::from("/nonexistent/mempool_pe.json");
    let result = Mempool::load(&path);
    assert!(result.is_err());
}

#[test]
fn test_persistence_roundtrip_consistency() {
    let temp_dir = std::env::temp_dir();
    let path = temp_dir.join("test_roundtrip_pe.json");

    let mut dag = BlockDag::new();
    let sk = SecretKey::generate();
    let vertex = make_test_vertex(&sk, 0, 1);
    let hash = vertex.hash();
    dag.insert(vertex);

    dag.save(&path).unwrap();
    let loaded = BlockDag::load(&path).unwrap();

    assert!(loaded.get(&hash).is_some());

    std::fs::remove_file(&path).ok();
}
