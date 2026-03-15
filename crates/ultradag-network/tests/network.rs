/// Module 8: P2P Network Layer — Production-grade tests

use ultradag_coin::address::{Address, SecretKey, Signature};
use ultradag_coin::block::header::BlockHeader;
use ultradag_coin::block::Block;
use ultradag_coin::consensus::vertex::DagVertex;
use ultradag_coin::tx::{CoinbaseTx, Transaction};
use ultradag_network::protocol::Message;
use ultradag_network::peer::{PeerRegistry, split_connection};

async fn tcp_pair() -> (tokio::net::TcpStream, tokio::net::TcpStream) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let client = tokio::spawn(async move {
        tokio::net::TcpStream::connect(addr).await.unwrap()
    });
    let (server, _) = listener.accept().await.unwrap();
    let client = client.await.unwrap();
    (server, client)
}

fn make_real_tx(sk: &SecretKey) -> Transaction {
    let mut transfer = ultradag_coin::TransferTx {
        from: sk.address(),
        to: Address::ZERO,
        amount: 1000,
        fee: 50,
        nonce: 0,
        pub_key: sk.verifying_key().to_bytes(),
        signature: Signature([0u8; 64]),
        memo: None,
    };
    transfer.signature = sk.sign(&transfer.signable_bytes());
    Transaction::Transfer(transfer)
}

fn make_real_vertex(sk: &SecretKey, round: u64) -> DagVertex {
    let validator = sk.address();
    let block = Block {
        header: BlockHeader {
            version: 1, height: round,
            timestamp: 1_000_000 + round as i64,
            prev_hash: [0u8; 32],
            merkle_root: [0u8; 32],
        },
        coinbase: CoinbaseTx { to: validator, amount: 0, height: round },
        transactions: vec![],
    };
    let mut v = DagVertex::new(
        block, vec![], round, validator,
        sk.verifying_key().to_bytes(), Signature([0u8; 64]),
    );
    v.signature = sk.sign(&v.signable_bytes());
    v
}

/// Handshake: Hello message roundtrips correctly.
/// Mutation: encode not including listen_port → decoded port is 0.
#[tokio::test]
async fn hello_handshake_roundtrip() {
    let (server, client) = tcp_pair().await;
    let (_, server_writer) = split_connection(server, "s".into());
    let (mut client_reader, _) = split_connection(client, "c".into());

    let msg = Message::Hello { version: 1, height: 42, listen_port: 9333 };
    server_writer.send(&msg).await.unwrap();

    let received = client_reader.recv().await.unwrap();
    match received {
        Message::Hello { version, height, listen_port } => {
            assert_eq!(version, 1);
            assert_eq!(height, 42);
            assert_eq!(listen_port, 9333);
        }
        other => panic!("expected Hello, got {:?}", other),
    }
}

/// Malformed message is rejected without crashing.
/// Mutation: decode accepting invalid JSON → test fails.
#[tokio::test]
async fn malformed_message_rejected() {
    let (server, client) = tcp_pair().await;
    let (mut server_reader, _) = split_connection(server, "s".into());
    let (_, client_writer) = split_connection(client, "c".into());

    // Send garbage bytes with valid length prefix
    client_writer.send_raw(b"this is not json").await.unwrap();

    // Reader should return an error, not panic
    let result = server_reader.recv().await;
    assert!(result.is_err(), "malformed message should return error");
    assert_eq!(result.unwrap_err().kind(), std::io::ErrorKind::InvalidData);
}

/// DagProposal roundtrips with all fields intact.
/// Mutation: DagVertex serialization dropping signature → deserialized sig is zero.
#[tokio::test]
async fn dag_proposal_roundtrip() {
    let sk = SecretKey::from_bytes([10u8; 32]);
    let vertex = make_real_vertex(&sk, 5);
    let original_hash = vertex.hash();
    let original_round = vertex.round;
    let original_validator = vertex.validator;
    let original_sig = vertex.signature;

    let (server, client) = tcp_pair().await;
    let (_, server_writer) = split_connection(server, "s".into());
    let (mut client_reader, _) = split_connection(client, "c".into());

    server_writer.send(&Message::DagProposal(vertex)).await.unwrap();

    let received = client_reader.recv().await.unwrap();
    match received {
        Message::DagProposal(v) => {
            assert_eq!(v.hash(), original_hash, "hash must match");
            assert_eq!(v.round, original_round, "round must match");
            assert_eq!(v.validator, original_validator, "validator must match");
            assert_eq!(v.signature, original_sig, "signature must match");
            assert!(v.verify_signature(), "signature must still be valid after roundtrip");
        }
        other => panic!("expected DagProposal, got {:?}", other),
    }
}

