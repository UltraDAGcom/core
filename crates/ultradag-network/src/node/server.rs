use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpListener;
use tokio::sync::{broadcast, Mutex, Notify, RwLock};
use tokio::time::Instant;
use tracing::{debug, info, warn};

use std::path::PathBuf;

use ultradag_coin::{BlockDag, DagVertex, FinalityTracker, Mempool, SecretKey, StateEngine, Transaction, sync_epoch_validators};
use ultradag_coin::consensus::dag::{DagInsertError, MAX_PARENTS};
use ultradag_coin::persistence::wal::FinalityWal;

use crate::peer::{split_connection, PeerReader, PeerRegistry};
use crate::protocol::Message;

/// Expected protocol version for Hello handshake.
const PROTOCOL_VERSION: u32 = 1;

/// Maximum number of connections allowed from a single IP address.
#[allow(dead_code)]
const MAX_CONNECTIONS_PER_IP: usize = 3;

/// Maximum number of suffix vertices to include in a GetCheckpoint response.
const MAX_CHECKPOINT_SUFFIX_VERTICES: usize = 500;

/// Maximum orphan buffer size in bytes (50MB).
const MAX_ORPHAN_BYTES: usize = 50 * 1024 * 1024;

/// Estimate the byte size of a DagVertex for orphan buffer accounting.
fn estimate_vertex_size(v: &DagVertex) -> usize {
    // 32 (hash) + block header (~100) + coinbase (~80) + txs + parents + round/validator/sig
    let tx_size: usize = v.block.transactions.len() * 300; // ~300 bytes per tx estimate
    let parent_size = v.parent_hashes.len() * 32;
    200 + tx_size + parent_size + 32 + 64 // base + txs + parents + pubkey + signature
}

/// Estimate total byte size of the orphan buffer.
fn orphan_buffer_bytes(orphans: &HashMap<[u8; 32], DagVertex>) -> usize {
    orphans.values().map(estimate_vertex_size).sum()
}

/// The P2P node server.
pub struct NodeServer {
    pub port: u16,
    pub state: Arc<RwLock<StateEngine>>,
    pub mempool: Arc<RwLock<Mempool>>,
    pub dag: Arc<RwLock<BlockDag>>,
    pub finality: Arc<RwLock<FinalityTracker>>,
    pub peers: PeerRegistry,
    pub vertex_tx: broadcast::Sender<DagVertex>,
    pub tx_tx: broadcast::Sender<Transaction>,
    /// Orphan vertices waiting for missing parents (P2P layer buffering).
    pub orphans: Arc<Mutex<HashMap<[u8; 32], DagVertex>>>,
    /// Notified when a new DAG vertex is inserted (for optimistic responsiveness).
    pub round_notify: Arc<Notify>,
    /// Pending checkpoints waiting for quorum signatures.
    pub pending_checkpoints: Arc<RwLock<HashMap<u64, ultradag_coin::Checkpoint>>>,
    /// Whether initial sync is complete.
    pub sync_complete: Arc<std::sync::atomic::AtomicBool>,
    /// Data directory for checkpoint persistence.
    pub data_dir: PathBuf,
    /// Validator secret key for co-signing checkpoints (None for observer nodes).
    pub validator_sk: Option<SecretKey>,
    /// Peers banned for sending too many rejected vertices. Maps IP → unban time.
    pub banned_peers: Arc<Mutex<HashMap<String, Instant>>>,
    /// Seed/bootstrap addresses for reconnection after peer loss.
    pub seed_addrs: Arc<Vec<String>>,
    /// Metrics for checkpoint production and synchronization.
    pub checkpoint_metrics: Arc<crate::CheckpointMetrics>,
    /// Write-ahead log for finalized vertex batches (crash recovery).
    pub wal: Arc<std::sync::Mutex<Option<FinalityWal>>>,
    /// Custom pruning depth (0 = archive mode, no pruning).
    pub pruning_depth: u64,
    /// Highest round reported by any peer via Hello/HelloAck.
    /// Used by fast-sync task to determine if we're caught up.
    pub peer_max_round: Arc<std::sync::atomic::AtomicU64>,
}

impl NodeServer {
    pub fn new(port: u16) -> Self {
        let (vertex_tx, _) = broadcast::channel(100);
        let (tx_tx, _) = broadcast::channel(1000);

        Self {
            port,
            state: Arc::new(RwLock::new(StateEngine::new_with_genesis())),
            mempool: Arc::new(RwLock::new(Mempool::new())),
            dag: Arc::new(RwLock::new(BlockDag::new())),
            // min_validators=1 for testnet - allows finality with any number of active validators
            // For mainnet, should be 4 (MIN_ACTIVE_VALIDATORS) to enforce BFT safety
            finality: Arc::new(RwLock::new(FinalityTracker::new(1))),
            peers: PeerRegistry::new(),
            vertex_tx,
            tx_tx,
            orphans: Arc::new(Mutex::new(HashMap::new())),
            round_notify: Arc::new(Notify::new()),
            pending_checkpoints: Arc::new(RwLock::new(HashMap::new())),
            sync_complete: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            data_dir: PathBuf::from("."),
            validator_sk: None,
            banned_peers: Arc::new(Mutex::new(HashMap::new())),
            seed_addrs: Arc::new(Vec::new()),
            checkpoint_metrics: Arc::new(crate::CheckpointMetrics::new()),
            wal: Arc::new(std::sync::Mutex::new(None)),
            pruning_depth: 1000,
            peer_max_round: Arc::new(std::sync::atomic::AtomicU64::new(0)),
        }
    }

    /// Set the data directory for checkpoint persistence.
    pub fn set_data_dir(&mut self, dir: PathBuf) {
        self.data_dir = dir;
    }

    /// Set the validator keypair for checkpoint co-signing.
    pub fn set_validator_sk(&mut self, sk: SecretKey) {
        self.validator_sk = Some(sk);
    }

    /// Set seed/bootstrap addresses for automatic reconnection.
    pub fn set_seed_addrs(&mut self, addrs: Vec<String>) {
        self.seed_addrs = Arc::new(addrs);
    }

    /// Initialize the write-ahead log for crash recovery.
    pub fn set_wal(&self, wal: FinalityWal) {
        *self.wal.lock().unwrap() = Some(wal);
    }

    /// Append a finalized vertex batch to the WAL. Non-fatal on failure.
    pub fn wal_append(&self, vertices: &[DagVertex], finalized_round: u64, state_root: [u8; 32]) {
        if let Ok(mut guard) = self.wal.lock() {
            if let Some(ref mut wal) = *guard {
                if let Err(e) = wal.append(vertices, finalized_round, state_root) {
                    tracing::warn!("WAL append failed: {} (crash recovery degraded)", e);
                }
            }
        }
    }

    /// Truncate the WAL after a successful full snapshot.
    pub fn wal_truncate(&self, snapshot_round: u64, state_root: [u8; 32]) {
        if let Ok(mut guard) = self.wal.lock() {
            if let Some(ref mut wal) = *guard {
                if let Err(e) = wal.truncate_after_snapshot(snapshot_round, state_root) {
                    tracing::warn!("WAL truncate failed: {}", e);
                }
            }
        }
    }

    /// Attempt fast-sync from a connected peer using checkpoint protocol.
    /// Sends GetCheckpoint to all connected peers. The CheckpointSync response
    /// is handled by the normal message loop (handle_peer).
    pub async fn request_fast_sync(&self) {
        let our_round = self.dag.read().await.current_round();
        info!("Requesting fast-sync from peers (our round: {})", our_round);
        self.peers.broadcast(&Message::GetCheckpoint { min_round: our_round }, "").await;
    }

    /// Maximum number of inbound peer connections to prevent resource exhaustion.
    /// Must be high enough for all validators (inbound + outbound) plus some observers.
    const MAX_INBOUND_PEERS: usize = 16;

    /// Check if an address refers to the local node (self-connection).
    /// Compares against known local addresses: loopback, 0.0.0.0, and the
    /// Fly.io `.internal` hostname derived from FLY_APP_NAME.
    pub fn is_self_address(&self, addr: &str) -> bool {
        // Check loopback and wildcard variants
        let loopback_addrs = [
            format!("127.0.0.1:{}", self.port),
            format!("0.0.0.0:{}", self.port),
            format!("[::1]:{}", self.port),
            format!("[::]:{}", self.port),
            format!("localhost:{}", self.port),
        ];
        for self_addr in &loopback_addrs {
            if addr == self_addr {
                return true;
            }
        }

        // Check Fly.io .internal hostname (e.g. ultradag-node-1.internal:9333)
        if let Ok(app_name) = std::env::var("FLY_APP_NAME") {
            let internal_addr = format!("{}.internal:{}", app_name, self.port);
            if addr == internal_addr {
                return true;
            }
        }

        // Check system hostname
        if let Ok(hostname) = hostname::get() {
            if let Some(hostname_str) = hostname.to_str() {
                if addr == format!("{}:{}", hostname_str, self.port) {
                    return true;
                }
            }
        }

        false
    }

