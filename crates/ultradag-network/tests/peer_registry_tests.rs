use ultradag_network::peer::registry::PeerRegistry;
use ultradag_network::peer::connection::split_connection;
use tokio::net::TcpListener;

#[tokio::test]
async fn new_registry_is_empty() {
    let reg = PeerRegistry::new();
    assert_eq!(reg.connected_count().await, 0);
    assert!(reg.known_peers().await.is_empty());
}

#[tokio::test]
async fn add_known_and_list() {
    let reg = PeerRegistry::new();
    reg.add_known("127.0.0.1:9000".into()).await;
    reg.add_known("127.0.0.1:9001".into()).await;
    let peers = reg.known_peers().await;
    assert_eq!(peers.len(), 2);
    assert!(peers.contains(&"127.0.0.1:9000".to_string()));
    assert!(peers.contains(&"127.0.0.1:9001".to_string()));
}

#[tokio::test]
async fn add_known_deduplicates() {
    let reg = PeerRegistry::new();
    reg.add_known("127.0.0.1:9000".into()).await;
    reg.add_known("127.0.0.1:9000".into()).await;
    assert_eq!(reg.known_peers().await.len(), 1);
}

#[tokio::test]
async fn add_writer_and_remove_peer() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let local_addr = listener.local_addr().unwrap();

    let connect_handle = tokio::spawn(async move {
        tokio::net::TcpStream::connect(local_addr).await.unwrap()
    });

    let (server_stream, _) = listener.accept().await.unwrap();
    let _client_stream = connect_handle.await.unwrap();

    let addr = "test-peer".to_string();
    let (_, writer) = split_connection(server_stream, addr.clone(), None);

    let reg = PeerRegistry::new();
    reg.add_writer(addr.clone(), writer).await;
    assert_eq!(reg.connected_count().await, 1);

    reg.remove_peer(&addr).await;
    assert_eq!(reg.connected_count().await, 0);
}

#[tokio::test]
async fn connected_listen_addrs_tracking() {
    let reg = PeerRegistry::new();
    
    reg.add_connected_listen_addr("peer1:9333".into()).await;
    reg.add_connected_listen_addr("peer2:9333".into()).await;
    
    let addrs = reg.connected_listen_addrs().await;
    assert_eq!(addrs.len(), 2);
    assert!(addrs.contains(&"peer1:9333".to_string()));
    assert!(addrs.contains(&"peer2:9333".to_string()));
    
    reg.remove_connected_listen_addr("peer1:9333").await;
    let addrs = reg.connected_listen_addrs().await;
    assert_eq!(addrs.len(), 1);
    assert!(addrs.contains(&"peer2:9333".to_string()));
}

#[tokio::test]
async fn writer_to_listen_mapping() {
    let reg = PeerRegistry::new();
    
    reg.link_writer_to_listen("192.168.1.1:54321".into(), "192.168.1.1:9333".into()).await;
    reg.add_connected_listen_addr("192.168.1.1:9333".into()).await;
    
    let addrs = reg.connected_listen_addrs().await;
    assert_eq!(addrs.len(), 1);
    
    reg.remove_peer("192.168.1.1:54321").await;
    
    let addrs = reg.connected_listen_addrs().await;
    assert_eq!(addrs.len(), 0);
}