/// Transaction roundtrips without data loss.
/// Mutation: Transaction serialization losing fee → deserialized fee is 0.
#[tokio::test]
async fn transaction_roundtrip() {
    let sk = SecretKey::from_bytes([20u8; 32]);
    let tx = make_real_tx(&sk);
    let original_hash = tx.hash();
    let original_from = tx.from();
    let original_amount = tx.amount();
    let original_fee = tx.fee();
    let original_nonce = tx.nonce();

    let (server, client) = tcp_pair().await;
    let (_, server_writer) = split_connection(server, "s".into());
    let (mut client_reader, _) = split_connection(client, "c".into());

    server_writer.send(&Message::NewTx(tx)).await.unwrap();

    let received = client_reader.recv().await.unwrap();
    match received {
        Message::NewTx(t) => {
            assert_eq!(t.hash(), original_hash);
            assert_eq!(t.from(), original_from);
            assert_eq!(t.nonce(), original_nonce);
            if let ultradag_coin::Transaction::Transfer(ref transfer) = t {
                assert_eq!(transfer.amount, original_amount);
                assert_eq!(transfer.fee, original_fee);
            } else {
                panic!("expected Transfer variant");
            }
            assert!(t.verify_signature(), "signature must still be valid");
        }
        other => panic!("expected NewTx, got {:?}", other),
    }
}

/// Ping receives Pong with matching nonce.
/// Mutation: Pong serialization dropping nonce → deserialized nonce is 0.
#[tokio::test]
async fn ping_pong_roundtrip() {
    let (server, client) = tcp_pair().await;
    let (_, server_writer) = split_connection(server, "s".into());
    let (mut client_reader, _) = split_connection(client, "c".into());

    let nonce = 0xDEADBEEFu64;
    server_writer.send(&Message::Ping(nonce)).await.unwrap();

    match client_reader.recv().await.unwrap() {
        Message::Ping(n) => assert_eq!(n, nonce),
        other => panic!("expected Ping, got {:?}", other),
    }

    // Also test Pong
    let (server2, client2) = tcp_pair().await;
    let (_, sw) = split_connection(server2, "s".into());
    let (mut cr, _) = split_connection(client2, "c".into());

    sw.send(&Message::Pong(nonce)).await.unwrap();
    match cr.recv().await.unwrap() {
        Message::Pong(n) => assert_eq!(n, nonce),
        other => panic!("expected Pong, got {:?}", other),
    }
}

/// Disconnected peer is removed from registry.
/// Mutation: remove_peer not deleting entry → count stays same.
#[tokio::test]
async fn disconnected_peer_removed() {
    let (server, client) = tcp_pair().await;
    let (_, writer) = split_connection(server, "peer-1".into());

    let reg = PeerRegistry::new();
    reg.add_writer("peer-1".into(), writer).await;
    assert_eq!(reg.connected_count().await, 1);

    // Simulate disconnect
    drop(client);

    // Remove from registry
    reg.remove_peer("peer-1").await;
    assert_eq!(reg.connected_count().await, 0);

    // NEGATIVE: sending to removed peer fails
    let result = reg.send_to("peer-1", &Message::Ping(1)).await;
    assert!(result.is_err());
    assert_eq!(result.unwrap_err().kind(), std::io::ErrorKind::NotConnected);
}

/// Sending to nonexistent peer returns error, not panic.
/// Mutation: send_to panicking on missing peer → test catches panic.
#[tokio::test]
async fn send_to_nonexistent_peer_returns_error() {
    let reg = PeerRegistry::new();

    let result = reg.send_to("ghost-peer", &Message::Ping(42)).await;
    assert!(result.is_err());
    assert_eq!(result.unwrap_err().kind(), std::io::ErrorKind::NotConnected);
}

