mod rpc;
mod validator;
mod rate_limit;
mod resource_monitor;

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use clap::Parser;
use tracing::{info, warn, error};

use ultradag_coin::{BlockDag, FinalityTracker, Mempool, SecretKey, StateEngine};
use ultradag_network::NodeServer;

#[derive(Parser)]
#[command(name = "ultradag-node", about = "UltraDAG full node")]
struct Args {
    /// Port to listen on for P2P
    #[arg(short, long, default_value = "9333")]
    port: u16,

    /// HTTP RPC port (P2P port + 1000 by default)
    #[arg(long)]
    rpc_port: Option<u16>,

    /// Seed peer addresses (host:port)
    #[arg(short, long)]
    seed: Vec<String>,

    /// Validator address (hex). If omitted, generates a new keypair.
    #[arg(short = 'v', long)]
    validator: Option<String>,

    /// Enable block production (validator mode)
    #[arg(long, default_value = "true")]
    validate: bool,

    /// Round duration in milliseconds
    #[arg(long, default_value = "5000")]
    round_ms: u64,

    /// Expected number of validators (fixes quorum threshold for testnet).
    /// Prevents phantom validator registrations from inflating the quorum.
    #[arg(long)]
    validators: Option<usize>,

    /// File containing allowed validator addresses (one hex address per line).
    /// When set, only listed validators count toward quorum/finality.
    /// Other nodes can connect, sync, and submit transactions as observers.
    #[arg(long)]
    validator_key: Option<String>,

    /// Data directory for persistence
    #[arg(long, default_value_t = default_data_dir())]
    data_dir: String,

    /// Disable automatic connection to public testnet bootstrap nodes.
    /// Use this for local development or private networks.
    #[arg(long)]
    no_bootstrap: bool,

    /// Number of finalized rounds to keep in the DAG.
    /// Older rounds are pruned to bound memory usage.
    #[arg(long, default_value = "1000")]
    pruning_depth: u64,

    /// Disable pruning. Keep full DAG history.
    /// Useful for archive nodes and block explorers.
    /// WARNING: Significantly increases memory and disk usage over time.
    #[arg(long)]
    archive: bool,

    /// Skip fast-sync on startup. Use local state only.
    #[arg(long)]
    skip_fast_sync: bool,
}

fn default_data_dir() -> String {
    std::env::var("HOME")
        .map(|h| PathBuf::from(h).join(".ultradag").join("node"))
        .unwrap_or_else(|_| PathBuf::from(".ultradag/node"))
        .to_string_lossy()
        .to_string()
}

/// Save all node state to disk.
async fn save_state(server: &NodeServer, data_dir: &std::path::Path) {
    let dag_path = data_dir.join("dag.json");
    let finality_path = data_dir.join("finality.json");
    let state_path = data_dir.join("state.json");
    let mempool_path = data_dir.join("mempool.json");

    let dag = server.dag.read().await;
    if let Err(e) = dag.save(&dag_path) {
        warn!("Failed to save DAG: {}", e);
    }
    drop(dag);

    let fin = server.finality.read().await;
    if let Err(e) = fin.save(&finality_path) {
        warn!("Failed to save finality: {}", e);
    }
    drop(fin);

    let state = server.state.read().await;
    if let Err(e) = state.save(&state_path) {
        warn!("Failed to save state: {}", e);
    }
    drop(state);

    let mp = server.mempool.read().await;
    if let Err(e) = mp.save(&mempool_path) {
        warn!("Failed to save mempool: {}", e);
    }
    drop(mp);

    info!("State saved to {}", data_dir.display());
}

/// Load all node state from disk if available.
async fn load_state(server: &NodeServer, data_dir: &std::path::Path) {
    let dag_path = data_dir.join("dag.json");
    let finality_path = data_dir.join("finality.json");
    let state_path = data_dir.join("state.json");
    let mempool_path = data_dir.join("mempool.json");

    if BlockDag::exists(&dag_path) {
        match BlockDag::load(&dag_path) {
            Ok(dag) => {
                *server.dag.write().await = dag;
                info!("Loaded DAG from disk");
            }
            Err(e) => warn!("Failed to load DAG: {}", e),
        }
    }

    if FinalityTracker::exists(&finality_path) {
        match FinalityTracker::load(&finality_path, 3) {
            Ok(fin) => {
                *server.finality.write().await = fin;
                info!("Loaded finality state from disk");
            }
            Err(e) => warn!("Failed to load finality: {}", e),
        }
    }

    if StateEngine::exists(&state_path) {
        match StateEngine::load(&state_path) {
            Ok(state) => {
                *server.state.write().await = state;
                info!("Loaded state engine from disk");
            }
            Err(e) => warn!("Failed to load state: {}", e),
        }
    }

    if Mempool::exists(&mempool_path) {
        match Mempool::load(&mempool_path) {
            Ok(mp) => {
                *server.mempool.write().await = mp;
                info!("Loaded mempool from disk");
            }
            Err(e) => warn!("Failed to load mempool: {}", e),
        }
    }
}

