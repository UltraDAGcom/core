use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpListener;
use tokio::sync::{broadcast, Mutex, Notify, RwLock};
use tokio::time::Instant;
use tracing::{debug, error, info, warn};

use std::path::PathBuf;

use ultradag_coin::{Address, BlockDag, DagVertex, FinalityTracker, Mempool, SecretKey, StateEngine, Transaction, sync_epoch_validators};
use ultradag_coin::consensus::dag::{DagInsertError, MAX_PARENTS};

use crate::peer::{split_connection, PeerReader, PeerRegistry, handshake_initiator, handshake_responder};
use crate::peer::noise::HANDSHAKE_TIMEOUT_SECS;
use crate::protocol::Message;

/// Expected protocol version for Hello handshake.
const PROTOCOL_VERSION: u32 = 1;

/// Maximum number of connections allowed from a single IP address.
const MAX_CONNECTIONS_PER_IP: usize = 3;

/// Maximum number of suffix vertices to include in a GetCheckpoint response.
const MAX_CHECKPOINT_SUFFIX_VERTICES: usize = 500;

/// Maximum orphan buffer size in bytes (50MB).
const MAX_ORPHAN_BYTES: usize = 50 * 1024 * 1024;

/// Maximum number of orphan entries in the buffer.
const MAX_ORPHAN_ENTRIES: usize = 1000;

/// Maximum orphan entries from a single peer. Prevents one malicious peer
/// from filling the entire orphan buffer with deep dependency chains,
/// crowding out legitimate orphans from other peers.
const MAX_ORPHAN_ENTRIES_PER_PEER: usize = 100;

/// Maximum accounts in a CheckpointSync state snapshot. Prevents OOM
/// from a malicious peer sending an enormous fabricated snapshot.
const MAX_SNAPSHOT_ACCOUNTS: usize = 10_000_000;

/// Maximum proposals in a CheckpointSync state snapshot.
const MAX_SNAPSHOT_PROPOSALS: usize = 10_000;

/// Default batch size for DAG vertex sync requests.
const DAG_SYNC_BATCH_SIZE: u32 = 50;

/// Maximum passes through the orphan buffer in resolve_orphans.
/// Prevents DoS via long dependency chains causing extended loop execution.
const MAX_ORPHAN_RESOLUTION_PASSES: usize = 10;

/// Maximum peers to include in a GetPeers response.
const MAX_PEERS_RESPONSE: usize = 100;

/// Minimum connected peers before heartbeat triggers seed reconnection.
/// Must be above the validator production gate (MIN_PEERS_FOR_PRODUCTION in validator.rs = 2)
/// to ensure production doesn't stall waiting for the next heartbeat cycle.
const MIN_PEERS_FOR_RECONNECT: usize = 4;

/// Check if an address refers to the local node (self-connection).
/// Single implementation used by NodeServer::is_self_address and try_connect_peer.
fn is_self_addr(addr: &str, port: u16) -> bool {
    let loopback_addrs = [
        format!("127.0.0.1:{}", port),
        format!("0.0.0.0:{}", port),
        format!("[::1]:{}", port),
        format!("[::]:{}", port),
        format!("localhost:{}", port),
    ];
    for self_addr in &loopback_addrs {
        if addr == self_addr {
            return true;
        }
    }

    // Check Fly.io .internal hostname (e.g. ultradag-node-1.internal:9333)
    if let Ok(app_name) = std::env::var("FLY_APP_NAME") {
        let internal_addr = format!("{}.internal:{}", app_name, port);
        if addr == internal_addr {
            return true;
        }
    }

    // Check system hostname
    if let Ok(hostname) = hostname::get() {
        if let Some(hostname_str) = hostname.to_str() {
            if addr == format!("{}:{}", hostname_str, port) {
                return true;
            }
        }
    }

    false
}

/// Round gap threshold to trigger fast-sync instead of incremental sync.
const FAST_SYNC_GAP_THRESHOLD: u64 = 100;

/// Compute the serialized byte size of a DagVertex for orphan buffer accounting.
/// Uses postcard serialization for exact sizing (handles variable-length governance txs correctly).
fn vertex_byte_size(v: &DagVertex) -> usize {
    postcard::to_allocvec(v).map(|b| b.len()).unwrap_or(500)
}

/// An orphaned vertex waiting for missing parents, tagged with its source peer.
pub struct OrphanEntry {
    pub vertex: DagVertex,
    pub peer: String,
}

/// Compute total byte size of the orphan buffer.
fn orphan_buffer_bytes(orphans: &HashMap<[u8; 32], OrphanEntry>) -> usize {
    orphans.values().map(|e| vertex_byte_size(&e.vertex)).fold(0usize, |acc, s| acc.saturating_add(s))
}