/// Multiple messages arrive in order.
/// Mutation: message framing being wrong → messages get interleaved.
#[tokio::test]
async fn multiple_messages_in_order() {
    let (server, client) = tcp_pair().await;
    let (_, sw) = split_connection(server, "s".into());
    let (mut cr, _) = split_connection(client, "c".into());

    let sk = SecretKey::from_bytes([30u8; 32]);
    let tx = make_real_tx(&sk);
    let vertex = make_real_vertex(&sk, 3);

    sw.send(&Message::Ping(1)).await.unwrap();
    sw.send(&Message::NewTx(tx.clone())).await.unwrap();
    sw.send(&Message::DagProposal(vertex.clone())).await.unwrap();
    sw.send(&Message::Pong(2)).await.unwrap();

    match cr.recv().await.unwrap() {
        Message::Ping(1) => {}
        other => panic!("msg 1: expected Ping(1), got {:?}", other),
    }
    match cr.recv().await.unwrap() {
        Message::NewTx(t) => assert_eq!(t.hash(), tx.hash()),
        other => panic!("msg 2: expected NewTx, got {:?}", other),
    }
    match cr.recv().await.unwrap() {
        Message::DagProposal(v) => assert_eq!(v.hash(), vertex.hash()),
        other => panic!("msg 3: expected DagProposal, got {:?}", other),
    }
    match cr.recv().await.unwrap() {
        Message::Pong(2) => {}
        other => panic!("msg 4: expected Pong(2), got {:?}", other),
    }
}

/// EOF on connection returns error, not panic.
/// Mutation: recv not handling EOF → infinite hang or panic.
#[tokio::test]
async fn eof_returns_error() {
    let (server, client) = tcp_pair().await;
    let (mut reader, _) = split_connection(server, "s".into());
    drop(client);

    let result = reader.recv().await;
    assert!(result.is_err(), "EOF should return error");
}

/// GetParents roundtrips with correct hashes.
#[tokio::test]
async fn get_parents_roundtrip() {
    let (server, client) = tcp_pair().await;
    let (_, sw) = split_connection(server, "s".into());
    let (mut cr, _) = split_connection(client, "c".into());

    let h1 = [0xAA; 32];
    let h2 = [0xBB; 32];
    sw.send(&Message::GetParents { hashes: vec![h1, h2] }).await.unwrap();

    match cr.recv().await.unwrap() {
        Message::GetParents { hashes } => {
            assert_eq!(hashes.len(), 2);
            assert_eq!(hashes[0], h1);
            assert_eq!(hashes[1], h2);
        }
        other => panic!("expected GetParents, got {:?}", other),
    }
}

/// ParentVertices roundtrips with vertices intact.
#[tokio::test]
async fn parent_vertices_roundtrip() {
    let sk = SecretKey::from_bytes([40u8; 32]);
    let v1 = make_real_vertex(&sk, 1);
    let v2 = make_real_vertex(&sk, 2);
    let h1 = v1.hash();
    let h2 = v2.hash();

    let (server, client) = tcp_pair().await;
    let (_, sw) = split_connection(server, "s".into());
    let (mut cr, _) = split_connection(client, "c".into());

    sw.send(&Message::ParentVertices { vertices: vec![v1, v2] }).await.unwrap();

    match cr.recv().await.unwrap() {
        Message::ParentVertices { vertices } => {
            assert_eq!(vertices.len(), 2);
            assert_eq!(vertices[0].hash(), h1);
            assert_eq!(vertices[1].hash(), h2);
            assert!(vertices[0].verify_signature(), "signature must survive roundtrip");
            assert!(vertices[1].verify_signature(), "signature must survive roundtrip");
        }
        other => panic!("expected ParentVertices, got {:?}", other),
    }
}

/// Message too large is rejected.
/// Mutation: recv not checking message size → OOM.
#[tokio::test]
async fn oversized_message_rejected() {
    let (server, client) = tcp_pair().await;
    let (mut server_reader, _) = split_connection(server, "s".into());
    let (_, client_writer) = split_connection(client, "c".into());

    // Send a length prefix indicating >10MB message
    client_writer.send_raw_len(11 * 1024 * 1024).await.unwrap();

    let result = server_reader.recv().await;
    assert!(result.is_err(), "oversized message should be rejected");
    assert_eq!(result.unwrap_err().kind(), std::io::ErrorKind::InvalidData);
}
