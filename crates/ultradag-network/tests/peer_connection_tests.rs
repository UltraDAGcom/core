use ultradag_network::peer::connection::split_connection;
use ultradag_network::protocol::Message;
use tokio::net::TcpListener;

async fn tcp_pair() -> (tokio::net::TcpStream, tokio::net::TcpStream) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let client = tokio::spawn(async move {
        tokio::net::TcpStream::connect(addr).await.unwrap()
    });
    let (server, _) = listener.accept().await.unwrap();
    let client = client.await.unwrap();
    (server, client)
}

#[tokio::test]
async fn split_connection_creates_reader_and_writer() {
    let (server, _client) = tcp_pair().await;
    let (reader, writer) = split_connection(server, "test".into(), None);
    assert_eq!(reader.addr, "test");
    assert_eq!(writer.addr, "test");
}

#[tokio::test]
async fn writer_send_reader_recv_roundtrip() {
    let (server, client) = tcp_pair().await;
    let (_server_reader, server_writer) = split_connection(server, "server".into(), None);
    let (mut client_reader, _client_writer) = split_connection(client, "client".into(), None);

    let msg = Message::Ping(42);
    server_writer.send(&msg).await.unwrap();

    let received = client_reader.recv().await.unwrap();
    match received {
        Message::Ping(n) => assert_eq!(n, 42),
        other => panic!("expected Ping(42), got {:?}", other),
    }
}

#[tokio::test]
async fn multiple_messages_roundtrip() {
    let (server, client) = tcp_pair().await;
    let (_, server_writer) = split_connection(server, "s".into(), None);
    let (mut client_reader, _) = split_connection(client, "c".into(), None);

    let messages = vec![
        Message::Ping(1),
        Message::Ping(2),
        Message::Ping(3),
    ];

    for msg in &messages {
        server_writer.send(msg).await.unwrap();
    }

    for expected in &messages {
        let received = client_reader.recv().await.unwrap();
        match (expected, received) {
            (Message::Ping(e), Message::Ping(r)) => assert_eq!(e, &r),
            _ => panic!("message mismatch"),
        }
    }
}

#[tokio::test]
async fn recv_eof_returns_error() {
    let (server, client) = tcp_pair().await;
    let (mut reader, _) = split_connection(server, "s".into(), None);

    drop(client);
    
    let result = reader.recv().await;
    assert!(result.is_err());
}

#[tokio::test]
async fn bidirectional_communication() {
    let (server, client) = tcp_pair().await;
    let (mut server_reader, server_writer) = split_connection(server, "server".into(), None);
    let (mut client_reader, client_writer) = split_connection(client, "client".into(), None);

    server_writer.send(&Message::Ping(100)).await.unwrap();
    let msg = client_reader.recv().await.unwrap();
    assert!(matches!(msg, Message::Ping(100)));

    client_writer.send(&Message::Ping(200)).await.unwrap();
    let msg = server_reader.recv().await.unwrap();
    assert!(matches!(msg, Message::Ping(200)));
}

#[tokio::test]
async fn concurrent_sends() {
    let (server, client) = tcp_pair().await;
    let (_, server_writer) = split_connection(server, "s".into(), None);
    let (mut client_reader, _) = split_connection(client, "c".into(), None);

    let writer1 = server_writer.clone();
    let writer2 = server_writer.clone();
    
    let send1 = tokio::spawn(async move {
        writer1.send(&Message::Ping(1)).await.unwrap();
    });
    
    let send2 = tokio::spawn(async move {
        writer2.send(&Message::Ping(2)).await.unwrap();
    });
    
    send1.await.unwrap();
    send2.await.unwrap();
    
    let mut received = vec![];
    for _ in 0..2 {
        if let Ok(msg) = client_reader.recv().await {
            received.push(msg);
        }
    }
    
    assert_eq!(received.len(), 2);
}

#[tokio::test]
async fn connection_drop_cleanup() {
    let (server, client) = tcp_pair().await;
    let (mut reader, _writer) = split_connection(server, "s".into(), None);
    
    drop(client);
    
    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
    
    let result = reader.recv().await;
    assert!(result.is_err());
}
