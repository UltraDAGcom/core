mod rpc;
mod validator;

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use clap::Parser;
use tracing::{info, warn, error};

use ultradag_coin::{Address, BlockDag, FinalityTracker, Mempool, SecretKey, StateEngine};
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

    /// Data directory for persistence
    #[arg(long, default_value_t = default_data_dir())]
    data_dir: String,
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

    let (validator_sk, secret_hex) = match &args.validator {
        Some(hex) => {
            let _addr = Address::from_hex(hex).expect("invalid validator address hex");
            let sk = SecretKey::generate();
            let sk_hex = sk.to_bytes().iter().map(|b| format!("{b:02x}")).collect::<String>();
            info!("Note: --validator flag provided but signing requires a keypair. Generated fresh keypair.");
            info!("  Secret key: {}", sk_hex);
            info!("  Address:    {}", sk.address().to_hex());
            (sk, Some(sk_hex))
        }
        None => {
            let sk = SecretKey::generate();
            let sk_hex = sk.to_bytes().iter().map(|b| format!("{b:02x}")).collect::<String>();
            info!("Generated validator keypair:");
            info!("  Secret key: {}", sk_hex);
            info!("  Address:    {}", sk.address().to_hex());
            (sk, Some(sk_hex))
        }
    };

    let server = Arc::new(NodeServer::new(args.port));

    // Load persisted state
    load_state(&server, &data_dir).await;

    // Rebuild validator set from DAG vertices (not from persisted snapshot).
    // Prevents stale validators from previous runs inflating the quorum threshold.
    {
        let dag = server.dag.read().await;
        let mut fin = server.finality.write().await;
        let validators = dag.all_validators();
        for addr in &validators {
            fin.register_validator(*addr);
        }
        if !validators.is_empty() {
            info!("Rebuilt validator set from DAG: {} validators", validators.len());
        }
    }

    // Set configured validator count AFTER loading state (load replaces the FinalityTracker).
    // This fixes the quorum threshold so phantom registrations can't inflate it.
    if let Some(n) = args.validators {
        let mut fin = server.finality.write().await;
        fin.set_configured_validators(n);
        drop(fin);
        info!("Configured validator count: {} (quorum threshold fixed)", n);
    }

    // Start RPC server
    let rpc_port = args.rpc_port.unwrap_or(args.port + 1000);
    let rpc_server = server.clone();
    tokio::spawn(async move {
        rpc::start_rpc(rpc_server, rpc_port).await;
    });

    // Connect to seed peers
    for seed in &args.seed {
        let s = server.clone();
        let addr = seed.clone();
        tokio::spawn(async move {
            if let Err(e) = s.connect_to(&addr).await {
                error!("Failed to connect to seed {}: {}", addr, e);
            }
        });
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

    // Listen for incoming connections (blocks forever)
    info!("UltraDAG node starting on port {}", args.port);
    info!("RPC available at http://127.0.0.1:{}", rpc_port);
    info!("Round duration: {}ms", args.round_ms);
    info!("Data directory: {}", data_dir.display());
    info!("DAG round: {}", server.dag.read().await.current_round());

    if let Some(sk) = &secret_hex {
        info!("Save your secret key to send transactions: {}", sk);
    }

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
