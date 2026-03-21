use std::collections::{HashMap, HashSet};
use std::net::IpAddr;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::time::Instant;
use tracing::{debug, info, warn};

use crate::peer::connection::PeerWriter;
use crate::protocol::Message;

/// Maximum ban duration in seconds (1 hour).
const MAX_BAN_DURATION_SECS: u64 = 3600;

/// Tracks a banned IP with exponential backoff.
struct BanEntry {
    banned_until: Instant,
    ban_count: u32,
}

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
    /// Addresses currently being connected to (prevents TOCTOU race in try_connect_peer).
    /// A task sets this before TCP connect and clears it after writer is added or on failure.
    connecting: Arc<RwLock<HashSet<String>>>,
    /// Banned IPs with exponential backoff. Duration doubles on each ban,
    /// capped at MAX_BAN_DURATION_SECS (1 hour).
    banned_peers: Arc<RwLock<HashMap<IpAddr, BanEntry>>>,
}

impl PeerRegistry {
    pub fn new() -> Self {
        Self {
            known: Arc::new(RwLock::new(HashSet::new())),
            writers: Arc::new(RwLock::new(HashMap::new())),
            connected_listen_addrs: Arc::new(RwLock::new(HashSet::new())),
            writer_to_listen: Arc::new(RwLock::new(HashMap::new())),
            connecting: Arc::new(RwLock::new(HashSet::new())),
            banned_peers: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Ban an IP address with exponential backoff.
    /// Duration = min(2^ban_count seconds, 3600 seconds).
    /// Each subsequent ban doubles the duration, up to 1 hour max.
    pub async fn ban_peer(&self, ip: IpAddr) {
        let mut bans = self.banned_peers.write().await;
        let ban_count = bans.get(&ip).map(|e| e.ban_count.saturating_add(1)).unwrap_or(1);
        let duration_secs = (1u64 << ban_count.min(31)).min(MAX_BAN_DURATION_SECS);
        let banned_until = Instant::now() + std::time::Duration::from_secs(duration_secs);
        info!("Banning peer {} for {}s (ban #{}).", ip, duration_secs, ban_count);
        bans.insert(ip, BanEntry { banned_until, ban_count });
    }

    /// Check if an IP is currently banned. Returns true if banned and ban has not expired.
    pub async fn is_banned(&self, ip: IpAddr) -> bool {
        let bans = self.banned_peers.read().await;
        if let Some(entry) = bans.get(&ip) {
            Instant::now() < entry.banned_until
        } else {
            false
        }
    }

    /// Remove expired bans to free memory. Called periodically from heartbeat.
    pub async fn cleanup_expired_bans(&self) {
        let mut bans = self.banned_peers.write().await;
        let now = Instant::now();
        let before = bans.len();
        bans.retain(|_, entry| now < entry.banned_until);
        let removed = before - bans.len();
        if removed > 0 {
            debug!("Cleaned up {} expired bans, {} remaining", removed, bans.len());
        }
    }

    /// Get the number of connected peers.
    pub async fn peer_count(&self) -> usize {
        self.writers.read().await.len()
    }

    /// Get the number of banned IPs.
    pub async fn ban_count(&self) -> usize {
        self.banned_peers.read().await.len()
    }

    /// Atomically mark an address as "connecting" to prevent duplicate connection attempts.
    /// Returns true if the address was not already being connected to (caller should proceed).
    /// Returns false if another task is already connecting (caller should skip).
    pub async fn start_connecting(&self, addr: &str) -> bool {
        let mut set = self.connecting.write().await;
        set.insert(addr.to_string())
    }

    /// Clear the "connecting" flag for an address after connection attempt completes.
    pub async fn finish_connecting(&self, addr: &str) {
        self.connecting.write().await.remove(addr);
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
        let (_, writer) = split_connection(server_stream, addr.clone(), None);

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

    #[tokio::test]
    async fn ban_peer_and_check() {
        let reg = PeerRegistry::new();
        let ip: IpAddr = "192.168.1.100".parse().unwrap();
        assert!(!reg.is_banned(ip).await);
        reg.ban_peer(ip).await;
        assert!(reg.is_banned(ip).await);
    }

    #[tokio::test]
    async fn ban_peer_exponential_backoff() {
        let reg = PeerRegistry::new();
        let ip: IpAddr = "10.0.0.1".parse().unwrap();
        // First ban: 2^1 = 2s
        reg.ban_peer(ip).await;
        // Second ban: 2^2 = 4s
        reg.ban_peer(ip).await;
        {
            let bans = reg.banned_peers.read().await;
            let entry = bans.get(&ip).unwrap();
            assert_eq!(entry.ban_count, 2);
        }
        assert!(reg.is_banned(ip).await);
    }

    #[tokio::test]
    async fn cleanup_expired_bans() {
        let reg = PeerRegistry::new();
        let ip: IpAddr = "10.0.0.2".parse().unwrap();
        // Manually insert an already-expired ban
        {
            let mut bans = reg.banned_peers.write().await;
            bans.insert(ip, super::BanEntry {
                banned_until: Instant::now() - std::time::Duration::from_secs(1),
                ban_count: 1,
            });
        }
        assert!(!reg.is_banned(ip).await);
        reg.cleanup_expired_bans().await;
        let bans = reg.banned_peers.read().await;
        assert!(bans.is_empty());
    }

    #[tokio::test]
    async fn ban_duration_capped_at_one_hour() {
        let reg = PeerRegistry::new();
        let ip: IpAddr = "10.0.0.3".parse().unwrap();
        // Simulate many bans to reach the cap
        for _ in 0..40 {
            reg.ban_peer(ip).await;
        }
        let bans = reg.banned_peers.read().await;
        let entry = bans.get(&ip).unwrap();
        // Duration should be capped at 3600s even with high ban_count
        let max_duration = std::time::Duration::from_secs(super::MAX_BAN_DURATION_SECS);
        let time_remaining = entry.banned_until.duration_since(Instant::now());
        assert!(time_remaining <= max_duration);
    }
}
