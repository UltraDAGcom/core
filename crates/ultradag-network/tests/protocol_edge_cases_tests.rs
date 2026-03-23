use ultradag_network::protocol::Message;
use ultradag_coin::{SecretKey, Signature};
use ultradag_coin::block::block::Block;
use ultradag_coin::block::header::BlockHeader;
use ultradag_coin::tx::CoinbaseTx;
use ultradag_coin::consensus::DagVertex;

#[test]
fn test_message_encode_decode() {
    let msg = Message::Ping(12345);
    let encoded = msg.encode().unwrap();
    
    assert!(encoded.len() > 4);
    
    let len_bytes = &encoded[0..4];
    let len = u32::from_be_bytes([len_bytes[0], len_bytes[1], len_bytes[2], len_bytes[3]]);
    assert_eq!(len as usize, encoded.len() - 4);
}

#[test]
fn test_message_decode_from_bytes() {
    let msg = Message::Pong(67890);
    let encoded = msg.encode().unwrap();
    
    let data = &encoded[4..];
    let decoded = Message::decode(data).unwrap();
    
    match decoded {
        Message::Pong(n) => assert_eq!(n, 67890),
        _ => panic!("Wrong message type"),
    }
}

#[test]
fn test_hello_ack_message() {
    let msg = Message::HelloAck {
        version: 1,
        height: 500,
    };
    
    let serialized = serde_json::to_vec(&msg).unwrap();
    let deserialized: Message = serde_json::from_slice(&serialized).unwrap();
    
    match deserialized {
        Message::HelloAck { version, height } => {
            assert_eq!(version, 1);
            assert_eq!(height, 500);
        }
        _ => panic!("Wrong message type"),
    }
}

#[test]
fn test_new_block_message() {
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
    
    let msg = Message::NewBlock(block);
    let serialized = serde_json::to_vec(&msg).unwrap();
    let deserialized: Message = serde_json::from_slice(&serialized).unwrap();
    
    match deserialized {
        Message::NewBlock(_) => {},
        _ => panic!("Wrong message type"),
    }
}

#[test]
fn test_get_blocks_message() {
    let msg = Message::GetBlocks {
        from_height: 100,
        max_count: 50,
    };
    
    let serialized = serde_json::to_vec(&msg).unwrap();
    let deserialized: Message = serde_json::from_slice(&serialized).unwrap();
    
    match deserialized {
        Message::GetBlocks { from_height, max_count } => {
            assert_eq!(from_height, 100);
            assert_eq!(max_count, 50);
        }
        _ => panic!("Wrong message type"),
    }
}

#[test]
fn test_blocks_response_message() {
    let sk = SecretKey::generate();
    let mut blocks = vec![];
    
    for i in 0..3 {
        let coinbase = CoinbaseTx {
            to: sk.address(),
            amount: 0,
            height: i,
        };
        let header = BlockHeader {
            version: 1,
            height: i,
            timestamp: 1_000_000 + i as i64,
            prev_hash: [0u8; 32],
            merkle_root: [0u8; 32],
        };
        blocks.push(Block {
            header,
            coinbase,
            transactions: vec![],
        });
    }
    
    let msg = Message::Blocks(blocks);
    let serialized = serde_json::to_vec(&msg).unwrap();
    let deserialized: Message = serde_json::from_slice(&serialized).unwrap();
    
    match deserialized {
        Message::Blocks(b) => assert_eq!(b.len(), 3),
        _ => panic!("Wrong message type"),
    }
}

#[test]
fn test_get_peers_message() {
    let msg = Message::GetPeers;
    let serialized = serde_json::to_vec(&msg).unwrap();
    let deserialized: Message = serde_json::from_slice(&serialized).unwrap();
    
    match deserialized {
        Message::GetPeers => {},
        _ => panic!("Wrong message type"),
    }
}

#[test]
fn test_peers_response_message() {
    let peers = vec![
        "192.168.1.1:9333".to_string(),
        "192.168.1.2:9333".to_string(),
        "192.168.1.3:9333".to_string(),
    ];
    
    let msg = Message::Peers(peers.clone());
    let serialized = serde_json::to_vec(&msg).unwrap();
    let deserialized: Message = serde_json::from_slice(&serialized).unwrap();
    
    match deserialized {
        Message::Peers(p) => assert_eq!(p.len(), 3),
        _ => panic!("Wrong message type"),
    }
}