/// Insert a vertex into the orphan buffer with per-peer and global eviction.
/// Returns false if rejected due to per-peer cap.
fn insert_orphan(orphans: &mut HashMap<[u8; 32], OrphanEntry>, hash: [u8; 32], vertex: DagVertex, peer: &str) -> bool {
    // Defense-in-depth: verify Ed25519 signature before buffering.
    // All call sites already verify signatures, but this prevents a future
    // code path from accidentally buffering unverified vertices.
    if !vertex.verify_signature() {
        warn!("Orphan buffer: rejecting vertex with invalid signature from {}", peer);
        return false;
    }

    // Per-peer cap: prevent one peer from monopolizing the buffer
    let peer_count = orphans.values().filter(|e| e.peer == peer).count();
    if peer_count >= MAX_ORPHAN_ENTRIES_PER_PEER {
        warn!("Orphan buffer: rejecting vertex from {} — per-peer limit ({}) reached", peer, MAX_ORPHAN_ENTRIES_PER_PEER);
        return false;
    }

    // Global cap: evict oldest entry when full
    if orphans.len() >= MAX_ORPHAN_ENTRIES || orphan_buffer_bytes(orphans) >= MAX_ORPHAN_BYTES {
        if let Some(evict_hash) = orphans.iter()
            .min_by_key(|(_, e)| e.vertex.round)
            .map(|(h, _)| *h)
        {
            orphans.remove(&evict_hash);
        }
    }
    orphans.insert(hash, OrphanEntry { vertex, peer: peer.to_string() });
    true
}