    /// Start listening for incoming connections.
    pub async fn listen(&self) -> std::io::Result<()> {
        let listener = TcpListener::bind(format!("[::]:{}", self.port)).await?;
        info!("Listening on port {}", self.port);

        loop {
            let (stream, addr) = listener.accept().await?;
            let addr_str = addr.to_string();

            // Reject self-connections (e.g. node connecting to its own listen port)
            if addr.ip().is_loopback() && addr.port() == self.port {
                info!("Skipping self-connection to {}", addr_str);
                drop(stream);
                continue;
            }

            // Reject connections beyond the peer limit
            let current_peers = self.peers.connected_count().await;
            if current_peers >= Self::MAX_INBOUND_PEERS {
                warn!("Rejecting connection from {} — peer limit ({}) reached", addr_str, Self::MAX_INBOUND_PEERS);
                drop(stream);
                continue;
            }

            // Deduplicate: reject if we already have a LIVE connection from the same IP.
            // Probe the existing writer with a Ping to verify it's alive first.
            let remote_ip = addr.ip().to_string();
            let existing_addrs = self.peers.connected_addrs().await;
            let matching_addr = existing_addrs.iter().find(|a| a.contains(&remote_ip)).cloned();
            if let Some(existing) = matching_addr {
                // Probe the existing writer — if it's dead, remove it and accept this connection
                let probe_result = tokio::time::timeout(
                    Duration::from_secs(2),
                    self.peers.send_to(&existing, &Message::Ping(0)),
                ).await;
                match probe_result {
                    Ok(Ok(())) => {
                        // Existing connection is alive — reject the new one
                        info!("Rejecting duplicate connection from {} — already connected via {}", addr_str, existing);
                        drop(stream);
                        continue;
                    }
                    _ => {
                        // Existing connection is dead — remove it and accept the new one
                        info!("Replacing dead connection {} with new connection from {}", existing, addr_str);
                        self.peers.remove_peer(&existing).await;
                    }
                }
            }

            // Check if this IP is banned
            {
                let banned = self.banned_peers.lock().await;
                if let Some(&until) = banned.get(&remote_ip) {
                    if Instant::now() < until {
                        drop(stream);
                        continue;
                    }
                }
            }

            info!("Incoming connection from {}", addr_str);

            let (reader, writer) = split_connection(stream, addr_str.clone());
            self.peers.add_writer(addr_str.clone(), writer).await;

            let state = self.state.clone();
            let mempool = self.mempool.clone();
            let dag = self.dag.clone();
            let finality = self.finality.clone();
            let peers = self.peers.clone();
            let vertex_tx = self.vertex_tx.clone();
            let tx_tx = self.tx_tx.clone();
            let orphans = self.orphans.clone();
            let round_notify = self.round_notify.clone();
            let pending_checkpoints = self.pending_checkpoints.clone();
            let data_dir = self.data_dir.clone();
            let validator_sk = self.validator_sk.clone();
            let banned_peers = self.banned_peers.clone();

            let listen_port = self.port;
            let checkpoint_metrics = self.checkpoint_metrics.clone();
            let wal = self.wal.clone();
            let sync_complete = self.sync_complete.clone();
            let peer_max_round = self.peer_max_round.clone();
            tokio::spawn(async move {
                // handle_peer may rename peer_addr via Hello; remove both keys on disconnect
                if let Err(e) = handle_peer(reader, &state, &mempool, &dag, &finality, &peers, &vertex_tx, &tx_tx, &orphans, listen_port, &round_notify, &pending_checkpoints, &data_dir, validator_sk.as_ref(), &banned_peers, &checkpoint_metrics, &wal, &sync_complete, &peer_max_round).await {
                    warn!("Peer {} disconnected: {}", addr_str, e);
                }
                // Remove by original ephemeral addr and any possible listen addr
                peers.remove_peer(&addr_str).await;
            });
        }
    }

    /// Spawn a background heartbeat task that pings all writers every 30 seconds
    /// and removes any that fail or timeout. This detects dead TCP connections
    /// whose kernel send buffers still accept writes (zombie writers).
    pub fn start_heartbeat(&self) {
        let peers = self.peers.clone();
        let seed_addrs = self.seed_addrs.clone();
        let listen_port = self.port;
        let dag = self.dag.clone();
        let state = self.state.clone();
        let mempool = self.mempool.clone();
        let finality = self.finality.clone();
        let vertex_tx = self.vertex_tx.clone();
        let tx_tx = self.tx_tx.clone();
        let orphans = self.orphans.clone();
        let round_notify = self.round_notify.clone();
        let pending_checkpoints = self.pending_checkpoints.clone();
        let data_dir = self.data_dir.clone();
        let validator_sk = self.validator_sk.clone();
        let banned_peers = self.banned_peers.clone();
        let checkpoint_metrics = self.checkpoint_metrics.clone();
        let wal = self.wal.clone();
        let sync_complete = self.sync_complete.clone();
        let peer_max_round = self.peer_max_round.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(30));
            // Consume first tick (fires immediately) — let seed connections establish first
            interval.tick().await;
            loop {
                interval.tick().await;
                let addrs = peers.connected_addrs().await;
                let mut dead = Vec::new();
                for addr in &addrs {
                    let result = tokio::time::timeout(
                        Duration::from_secs(3),
                        peers.send_to(addr, &Message::Ping(0)),
                    ).await;
                    match result {
                        Ok(Ok(())) => {}
                        _ => {
                            warn!("Heartbeat: peer {} is dead, removing", addr);
                            dead.push(addr.clone());
                        }
                    }
                }
                for addr in &dead {
                    peers.remove_peer(addr).await;
                    peers.remove_connected_listen_addr(addr).await;
                }
                if !dead.is_empty() {
                    let remaining = peers.connected_count().await;
                    info!("Heartbeat: removed {} dead peers, {} remaining", dead.len(), remaining);
                }

                // Reconnect to seeds if peer count is low
                let peer_count = peers.connected_count().await;
                if peer_count < 3 && !seed_addrs.is_empty() {
                    info!("Heartbeat: low peer count ({}), reconnecting to seeds...", peer_count);
                    for addr in seed_addrs.iter() {
                        tokio::spawn(try_connect_peer(
                            addr.clone(),
                            listen_port,
                            dag.clone(),
                            peers.clone(),
                            state.clone(),
                            mempool.clone(),
                            finality.clone(),
                            vertex_tx.clone(),
                            tx_tx.clone(),
                            orphans.clone(),
                            round_notify.clone(),
                            pending_checkpoints.clone(),
                            data_dir.clone(),
                            validator_sk.clone(),
                            banned_peers.clone(),
                            checkpoint_metrics.clone(),
                            wal.clone(),
                            sync_complete.clone(),
                            peer_max_round.clone(),
                        ));
                    }
                }
            }
        });
    }

    /// Connect to a seed peer.
    pub async fn connect_to(&self, addr: &str) -> std::io::Result<()> {
        if self.is_self_address(addr) {
            info!("Skipping self-connection to {}", addr);
            return Ok(());
        }

        // Check if we already have a live connection to this listen address.
        // Without this check, a racing reconnection overwrites the writer in
        // PeerRegistry; when the OLD handle_peer exits, it removes the NEW
        // writer, breaking the new connection. This was the #1 cause of
        // cascading network stalls.
        if self.peers.is_listen_addr_connected(addr).await {
            return Ok(());
        }
        if self.peers.send_to(addr, &Message::Ping(0)).await.is_ok() {
            return Ok(());
        }

        let stream = tokio::net::TcpStream::connect(addr).await?;
        let addr_str = addr.to_string();

        // Check if the resolved remote IP is our own (catches DNS-based self-connections)
        if let Ok(remote) = stream.peer_addr() {
            if let Ok(local) = stream.local_addr() {
                if remote.ip() == local.ip() && remote.port() == self.port {
                    info!("Skipping self-connection to {}", addr);
                    return Ok(());
                }
            }
        }

        info!("Connected to {}", addr_str);

        self.peers.add_known(addr_str.clone()).await;
        self.peers.add_connected_listen_addr(addr_str.clone()).await;

        let (reader, writer) = split_connection(stream, addr_str.clone());
        self.peers.add_writer(addr_str.clone(), writer).await;

        // Send hello with current DAG round
        let current_round = self.dag.read().await.current_round();
        self.peers
            .send_to(&addr_str, &Message::Hello {
                version: 1,
                height: current_round,
                listen_port: self.port,
            })
            .await?;

        let state = self.state.clone();
        let mempool = self.mempool.clone();
        let dag = self.dag.clone();
        let finality = self.finality.clone();
        let peers = self.peers.clone();
        let vertex_tx = self.vertex_tx.clone();
        let tx_tx = self.tx_tx.clone();
        let orphans = self.orphans.clone();
        let round_notify = self.round_notify.clone();
        let pending_checkpoints = self.pending_checkpoints.clone();
        let data_dir = self.data_dir.clone();
        let validator_sk = self.validator_sk.clone();
        let banned_peers = self.banned_peers.clone();
        let listen_port = self.port;
        let checkpoint_metrics = self.checkpoint_metrics.clone();
        let wal = self.wal.clone();
        let sync_complete = self.sync_complete.clone();
        let peer_max_round = self.peer_max_round.clone();

        tokio::spawn(async move {
            if let Err(e) = handle_peer(reader, &state, &mempool, &dag, &finality, &peers, &vertex_tx, &tx_tx, &orphans, listen_port, &round_notify, &pending_checkpoints, &data_dir, validator_sk.as_ref(), &banned_peers, &checkpoint_metrics, &wal, &sync_complete, &peer_max_round).await {
                warn!("Peer {} disconnected: {}", addr_str, e);
            }
            peers.remove_peer(&addr_str).await;
            peers.remove_connected_listen_addr(&addr_str).await;
        });

        Ok(())
    }
}

