use std::collections::HashMap;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::sync::{broadcast, Mutex, Notify, RwLock};
use tracing::{info, warn};

use ultradag_coin::{BlockDag, DagVertex, FinalityTracker, Mempool, StateEngine, Transaction, sync_epoch_validators};
use ultradag_coin::consensus::dag::DagInsertError;

use crate::peer::{split_connection, PeerReader, PeerRegistry};
use crate::protocol::Message;

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
            finality: Arc::new(RwLock::new(FinalityTracker::new(3))),
            peers: PeerRegistry::new(),
            vertex_tx,
            tx_tx,
            orphans: Arc::new(Mutex::new(HashMap::new())),
            round_notify: Arc::new(Notify::new()),
        }
    }

    /// Start listening for incoming connections.
    pub async fn listen(&self) -> std::io::Result<()> {
        let listener = TcpListener::bind(format!("[::]:{}", self.port)).await?;
        info!("Listening on port {}", self.port);

        loop {
            let (stream, addr) = listener.accept().await?;
            let addr_str = addr.to_string();
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

            let listen_port = self.port;
            tokio::spawn(async move {
                // handle_peer may rename peer_addr via Hello; remove both keys on disconnect
                if let Err(e) = handle_peer(reader, &state, &mempool, &dag, &finality, &peers, &vertex_tx, &tx_tx, &orphans, listen_port, &round_notify).await {
                    warn!("Peer {} disconnected: {}", addr_str, e);
                }
                // Remove by original ephemeral addr and any possible listen addr
                peers.remove_peer(&addr_str).await;
            });
        }
    }

    /// Connect to a seed peer.
    pub async fn connect_to(&self, addr: &str) -> std::io::Result<()> {
        let stream = tokio::net::TcpStream::connect(addr).await?;
        let addr_str = addr.to_string();
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
        let listen_port = self.port;

        tokio::spawn(async move {
            if let Err(e) = handle_peer(reader, &state, &mempool, &dag, &finality, &peers, &vertex_tx, &tx_tx, &orphans, listen_port, &round_notify).await {
                warn!("Peer {} disconnected: {}", addr_str, e);
            }
            peers.remove_peer(&addr_str).await;
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
    round_notify: &Arc<Notify>,
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
                    round_notify.notify_one();

                    let validator = vertex.validator;
                    // Register validator + check finality (multi-pass)
                    {
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

                        if !all_finalized.is_empty() {
                            info!("Orphan resolve: finalized {} vertices", all_finalized.len());
                            let finalized_vertices: Vec<DagVertex> = all_finalized
                                .iter()
                                .filter_map(|h| dag_r.get(h).cloned())
                                .collect();
                            drop(dag_r);
                            let mut state_w = state.write().await;
                            let prev_round = state_w.last_finalized_round();
                            if let Err(e) = state_w.apply_finalized_vertices(&finalized_vertices) {
                                warn!("Failed to apply finalized vertices: {}", e);
                            } else {
                                if state_w.epoch_just_changed(prev_round) {
                                    sync_epoch_validators(&mut fin, &state_w);
                                    info!("Epoch transition to epoch {} — active set: {} validators",
                                        state_w.current_epoch(), state_w.active_validators().len());
                                }
                                let mut mp = mempool.write().await;
                                for v in &finalized_vertices {
                                    for tx in &v.block.transactions {
                                        mp.remove(&tx.hash());
                                    }
                                }
                            }
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
const MAX_PEERS: usize = 8;

/// Connect to a peer address if not already connected.
/// Establishes a TCP connection, sends Hello, and keeps a drain loop
/// for the reader so the connection stays alive.
async fn try_connect_peer(
    addr: String,
    listen_port: u16,
    dag: Arc<RwLock<BlockDag>>,
    peers: PeerRegistry,
) {
    // Don't connect to ourselves
    let self_addr = format!("127.0.0.1:{}", listen_port);
    if addr == self_addr || addr == format!("0.0.0.0:{}", listen_port) {
        return;
    }

    // Check if already at max peers
    if peers.connected_count().await >= MAX_PEERS {
        return;
    }

    // Check if we already have a connection to this listen address
    // (covers both outbound connections keyed by listen addr AND
    // inbound connections registered via Hello)
    if peers.is_listen_addr_connected(&addr).await {
        return;
    }

    // Also check if we have a direct writer (outbound connection)
    if peers.send_to(&addr, &Message::Ping(0)).await.is_ok() {
        return;
    }

    match tokio::net::TcpStream::connect(&addr).await {
        Ok(stream) => {
            info!("Peer discovery: connected to {}", addr);
            peers.add_known(addr.clone()).await;
            peers.add_connected_listen_addr(addr.clone()).await;

            let (mut reader, writer) = split_connection(stream, addr.clone());
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

            // Drain reader to keep TCP connection alive until remote closes
            let peers_clone = peers.clone();
            let addr_clone = addr.clone();
            tokio::spawn(async move {
                loop {
                    match reader.recv().await {
                        Ok(_) => {} // discard — vertices are received via the listener side
                        Err(_) => break,
                    }
                }
                peers_clone.remove_peer(&addr_clone).await;
            });
        }
        Err(e) => {
            warn!("Peer discovery: failed to connect to {}: {}", addr, e);
        }
    }
}

async fn handle_peer(
    mut reader: PeerReader,
    state: &Arc<RwLock<StateEngine>>,
    mempool: &Arc<RwLock<Mempool>>,
    dag: &Arc<RwLock<BlockDag>>,
    finality: &Arc<RwLock<FinalityTracker>>,
    peers: &PeerRegistry,
    _vertex_tx: &broadcast::Sender<DagVertex>,
    tx_tx: &broadcast::Sender<Transaction>,
    orphans: &Arc<Mutex<HashMap<[u8; 32], DagVertex>>>,
    listen_port: u16,
    round_notify: &Arc<Notify>,
) -> std::io::Result<()> {
    let peer_addr = reader.addr.clone();

    loop {
        let msg = reader.recv().await?;

        match msg {
            Message::Hello { version: _, height, listen_port } => {
                let our_round = dag.read().await.current_round();
                peers
                    .send_to(&peer_addr, &Message::HelloAck {
                        version: 1,
                        height: our_round,
                    })
                    .await?;

                // If peer is ahead, request DAG vertices
                if height > our_round {
                    peers
                        .send_to(&peer_addr, &Message::GetDagVertices {
                            from_round: our_round + 1,
                            max_count: 100,
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
                    peers.add_connected_listen_addr(listen_addr).await;
                }

                // Request peer list for mesh discovery
                let _ = peers.send_to(&peer_addr, &Message::GetPeers).await;
            }

            Message::HelloAck { height, .. } => {
                let our_round = dag.read().await.current_round();
                if height > our_round {
                    peers
                        .send_to(&peer_addr, &Message::GetDagVertices {
                            from_round: our_round + 1,
                            max_count: 100,
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
                let mut mp = mempool.write().await;
                if mp.insert(tx.clone()) {
                    let _ = tx_tx.send(tx.clone());
                    drop(mp);
                    peers.broadcast(&Message::NewTx(tx), &peer_addr).await;
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
                        let dag_w = dag.read().await;
                        if let Some([hash1, hash2]) = dag_w.get_equivocation_evidence(&validator, round) {
                            if let (Some(v1), Some(v2)) = (dag_w.get(&hash1), dag_w.get(&hash2)) {
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
                    Err(DagInsertError::MissingParents(missing)) => {
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

                    // Check finality and apply to state (multi-pass for parent finality guarantee)
                    {
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

                        if !all_finalized.is_empty() {
                            info!("DAG-BFT finalized {} vertices", all_finalized.len());

                            let finalized_vertices: Vec<DagVertex> = all_finalized
                                .iter()
                                .filter_map(|h| dag_r.get(h).cloned())
                                .collect();

                            drop(dag_r);

                            let mut state_w = state.write().await;
                            let prev_round = state_w.last_finalized_round();
                            if let Err(e) = state_w.apply_finalized_vertices(&finalized_vertices) {
                                warn!("Failed to apply finalized vertices to state: {}", e);
                            } else {
                                // Epoch transition: sync active validator set to FinalityTracker
                                if state_w.epoch_just_changed(prev_round) {
                                    sync_epoch_validators(&mut fin, &state_w);
                                    info!("Epoch transition to epoch {} — active set: {} validators",
                                        state_w.current_epoch(), state_w.active_validators().len());
                                }
                                let mut mp = mempool.write().await;
                                for v in &finalized_vertices {
                                    for tx in &v.block.transactions {
                                        mp.remove(&tx.hash());
                                    }
                                }
                            }
                        }
                    }

                    peers.broadcast(&Message::DagProposal(vertex), &peer_addr).await;

                    // Try to resolve orphaned vertices now that a new vertex was inserted
                    resolve_orphans(orphans, dag, finality, state, mempool, peers, &peer_addr, round_notify).await;
                }
            }

            Message::GetDagVertices { from_round, max_count } => {
                let dag_r = dag.read().await;
                let mut vertices = Vec::new();
                for round in from_round..from_round + max_count as u64 {
                    for v in dag_r.vertices_in_round(round) {
                        vertices.push(v.clone());
                    }
                    if vertices.len() >= max_count as usize {
                        break;
                    }
                }
                drop(dag_r);
                peers.send_to(&peer_addr, &Message::DagVertices(vertices)).await?;
            }

            Message::DagVertices(vertices) => {
                let mut new_validators = Vec::new();
                let mut failed_vertices = Vec::new();
                let mut all_missing_parents: Vec<[u8; 32]> = Vec::new();
                {
                    let mut dag_w = dag.write().await;
                    for vertex in vertices {
                        // Verify signature before accepting (same as DagProposal)
                        if !vertex.verify_signature() {
                            warn!("Rejected sync vertex with invalid signature from {}", peer_addr);
                            continue;
                        }

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
                            Err(DagInsertError::Equivocation { .. }) => {
                                warn!(
                                    "Equivocation in sync vertex from validator {} round {} (peer {})",
                                    validator, vertex.round, peer_addr,
                                );
                                // Broadcast equivocation evidence
                                if let Some([h1, h2]) = dag_w.get_equivocation_evidence(&validator, vertex.round) {
                                    if let (Some(v1), Some(v2)) = (dag_w.get(&h1).cloned(), dag_w.get(&h2).cloned()) {
                                        let evidence_msg = Message::EquivocationEvidence {
                                            vertex1: v1,
                                            vertex2: v2,
                                        };
                                        drop(dag_w);
                                        peers.broadcast(&evidence_msg, "").await;
                                        break;
                                    }
                                }
                            }
                        }
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
                    round_notify.notify_one();
                    // Multi-pass finality
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
                    if !all_finalized.is_empty() {
                        info!("Sync finalized {} vertices", all_finalized.len());
                        let finalized_vertices: Vec<DagVertex> = all_finalized
                            .iter()
                            .filter_map(|h| dag_r.get(h).cloned())
                            .collect();
                        drop(dag_r);
                        let mut state_w = state.write().await;
                        let prev_round = state_w.last_finalized_round();
                        if let Err(e) = state_w.apply_finalized_vertices(&finalized_vertices) {
                            warn!("Failed to apply sync-finalized vertices: {}", e);
                        } else {
                            if state_w.epoch_just_changed(prev_round) {
                                sync_epoch_validators(&mut fin, &state_w);
                                info!("Epoch transition to epoch {} — active set: {} validators",
                                    state_w.current_epoch(), state_w.active_validators().len());
                            }
                            let mut mp = mempool.write().await;
                            for v in &finalized_vertices {
                                for tx in &v.block.transactions {
                                    mp.remove(&tx.hash());
                                }
                            }
                        }
                    }
                    drop(fin);
                    resolve_orphans(orphans, dag, finality, state, mempool, peers, &peer_addr, round_notify).await;
                }
            }

            Message::GetPeers => {
                let known = peers.known_peers().await;
                peers.send_to(&peer_addr, &Message::Peers(known)).await?;
            }

            Message::Peers(addrs) => {
                for addr in &addrs {
                    peers.add_known(addr.clone()).await;
                }
                // Connect to learned peers for mesh topology
                if peers.connected_count().await < MAX_PEERS {
                    for addr in addrs {
                        tokio::spawn(try_connect_peer(
                            addr,
                            listen_port,
                            dag.clone(),
                            peers.clone(),
                        ));
                    }
                }
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
                    warn!(
                        "Marked validator {} as Byzantine due to equivocation in round {} (evidence from {})",
                        vertex1.validator, vertex1.round, peer_addr
                    );
                    
                    // Broadcast evidence to all other peers
                    drop(dag_w);
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

                for vertex in vertices.into_iter().take(50) {
                    if !vertex.verify_signature() {
                        continue;
                    }
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
                    round_notify.notify_one();
                    resolve_orphans(orphans, dag, finality, state, mempool, peers, &peer_addr, round_notify).await;
                }
            }

            Message::CheckpointProposal(_checkpoint) => {
                // TODO: Verify checkpoint, add our signature if valid, broadcast signature
                tracing::debug!("Received checkpoint proposal (not yet implemented)");
            }

            Message::CheckpointSignatureMsg { .. } => {
                // TODO: Accumulate signatures, save checkpoint when quorum reached
                tracing::debug!("Received checkpoint signature (not yet implemented)");
            }

            Message::GetCheckpoint { .. } => {
                // TODO: Send latest accepted checkpoint + suffix + state
                tracing::debug!("Received checkpoint request (not yet implemented)");
            }

            Message::CheckpointSync { .. } => {
                // TODO: Verify and apply checkpoint for fast-sync
                tracing::debug!("Received checkpoint sync (not yet implemented)");
            }
        }
    }
}

fn hex_short(hash: &[u8; 32]) -> String {
    hash[..4].iter().map(|b| format!("{b:02x}")).collect()
}
