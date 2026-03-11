use ultradag_network::protocol::Message;
use ultradag_coin::{SecretKey, Signature, Address, Checkpoint};
use ultradag_coin::block::block::Block;
use ultradag_coin::block::header::BlockHeader;
use ultradag_coin::tx::CoinbaseTx;
use ultradag_coin::consensus::DagVertex;

#[test]
fn test_checkpoint_message_serialization() {
    let checkpoint = Checkpoint {
        round: 100,
        state_root: [1u8; 32],
        dag_tip: [2u8; 32],
        total_supply: 1_000_000,
        prev_checkpoint_hash: [0u8; 32],
            signatures: vec![],
    };
    
    let msg = Message::CheckpointProposal(checkpoint.clone());
    let serialized = serde_json::to_vec(&msg).unwrap();
    let deserialized: Message = serde_json::from_slice(&serialized).unwrap();
    
    match deserialized {
        Message::CheckpointProposal(cp) => {
            assert_eq!(cp.round, 100);
            assert_eq!(cp.total_supply, 1_000_000);
        }
        _ => panic!("Wrong message type"),
    }
}

#[test]
fn test_get_checkpoint_request() {
    let msg = Message::GetCheckpoint { min_round: 50 };
    let serialized = serde_json::to_vec(&msg).unwrap();
    let deserialized: Message = serde_json::from_slice(&serialized).unwrap();
    
    match deserialized {
        Message::GetCheckpoint { min_round } => assert_eq!(min_round, 50),
        _ => panic!("Wrong message type"),
    }
}

#[test]
fn test_dag_proposal_with_multiple_parents() {
    let sk = SecretKey::generate();
    
    let coinbase = CoinbaseTx {
        to: sk.address(),
        amount: 5_000_000_000,
        height: 1,
    };
    let header = BlockHeader {
        version: 1,
        height: 1,
        timestamp: 1_000_001,
        prev_hash: [1u8; 32],
        merkle_root: [0u8; 32],
    };
    let block = Block {
        header,
        coinbase,
        transactions: vec![],
    };
    
    let mut vertex = DagVertex::new(
        block,
        vec![[1u8; 32], [2u8; 32], [3u8; 32]],
        1,
        sk.address(),
        sk.verifying_key().to_bytes(),
        Signature([0u8; 64]),
    );
    vertex.signature = sk.sign(&vertex.signable_bytes());
    
    assert_eq!(vertex.parent_hashes.len(), 3);
    assert_eq!(vertex.round, 1);
}

#[test]
fn test_get_dag_vertices_request() {
    let msg = Message::GetDagVertices {
        from_round: 0,
        max_count: 10,
    };
    
    let serialized = serde_json::to_vec(&msg).unwrap();
    let deserialized: Message = serde_json::from_slice(&serialized).unwrap();
    
    match deserialized {
        Message::GetDagVertices { from_round, max_count } => {
            assert_eq!(from_round, 0);
            assert_eq!(max_count, 10);
        }
        _ => panic!("Wrong message type"),
    }
}

#[test]
fn test_dag_vertices_response() {
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
    
    let msg = Message::DagVertices(vec![vertex]);
    let serialized = serde_json::to_vec(&msg).unwrap();
    let deserialized: Message = serde_json::from_slice(&serialized).unwrap();
    
    match deserialized {
        Message::DagVertices(vertices) => assert_eq!(vertices.len(), 1),
        _ => panic!("Wrong message type"),
    }
}

#[test]
fn test_ping_pong_roundtrip() {
    let ping = Message::Ping(12345);
    let serialized = serde_json::to_vec(&ping).unwrap();
    let deserialized: Message = serde_json::from_slice(&serialized).unwrap();
    
    match deserialized {
        Message::Ping(nonce) => {
            assert_eq!(nonce, 12345);
            let pong = Message::Pong(nonce);
            let pong_serialized = serde_json::to_vec(&pong).unwrap();
            let pong_deserialized: Message = serde_json::from_slice(&pong_serialized).unwrap();
            match pong_deserialized {
                Message::Pong(n) => assert_eq!(n, 12345),
                _ => panic!("Wrong pong"),
            }
        }
        _ => panic!("Wrong ping"),
    }
}