/// Try to insert orphaned vertices whose parents may now exist.
/// Returns hashes of parents still missing (for further fetching).
async fn resolve_orphans(
    orphans: &Arc<Mutex<HashMap<[u8; 32], DagVertex>>>,
    dag: &Arc<RwLock<BlockDag>>,
    finality: &Arc<RwLock<FinalityTracker>>,
    state: &Arc<RwLock<StateEngine>>,
    mempool: &Arc<RwLock<Mempool>>,
    peers: &PeerRegistry,
    peer_addr: &str,
    _round_notify: &Arc<Notify>,
    wal: &Arc<std::sync::Mutex<Option<FinalityWal>>>,
) {
    let mut resolved = true;
    while resolved {
        resolved = false;
        let candidates: Vec<([u8; 32], DagVertex)> = {
            let orph = orphans.lock().await;
            orph.iter().map(|(h, v)| (*h, v.clone())).collect()
        };
        for (hash, vertex) in candidates {
            let result = {
                let mut dag_w = dag.write().await;
                dag_w.try_insert(vertex.clone())
            };
            match result {
                Ok(true) => {
                    orphans.lock().await.remove(&hash);
                    resolved = true;
                    // Don't notify here — orphan resolution is bulk context.
                    // The single-vertex DagProposal handler already notifies.

                    let validator = vertex.validator;
                    // Register validator + check finality (multi-pass)
                    // Lock ordering: finality+dag → drop both → state (matches DagProposal handler)
                    let (all_finalized, finalized_vertices) = {
                        let mut fin = finality.write().await;
                        fin.register_validator(validator);
                        let dag_r = dag.read().await;

                        let mut all_finalized = Vec::new();
                        loop {
                            let newly_finalized = fin.find_newly_finalized(&dag_r);
                            if newly_finalized.is_empty() {
                                break;
                            }
                            all_finalized.extend(newly_finalized);
                        }

                        let finalized_vertices: Vec<DagVertex> = all_finalized
                            .iter()
                            .filter_map(|h| dag_r.get(h).cloned())
                            .collect();
                        // Drop finality + dag locks before state application
                        (all_finalized, finalized_vertices)
                    };

                    if !all_finalized.is_empty() {
                        info!("Orphan resolve: finalized {} vertices", all_finalized.len());
                        let epoch_changed;
                        {
                            let mut state_w = state.write().await;
                            let prev_round = state_w.last_finalized_round();
                            if let Err(e) = state_w.apply_finalized_vertices(&finalized_vertices) {
                                warn!("Failed to apply finalized vertices: {}", e);
                                epoch_changed = false;
                            } else {
                                epoch_changed = state_w.epoch_just_changed(prev_round);
                                // WAL: log finalized vertices for crash recovery
                                let fin_round = state_w.last_finalized_round().unwrap_or(0);
                                let sr = ultradag_coin::consensus::compute_state_root(&state_w.snapshot());
                                if let Ok(mut wg) = wal.lock() {
                                    if let Some(ref mut w) = *wg {
                                        let finalized_vec: Vec<DagVertex> = finalized_vertices.clone();
                                        if let Err(e) = w.append(&finalized_vec, fin_round, sr) {
                                            warn!("WAL append failed: {}", e);
                                        }
                                    }
                                }
                                let mut mp = mempool.write().await;
                                for v in &finalized_vertices {
                                    for tx in &v.block.transactions {
                                        mp.remove(&tx.hash());
                                    }
                                }
                            }
                        } // state_w dropped here

                        // Epoch transition: acquire finality AFTER dropping state
                        if epoch_changed {
                            let mut fin = finality.write().await;
                            let state_r = state.read().await;
                            sync_epoch_validators(&mut fin, &state_r);
                            info!("Epoch transition to epoch {} — active set: {} validators",
                                state_r.current_epoch(), state_r.active_validators().len());
                        }
                    }
                    peers.broadcast(&Message::DagProposal(vertex), peer_addr).await;
                }
                Err(DagInsertError::MissingParents(missing)) => {
                    // Still missing parents — request them from peer
                    let hashes: Vec<[u8; 32]> = missing.into_iter().take(32).collect();
                    let _ = peers.send_to(peer_addr, &Message::GetParents { hashes }).await;
                }
                _ => {
                    // Equivocation or duplicate — remove from orphan buffer
                    orphans.lock().await.remove(&hash);
                }
            }
        }
    }
}

/// Maximum number of outbound peer connections.
const MAX_OUTBOUND_PEERS: usize = 8;

/// Connect to a peer address if not already connected.
/// Establishes a TCP connection, sends Hello, and keeps a drain loop
/// for the reader so the connection stays alive.
async fn try_connect_peer(
    addr: String,
    listen_port: u16,
    dag: Arc<RwLock<BlockDag>>,
    peers: PeerRegistry,
    state: Arc<RwLock<StateEngine>>,
    mempool: Arc<RwLock<Mempool>>,
    finality: Arc<RwLock<FinalityTracker>>,
    vertex_tx: broadcast::Sender<DagVertex>,
    tx_tx: broadcast::Sender<Transaction>,
    orphans: Arc<Mutex<HashMap<[u8; 32], DagVertex>>>,
    round_notify: Arc<Notify>,
    pending_checkpoints: Arc<RwLock<HashMap<u64, ultradag_coin::Checkpoint>>>,
    data_dir: std::path::PathBuf,
    validator_sk: Option<SecretKey>,
    banned_peers: Arc<Mutex<HashMap<String, Instant>>>,
    checkpoint_metrics: Arc<crate::CheckpointMetrics>,
    wal: Arc<std::sync::Mutex<Option<FinalityWal>>>,
    sync_complete: Arc<std::sync::atomic::AtomicBool>,
    peer_max_round: Arc<std::sync::atomic::AtomicU64>,
) {
    // Don't connect to ourselves — check loopback, wildcard, .internal hostname
    let loopback_addrs = [
        format!("127.0.0.1:{}", listen_port),
        format!("0.0.0.0:{}", listen_port),
        format!("[::1]:{}", listen_port),
        format!("[::]:{}", listen_port),
        format!("localhost:{}", listen_port),
    ];
    for self_addr in &loopback_addrs {
        if addr == *self_addr {
            info!("Skipping self-connection to {}", addr);
            return;
        }
    }
    // Check Fly.io .internal hostname
    if let Ok(app_name) = std::env::var("FLY_APP_NAME") {
        let internal_addr = format!("{}.internal:{}", app_name, listen_port);
        if addr == internal_addr {
            info!("Skipping self-connection to {}", addr);
            return;
        }
    }
    // Check system hostname
    if let Ok(hostname) = hostname::get() {
        if let Some(hostname_str) = hostname.to_str() {
            if addr == format!("{}:{}", hostname_str, listen_port) {
                info!("Skipping self-connection to {}", addr);
                return;
            }
        }
    }

    // Check if already at max peers
    if peers.connected_count().await >= MAX_OUTBOUND_PEERS {
        return;
    }

    // Check if we already have a connection to this listen address
    if peers.is_listen_addr_connected(&addr).await {
        return;
    }

    // Also check if we have a direct writer (outbound connection)
    if peers.send_to(&addr, &Message::Ping(0)).await.is_ok() {
        return;
    }

    // Resolve DNS to IP and check for self-connection or duplicate
    if let Ok(mut resolved) = tokio::net::lookup_host(&addr).await {
        if let Some(socket_addr) = resolved.next() {
            // Self-connection check: if DNS resolves to loopback with our port
            if socket_addr.ip().is_loopback() && socket_addr.port() == listen_port {
                info!("Skipping self-connection to {}", addr);
                return;
            }
            // Duplicate connection check
            let resolved_ip = socket_addr.ip().to_string();
            let existing = peers.connected_addrs().await;
            for existing_addr in &existing {
                if existing_addr.contains(&resolved_ip) {
                    return; // Already connected to this IP
                }
            }
        }
    }

    match tokio::net::TcpStream::connect(&addr).await {
        Ok(stream) => {
            // After connecting, check if we connected to ourselves or a duplicate
            if let Ok(remote) = stream.peer_addr() {
                // Self-connection: remote IP matches local IP and remote port is our listen port
                if let Ok(local) = stream.local_addr() {
                    if remote.ip() == local.ip() && remote.port() == listen_port {
                        info!("Skipping self-connection to {}", addr);
                        return;
                    }
                }
                let remote_ip = remote.ip().to_string();
                let existing = peers.connected_addrs().await;
                for existing_addr in &existing {
                    if existing_addr.contains(&remote_ip) {
                        info!("Peer discovery: skipping {} — already connected to IP {}", addr, remote_ip);
                        return;
                    }
                }
            }

            // Re-check: another task may have connected in the meantime
            if peers.is_listen_addr_connected(&addr).await {
                drop(stream);
                return;
            }

            info!("Peer discovery: connected to {}", addr);
            peers.add_known(addr.clone()).await;
            peers.add_connected_listen_addr(addr.clone()).await;

            let (reader, writer) = split_connection(stream, addr.clone());
            peers.add_writer(addr.clone(), writer).await;

            // Send hello so the remote knows our listen port
            let current_round = dag.read().await.current_round();
            let _ = peers
                .send_to(&addr, &Message::Hello {
                    version: 1,
                    height: current_round,
                    listen_port,
                })
                .await;

            // Process incoming messages (bidirectional connection).
            // Box::pin breaks the async opaque type cycle between try_connect_peer and handle_peer.
            let peers_clone = peers.clone();
            let addr_clone = addr.clone();
            tokio::spawn(async move {
                let fut = Box::pin(handle_peer(
                    reader, &state, &mempool, &dag, &finality, &peers_clone,
                    &vertex_tx, &tx_tx, &orphans, listen_port, &round_notify,
                    &pending_checkpoints, &data_dir, validator_sk.as_ref(),
                    &banned_peers, &checkpoint_metrics, &wal, &sync_complete,
                    &peer_max_round,
                ));
                if let Err(e) = fut.await {
                    warn!("Peer {} disconnected: {}", addr_clone, e);
                }
                peers_clone.remove_peer(&addr_clone).await;
                peers_clone.remove_connected_listen_addr(&addr_clone).await;
            });
        }
        Err(e) => {
            warn!("Peer discovery: failed to connect to {}: {}", addr, e);
        }
    }
}