#[test]
fn test_equivocation_evidence_message() {
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
    let mut vertex1 = DagVertex::new(
        block1,
        vec![[0u8; 32]],
        0,
        sk.address(),
        sk.verifying_key().to_bytes(),
        Signature([0u8; 64]),
    );
    vertex1.signature = sk.sign(&vertex1.signable_bytes());
    
    let coinbase2 = CoinbaseTx {
        to: sk.address(),
        amount: 0,
        height: 0,
    };
    let header2 = BlockHeader {
        version: 1,
        height: 0,
        timestamp: 1_000_001,
        prev_hash: [1u8; 32],
        merkle_root: [0u8; 32],
    };
    let block2 = Block {
        header: header2,
        coinbase: coinbase2,
        transactions: vec![],
    };
    let mut vertex2 = DagVertex::new(
        block2,
        vec![[0u8; 32]],
        0,
        sk.address(),
        sk.verifying_key().to_bytes(),
        Signature([0u8; 64]),
    );
    vertex2.signature = sk.sign(&vertex2.signable_bytes());
    
    let msg = Message::EquivocationEvidence {
        vertex1: vertex1.clone(),
        vertex2: vertex2.clone(),
    };
    
    let serialized = serde_json::to_vec(&msg).unwrap();
    let deserialized: Message = serde_json::from_slice(&serialized).unwrap();
    
    match deserialized {
        Message::EquivocationEvidence { vertex1: v1, vertex2: v2 } => {
            assert_eq!(v1.round, 0);
            assert_eq!(v2.round, 0);
            assert_eq!(v1.validator, v2.validator);
        }
        _ => panic!("Wrong message type"),
    }
}

#[test]
fn test_get_parents_message() {
    let hashes = vec![[1u8; 32], [2u8; 32], [3u8; 32]];
    
    let msg = Message::GetParents {
        hashes: hashes.clone(),
    };
    
    let serialized = serde_json::to_vec(&msg).unwrap();
    let deserialized: Message = serde_json::from_slice(&serialized).unwrap();
    
    match deserialized {
        Message::GetParents { hashes: h } => assert_eq!(h.len(), 3),
        _ => panic!("Wrong message type"),
    }
}

#[test]
fn test_parent_vertices_message() {
    let sk = SecretKey::generate();
    let mut vertices = vec![];
    
    for i in 0..2 {
        let coinbase = CoinbaseTx {
            to: sk.address(),
            amount: 0,
            height: i,
        };
        let header = BlockHeader {
            version: 1,
            height: i,
            timestamp: 1_000_000 + i as i64,
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
            i,
            sk.address(),
            sk.verifying_key().to_bytes(),
            Signature([0u8; 64]),
        );
        vertex.signature = sk.sign(&vertex.signable_bytes());
        vertices.push(vertex);
    }
    
    let msg = Message::ParentVertices { vertices };
    let serialized = serde_json::to_vec(&msg).unwrap();
    let deserialized: Message = serde_json::from_slice(&serialized).unwrap();
    
    match deserialized {
        Message::ParentVertices { vertices: v } => assert_eq!(v.len(), 2),
        _ => panic!("Wrong message type"),
    }
}

#[test]
fn test_checkpoint_signature_msg() {
    use ultradag_coin::consensus::CheckpointSignature;
    
    let sk = SecretKey::generate();
    let checkpoint_hash = [5u8; 32];
    let sig_bytes = sk.sign(&checkpoint_hash);
    
    let checkpoint_sig = CheckpointSignature {
        validator: sk.address(),
        pub_key: sk.verifying_key().to_bytes(),
        signature: sig_bytes,
    };
    
    let msg = Message::CheckpointSignatureMsg {
        round: 100,
        checkpoint_hash,
        signature: checkpoint_sig,
    };
    
    let serialized = serde_json::to_vec(&msg).unwrap();
    let deserialized: Message = serde_json::from_slice(&serialized).unwrap();
    
    match deserialized {
        Message::CheckpointSignatureMsg { round, .. } => {
            assert_eq!(round, 100);
        }
        _ => panic!("Wrong message type"),
    }
}

#[test]
fn test_checkpoint_sync_message() {
    use ultradag_coin::{Checkpoint};
    use ultradag_coin::state::persistence::StateSnapshot;
    
    let sk = SecretKey::generate();
    
    let checkpoint = Checkpoint {
        round: 100,
        state_root: [1u8; 32],
        dag_tip: [2u8; 32],
        total_supply: 1_000_000,
        prev_checkpoint_hash: [0u8; 32],
            signatures: vec![],
    };
    
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
    
    let state = StateSnapshot {
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
        bridge_reserve: 0,
        bridge_attestations: vec![],
        bridge_signatures: vec![],
        bridge_nonce: 0,
        bridge_contract_address: [0u8; 20],
        used_release_nonces: vec![],
        bridge_release_votes: vec![],
    };

    let msg = Message::CheckpointSync {
        checkpoint,
        suffix_vertices: vec![vertex],
        state_at_checkpoint: state,
        checkpoint_chain: vec![],
    };
    
    let serialized = serde_json::to_vec(&msg).unwrap();
    let deserialized: Message = serde_json::from_slice(&serialized).unwrap();
    
    match deserialized {
        Message::CheckpointSync { checkpoint, suffix_vertices, .. } => {
            assert_eq!(checkpoint.round, 100);
            assert_eq!(suffix_vertices.len(), 1);
        }
        _ => panic!("Wrong message type"),
    }
}

#[test]
fn test_message_max_size_enforcement() {
    use ultradag_network::protocol::MAX_MESSAGE_SIZE;
    
    let oversized = vec![0u8; MAX_MESSAGE_SIZE + 1];
    let result = Message::decode(&oversized);
    
    assert!(result.is_err());
}