#[test]
fn test_hello_message_with_listen_port() {
    let msg = Message::Hello {
        version: 1,
        height: 100,
        listen_port: 9333,
    };
    
    let serialized = serde_json::to_vec(&msg).unwrap();
    let deserialized: Message = serde_json::from_slice(&serialized).unwrap();
    
    match deserialized {
        Message::Hello { version, height, listen_port } => {
            assert_eq!(version, 1);
            assert_eq!(height, 100);
            assert_eq!(listen_port, 9333);
        }
        _ => panic!("Wrong message type"),
    }
}

#[test]
fn test_transaction_broadcast() {
    use ultradag_coin::{Transaction, TransferTx};
    
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

    let msg = Message::NewTx(Transaction::Transfer(tx));
    let serialized = serde_json::to_vec(&msg).unwrap();
    let deserialized: Message = serde_json::from_slice(&serialized).unwrap();
    
    match deserialized {
        Message::NewTx(_) => {},
        _ => panic!("Wrong message type"),
    }
}

#[test]
fn test_checkpoint_with_signatures() {
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
    
    let msg = Message::CheckpointProposal(checkpoint.clone());
    let serialized = serde_json::to_vec(&msg).unwrap();
    let deserialized: Message = serde_json::from_slice(&serialized).unwrap();
    
    match deserialized {
        Message::CheckpointProposal(cp) => {
            assert_eq!(cp.signatures.len(), 2);
        }
        _ => panic!("Wrong message type"),
    }
}

#[test]
fn test_dag_vertex_with_transactions() {
    use ultradag_coin::{Transaction, TransferTx};
    
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
        transactions: vec![Transaction::Transfer(tx)],
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
    
    assert_eq!(vertex.block.transactions.len(), 1);
}

#[test]
fn test_message_size_limits() {
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
    
    let msg = Message::DagProposal(vertex);
    let serialized = serde_json::to_vec(&msg).unwrap();
    
    assert!(serialized.len() < 10_000);
}

#[test]
fn test_orphan_vertex_handling() {
    let sk = SecretKey::generate();
    
    let coinbase = CoinbaseTx {
        to: sk.address(),
        amount: 5_000_000_000,
        height: 5,
    };
    let header = BlockHeader {
        version: 1,
        height: 5,
        timestamp: 1_000_005,
        prev_hash: [5u8; 32],
        merkle_root: [0u8; 32],
    };
    let block = Block {
        header,
        coinbase,
        transactions: vec![],
    };
    
    let mut vertex = DagVertex::new(
        block,
        vec![[10u8; 32], [11u8; 32]],
        5,
        sk.address(),
        sk.verifying_key().to_bytes(),
        Signature([0u8; 64]),
    );
    vertex.signature = sk.sign(&vertex.signable_bytes());
    
    assert_eq!(vertex.parent_hashes.len(), 2);
    assert_eq!(vertex.round, 5);
}

#[test]
fn test_checkpoint_sync_request() {
    let msg = Message::GetCheckpoint { min_round: 100 };
    let serialized = serde_json::to_vec(&msg).unwrap();
    
    assert!(serialized.len() < 100);
    
    let deserialized: Message = serde_json::from_slice(&serialized).unwrap();
    match deserialized {
        Message::GetCheckpoint { min_round } => assert_eq!(min_round, 100),
        _ => panic!("Wrong message type"),
    }
}

#[test]
fn test_multiple_dag_vertices_batch() {
    let sk = SecretKey::generate();
    let mut vertices = vec![];
    
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
        vertices.push(vertex);
    }
    
    let msg = Message::DagVertices(vertices);
    let serialized = serde_json::to_vec(&msg).unwrap();
    let deserialized: Message = serde_json::from_slice(&serialized).unwrap();
    
    match deserialized {
        Message::DagVertices(v) => assert_eq!(v.len(), 5),
        _ => panic!("Wrong message type"),
    }
}

#[test]
fn test_message_deterministic_serialization() {
    let msg1 = Message::Ping(42);
    let msg2 = Message::Ping(42);
    
    let ser1 = serde_json::to_vec(&msg1).unwrap();
    let ser2 = serde_json::to_vec(&msg2).unwrap();
    
    assert_eq!(ser1, ser2);
}