/// Shared finality+state application logic used by DagProposal, DagVertices,
/// resolve_orphans, and ParentVertices handlers.
///
/// Lock ordering: finality+dag (scoped) -> drop -> state (scoped) -> drop -> finality for epoch.
///
/// Returns the list of finalized vertices and the last finalized round (or 0 if none).
async fn apply_finality_and_state(
    new_validators: &[Address],
    dag: &Arc<RwLock<BlockDag>>,
    finality: &Arc<RwLock<FinalityTracker>>,
    state: &Arc<RwLock<StateEngine>>,
    mempool: &Arc<RwLock<Mempool>>,
    fatal_shutdown: &Arc<std::sync::atomic::AtomicBool>,
    fatal_exit_code: &Arc<std::sync::atomic::AtomicI32>,
) -> (Vec<DagVertex>, u64) {
    // Phase 1: Register validators and find newly-finalized vertices.
    // Hold finality+dag locks, then drop both before touching state.
    let (all_finalized, finalized_vertices) = {
        let mut fin = finality.write().await;
        for v in new_validators {
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
        // Drop finality + dag locks before state application
        (all_finalized, finalized_vertices)
    };

    if all_finalized.is_empty() {
        return (Vec::new(), 0);
    }

    info!("Finalized {} vertices", all_finalized.len());

    // Phase 2: Apply finalized vertices to state. Hold state lock, then drop.
    let epoch_changed;
    let last_fin_round;
    {
        let mut state_w = state.write().await;
        let prev_round = state_w.last_finalized_round();
        if let Err(e) = state_w.apply_finalized_vertices(&finalized_vertices) {
            if e.is_fatal() {
                // Supply invariant violations are unrecoverable.
                // Signal graceful shutdown so state is flushed before exit.
                tracing::error!("FATAL: {} — initiating graceful shutdown", e);
                fatal_exit_code.store(101, std::sync::atomic::Ordering::SeqCst);
                fatal_shutdown.store(true, std::sync::atomic::Ordering::SeqCst);
                return (finalized_vertices, 0);
            }
            warn!("Failed to apply finalized vertices: {}", e);
            return (finalized_vertices, 0);
        }
        epoch_changed = state_w.epoch_just_changed(prev_round);
        last_fin_round = state_w.last_finalized_round().unwrap_or(0);

        // Remove finalized transactions from mempool
        let mut mp = mempool.write().await;
        for v in &finalized_vertices {
            for tx in &v.block.transactions {
                mp.remove(&tx.hash());
            }
        }
    } // state_w dropped here

    // Phase 3: Epoch transition — acquire finality AFTER dropping state to prevent deadlock.
    if epoch_changed {
        let mut fin = finality.write().await;
        let state_r = state.read().await;
        sync_epoch_validators(&mut fin, &state_r);
        info!(
            "Epoch transition to epoch {} — active set: {} validators",
            state_r.current_epoch(),
            state_r.active_validators().len()
        );
    }

    (finalized_vertices, last_fin_round)
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
    /// Each entry tracks source peer for per-peer cap enforcement.
    pub orphans: Arc<Mutex<HashMap<[u8; 32], OrphanEntry>>>,
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
    /// Seed/bootstrap addresses for reconnection after peer loss.
    pub seed_addrs: Arc<Vec<String>>,
    /// Metrics for checkpoint production and synchronization.
    pub checkpoint_metrics: Arc<crate::CheckpointMetrics>,
    /// Custom pruning depth (0 = archive mode, no pruning).
    pub pruning_depth: u64,
    /// Highest round reported by any peer via Hello/HelloAck.
    /// Used by fast-sync task to determine if we're caught up.
    pub peer_max_round: Arc<std::sync::atomic::AtomicU64>,
    /// Testnet mode: enables secret-key-in-body RPC endpoints (/tx, /stake, /unstake,
    /// /proposal, /vote, /faucet, /keygen). Disabled on mainnet — only /tx/submit accepted.
    pub testnet_mode: bool,
    /// Set to true when a fatal condition requires graceful shutdown.
    /// The main loop observes this, saves state, then exits with `fatal_exit_code`.
    pub fatal_shutdown: Arc<std::sync::atomic::AtomicBool>,
    /// Exit code for fatal shutdown (100 = circuit breaker, 101 = supply invariant).
    pub fatal_exit_code: Arc<std::sync::atomic::AtomicI32>,
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
            // min_validators=1 for testnet flexibility. Mainnet: FinalityTracker::new(4)
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
            seed_addrs: Arc::new(Vec::new()),
            checkpoint_metrics: Arc::new(crate::CheckpointMetrics::new()),
            pruning_depth: 1000,
            peer_max_round: Arc::new(std::sync::atomic::AtomicU64::new(0)),
            testnet_mode: true,
            fatal_shutdown: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            fatal_exit_code: Arc::new(std::sync::atomic::AtomicI32::new(0)),
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
        is_self_addr(addr, self.port)
    }

    /// Start listening for incoming connections.
    pub async fn listen(&self) -> std::io::Result<()> {
        let listener = TcpListener::bind(format!("[::]:{}", self.port)).await?;
        info!("Listening on port {}", self.port);

        loop {
            let (stream, addr) = listener.accept().await?;
            let addr_str = addr.to_string();

            // Reject self-connections from loopback addresses.
            // Inbound connections have ephemeral source ports, so we only check IP.
            // The IP dedup check below handles non-loopback self-connections.
            if addr.ip().is_loopback() {
                info!("Skipping loopback connection from {}", addr_str);
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

            // Enforce per-IP connection limit to prevent DoS
            let remote_ip = addr.ip();
            let ip_connections = self.peers.connected_addrs().await
                .iter()
                .filter(|a| a.contains(&remote_ip.to_string()))
                .count();
            if ip_connections >= MAX_CONNECTIONS_PER_IP {
                warn!("Rejecting connection from {} — IP limit ({}) reached", addr_str, MAX_CONNECTIONS_PER_IP);
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

            info!("Incoming connection from {}", addr_str);

            let state = self.state.clone();
            let mempool = self.mempool.clone();
            let dag = self.dag.clone();
            let finality = self.finality.clone();
            let peers = self.peers.clone();
            let tx_tx = self.tx_tx.clone();
            let orphans = self.orphans.clone();
            let round_notify = self.round_notify.clone();
            let pending_checkpoints = self.pending_checkpoints.clone();
            let data_dir = self.data_dir.clone();
            let validator_sk = self.validator_sk.clone();
            let checkpoint_metrics = self.checkpoint_metrics.clone();
            let sync_complete = self.sync_complete.clone();
            let peer_max_round = self.peer_max_round.clone();
            let fatal_shutdown = self.fatal_shutdown.clone();
            let fatal_exit_code = self.fatal_exit_code.clone();
            tokio::spawn(async move {
                // Noise handshake before any application messages
                let mut stream = stream;
                let noise_transport = match tokio::time::timeout(
                    Duration::from_secs(HANDSHAKE_TIMEOUT_SECS),
                    handshake_responder(&mut stream, validator_sk.as_ref()),
                ).await {
                    Ok(Ok(result)) => {
                        if let Some(ref id) = result.peer_identity {
                            info!("Peer {} authenticated as validator {}", addr_str, id.address.short());
                        }
                        Some(result.transport)
                    }
                    Ok(Err(e)) => {
                        warn!("Noise handshake failed with {}: {}", addr_str, e);
                        return;
                    }
                    Err(_) => {
                        warn!("Noise handshake timed out with {}", addr_str);
                        return;
                    }
                };

                let (reader, writer) = split_connection(stream, addr_str.clone(), noise_transport);
                peers.add_writer(addr_str.clone(), writer).await;

                let ctx = PeerContext {
                    state: &state,
                    mempool: &mempool,
                    dag: &dag,
                    finality: &finality,
                    peers: &peers,
                    tx_tx: &tx_tx,
                    orphans: &orphans,
                    round_notify: &round_notify,
                    pending_checkpoints: &pending_checkpoints,
                    data_dir: &data_dir,
                    validator_sk: validator_sk.as_ref(),
                    checkpoint_metrics: &checkpoint_metrics,
                    sync_complete: &sync_complete,
                    peer_max_round: &peer_max_round,
                    fatal_shutdown: &fatal_shutdown,
                    fatal_exit_code: &fatal_exit_code,
                };
                // handle_peer may rename peer_addr via Hello; remove both keys on disconnect
                if let Err(e) = handle_peer(reader, &ctx).await {
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
        let tx_tx = self.tx_tx.clone();
        let orphans = self.orphans.clone();
        let round_notify = self.round_notify.clone();
        let pending_checkpoints = self.pending_checkpoints.clone();
        let data_dir = self.data_dir.clone();
        let validator_sk = self.validator_sk.clone();
        let checkpoint_metrics = self.checkpoint_metrics.clone();
        let sync_complete = self.sync_complete.clone();
        let peer_max_round = self.peer_max_round.clone();
        let fatal_shutdown = self.fatal_shutdown.clone();
        let fatal_exit_code = self.fatal_exit_code.clone();
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
                if peer_count < MIN_PEERS_FOR_RECONNECT && !seed_addrs.is_empty() {
                    info!("Heartbeat: low peer count ({}), reconnecting to seeds...", peer_count);
                    for addr in seed_addrs.iter() {
                        let state = state.clone();
                        let mempool = mempool.clone();
                        let dag = dag.clone();
                        let finality = finality.clone();
                        let peers = peers.clone();
                        let tx_tx = tx_tx.clone();
                        let orphans = orphans.clone();
                        let round_notify = round_notify.clone();
                        let pending_checkpoints = pending_checkpoints.clone();
                        let data_dir = data_dir.clone();
                        let validator_sk = validator_sk.clone();
                        let checkpoint_metrics = checkpoint_metrics.clone();
                        let sync_complete = sync_complete.clone();
                        let peer_max_round = peer_max_round.clone();
                        let fatal_shutdown = fatal_shutdown.clone();
                        let fatal_exit_code = fatal_exit_code.clone();
                        let addr = addr.clone();
                        tokio::spawn(async move {
                            let ctx = PeerContext {
                                state: &state,
                                mempool: &mempool,
                                dag: &dag,
                                finality: &finality,
                                peers: &peers,
                                tx_tx: &tx_tx,
                                orphans: &orphans,
                                round_notify: &round_notify,
                                pending_checkpoints: &pending_checkpoints,
                                data_dir: &data_dir,
                                validator_sk: validator_sk.as_ref(),
                                checkpoint_metrics: &checkpoint_metrics,
                                sync_complete: &sync_complete,
                                peer_max_round: &peer_max_round,
                                fatal_shutdown: &fatal_shutdown,
                                fatal_exit_code: &fatal_exit_code,
                            };
                            try_connect_peer(addr, listen_port, &ctx).await;
                        });
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

        // Noise handshake as initiator before any application messages
        let mut stream = stream;
        let noise_transport = match tokio::time::timeout(
            Duration::from_secs(HANDSHAKE_TIMEOUT_SECS),
            handshake_initiator(&mut stream, self.validator_sk.as_ref()),
        ).await {
            Ok(Ok(result)) => {
                if let Some(ref id) = result.peer_identity {
                    info!("Seed peer {} authenticated as validator {}", addr_str, id.address.short());
                }
                Some(result.transport)
            }
            Ok(Err(e)) => {
                warn!("Noise handshake failed with seed {}: {}", addr_str, e);
                return Err(std::io::Error::new(std::io::ErrorKind::Other, format!("noise handshake: {}", e)));
            }
            Err(_) => {
                warn!("Noise handshake timed out with seed {}", addr_str);
                return Err(std::io::Error::new(std::io::ErrorKind::TimedOut, "noise handshake timeout"));
            }
        };

        self.peers.add_known(addr_str.clone()).await;
        self.peers.add_connected_listen_addr(addr_str.clone()).await;

        let (reader, writer) = split_connection(stream, addr_str.clone(), noise_transport);
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
        let tx_tx = self.tx_tx.clone();
        let orphans = self.orphans.clone();
        let round_notify = self.round_notify.clone();
        let pending_checkpoints = self.pending_checkpoints.clone();
        let data_dir = self.data_dir.clone();
        let validator_sk = self.validator_sk.clone();
        let checkpoint_metrics = self.checkpoint_metrics.clone();
        let sync_complete = self.sync_complete.clone();
        let peer_max_round = self.peer_max_round.clone();
        let fatal_shutdown = self.fatal_shutdown.clone();
        let fatal_exit_code = self.fatal_exit_code.clone();

        tokio::spawn(async move {
            let ctx = PeerContext {
                state: &state,
                mempool: &mempool,
                dag: &dag,
                finality: &finality,
                peers: &peers,
                tx_tx: &tx_tx,
                orphans: &orphans,
                round_notify: &round_notify,
                pending_checkpoints: &pending_checkpoints,
                data_dir: &data_dir,
                validator_sk: validator_sk.as_ref(),
                checkpoint_metrics: &checkpoint_metrics,
                sync_complete: &sync_complete,
                peer_max_round: &peer_max_round,
                fatal_shutdown: &fatal_shutdown,
                fatal_exit_code: &fatal_exit_code,
            };
            if let Err(e) = handle_peer(reader, &ctx).await {
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
    orphans: &Arc<Mutex<HashMap<[u8; 32], OrphanEntry>>>,
    dag: &Arc<RwLock<BlockDag>>,
    finality: &Arc<RwLock<FinalityTracker>>,
    state: &Arc<RwLock<StateEngine>>,
    mempool: &Arc<RwLock<Mempool>>,
    peers: &PeerRegistry,
    peer_addr: &str,
    _round_notify: &Arc<Notify>,
    fatal_shutdown: &Arc<std::sync::atomic::AtomicBool>,
    fatal_exit_code: &Arc<std::sync::atomic::AtomicI32>,
) {
    let mut resolved = true;
    let mut passes = 0;
    while resolved && passes < MAX_ORPHAN_RESOLUTION_PASSES {
        passes += 1;
        resolved = false;
        let candidates: Vec<([u8; 32], DagVertex)> = {
            let orph = orphans.lock().await;
            orph.iter().map(|(h, e)| (*h, e.vertex.clone())).collect()
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
                    apply_finality_and_state(
                        &[validator], dag, finality, state, mempool,
                        fatal_shutdown, fatal_exit_code,
                    ).await;
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
    ctx: &PeerContext<'_>,
) {
    let dag = ctx.dag;
    let peers = ctx.peers;
    let state = ctx.state;
    let mempool = ctx.mempool;
    let finality = ctx.finality;
    let tx_tx = ctx.tx_tx;
    let orphans = ctx.orphans;
    let round_notify = ctx.round_notify;
    let pending_checkpoints = ctx.pending_checkpoints;
    let data_dir = ctx.data_dir;
    let validator_sk = ctx.validator_sk;
    let checkpoint_metrics = ctx.checkpoint_metrics;
    let sync_complete = ctx.sync_complete;
    let peer_max_round = ctx.peer_max_round;
    let fatal_shutdown = ctx.fatal_shutdown;
    let fatal_exit_code = ctx.fatal_exit_code;
    // Don't connect to ourselves
    if is_self_addr(&addr, listen_port) {
        info!("Skipping self-connection to {}", addr);
        return;
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

    // Atomically mark as connecting to prevent TOCTOU race
    // (another task could start connecting between our checks and TCP connect)
    if !peers.start_connecting(&addr).await {
        return; // Another task is already connecting to this address
    }

    // Resolve DNS to IP and check for self-connection or duplicate
    if let Ok(mut resolved) = tokio::net::lookup_host(&addr).await {
        if let Some(socket_addr) = resolved.next() {
            // Self-connection check: if DNS resolves to loopback with our port
            if socket_addr.ip().is_loopback() && socket_addr.port() == listen_port {
                info!("Skipping self-connection to {}", addr);
                peers.finish_connecting(&addr).await;
                return;
            }
            // Duplicate connection check
            let resolved_ip = socket_addr.ip().to_string();
            let existing = peers.connected_addrs().await;
            for existing_addr in &existing {
                if existing_addr.contains(&resolved_ip) {
                    peers.finish_connecting(&addr).await;
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
                        peers.finish_connecting(&addr).await;
                        return;
                    }
                }
                let remote_ip = remote.ip().to_string();
                let existing = peers.connected_addrs().await;
                for existing_addr in &existing {
                    if existing_addr.contains(&remote_ip) {
                        info!("Peer discovery: skipping {} — already connected to IP {}", addr, remote_ip);
                        peers.finish_connecting(&addr).await;
                        return;
                    }
                }
            }

            // Re-check: another task may have connected in the meantime
            if peers.is_listen_addr_connected(&addr).await {
                drop(stream);
                peers.finish_connecting(&addr).await;
                return;
            }

            info!("Peer discovery: connected to {}", addr);

            // Noise handshake as initiator before any application messages
            let mut stream = stream;
            let noise_transport = match tokio::time::timeout(
                Duration::from_secs(HANDSHAKE_TIMEOUT_SECS),
                handshake_initiator(&mut stream, validator_sk),
            ).await {
                Ok(Ok(result)) => {
                    if let Some(ref id) = result.peer_identity {
                        info!("Peer {} authenticated as validator {}", addr, id.address.short());
                    }
                    Some(result.transport)
                }
                Ok(Err(e)) => {
                    warn!("Noise handshake failed with {}: {}", addr, e);
                    peers.finish_connecting(&addr).await;
                    return;
                }
                Err(_) => {
                    warn!("Noise handshake timed out with {}", addr);
                    peers.finish_connecting(&addr).await;
                    return;
                }
            };

            peers.finish_connecting(&addr).await;
            peers.add_known(addr.clone()).await;
            peers.add_connected_listen_addr(addr.clone()).await;

            let (reader, writer) = split_connection(stream, addr.clone(), noise_transport);
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
            let state = state.clone();
            let mempool = mempool.clone();
            let dag = dag.clone();
            let finality = finality.clone();
            let peers_clone = peers.clone();
            let tx_tx = tx_tx.clone();
            let orphans = orphans.clone();
            let round_notify = round_notify.clone();
            let pending_checkpoints = pending_checkpoints.clone();
            let data_dir = data_dir.clone();
            let validator_sk = validator_sk.cloned();
            let checkpoint_metrics = checkpoint_metrics.clone();
            let sync_complete = sync_complete.clone();
            let peer_max_round = peer_max_round.clone();
            let fatal_shutdown = fatal_shutdown.clone();
            let fatal_exit_code = fatal_exit_code.clone();
            let addr_clone = addr.clone();
            tokio::spawn(async move {
                let inner_ctx = PeerContext {
                    state: &state,
                    mempool: &mempool,
                    dag: &dag,
                    finality: &finality,
                    peers: &peers_clone,
                    tx_tx: &tx_tx,
                    orphans: &orphans,
                    round_notify: &round_notify,
                    pending_checkpoints: &pending_checkpoints,
                    data_dir: &data_dir,
                    validator_sk: validator_sk.as_ref(),
                    checkpoint_metrics: &checkpoint_metrics,
                    sync_complete: &sync_complete,
                    peer_max_round: &peer_max_round,
                    fatal_shutdown: &fatal_shutdown,
                    fatal_exit_code: &fatal_exit_code,
                };
                let fut = Box::pin(handle_peer(reader, &inner_ctx));
                if let Err(e) = fut.await {
                    warn!("Peer {} disconnected: {}", addr_clone, e);
                }
                peers_clone.remove_peer(&addr_clone).await;
                peers_clone.remove_connected_listen_addr(&addr_clone).await;
            });
        }
        Err(e) => {
            peers.finish_connecting(&addr).await;
            warn!("Peer discovery: failed to connect to {}: {}", addr, e);
        }
    }
}

/// Shared context for peer message handling.
/// Eliminates the 18-parameter function signature.
struct PeerContext<'a> {
    state: &'a Arc<RwLock<StateEngine>>,
    mempool: &'a Arc<RwLock<Mempool>>,
    dag: &'a Arc<RwLock<BlockDag>>,
    finality: &'a Arc<RwLock<FinalityTracker>>,
    peers: &'a PeerRegistry,
    tx_tx: &'a broadcast::Sender<Transaction>,
    orphans: &'a Arc<Mutex<HashMap<[u8; 32], OrphanEntry>>>,
    round_notify: &'a Arc<Notify>,
    pending_checkpoints: &'a Arc<RwLock<HashMap<u64, ultradag_coin::Checkpoint>>>,
    data_dir: &'a PathBuf,
    validator_sk: Option<&'a SecretKey>,
    checkpoint_metrics: &'a Arc<crate::CheckpointMetrics>,
    sync_complete: &'a Arc<std::sync::atomic::AtomicBool>,
    peer_max_round: &'a Arc<std::sync::atomic::AtomicU64>,
    fatal_shutdown: &'a Arc<std::sync::atomic::AtomicBool>,
    fatal_exit_code: &'a Arc<std::sync::atomic::AtomicI32>,
}

async fn handle_peer(
    mut reader: PeerReader,
    ctx: &PeerContext<'_>,
) -> std::io::Result<()> {
    let state = ctx.state;
    let mempool = ctx.mempool;
    let dag = ctx.dag;
    let finality = ctx.finality;
    let peers = ctx.peers;
    let tx_tx = ctx.tx_tx;
    let orphans = ctx.orphans;
    let round_notify = ctx.round_notify;
    let pending_checkpoints = ctx.pending_checkpoints;
    let data_dir = ctx.data_dir;
    let validator_sk = ctx.validator_sk;
    let checkpoint_metrics = ctx.checkpoint_metrics;
    let sync_complete = ctx.sync_complete;
    let peer_max_round = ctx.peer_max_round;
    let fatal_shutdown = ctx.fatal_shutdown;
    let fatal_exit_code = ctx.fatal_exit_code;

    let peer_addr = reader.addr.clone();
    let mut allowlist_rejections: u32 = 0;
    let mut last_round_hash_request: Option<Instant> = None;
    let mut last_get_dag_vertices: Option<Instant> = None;

    // Per-peer aggregate message rate limiting: track messages in a sliding window.
    // Disconnect peers that exceed MAX_MESSAGES_PER_WINDOW within RATE_WINDOW_SECS.
    const MAX_MESSAGES_PER_WINDOW: u32 = 500;
    const RATE_WINDOW_SECS: u64 = 60;
    let mut message_count: u32 = 0;
    let mut window_start = Instant::now();

    loop {
        let msg = reader.recv().await?;

        // Aggregate rate limiting: disconnect abusive peers
        message_count += 1;
        let elapsed = window_start.elapsed();
        if elapsed >= std::time::Duration::from_secs(RATE_WINDOW_SECS) {
            // Reset window
            message_count = 1;
            window_start = Instant::now();
        } else if message_count > MAX_MESSAGES_PER_WINDOW {
            warn!("Peer {} exceeded message rate limit ({} msgs in {}s), disconnecting",
                peer_addr, message_count, elapsed.as_secs());
            return Ok(());
        }

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

                // Track current network round from this peer.
                // Uses store() not fetch_max() so the value reflects the actual current
                // network state. After a clean deploy (network reset), peers report low
                // rounds and peer_max_round must decrease accordingly; fetch_max would
                // keep the stale high value forever.
                peer_max_round.store(height, std::sync::atomic::Ordering::Relaxed);

                // If peer is ahead, sync from them
                if height > our_round {
                    let gap = height - our_round;
                    if gap > FAST_SYNC_GAP_THRESHOLD {
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
                            max_count: DAG_SYNC_BATCH_SIZE,
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
                // Track current network round (see Hello handler for rationale on store vs fetch_max)
                peer_max_round.store(height, std::sync::atomic::Ordering::Relaxed);

                let our_round = dag.read().await.current_round();
                if height > our_round {
                    let gap = height - our_round;
                    if gap > FAST_SYNC_GAP_THRESHOLD {
                        info!("Large gap ({} rounds behind peer {}), requesting checkpoint + incremental sync", gap, peer_addr);
                        peers.send_to(&peer_addr, &Message::GetCheckpoint { min_round: our_round }).await?;
                    }
                    peers
                        .send_to(&peer_addr, &Message::GetDagVertices {
                            from_round: our_round + 1,
                            max_count: DAG_SYNC_BATCH_SIZE,
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

                        // Slashing is applied deterministically during finalized vertex
                        // application in StateEngine, not here. We only broadcast evidence
                        // so other nodes can detect and record the equivocation.

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
                    Err(DagInsertError::FutureRound) => {
                        debug!("Skipped vertex from {} round={}: future round", peer_addr, round);
                        continue;
                    }
                    Err(DagInsertError::FutureTimestamp) => {
                        debug!("Skipped vertex from {} round={}: future timestamp", peer_addr, round);
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
                            insert_orphan(&mut orph, vertex_hash, vertex, &peer_addr);
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

                    apply_finality_and_state(
                        &[validator], dag, finality, state, mempool,
                        fatal_shutdown, fatal_exit_code,
                    ).await;

                    peers.broadcast(&Message::DagProposal(vertex), &peer_addr).await;

                    // Try to resolve orphaned vertices now that a new vertex was inserted
                    resolve_orphans(orphans, dag, finality, state, mempool, peers, &peer_addr, round_notify, fatal_shutdown, fatal_exit_code).await;
                }
            }

            Message::GetDagVertices { from_round, max_count } => {
                // Rate limit: at most one GetDagVertices per 2s per peer
                let now = Instant::now();
                if let Some(last) = last_get_dag_vertices {
                    if now.duration_since(last) < Duration::from_secs(2) {
                        continue;
                    }
                }
                last_get_dag_vertices = Some(now);

                // Use try_read to avoid blocking if DAG is locked by validator/sync
                let Some(dag_r) = dag.try_read().ok() else {
                    warn!("GetDagVertices: DAG lock contended, skipping for {}", peer_addr);
                    continue;
                };
                // Clamp from_round to pruning_floor — pruned rounds have no vertices.
                let floor = dag_r.pruning_floor();
                let effective_from = from_round.max(floor);
                // Cap max_count to prevent CPU exhaustion from huge range iterations
                let capped_count = (max_count as usize).min(50);
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
                // Rate limit: at most one GetRoundHashes per 10s per peer
                let now = Instant::now();
                if let Some(last) = last_round_hash_request {
                    if now.duration_since(last) < Duration::from_secs(10) {
                        continue;
                    }
                }
                last_round_hash_request = Some(now);

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
                        .filter(|h| dag_r.get(h).is_none())
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
                            Err(DagInsertError::FutureRound) | Err(DagInsertError::FutureTimestamp) => {
                                debug!("Skipped sync vertex from {} round={}: future round/timestamp", peer_addr, vertex.round);
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

                    // Slashing applied deterministically during finalized vertex
                    // application — not here. Broadcast evidence so peers record it.
                    for msg in equivocation_msgs {
                        peers.broadcast(&msg, "").await;
                    }
                }
                // Buffer failed inserts as orphans (outside dag lock)
                if !failed_vertices.is_empty() {
                    let mut orph = orphans.lock().await;
                    for (hash, vertex) in failed_vertices {
                        insert_orphan(&mut orph, hash, vertex, &peer_addr);
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
                    apply_finality_and_state(
                        &new_validators, dag, finality, state, mempool,
                        fatal_shutdown, fatal_exit_code,
                    ).await;
                    resolve_orphans(orphans, dag, finality, state, mempool, peers, &peer_addr, round_notify, fatal_shutdown, fatal_exit_code).await;
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
                        max_count: DAG_SYNC_BATCH_SIZE,
                    }).await;
                }
            }

            Message::GetPeers => {
                let mut known = peers.known_peers().await;
                known.truncate(MAX_PEERS_RESPONSE); // Cap response size to prevent topology leakage
                peers.send_to(&peer_addr, &Message::Peers(known)).await?;
            }

            Message::Peers(addrs) => {
                // Cap incoming peer list to prevent large allocation abuse
                for addr in addrs.iter().take(MAX_PEERS_RESPONSE) {
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
                    drop(dag_w);

                    // Slashing applied deterministically during finalized vertex
                    // application — not here. Broadcast evidence so peers record it.
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
                let mut new_validators: Vec<Address> = Vec::new();

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
                            new_validators.push(validator);
                        }
                        Err(DagInsertError::MissingParents(missing)) => {
                            all_missing.extend(&missing);
                            let mut orph = orphans.lock().await;
                            insert_orphan(&mut orph, hash, vertex, &peer_addr);
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

                // If we inserted any parent vertices, run finality and resolve orphans
                if !new_validators.is_empty() {
                    // Don't notify here — ParentVertices is bulk sync context.
                    apply_finality_and_state(
                        &new_validators, dag, finality, state, mempool,
                        fatal_shutdown, fatal_exit_code,
                    ).await;
                    resolve_orphans(orphans, dag, finality, state, mempool, peers, &peer_addr, round_notify, fatal_shutdown, fatal_exit_code).await;
                }
            }

            Message::CheckpointProposal(mut checkpoint) => {
                // 1. Verify the round is one we have finalized
                let fin = finality.read().await;
                let our_finalized = fin.last_finalized_round();
                drop(fin);

                if checkpoint.round > our_finalized {
                    // We haven't finalized this round yet — store for later
                    let mut pending = pending_checkpoints.write().await;
                    pending.insert(checkpoint.round, checkpoint);
                    // Evict oldest if over cap (same cap as co-signing path)
                    while pending.len() > 10 {
                        if let Some(&oldest) = pending.keys().min() {
                            pending.remove(&oldest);
                        } else {
                            break;
                        }
                    }
                    drop(pending);
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
                
                // Verify the signature is valid before storing
                if !checkpoint.verify_signature(&signature) {
                    warn!("CheckpointSignature from {:?} has invalid signature", &signature.pub_key[..4]);
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
                    (active.len() * 2).div_ceil(3) // ceil(2n/3)
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
                
                // Checkpoint chain verification: defense-in-depth against eclipse attacks.
                // Walks prev_checkpoint_hash chain back to genesis if local checkpoints exist.
                // If local checkpoints are unavailable (fresh/syncing node), skip chain verification
                // and rely on quorum signature verification below (the primary trust mechanism).
                {
                    let dir = data_dir;
                    let local_checkpoints = ultradag_coin::persistence::list_checkpoints(dir);
                    if !local_checkpoints.is_empty() {
                        let checkpoint_loader = |hash: [u8; 32]| -> Option<ultradag_coin::Checkpoint> {
                            // Search all local checkpoints by round, match by hash
                            let rounds = ultradag_coin::persistence::list_checkpoints(dir);
                            for round in rounds {
                                if let Some(cp) = ultradag_coin::persistence::load_checkpoint_by_round(dir, round) {
                                    let cp_hash = ultradag_coin::consensus::compute_checkpoint_hash(&cp);
                                    if cp_hash == hash {
                                        return Some(cp);
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
                                error!("Checkpoint chain verification FAILED: {} — disconnecting peer {}", e, peer_addr);
                                checkpoint_metrics.record_fast_sync_failure();
                                return Ok(());
                            }
                        }
                    } else {
                        info!("No local checkpoints — skipping chain verification for round {} (relying on quorum signatures)", checkpoint.round);
                    }
                }
                
                // Validate snapshot size before processing to prevent OOM from
                // malicious peers sending fabricated snapshots with millions of entries.
                if state_at_checkpoint.accounts.len() > MAX_SNAPSHOT_ACCOUNTS {
                    warn!("CheckpointSync: rejecting snapshot with {} accounts (max {})",
                        state_at_checkpoint.accounts.len(), MAX_SNAPSHOT_ACCOUNTS);
                    checkpoint_metrics.record_fast_sync_failure();
                    continue;
                }
                if state_at_checkpoint.proposals.len() > MAX_SNAPSHOT_PROPOSALS {
                    warn!("CheckpointSync: rejecting snapshot with {} proposals (max {})",
                        state_at_checkpoint.proposals.len(), MAX_SNAPSHOT_PROPOSALS);
                    checkpoint_metrics.record_fast_sync_failure();
                    continue;
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
                    (active.len() * 2).div_ceil(3)
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

/// Format first 4 bytes of a 32-byte hash as 8-char hex string.
pub fn hex_short(hash: &[u8; 32]) -> String {
    hash[..4].iter().map(|b| format!("{b:02x}")).collect()
}
