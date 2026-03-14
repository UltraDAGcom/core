use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, warn};

use crate::peer::connection::PeerWriter;
use crate::protocol::Message;

/// Tracks known and connected peers with write handles for broadcasting.
#[derive(Clone)]
pub struct PeerRegistry {
    known: Arc<RwLock<HashSet<String>>>,
    writers: Arc<RwLock<HashMap<String, PeerWriter>>>,
    /// Canonical listen addresses of currently connected peers.
    /// Prevents try_connect_peer from creating duplicate connections
    /// when the writer key uses an ephemeral port.
    connected_listen_addrs: Arc<RwLock<HashSet<String>>>,
    /// Maps writer key (possibly ephemeral port) → canonical listen address.
    /// Used to clean up connected_listen_addrs when a peer is removed by writer key.
    writer_to_listen: Arc<RwLock<HashMap<String, String>>>,
}

impl PeerRegistry {
    pub fn new() -> Self {
        Self {
            known: Arc::new(RwLock::new(HashSet::new())),
            writers: Arc::new(RwLock::new(HashMap::new())),
            connected_listen_addrs: Arc::new(RwLock::new(HashSet::new())),
            writer_to_listen: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn add_known(&self, addr: String) {
        let mut known = self.known.write().await;
        // Cap at 1000 known peers to prevent memory exhaustion via malicious Peers messages
        if known.len() < 1000 {
            known.insert(addr);
        }
    }

    pub async fn add_writer(&self, addr: String, writer: PeerWriter) {
        self.writers.write().await.insert(addr, writer);
    }

    pub async fn remove_peer(&self, addr: &str) {
        self.writers.write().await.remove(addr);
        // Also remove the linked listen address (handles ephemeral → canonical mapping)
        if let Some(listen_addr) = self.writer_to_listen.write().await.remove(addr) {
            self.connected_listen_addrs.write().await.remove(&listen_addr);
        }
    }

    /// Record a peer's canonical listen address as connected,
    /// and map the writer key to the listen address for cleanup.
    pub async fn add_connected_listen_addr(&self, listen_addr: String) {
        self.connected_listen_addrs.write().await.insert(listen_addr);
    }

    /// Link a writer key to a canonical listen address so dead peer cleanup
    /// can remove the correct listen address when only the writer key is known.
    pub async fn link_writer_to_listen(&self, writer_key: String, listen_addr: String) {
        self.writer_to_listen.write().await.insert(writer_key, listen_addr);
    }

    /// Remove a peer's canonical listen address from the connected set.
    pub async fn remove_connected_listen_addr(&self, addr: &str) {
        self.connected_listen_addrs.write().await.remove(addr);
        // Also clean up the reverse mapping
        self.writer_to_listen.write().await.retain(|_, v| v != addr);
    }

    /// Check if we have a connection to a peer by listen address.
    pub async fn is_listen_addr_connected(&self, addr: &str) -> bool {
        self.connected_listen_addrs.read().await.contains(addr)
    }

    pub async fn known_peers(&self) -> Vec<String> {
        self.known.read().await.iter().cloned().collect()
    }

    pub async fn connected_count(&self) -> usize {
        self.writers.read().await.len()
    }

    /// Return list of connected peer addresses (writer keys).
    pub async fn connected_addrs(&self) -> Vec<String> {
        self.writers.read().await.keys().cloned().collect()
    }

    /// Return list of connected listen addresses.
    pub async fn connected_listen_addrs(&self) -> Vec<String> {
        self.connected_listen_addrs.read().await.iter().cloned().collect()
    }

    /// Broadcast a message to all connected peers except `exclude`.
    /// Sends concurrently to all peers to avoid head-of-line blocking
    /// from slow connections (sequential sends caused 3-18s delays).
    /// Automatically removes peers that fail to send (broken connections).
    pub async fn broadcast(&self, msg: &Message, exclude: &str) {
        let sends: Vec<(String, PeerWriter)> = {
            let writers = self.writers.read().await;
            writers.iter()
                .filter(|(addr, _)| *addr != exclude)
                .map(|(addr, writer)| (addr.clone(), writer.clone()))
                .collect()
        };

        debug!("Broadcasting to {} peers", sends.len());

        let mut handles = Vec::with_capacity(sends.len());
        for (addr, writer) in sends {
            let msg_clone = msg.clone();
            handles.push(tokio::spawn(async move {
                match tokio::time::timeout(
                    std::time::Duration::from_secs(5),
                    writer.send(&msg_clone),
                ).await {
                    Ok(Ok(())) => None,
                    Ok(Err(e)) => {
                        warn!("Failed to send to {}: {} — removing dead peer", addr, e);
                        Some(addr)
                    }
                    Err(_) => {
                        warn!("Timeout sending to {} — removing dead peer", addr);
                        Some(addr)
                    }
                }
            }));
        }

        let mut dead_peers = Vec::new();
        for handle in handles {
            if let Ok(Some(addr)) = handle.await {
                dead_peers.push(addr);
            }
        }

        // Remove dead peers outside the read lock
        if !dead_peers.is_empty() {
            for addr in &dead_peers {
                self.remove_peer(addr).await;
            }
        }
    }

    /// Send a message to a specific peer.
    /// Clones the writer before dropping the read lock to avoid holding it across async I/O.
    pub async fn send_to(&self, addr: &str, msg: &Message) -> std::io::Result<()> {
        let writer = {
            let writers = self.writers.read().await;
            writers.get(addr).cloned()
        };
        if let Some(writer) = writer {
            writer.send(msg).await
        } else {
            Err(std::io::Error::new(
                std::io::ErrorKind::NotConnected,
                "peer not connected",
            ))
        }
    }
}

impl Default for PeerRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::peer::connection::split_connection;
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
        let client_stream = connect_handle.await.unwrap();

        let addr = "test-peer".to_string();
        let (_, writer) = split_connection(server_stream, addr.clone());

        let reg = PeerRegistry::new();
        assert_eq!(reg.connected_count().await, 0);

        reg.add_writer(addr.clone(), writer).await;
        assert_eq!(reg.connected_count().await, 1);

        reg.remove_peer(&addr).await;
        assert_eq!(reg.connected_count().await, 0);

        drop(client_stream);
    }

    #[tokio::test]
    async fn send_to_unknown_peer_fails() {
        let reg = PeerRegistry::new();
        let msg = Message::Ping(1);
        let result = reg.send_to("nonexistent", &msg).await;
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().kind(), std::io::ErrorKind::NotConnected);
    }
}