/// Connect to a peer with exponential backoff retry.
/// Tries up to `max_retries` times with delays of 2, 4, 8, 16, 32 seconds.
async fn connect_with_retry(server: &Arc<NodeServer>, addr: &str, max_retries: u32) -> bool {
    for attempt in 0..=max_retries {
        match server.connect_to(addr).await {
            Ok(()) => {
                info!("Connected to bootstrap node {}", addr);
                return true;
            }
            Err(e) => {
                if attempt < max_retries {
                    let delay = Duration::from_secs(2u64.pow(attempt + 1));
                    warn!(
                        "Failed to connect to {} (attempt {}/{}): {}. Retrying in {}s...",
                        addr,
                        attempt + 1,
                        max_retries + 1,
                        e,
                        delay.as_secs()
                    );
                    tokio::time::sleep(delay).await;
                } else {
                    warn!(
                        "Failed to connect to {} after {} attempts: {}",
                        addr,
                        max_retries + 1,
                        e
                    );
                }
            }
        }
    }
    false
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("ultradag=info".parse().unwrap()),
        )
        .init();

    let args = Args::parse();

    let data_dir = PathBuf::from(&args.data_dir);
    std::fs::create_dir_all(&data_dir).expect("Failed to create data directory");

    // Load or generate validator keypair (persisted to data_dir/validator.key)
    let key_path = data_dir.join("validator.key");
    let validator_sk = if key_path.exists() {
        let hex = std::fs::read_to_string(&key_path).expect("Failed to read validator key");
        let hex = hex.trim();
        let mut bytes = [0u8; 32];
        for (i, chunk) in hex.as_bytes().chunks(2).enumerate() {
            bytes[i] = u8::from_str_radix(std::str::from_utf8(chunk).unwrap(), 16).unwrap();
        }
        let sk = SecretKey::from_bytes(bytes);
        info!("Loaded validator keypair from disk:");
        info!("  Address: {}", sk.address().to_hex());
        sk
    } else {
        let sk = SecretKey::generate();
        let sk_hex: String = sk.to_bytes().iter().map(|b| format!("{b:02x}")).collect();
        std::fs::write(&key_path, &sk_hex).expect("Failed to save validator key");
        info!("Generated and saved validator keypair:");
        info!("  Secret key: {}", sk_hex);
        info!("  Address:    {}", sk.address().to_hex());
        sk
    };

    let mut server_inner = NodeServer::new(args.port);
    server_inner.set_data_dir(data_dir.clone());
    server_inner.set_validator_sk(validator_sk.clone());
    let server = Arc::new(server_inner);

    // Load persisted state
    load_state(&server, &data_dir).await;

    // Load permissioned validator allowlist BEFORE rebuilding validator set from DAG.
    // This ensures the allowlist gates which validators get registered during rebuild.
    // set_allowed_validators also purges any already-registered non-allowed validators.
    if let Some(ref key_file) = args.validator_key {
        let content = std::fs::read_to_string(key_file)
            .unwrap_or_else(|e| panic!("Failed to read validator key file {}: {}", key_file, e));
        let mut allowed = std::collections::HashSet::new();
        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if line.len() != 64 {
                warn!("Skipping invalid validator address (expected 64 hex chars): {}", line);
                continue;
            }
            let mut bytes = [0u8; 32];
            let valid = line.as_bytes().chunks(2).enumerate().all(|(i, chunk)| {
                u8::from_str_radix(std::str::from_utf8(chunk).unwrap_or("xx"), 16)
                    .map(|b| { bytes[i] = b; })
                    .is_ok()
            });
            if !valid {
                warn!("Skipping invalid hex in validator address: {}", line);
                continue;
            }
            let addr = ultradag_coin::Address(bytes);
            allowed.insert(addr);
        }
        if allowed.is_empty() {
            warn!("Validator key file {} contains no valid addresses!", key_file);
        } else {
            let count = allowed.len();
            let mut fin = server.finality.write().await;
            fin.set_allowed_validators(allowed);
            fin.set_configured_validators(count);
            drop(fin);
            info!("Loaded {} allowed validators from {} (quorum threshold fixed)", count, key_file);
        }
    }

    // Rebuild validator set from DAG vertices (not from persisted snapshot).
    // The allowlist (if set above) gates which validators get registered.
    {
        let dag = server.dag.read().await;
        let mut fin = server.finality.write().await;
        let validators = dag.all_validators();
        for addr in &validators {
            fin.register_validator(*addr);
        }
        let registered = fin.validator_count();
        if !validators.is_empty() {
            info!("Rebuilt validator set from DAG: {}/{} validators registered", registered, validators.len());
        }
    }

    // Set configured validator count (only if --validator-key wasn't used, since that sets it already).
    if args.validator_key.is_none() {
        if let Some(n) = args.validators {
            let mut fin = server.finality.write().await;
            fin.set_configured_validators(n);
            drop(fin);
            info!("Configured validator count: {} (quorum threshold fixed)", n);
        }
    }

    // Start RPC server
    let rpc_port = args.rpc_port.unwrap_or(args.port + 1000);
    let rpc_server = server.clone();
    tokio::spawn(async move {
        info!("Starting RPC server on port {}...", rpc_port);
        rpc::start_rpc(rpc_server, rpc_port).await;
        error!("RPC server task exited unexpectedly!");
    });

    // Determine seed peers: explicit --seed, bootstrap nodes, or none
    let seeds: Vec<String> = if !args.seed.is_empty() {
        args.seed.clone()
    } else if !args.no_bootstrap {
        ultradag_network::TESTNET_BOOTSTRAP_NODES
            .iter()
            .map(|s| s.to_string())
            .collect()
    } else {
        vec![]
    };

    let use_bootstrap = args.seed.is_empty() && !args.no_bootstrap;

    if use_bootstrap && !seeds.is_empty() {
        info!("No --seed provided, connecting to {} public bootstrap nodes...", seeds.len());
        let server_clone = server.clone();
        let bootstrap_seeds = seeds.clone();
        tokio::spawn(async move {
            let mut connected_any = false;
            for addr in &bootstrap_seeds {
                if connect_with_retry(&server_clone, addr, 4).await {
                    connected_any = true;
                }
            }
            if !connected_any {
                warn!("Could not connect to any bootstrap node. Running in isolation.");
                warn!("Use --seed <addr:port> to connect to a specific peer.");
            }
        });
    } else if !seeds.is_empty() {
        // Connect to explicit seed peers with retry
        let server_clone = server.clone();
        let seed_list = seeds.clone();
        tokio::spawn(async move {
            for addr in &seed_list {
                if connect_with_retry(&server_clone, addr, 4).await {
                    info!("Connected to seed {}", addr);
                }
            }
        });
    }

    // Attempt fast-sync from checkpoint if we're behind
    if !args.skip_fast_sync {
        let server_clone = server.clone();
        tokio::spawn(async move {
            // Wait for peer connections to establish
            tokio::time::sleep(Duration::from_secs(5)).await;
            server_clone.request_fast_sync().await;
        });
    } else {
        info!("Fast-sync disabled (--skip-fast-sync)");
    }

    // Start validator loop
    let cancel = Arc::new(AtomicBool::new(false));
    if args.validate {
        let server_clone = server.clone();
        let cancel_clone = cancel.clone();
        let round_duration = Duration::from_millis(args.round_ms);
        let dd = data_dir.clone();
        tokio::spawn(async move {
            validator::validator_loop(server_clone, validator_sk, round_duration, cancel_clone, dd).await;
        });
    }

    // Periodic peer exchange for mesh topology (every 30 seconds)
    {
        let peer_server = server.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(30));
            interval.tick().await; // skip immediate tick
            loop {
                interval.tick().await;
                peer_server
                    .peers
                    .broadcast(&ultradag_network::Message::GetPeers, "")
                    .await;
            }
        });
    }

    // Periodic reconnection: if peer count drops below threshold,
    // re-attempt connections to seed/bootstrap nodes. This ensures nodes recover
    // from network partitions, deploy restarts, and transient failures.
    if !seeds.is_empty() {
        let reconnect_server = server.clone();
        let reconnect_seeds = seeds.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(60));
            interval.tick().await; // skip immediate tick
            loop {
                interval.tick().await;
                let peer_count = reconnect_server.peers.connected_count().await;
                if peer_count < 3 {
                    info!("Low peer count ({}) — re-attempting seed connections...", peer_count);
                    for addr in &reconnect_seeds {
                        if let Err(e) = reconnect_server.connect_to(addr).await {
                            warn!("Reconnect to {} failed: {}", addr, e);
                        }
                    }
                }
            }
        });
    }

    // Listen for incoming connections (blocks forever)
    info!("UltraDAG node starting on port {}", args.port);
    info!("RPC available at http://127.0.0.1:{}", rpc_port);
    info!("Round duration: {}ms", args.round_ms);
    info!("Data directory: {}", data_dir.display());
    info!("DAG round: {}", server.dag.read().await.current_round());
    if use_bootstrap {
        info!("Bootstrap: connecting to public testnet nodes");
    } else if !args.seed.is_empty() {
        info!("Seeds: {:?}", args.seed);
    } else {
        info!("Bootstrap: disabled (--no-bootstrap). Running in isolation.");
    }

    info!("Validator key persisted at: {}", key_path.display());

    // Set up SIGTERM/SIGINT handler for graceful shutdown with persistence
    let shutdown_server = server.clone();
    let shutdown_cancel = cancel.clone();
    let shutdown_dir = data_dir.clone();
    tokio::spawn(async move {
        tokio::signal::ctrl_c().await.ok();
        info!("Received shutdown signal, saving state...");
        shutdown_cancel.store(true, Ordering::Relaxed);
        save_state(&shutdown_server, &shutdown_dir).await;
        info!("Shutdown complete.");
        std::process::exit(0);
    });

    if let Err(e) = server.listen().await {
        error!("Server error: {}", e);
        cancel.store(true, Ordering::Relaxed);
        save_state(&server, &data_dir).await;
    }
}