/// Maximum allowlist rejections before logging stops (to avoid log spam).
#[allow(dead_code)]
const MAX_ALLOWLIST_REJECTIONS: u32 = 10;

async fn handle_peer(
    mut reader: PeerReader,
    state: &Arc<RwLock<StateEngine>>,
    mempool: &Arc<RwLock<Mempool>>,
    dag: &Arc<RwLock<BlockDag>>,
    finality: &Arc<RwLock<FinalityTracker>>,
    peers: &PeerRegistry,
    vertex_tx: &broadcast::Sender<DagVertex>,
    tx_tx: &broadcast::Sender<Transaction>,
    orphans: &Arc<Mutex<HashMap<[u8; 32], DagVertex>>>,
    _listen_port: u16,
    round_notify: &Arc<Notify>,
    pending_checkpoints: &Arc<RwLock<HashMap<u64, ultradag_coin::Checkpoint>>>,
    data_dir: &PathBuf,
    validator_sk: Option<&SecretKey>,
    banned_peers: &Arc<Mutex<HashMap<String, Instant>>>,
    checkpoint_metrics: &Arc<crate::CheckpointMetrics>,
    wal: &Arc<std::sync::Mutex<Option<FinalityWal>>>,
    sync_complete: &Arc<std::sync::atomic::AtomicBool>,
    peer_max_round: &Arc<std::sync::atomic::AtomicU64>,
) -> std::io::Result<()> {
    let peer_addr = reader.addr.clone();
    let mut allowlist_rejections: u32 = 0;

    loop {
        let msg = reader.recv().await?;

        // Yield to the runtime after each message so other tasks (RPC, validator loop)
        // get a chance to run. Without this, a burst of messages from one peer can
        // starve the entire runtime.
        tokio::task::yield_now().await;

        match msg {
            Message::Hello { version, height, listen_port } => {
                if version != PROTOCOL_VERSION {
                    warn!("Peer {} sent Hello with unsupported protocol version {} (expected {}), disconnecting",
                        peer_addr, version, PROTOCOL_VERSION);
                    return Ok(());
                }
                let our_round = dag.read().await.current_round();
                peers
                    .send_to(&peer_addr, &Message::HelloAck {
                        version: 1,
                        height: our_round,
                    })
                    .await?;

                // Track highest known peer round for sync progress detection
                peer_max_round.fetch_max(height, std::sync::atomic::Ordering::Relaxed);

                // If peer is ahead, sync from them
                if height > our_round {
                    let gap = height - our_round;
                    if gap > 100 {
                        // Large gap — try checkpoint AND incremental sync in parallel.
                        // Checkpoint may fail (no quorum-accepted checkpoints), so
                        // incremental sync provides fallback.
                        info!("Large gap ({} rounds behind peer {}), requesting checkpoint + incremental sync", gap, peer_addr);
                        peers.send_to(&peer_addr, &Message::GetCheckpoint { min_round: our_round }).await?;
                    }
                    // Always request incremental DAG vertices (works even when checkpoint fails)
                    peers
                        .send_to(&peer_addr, &Message::GetDagVertices {
                            from_round: our_round + 1,
                            max_count: 500,
                        })
                        .await?;
                }

                // Register peer's canonical listen address for discovery
                // Handle both IPv4 "1.2.3.4:port" and IPv6 "[::1]:port" formats
                let listen_addr = if peer_addr.starts_with('[') {
                    // IPv6: "[addr]:port" → extract addr part including brackets
                    if let Some(bracket_end) = peer_addr.find(']') {
                        let ip_part = &peer_addr[..=bracket_end];
                        Some(format!("{}:{}", ip_part, listen_port))
                    } else {
                        None
                    }
                } else {
                    // IPv4: "addr:port" → extract addr before last colon
                    peer_addr.rsplit_once(':').map(|(ip, _)| format!("{}:{}", ip, listen_port))
                };
                if let Some(listen_addr) = listen_addr {
                    peers.add_known(listen_addr.clone()).await;
                    // Also mark this as a connected listen address so
                    // try_connect_peer won't create duplicate connections
                    peers.add_connected_listen_addr(listen_addr.clone()).await;
                    // Link ephemeral writer key → canonical listen addr for dead peer cleanup
                    peers.link_writer_to_listen(peer_addr.to_string(), listen_addr).await;
                }

                // Request peer list for mesh discovery
                let _ = peers.send_to(&peer_addr, &Message::GetPeers).await;
            }

            Message::HelloAck { version, height } => {
                if version != PROTOCOL_VERSION {
                    warn!("Peer {} sent HelloAck with unsupported protocol version {} (expected {}), disconnecting",
                        peer_addr, version, PROTOCOL_VERSION);
                    return Ok(());
                }
                // Track highest known peer round for sync progress detection
                peer_max_round.fetch_max(height, std::sync::atomic::Ordering::Relaxed);

                let our_round = dag.read().await.current_round();
                if height > our_round {
                    let gap = height - our_round;
                    if gap > 100 {
                        info!("Large gap ({} rounds behind peer {}), requesting checkpoint + incremental sync", gap, peer_addr);
                        peers.send_to(&peer_addr, &Message::GetCheckpoint { min_round: our_round }).await?;
                    }
                    peers
                        .send_to(&peer_addr, &Message::GetDagVertices {
                            from_round: our_round + 1,
                            max_count: 500,
                        })
                        .await?;
                }
                // Request peer list for mesh discovery
                let _ = peers.send_to(&peer_addr, &Message::GetPeers).await;
            }

            Message::NewBlock(_block) => {
                // Blocks are no longer propagated separately - they're part of DAG vertices
                // This message type is deprecated in the new architecture
                warn!("Received deprecated NewBlock message from {}", peer_addr);
            }

            Message::NewTx(tx) => {
                // Verify signature before accepting into mempool to prevent
                // forged transactions from consuming mempool space and bandwidth.
                if !tx.verify_signature() {
                    warn!("Rejected NewTx with invalid signature from {}", peer_addr);
                } else {
                    let mut mp = mempool.write().await;
                    if mp.insert(tx.clone()) {
                        let _ = tx_tx.send(tx.clone());
                        drop(mp);
                        peers.broadcast(&Message::NewTx(tx), &peer_addr).await;
                    }
                }
            }

            Message::GetBlocks { .. } => {
                // Deprecated - we sync DAG vertices, not blocks
                warn!("Received deprecated GetBlocks message from {}", peer_addr);
            }

            Message::Blocks(_) => {
                // Deprecated - we sync DAG vertices, not blocks
                warn!("Received deprecated Blocks message from {}", peer_addr);
            }

            Message::DagProposal(vertex) => {
                // Verify Ed25519 signature before accepting
                if !vertex.verify_signature() {
                    warn!("Rejected DAG vertex with invalid signature from {}", peer_addr);
                    continue;
                }

                let vertex_hash = vertex.hash();
                let validator = vertex.validator;
                let round = vertex.round;

                // Reject vertices from validators not in our allowlist (if one is set).
                // Don't disconnect or ban — the peer may be an observer or syncing node
                // that also needs DAG sync over this same connection.
                {
                    let fin_r = finality.read().await;
                    if !fin_r.validator_set().is_allowed(&validator) {
                        allowlist_rejections += 1;
                        if allowlist_rejections == 1 {
                            warn!("Dropped vertex from non-allowlisted validator {}.. round={}", &validator.to_hex()[..8], round);
                        }
                        continue;
                    }
                }

                // Atomic equivocation check + insert (no TOCTOU race)
                let insert_result = {
                    let mut dag_w = dag.write().await;
                    dag_w.try_insert(vertex.clone())
                };

                match insert_result {
                    Err(DagInsertError::Equivocation { .. }) => {
                        warn!(
                            "Detected equivocation from validator {} in round {} (peer {})",
                            validator, round, peer_addr,
                        );
                        
                        // Execute slashing immediately
                        let mut state_w = state.write().await;
                        let stake_before = state_w.stake_of(&validator);
                        state_w.slash(&validator);
                        let stake_after = state_w.stake_of(&validator);
                        let burned = stake_before.saturating_sub(stake_after);
                        drop(state_w);
                        
                        warn!(
                            "Slashed validator {}: burned {} UDAG for equivocation (stake: {} -> {})",
                            validator.to_hex(),
                            burned as f64 / 100_000_000.0,
                            stake_before as f64 / 100_000_000.0,
                            stake_after as f64 / 100_000_000.0
                        );
                        
                        let dag_w = dag.read().await;
                        if let Some([hash1, hash2]) = dag_w.get_equivocation_evidence(&validator, round) {
                            if let (Some(v1), Some(v2)) = (dag_w.get_including_equivocations(&hash1), dag_w.get_including_equivocations(&hash2)) {
                                let evidence_msg = Message::EquivocationEvidence {
                                    vertex1: v1.clone(),
                                    vertex2: v2.clone(),
                                };
                                drop(dag_w);
                                peers.broadcast(&evidence_msg, "").await;
                            }
                        }
                        continue;
                    }
                    Err(DagInsertError::TooManyParents) => {
                        warn!("Rejected vertex from {} with too many parents (>{MAX_PARENTS})", peer_addr);
                        continue;
                    }
                    Err(DagInsertError::MissingParents(missing)) => {
                        warn!(
                            "Orphaned vertex {} round={} from {} — missing {} parents, buffering",
                            hex_short(&vertex_hash), round, peer_addr, missing.len(),
                        );
                        // Buffer as orphan and request missing parents from peer
                        {
                            let mut orph = orphans.lock().await;
                            if orph.len() < 1000 && orphan_buffer_bytes(&orph) < MAX_ORPHAN_BYTES {
                                orph.insert(vertex_hash, vertex);
                            }
                        }
                        // Request the missing parent vertices (cap at 32)
                        let hashes: Vec<[u8; 32]> = missing.into_iter().take(32).collect();
                        let _ = peers.send_to(&peer_addr, &Message::GetParents { hashes }).await;
                        continue;
                    }
                    Ok(false) => {
                        // Duplicate — ignore
                        continue;
                    }
                    Ok(true) => {
                        round_notify.notify_one();
                    }
                }

                // Vertex was inserted successfully
                {
                    info!(
                        "DAG vertex {} round={} from {}",
                        hex_short(&vertex_hash),
                        round,
                        peer_addr,
                    );

                    // Check finality (multi-pass for parent finality guarantee)
                    let (all_finalized, finalized_vertices) = {
                        let mut fin = finality.write().await;
                        fin.register_validator(validator);
                        let dag_r = dag.read().await;

                        let mut all_finalized = Vec::new();
                        loop {
                            let newly_finalized = fin.find_newly_finalized(&dag_r);
                            if newly_finalized.is_empty() {
                                break;
                            }
                            all_finalized.extend(newly_finalized);
                        }

                        let finalized_vertices: Vec<DagVertex> = all_finalized
                            .iter()
                            .filter_map(|h| dag_r.get(h).cloned())
                            .collect();
                        // Drop finality + dag locks before state application
                        (all_finalized, finalized_vertices)
                    };

                    if !all_finalized.is_empty() {
                        info!("DAG-BFT finalized {} vertices", all_finalized.len());

                        // Track whether epoch changed for finality sync below
                        let epoch_changed;
                        {
                            let mut state_w = state.write().await;
                            let prev_round = state_w.last_finalized_round();
                            if let Err(e) = state_w.apply_finalized_vertices(&finalized_vertices) {
                                warn!("Failed to apply finalized vertices to state: {}", e);
                                epoch_changed = false;
                            } else {
                                epoch_changed = state_w.epoch_just_changed(prev_round);
                                // WAL: log finalized vertices for crash recovery
                                let fin_round = state_w.last_finalized_round().unwrap_or(0);
                                let sr = ultradag_coin::consensus::compute_state_root(&state_w.snapshot());
                                if let Ok(mut wg) = wal.lock() {
                                    if let Some(ref mut w) = *wg {
                                        let finalized_vec: Vec<DagVertex> = finalized_vertices.clone();
                                        if let Err(e) = w.append(&finalized_vec, fin_round, sr) {
                                            warn!("WAL append failed: {}", e);
                                        }
                                    }
                                }
                                let mut mp = mempool.write().await;
                                for v in &finalized_vertices {
                                    for tx in &v.block.transactions {
                                        mp.remove(&tx.hash());
                                    }
                                }
                            }
                        } // state_w dropped here

                        // Epoch transition: acquire finality AFTER dropping state to prevent deadlock.
                        // All other code paths (resolve_orphans, DagVertices, validator loop) acquire
                        // finality before state — we must not hold state while acquiring finality.
                        if epoch_changed {
                            let mut fin = finality.write().await;
                            let state_r = state.read().await;
                            sync_epoch_validators(&mut fin, &state_r);
                            info!("Epoch transition to epoch {} — active set: {} validators",
                                state_r.current_epoch(), state_r.active_validators().len());
                        }
                    }

                    peers.broadcast(&Message::DagProposal(vertex), &peer_addr).await;

                    // Try to resolve orphaned vertices now that a new vertex was inserted
                    resolve_orphans(orphans, dag, finality, state, mempool, peers, &peer_addr, round_notify, wal).await;
                }
            }

            Message::GetDagVertices { from_round, max_count } => {
                // Use try_read to avoid blocking if DAG is locked by validator/sync
                let Some(dag_r) = dag.try_read().ok() else {
                    warn!("GetDagVertices: DAG lock contended, skipping for {}", peer_addr);
                    continue;
                };
                // Clamp from_round to pruning_floor — pruned rounds have no vertices.
                let floor = dag_r.pruning_floor();
                let effective_from = from_round.max(floor);
                // Cap max_count to prevent CPU exhaustion from huge range iterations
                let capped_count = (max_count as usize).min(500);
                let end_round = effective_from.saturating_add(capped_count as u64);
                let mut vertices = Vec::new();
                for round in effective_from..end_round {
                    for v in dag_r.vertices_in_round(round) {
                        vertices.push(v.clone());
                    }
                    if vertices.len() >= capped_count {
                        break;
                    }
                }
                drop(dag_r);
                peers.send_to(&peer_addr, &Message::DagVertices(vertices)).await?;
            }

            Message::GetRoundHashes { from_round, to_round } => {
                let Some(dag_r) = dag.try_read().ok() else {
                    continue; // DAG locked, skip — will be retried next cycle
                };
                let floor = dag_r.pruning_floor();
                let current = dag_r.current_round();
                let effective_from = from_round.max(floor);
                // Cap range to 1000 rounds to prevent abuse
                let effective_to = to_round.min(current).min(effective_from.saturating_add(1000));
                let mut rounds = Vec::new();
                for round in effective_from..=effective_to {
                    let hashes = dag_r.hashes_in_round(round);
                    if !hashes.is_empty() {
                        rounds.push((round, hashes.to_vec()));
                    }
                }
                drop(dag_r);
                peers.send_to(&peer_addr, &Message::RoundHashes { rounds }).await?;
            }

            Message::RoundHashes { rounds } => {
                let missing_hashes: Vec<[u8; 32]> = {
                    let dag_r = dag.read().await;
                    rounds.iter()
                        .flat_map(|(_, hashes)| hashes.iter())
                        .filter(|h| dag_r.get(*h).is_none())
                        .copied()
                        .collect()
                };
                if !missing_hashes.is_empty() {
                    info!("Hash gossip: {} missing vertices from {}, requesting", missing_hashes.len(), peer_addr);
                    // Request in batches of 32 (matching GetParents cap)
                    for chunk in missing_hashes.chunks(32) {
                        let _ = peers.send_to(&peer_addr, &Message::GetParents { hashes: chunk.to_vec() }).await;
                    }
                }
            }

            Message::DagVertices(vertices) => {
                let mut new_validators = Vec::new();
                let mut failed_vertices = Vec::new();
                let mut all_missing_parents: Vec<[u8; 32]> = Vec::new();
                // Filter out non-allowlisted vertices BEFORE acquiring dag write lock
                let filtered: Vec<DagVertex> = {
                    let fin_r = finality.read().await;
                    vertices.into_iter().filter(|v| {
                        v.verify_signature() && fin_r.validator_set().is_allowed(&v.validator)
                    }).collect()
                };
                {
                    let mut dag_w = dag.write().await;
                    let mut equivocation_msgs: Vec<Message> = Vec::new();
                    let mut equivocating_validators: Vec<ultradag_coin::Address> = Vec::new();
                    for vertex in filtered {
                        let validator = vertex.validator;
                        let hash = vertex.hash();
                        match dag_w.try_insert(vertex.clone()) {
                            Ok(true) => {
                                new_validators.push(validator);
                            }
                            Ok(false) => {
                                // Duplicate — ignore
                            }
                            Err(DagInsertError::MissingParents(missing)) => {
                                all_missing_parents.extend(&missing);
                                failed_vertices.push((hash, vertex));
                            }
                            Err(DagInsertError::TooManyParents) => {
                                // Silently reject
                            }
                            Err(DagInsertError::Equivocation { .. }) => {
                                warn!(
                                    "Equivocation in sync vertex from validator {} round {} (peer {})",
                                    validator, vertex.round, peer_addr,
                                );
                                equivocating_validators.push(validator);
                                // Collect equivocation evidence for broadcast after loop
                                // (don't break — remaining vertices may be legitimate)
                                if let Some([h1, h2]) = dag_w.get_equivocation_evidence(&validator, vertex.round) {
                                    if let (Some(v1), Some(v2)) = (dag_w.get_including_equivocations(&h1).cloned(), dag_w.get_including_equivocations(&h2).cloned()) {
                                        equivocation_msgs.push(Message::EquivocationEvidence {
                                            vertex1: v1,
                                            vertex2: v2,
                                        });
                                    }
                                }
                            }
                        }
                    }
                    drop(dag_w);
                    
                    // Execute slashing for all equivocating validators
                    if !equivocating_validators.is_empty() {
                        let mut state_w = state.write().await;
                        for validator in equivocating_validators {
                            let stake_before = state_w.stake_of(&validator);
                            state_w.slash(&validator);
                            let stake_after = state_w.stake_of(&validator);
                            let burned = stake_before.saturating_sub(stake_after);
                            
                            warn!(
                                "Slashed validator {}: burned {} UDAG for equivocation (stake: {} -> {})",
                                validator.to_hex(),
                                burned as f64 / 100_000_000.0,
                                stake_before as f64 / 100_000_000.0,
                                stake_after as f64 / 100_000_000.0
                            );
                        }
                        drop(state_w);
                    }
                    
                    // Broadcast equivocation evidence after loop
                    for msg in equivocation_msgs {
                        peers.broadcast(&msg, "").await;
                    }
                }
                // Buffer failed inserts as orphans (outside dag lock)
                if !failed_vertices.is_empty() {
                    let mut orph = orphans.lock().await;
                    for (hash, vertex) in failed_vertices {
                        if orph.len() < 1000 && orphan_buffer_bytes(&orph) < MAX_ORPHAN_BYTES {
                            orph.insert(hash, vertex);
                        }
                    }
                }
                // Request missing parent vertices from the peer
                if !all_missing_parents.is_empty() {
                    all_missing_parents.sort_unstable();
                    all_missing_parents.dedup();
                    let hashes: Vec<[u8; 32]> = all_missing_parents.into_iter().take(32).collect();
                    let _ = peers.send_to(&peer_addr, &Message::GetParents { hashes }).await;
                }
                if !new_validators.is_empty() {
                    // Don't notify here — DagVertices is bulk sync context.
                    // Multi-pass finality — drop finality+dag before state to avoid deadlock
                    let (all_finalized, finalized_vertices) = {
                        let mut fin = finality.write().await;
                        for v in &new_validators {
                            fin.register_validator(*v);
                        }
                        let dag_r = dag.read().await;
                        let mut all_finalized = Vec::new();
                        loop {
                            let newly_finalized = fin.find_newly_finalized(&dag_r);
                            if newly_finalized.is_empty() {
                                break;
                            }
                            all_finalized.extend(newly_finalized);
                        }
                        let finalized_vertices: Vec<DagVertex> = all_finalized
                            .iter()
                            .filter_map(|h| dag_r.get(h).cloned())
                            .collect();
                        (all_finalized, finalized_vertices)
                    }; // finality + dag locks dropped here
                    if !all_finalized.is_empty() {
                        info!("Sync finalized {} vertices", all_finalized.len());
                        // Apply finalized vertices and check for epoch change.
                        // IMPORTANT: drop state_w before acquiring finality.write()
                        // to maintain lock ordering (finality → state) and avoid deadlock.
                        let (epoch_changed, epoch_info) = {
                            let mut state_w = state.write().await;
                            let prev_round = state_w.last_finalized_round();
                            if let Err(e) = state_w.apply_finalized_vertices(&finalized_vertices) {
                                warn!("Failed to apply sync-finalized vertices: {}", e);
                                (false, None)
                            } else {
                                let changed = state_w.epoch_just_changed(prev_round);
                                let info = if changed {
                                    Some((state_w.current_epoch(), state_w.active_validators().len()))
                                } else {
                                    None
                                };
                                // WAL: log finalized vertices for crash recovery
                                let fin_round = state_w.last_finalized_round().unwrap_or(0);
                                let sr = ultradag_coin::consensus::compute_state_root(&state_w.snapshot());
                                if let Ok(mut wg) = wal.lock() {
                                    if let Some(ref mut w) = *wg {
                                        let finalized_vec: Vec<DagVertex> = finalized_vertices.clone();
                                        if let Err(e) = w.append(&finalized_vec, fin_round, sr) {
                                            warn!("WAL append failed: {}", e);
                                        }
                                    }
                                }
                                let mut mp = mempool.write().await;
                                for v in &finalized_vertices {
                                    for tx in &v.block.transactions {
                                        mp.remove(&tx.hash());
                                    }
                                }
                                (changed, info)
                            }
                        }; // state_w dropped here
                        if epoch_changed {
                            let mut fin = finality.write().await;
                            let state_r = state.read().await;
                            sync_epoch_validators(&mut fin, &state_r);
                            if let Some((epoch, count)) = epoch_info {
                                info!("Epoch transition to epoch {} — active set: {} validators",
                                    epoch, count);
                            }
                        }
                    }
                    resolve_orphans(orphans, dag, finality, state, mempool, peers, &peer_addr, round_notify, wal).await;
                }

                // Sync continuation: if we received new vertices, request the next batch.
                // This creates a pull-based sync loop until we catch up or get an empty batch.
                if !new_validators.is_empty() {
                    let our_round = dag.read().await.current_round();
                    // Don't set sync_complete here — let the fast-sync task in main.rs
                    // handle it after comparing our round against peers' heights.
                    // Setting it too early (e.g. our_round > 10) causes the validator
                    // to start producing while still hundreds of rounds behind.
                    let _ = peers.send_to(&peer_addr, &Message::GetDagVertices {
                        from_round: our_round + 1,
                        max_count: 100,
                    }).await;
                }
            }

            Message::GetPeers => {
                let mut known = peers.known_peers().await;
                known.truncate(100); // Cap response size to prevent topology leakage
                peers.send_to(&peer_addr, &Message::Peers(known)).await?;
            }

            Message::Peers(addrs) => {
                for addr in &addrs {
                    peers.add_known(addr.clone()).await;
                }
                // Connect to learned peers for mesh topology
                // Peer connections are handled by the heartbeat task which
                // reconnects to seeds when peer count is low. We just store
                // the addresses here — connecting from within handle_peer
                // would create an async type cycle.
            }

            Message::Ping(nonce) => {
                peers.send_to(&peer_addr, &Message::Pong(nonce)).await?;
            }

            Message::Pong(_) => {}

            Message::EquivocationEvidence { vertex1, vertex2 } => {
                // Process equivocation evidence from peer
                let mut dag_w = dag.write().await;
                let newly_marked = dag_w.process_equivocation_evidence(&vertex1, &vertex2);
                
                if newly_marked {
                    let validator_addr = vertex1.validator;
                    warn!(
                        "Marked validator {} as Byzantine due to equivocation in round {} (evidence from {})",
                        validator_addr, vertex1.round, peer_addr
                    );
                    
                    // Execute slashing immediately
                    drop(dag_w);
                    let mut state_w = state.write().await;
                    let stake_before = state_w.stake_of(&validator_addr);
                    state_w.slash(&validator_addr);
                    let stake_after = state_w.stake_of(&validator_addr);
                    let burned = stake_before.saturating_sub(stake_after);
                    drop(state_w);
                    
                    warn!(
                        "Slashed validator {}: burned {} UDAG for equivocation (stake: {} -> {})",
                        validator_addr.to_hex(),
                        burned as f64 / 100_000_000.0,
                        stake_before as f64 / 100_000_000.0,
                        stake_after as f64 / 100_000_000.0
                    );
                    
                    // Broadcast evidence to all other peers
                    let evidence_msg = Message::EquivocationEvidence {
                        vertex1: vertex1.clone(),
                        vertex2: vertex2.clone(),
                    };
                    peers.broadcast(&evidence_msg, &peer_addr).await;
                }
            }

            Message::GetParents { hashes } => {
                // Look up each requested hash in our DAG (cap at 32)
                let dag_r = dag.read().await;
                let vertices: Vec<DagVertex> = hashes
                    .iter()
                    .take(32)
                    .filter_map(|h| dag_r.get(h).cloned())
                    .collect();
                drop(dag_r);
                if !vertices.is_empty() {
                    let _ = peers.send_to(&peer_addr, &Message::ParentVertices { vertices }).await;
                }
            }

            Message::ParentVertices { vertices } => {
                // Received parent vertices we requested — insert and resolve orphans.
                let mut all_missing: Vec<[u8; 32]> = Vec::new();
                let mut inserted_any = false;

                // Filter out non-allowlisted vertices before processing
                let filtered: Vec<DagVertex> = {
                    let fin_r = finality.read().await;
                    vertices.into_iter().take(50).filter(|v| {
                        v.verify_signature() && fin_r.validator_set().is_allowed(&v.validator)
                    }).collect()
                };

                for vertex in filtered {
                    let validator = vertex.validator;
                    let hash = vertex.hash();
                    let insert_result = {
                        let mut dag_w = dag.write().await;
                        dag_w.try_insert(vertex.clone())
                    };
                    match insert_result {
                        Ok(true) => {
                            inserted_any = true;
                            let mut fin = finality.write().await;
                            fin.register_validator(validator);
                        }
                        Err(DagInsertError::MissingParents(missing)) => {
                            all_missing.extend(&missing);
                            let mut orph = orphans.lock().await;
                            if orph.len() < 1000 && orphan_buffer_bytes(&orph) < MAX_ORPHAN_BYTES {
                                orph.insert(hash, vertex);
                            }
                        }
                        _ => {}
                    }
                }

                // Request any still-missing parents (recursive resolution)
                if !all_missing.is_empty() {
                    all_missing.sort_unstable();
                    all_missing.dedup();
                    let dag_r = dag.read().await;
                    let still_missing: Vec<[u8; 32]> = all_missing
                        .into_iter()
                        .filter(|h| dag_r.get(h).is_none())
                        .take(32)
                        .collect();
                    drop(dag_r);
                    if !still_missing.is_empty() {
                        let _ = peers.send_to(&peer_addr, &Message::GetParents { hashes: still_missing }).await;
                    }
                }

                // If we inserted any parent vertices, try to resolve orphans
                if inserted_any {
                    // Don't notify here — ParentVertices is bulk sync context.
                    resolve_orphans(orphans, dag, finality, state, mempool, peers, &peer_addr, round_notify, wal).await;
                }
            }

            Message::CheckpointProposal(mut checkpoint) => {
                // 1. Verify the round is one we have finalized
                let fin = finality.read().await;
                let our_finalized = fin.last_finalized_round();
                drop(fin);

                if checkpoint.round > our_finalized {
                    // We haven't finalized this round yet — store for later
                    pending_checkpoints.write().await.insert(checkpoint.round, checkpoint);
                    continue;
                }

                // 2. Reject stale checkpoints — we've moved past this round,
                // our current state includes later vertices so state_root won't match
                if checkpoint.round < our_finalized {
                    debug!(
                        "Ignoring stale checkpoint for round {} (our finalized={})",
                        checkpoint.round, our_finalized
                    );
                    continue;
                }

                // checkpoint.round == our_finalized: verify state_root matches
                // We need to compute state at the checkpoint round, not current state
                // (which may have advanced beyond checkpoint.round)
                let state_r = state.read().await;
                let checkpoint_round_finalized = state_r.last_finalized_round().unwrap_or(0);
                
                // Only validate if we've applied state up to exactly this checkpoint round
                if checkpoint_round_finalized != checkpoint.round {
                    debug!(
                        "Checkpoint at round {} but our state is at round {} - skipping validation",
                        checkpoint.round, checkpoint_round_finalized
                    );
                    drop(state_r);
                    continue;
                }
                
                let our_snapshot = state_r.snapshot();
                let our_root = ultradag_coin::consensus::compute_state_root(&our_snapshot);
                drop(state_r);
                
                if our_root != checkpoint.state_root {
                    warn!(
                        "Checkpoint at round {} has mismatched state_root — possible fork",
                        checkpoint.round
                    );
                    checkpoint_metrics.record_checkpoint_validation_failure();
                    continue;
                }

                // 3. Verify the proposer's signature
                let valid_signers = checkpoint.valid_signers();
                if valid_signers.is_empty() {
                    warn!("Checkpoint proposal has no valid signatures");
                    checkpoint_metrics.record_checkpoint_validation_failure();
                    continue;
                }

                // 4. Co-sign with our validator key (if available)
                if let Some(sk) = validator_sk {
                    checkpoint.sign(sk);
                    // Broadcast our signature to peers
                    if let Some(sig) = checkpoint.signatures.last().cloned() {
                        peers.broadcast(&Message::CheckpointSignatureMsg {
                            round: checkpoint.round,
                            checkpoint_hash: checkpoint.checkpoint_hash(),
                            signature: sig,
                        }, &peer_addr).await;
                    }
                    
                    // Record co-signing metrics
                    checkpoint_metrics.record_checkpoint_cosigned(checkpoint.signatures.len() as u64);
                }

                // 5. Store as pending (waiting for quorum), with eviction cap
                let round = checkpoint.round;
                let mut pending = pending_checkpoints.write().await;
                pending.insert(round, checkpoint);
                // Evict oldest pending checkpoints if over cap
                while pending.len() > 10 {
                    if let Some(&oldest) = pending.keys().min() {
                        pending.remove(&oldest);
                    } else {
                        break;
                    }
                }
                checkpoint_metrics.update_pending_checkpoints_count(pending.len() as u64);
                drop(pending);

                info!("Received and co-signed checkpoint proposal for round {}", round);
            }

            Message::CheckpointSignatureMsg { round, checkpoint_hash, signature } => {
                // Verify signer is an active validator first
                let active = {
                    let state_r = state.read().await;
                    state_r.active_validators().to_vec()
                };
                if !active.contains(&signature.validator) {
                    continue;
                }
                
                // Find the pending checkpoint for this round
                let mut pending = pending_checkpoints.write().await;
                let checkpoint = match pending.get_mut(&round) {
                    Some(cp) => cp,
                    None => continue, // No pending checkpoint for this round
                };
                
                // Verify the hash matches
                if checkpoint.checkpoint_hash() != checkpoint_hash {
                    warn!("CheckpointSignature has wrong hash for round {}", round);
                    continue;
                }
                
                // Add signature if not duplicate
                let already_signed = checkpoint.signatures.iter()
                    .any(|s| s.validator == signature.validator);
                if !already_signed {
                    checkpoint.signatures.push(signature);
                }
                
                // Check if we have quorum
                let quorum = if active.is_empty() {
                    2 // Minimum for testing
                } else {
                    (active.len() * 2 + 2) / 3 // ceil(2n/3)
                };
                
                if checkpoint.is_accepted(&active, quorum) {
                    // Checkpoint accepted — save to disk
                    let accepted = checkpoint.clone();
                    drop(pending);

                    match ultradag_coin::persistence::save_checkpoint(data_dir, &accepted) {
                        Ok(_) => checkpoint_metrics.record_checkpoint_persist_success(),
                        Err(e) => {
                            warn!("Failed to save accepted checkpoint: {}", e);
                            checkpoint_metrics.record_checkpoint_persist_failure();
                        }
                    }
                    
                    // Record quorum achievement
                    checkpoint_metrics.record_checkpoint_quorum_reached();

                    info!(
                        "Checkpoint accepted at round {} with {} signatures",
                        accepted.round,
                        accepted.signatures.len()
                    );

                    // Clean up old pending checkpoints
                    let mut pending = pending_checkpoints.write().await;
                    let rounds: Vec<u64> = pending.keys().copied().collect();
                    for r in rounds.iter().filter(|&&r| r < accepted.round) {
                        pending.remove(r);
                    }
                    checkpoint_metrics.update_pending_checkpoints_count(pending.len() as u64);
                }
            }

            Message::GetCheckpoint { min_round } => {
                // Rate-limit checkpoint sync — it's expensive (locks DAG + state).
                // Use try_read to avoid blocking if locks are contended.
                let checkpoint = match ultradag_coin::persistence::load_latest_checkpoint(data_dir) {
                    Some(cp) if cp.round >= min_round => {
                        checkpoint_metrics.record_checkpoint_load_success();
                        Some(cp)
                    }
                    Some(_) => None, // Checkpoint too old
                    None => {
                        // Try pending checkpoints
                        match pending_checkpoints.try_read() {
                            Ok(pending) => pending.values()
                                .filter(|cp| cp.round >= min_round)
                                .max_by_key(|cp| cp.round)
                                .cloned(),
                            Err(_) => None,
                        }
                    }
                };

                if let Some(checkpoint) = checkpoint {
                    // Use try_read to avoid blocking — if locks are held, skip this request.
                    // The peer will retry and eventually get served.
                    let Some(dag_r) = dag.try_read().ok() else {
                        warn!("GetCheckpoint: DAG lock contended, skipping for {}", peer_addr);
                        continue;
                    };
                    let current_round = dag_r.current_round();
                    let mut suffix_vertices = Vec::new();
                    'outer: for r in checkpoint.round..=current_round {
                        for vertex in dag_r.vertices_in_round(r) {
                            suffix_vertices.push(vertex.clone());
                            if suffix_vertices.len() >= MAX_CHECKPOINT_SUFFIX_VERTICES {
                                break 'outer;
                            }
                        }
                    }
                    drop(dag_r);

                    // Load the state snapshot saved at checkpoint production time.
                    // Using current state would be wrong — it has advanced past the checkpoint round,
                    // so its state_root wouldn't match the checkpoint's state_root.
                    let state_snapshot = match ultradag_coin::persistence::load_checkpoint_state(data_dir, checkpoint.round) {
                        Some(snap) => snap,
                        None => {
                            // Fallback to current state if no saved snapshot (legacy checkpoints)
                            warn!("GetCheckpoint: no saved state for checkpoint round {}, using current state (may cause state_root mismatch)", checkpoint.round);
                            let Some(state_r) = state.try_read().ok() else {
                                warn!("GetCheckpoint: state lock contended, skipping for {}", peer_addr);
                                continue;
                            };
                            let snap = state_r.snapshot();
                            drop(state_r);
                            snap
                        }
                    };

                    let _ = peers.send_to(&peer_addr, &Message::CheckpointSync {
                        checkpoint,
                        suffix_vertices,
                        state_at_checkpoint: state_snapshot,
                    }).await;

                    info!("Sent checkpoint sync for round {} to {}", min_round, peer_addr);
                }
            }

            Message::CheckpointSync { checkpoint, suffix_vertices, state_at_checkpoint } => {
                checkpoint_metrics.record_fast_sync_attempt();
                let sync_start = std::time::Instant::now();
                
                // CRITICAL: Verify checkpoint chain links back to genesis
                // This prevents trust-on-first-use eclipse attacks where a malicious peer
                // feeds a forged checkpoint with fake validator set
                {
                    let dir = data_dir;
                    let checkpoint_loader = |hash: [u8; 32]| -> Option<ultradag_coin::Checkpoint> {
                        // Try to load checkpoint from disk by searching for matching hash
                        let checkpoints = ultradag_coin::persistence::list_checkpoints(dir);
                        for round in checkpoints {
                            if let Some(cp) = ultradag_coin::persistence::load_latest_checkpoint(dir) {
                                if cp.round == round {
                                    let cp_hash = ultradag_coin::consensus::compute_checkpoint_hash(&cp);
                                    if cp_hash == hash {
                                        return Some(cp);
                                    }
                                }
                            }
                        }
                        None
                    };
                    
                    match ultradag_coin::consensus::verify_checkpoint_chain(&checkpoint, checkpoint_loader) {
                        Ok(()) => {
                            info!("Checkpoint chain verification passed for round {}", checkpoint.round);
                        }
                        Err(e) => {
                            warn!("Checkpoint chain verification failed: {} — ignoring checkpoint, will use incremental sync", e);
                            checkpoint_metrics.record_fast_sync_failure();
                            // Don't disconnect — the peer is likely honest but we can't
                            // verify the chain (e.g. missing intermediate checkpoints).
                            // Incremental DAG sync via GetDagVertices still works.
                            continue;
                        }
                    }
                }
                
                // Fix 1: Use the checkpoint's own state snapshot for validator trust,
                // not the local state engine (which may be empty on fresh nodes).
                let checkpoint_state = ultradag_coin::StateEngine::from_snapshot(state_at_checkpoint.clone());
                let active = checkpoint_state.active_validators().to_vec();
                let quorum = if active.is_empty() {
                    // Pre-staking: accept if checkpoint has at least 2 valid signers
                    let valid_count = checkpoint.valid_signers().len();
                    if valid_count < 2 {
                        warn!("Received CheckpointSync with only {} valid signers (need ≥2)", valid_count);
                        checkpoint_metrics.record_fast_sync_failure();
                        continue;
                    }
                    2 // Pre-staking: minimum BFT quorum
                } else {
                    (active.len() * 2 + 2) / 3
                };

                if !active.is_empty() && !checkpoint.is_accepted(&active, quorum) {
                    warn!("Received CheckpointSync with insufficient signatures ({} valid, need {})",
                        checkpoint.valid_signers().len(), quorum);
                    checkpoint_metrics.record_fast_sync_failure();
                    continue;
                }

                // Verify state root
                let computed_root = ultradag_coin::consensus::compute_state_root(&state_at_checkpoint);
                if computed_root != checkpoint.state_root {
                    warn!("CheckpointSync state_root mismatch — rejecting");
                    checkpoint_metrics.record_fast_sync_failure();
                    continue;
                }

                // Calculate bytes before moving state_at_checkpoint
                let bytes_downloaded = serde_json::to_vec(&state_at_checkpoint)
                    .map(|v| v.len())
                    .unwrap_or(0) as u64;

                // Apply state snapshot
                {
                    let mut state_w = state.write().await;
                    state_w.load_snapshot(state_at_checkpoint);
                }

                // Clear mempool — old transactions may reference stale nonces/balances
                {
                    let mut mp = mempool.write().await;
                    let cleared = mp.len();
                    mp.clear();
                    if cleared > 0 {
                        info!("CheckpointSync: cleared {} stale mempool transactions", cleared);
                    }
                }

                // Clear orphan buffer — orphans from the old DAG reference pruned parents
                {
                    let mut orph = orphans.lock().await;
                    let cleared = orph.len();
                    orph.clear();
                    if cleared > 0 {
                        info!("CheckpointSync: cleared {} stale orphan vertices", cleared);
                    }
                }

                // Fix 3: Insert suffix vertices with signature verification
                let mut inserted = 0;
                let mut skipped = 0;
                {
                    let mut dag_w = dag.write().await;
                    // Set pruning floor to checkpoint round
                    dag_w.set_pruning_floor(checkpoint.round);
                    for vertex in suffix_vertices {
                        if !vertex.verify_signature() {
                            warn!("CheckpointSync: skipping suffix vertex with invalid signature (round {})", vertex.round);
                            skipped += 1;
                            continue;
                        }
                        if dag_w.try_insert(vertex).is_ok() {
                            inserted += 1;
                        }
                    }
                }

                // Fix 2: Reset FinalityTracker to reflect synced state
                {
                    let mut fin = finality.write().await;
                    fin.reset_to_checkpoint(checkpoint.round);
                    // Register validators from checkpoint state
                    for addr in &active {
                        fin.register_validator(*addr);
                    }
                    // Also register validators from suffix vertices
                    let dag_r = dag.read().await;
                    for addr in dag_r.all_validators() {
                        fin.register_validator(addr);
                    }
                }

                if skipped > 0 {
                    warn!("CheckpointSync: skipped {} vertices with invalid signatures", skipped);
                }
                
                // Record successful fast-sync metrics
                let sync_duration_ms = sync_start.elapsed().as_millis() as u64;
                checkpoint_metrics.record_fast_sync_success(sync_duration_ms, bytes_downloaded);
                checkpoint_metrics.record_checkpoint_load_success();
                
                info!(
                    "Fast-synced from checkpoint at round {}, inserted {} suffix vertices ({}ms, {} bytes)",
                    checkpoint.round,
                    inserted,
                    sync_duration_ms,
                    bytes_downloaded
                );
                sync_complete.store(true, std::sync::atomic::Ordering::Relaxed);
                info!("Sync complete — validator production enabled");
            }
        }
    }
}

fn hex_short(hash: &[u8; 32]) -> String {
    hash[..4].iter().map(|b| format!("{b:02x}")).collect()
}
