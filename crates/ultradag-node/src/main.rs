mod rpc;
mod validator;
mod rate_limit;

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use clap::Parser;
use tracing::{debug, info, warn, error};

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

    /// Validator private key (64-char hex). Use instead of auto-generated key.
    /// Overrides any key saved in data_dir/validator.key.
    #[arg(long)]
    pkey: Option<String>,

    /// Auto-stake this many UDAG after startup and sync.
    /// Submits a stake transaction if balance is sufficient and not already staked.
    /// Example: --auto-stake 10000 stakes 10,000 UDAG.
    #[arg(long)]
    auto_stake: Option<u64>,

    /// Enable testnet mode: exposes secret-key-in-body RPC endpoints
    /// (/tx, /stake, /unstake, /proposal, /vote, /faucet, /keygen).
    /// When disabled (mainnet), only /tx/submit (pre-signed) is accepted.
    #[arg(long, default_value = "true")]
    testnet: bool,
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
    let dag_path = data_dir.join("dag.bin");
    let finality_path = data_dir.join("finality.bin");
    let state_path = data_dir.join("state.redb");
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
    let dag_path = data_dir.join("dag.bin");
    let finality_path = data_dir.join("finality.bin");
    let state_path = data_dir.join("state.redb");
    let mempool_path = data_dir.join("mempool.json");

    info!("Loading state from: {}", data_dir.display());

    if BlockDag::exists(&dag_path) {
        match BlockDag::load(&dag_path) {
            Ok(dag) => {
                let current_round = dag.current_round();
                info!("Loaded DAG from disk: current_round={}", current_round);
                *server.dag.write().await = dag;
            }
            Err(e) => warn!("Failed to load DAG: {}", e),
        }
    } else {
        info!("No DAG state file found, starting fresh");
    }

    if FinalityTracker::exists(&finality_path) {
        match FinalityTracker::load(&finality_path, 3) {
            Ok(fin) => {
                let last_fin = fin.last_finalized_round();
                info!("Loaded finality state from disk: last_finalized_round={}", last_fin);
                *server.finality.write().await = fin;
            }
            Err(e) => warn!("Failed to load finality: {}", e),
        }
    } else {
        info!("No finality state file found, starting fresh");
    }

    if StateEngine::exists(&state_path) {
        match StateEngine::load(&state_path) {
            Ok(state) => {
                let supply = state.total_supply();
                let last_round = state.last_finalized_round();
                info!("Loaded state engine from disk: total_supply={}, last_finalized_round={:?}", supply, last_round);
                *server.state.write().await = state;
            }
            Err(e) => warn!("Failed to load state: {}", e),
        }
    } else {
        info!("No state file found, starting fresh");
    }

    if Mempool::exists(&mempool_path) {
        match Mempool::load(&mempool_path) {
            Ok(mp) => {
                let tx_count = mp.len();
                info!("Loaded mempool from disk: {} transactions", tx_count);
                *server.mempool.write().await = mp;
            }
            Err(e) => warn!("Failed to load mempool: {}", e),
        }
    } else {
        info!("No mempool file found, starting fresh");
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
                .add_directive("ultradag=info".parse().unwrap_or_else(|_| {
                    tracing::Level::INFO.into()
                })),
        )
        .init();

    let args = Args::parse();

    // Verify genesis checkpoint hash (panics on mainnet builds with placeholder hash)
    ultradag_coin::constants::verify_genesis_checkpoint_hash();

    // Validate CLI arguments
    if args.round_ms == 0 {
        error!("--round-ms must be at least 1 (recommended: 1000+)");
        std::process::exit(1);
    }
    if let Some(0) = args.validators {
        error!("--validators must be at least 1");
        std::process::exit(1);
    }
    if args.pruning_depth == 0 && !args.archive {
        error!("--pruning-depth must be at least 1 (use --archive to disable pruning)");
        std::process::exit(1);
    }

    let data_dir = PathBuf::from(&args.data_dir);
    if let Err(e) = std::fs::create_dir_all(&data_dir) {
        error!("Failed to create data directory {:?}: {}", data_dir, e);
        std::process::exit(1);
    }

    // Load validator keypair: --pkey flag > disk > generate new
    let key_path = data_dir.join("validator.key");
    let validator_sk = if let Some(ref pkey_hex) = args.pkey {
        let pkey_hex = pkey_hex.trim();
        if pkey_hex.len() != 64 || !pkey_hex.chars().all(|c| c.is_ascii_hexdigit()) {
            error!("--pkey must be exactly 64 hex characters");
            std::process::exit(1);
        }
        let mut bytes = [0u8; 32];
        for (i, chunk) in pkey_hex.as_bytes().chunks(2).enumerate() {
            let hex_str = std::str::from_utf8(chunk).unwrap_or_else(|_| {
                error!("Invalid UTF-8 in --pkey hex string");
                std::process::exit(1);
            });
            bytes[i] = u8::from_str_radix(hex_str, 16).unwrap_or_else(|_| {
                error!("Invalid hex digit in --pkey: {}", hex_str);
                std::process::exit(1);
            });
        }
        let sk = SecretKey::from_bytes(bytes);
        info!("Using validator keypair from --pkey:");
        info!("  Address: {}", sk.address().to_hex());
        sk
    } else if key_path.exists() {
        let hex = match std::fs::read_to_string(&key_path) {
            Ok(h) => h,
            Err(e) => {
                error!("Failed to read validator key from {:?}: {}", key_path, e);
                std::process::exit(1);
            }
        };
        let hex = hex.trim();
        let mut bytes = [0u8; 32];
        for (i, chunk) in hex.as_bytes().chunks(2).enumerate() {
            let hex_str = std::str::from_utf8(chunk).unwrap_or_else(|_| {
                error!("Invalid UTF-8 in validator key file");
                std::process::exit(1);
            });
            bytes[i] = u8::from_str_radix(hex_str, 16).unwrap_or_else(|_| {
                error!("Invalid hex digit in validator key file: {}", hex_str);
                std::process::exit(1);
            });
        }
        let sk = SecretKey::from_bytes(bytes);
        info!("Loaded validator keypair from disk:");
        info!("  Address: {}", sk.address().to_hex());
        sk
    } else {
        let sk = SecretKey::generate();
        let sk_hex: String = sk.to_bytes().iter().map(|b| format!("{b:02x}")).collect();
        if let Err(e) = std::fs::write(&key_path, &sk_hex) {
            error!("Failed to save validator key to {:?}: {}", key_path, e);
            std::process::exit(1);
        }
        info!("Generated and saved validator keypair:");
        debug!("  Secret key: {}", sk_hex);
        info!("  Address:    {}", sk.address().to_hex());
        sk
    };

    // Determine seed peers early so we can pass them to NodeServer for heartbeat reconnection
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

    let mut server_inner = NodeServer::new(args.port);
    server_inner.set_data_dir(data_dir.clone());
    server_inner.set_validator_sk(validator_sk.clone());
    server_inner.pruning_depth = if args.archive { 0 } else { args.pruning_depth };
    server_inner.testnet_mode = args.testnet;
    server_inner.set_seed_addrs(seeds.clone());
    if !args.testnet {
        info!("Mainnet mode: secret-key RPC endpoints disabled. Use /tx/submit with pre-signed transactions.");
    }
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
            server.state.write().await.set_configured_validator_count(count as u64);
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

    // Reconcile FinalityTracker with StateEngine after restart.
    // The FinalityTracker and StateEngine persist last_finalized_round independently.
    // If the FinalityTracker advanced ahead (finalized vertices not applied to state),
    // reset it to the state engine's level so finality can re-discover and apply them.
    {
        let state_r = server.state.read().await;
        let state_fin = state_r.last_finalized_round().unwrap_or(0);
        drop(state_r);
        let mut fin = server.finality.write().await;
        let tracker_fin = fin.last_finalized_round();
        if tracker_fin > state_fin && state_fin > 0 {
            warn!("FinalityTracker ahead of StateEngine ({} vs {}), resetting to state level", tracker_fin, state_fin);
            fin.reset_to_checkpoint(state_fin);
            // Re-populate finalized set: vertices at or below state_fin that are in the DAG
            // should be considered finalized (they were applied to state).
            let dag = server.dag.read().await;
            for round in 0..=state_fin {
                for hash in dag.hashes_in_round(round) {
                    fin.mark_as_finalized(*hash);
                }
            }
        }
    }

    // Set configured validator count (only if --validator-key wasn't used, since that sets it already).
    if args.validator_key.is_none() {
        if let Some(n) = args.validators {
            let mut fin = server.finality.write().await;
            fin.set_configured_validators(n);
            drop(fin);
            server.state.write().await.set_configured_validator_count(n as u64);
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

    // Sync task: wait until our round is within 5 of peer_max_round.
    // Keeps retrying as long as progress is being made. Only gives up
    // after 60s of zero progress (truly stuck).
    if !args.skip_fast_sync {
        let server_clone = server.clone();
        tokio::spawn(async move {
            // Wait for peer connections to establish
            tokio::time::sleep(Duration::from_secs(10)).await;

            let mut last_progress_round = 0u64;
            let mut stall_count = 0u32;
            let mut hello_wait_count = 0u32;
            let mut checkpoint_attempts = 0u32;
            let sync_start = std::time::Instant::now();

            loop {
                let our_round = server_clone.dag.read().await.current_round();
                let peer_count = server_clone.peers.connected_count().await;
                let network_round = server_clone.peer_max_round.load(std::sync::atomic::Ordering::Relaxed);

                if peer_count == 0 {
                    info!("Sync: no peers connected yet, waiting...");
                    tokio::time::sleep(Duration::from_secs(5)).await;
                    continue;
                }

                // If we don't know the network round yet, wait for Hello/HelloAck.
                // But if peers are connected and still reporting round 0 after 15s,
                // this is a fresh network — nothing to sync.
                if network_round == 0 {
                    hello_wait_count += 1;
                    if hello_wait_count >= 3 && peer_count >= 2 {
                        info!("Sync complete: fresh network (all peers at round 0)");
                        server_clone.sync_complete.store(true, std::sync::atomic::Ordering::Relaxed);
                        break;
                    }
                    info!("Sync: waiting for peer Hello (our_round={})", our_round);
                    tokio::time::sleep(Duration::from_secs(5)).await;
                    continue;
                }

                // Check if we're caught up (within 5 rounds of network frontier)
                if our_round + 5 >= network_round {
                    info!("Sync complete: our_round={} network_round={} — caught up", our_round, network_round);
                    server_clone.sync_complete.store(true, std::sync::atomic::Ordering::Relaxed);
                    break;
                }

                let gap = network_round - our_round;

                // Hard timeout: if syncing for > 120s total, give up and produce
                if sync_start.elapsed() > Duration::from_secs(120) {
                    warn!("Sync timeout (120s) at round {} (network at {}) — enabling production", our_round, network_round);
                    server_clone.sync_complete.store(true, std::sync::atomic::Ordering::Relaxed);
                    break;
                }

                // Track progress — only reset stall counter on meaningful progress (10+ rounds)
                if our_round >= last_progress_round + 10 {
                    stall_count = 0;
                    last_progress_round = our_round;
                } else if our_round <= last_progress_round {
                    stall_count += 1;
                }
                // Small progress (1-9 rounds) doesn't reset stall counter

                // If stalled for 40s (4 x 10s checks), give up and produce
                if stall_count >= 4 {
                    warn!("Sync stalled for 40s at round {} (network at {}) — enabling production", our_round, network_round);
                    server_clone.sync_complete.store(true, std::sync::atomic::Ordering::Relaxed);
                    break;
                }

                // Request vertices — try checkpoint first for large gaps (max 3 attempts)
                if gap > 100 && checkpoint_attempts < 3 {
                    checkpoint_attempts += 1;
                    info!("Sync: our_round={} network={} gap={} — requesting checkpoint (attempt {})",
                        our_round, network_round, gap, checkpoint_attempts);
                    server_clone.request_fast_sync().await;
                } else {
                    // Use incremental sync (always works if peers have the data)
                    info!("Sync: requesting vertices round={} network={} gap={}", our_round, network_round, gap);
                    let from = our_round.saturating_add(1);
                    server_clone.peers.broadcast(
                        &ultradag_network::Message::GetDagVertices { from_round: from, max_count: 50 },
                        "",
                    ).await;
                }

                tokio::time::sleep(Duration::from_secs(10)).await;
            }
        });
    } else {
        info!("Fast-sync disabled (--skip-fast-sync)");
        server.sync_complete.store(true, std::sync::atomic::Ordering::Relaxed);
    }

    // Auto-stake: submit a stake transaction after sync if requested
    if let Some(auto_stake_udag) = args.auto_stake {
        let auto_stake_sats = auto_stake_udag.saturating_mul(ultradag_coin::SATS_PER_UDAG);
        let server_clone = server.clone();
        let auto_sk = validator_sk.clone();
        tokio::spawn(async move {
            // Wait for fast-sync and peer connections to settle
            tokio::time::sleep(Duration::from_secs(20)).await;

            let sender = auto_sk.address();

            // Check minimum stake (static check, no locks needed)
            if auto_stake_sats < ultradag_coin::MIN_STAKE_SATS {
                warn!("Auto-stake: {} UDAG below minimum stake of {} UDAG",
                    auto_stake_udag,
                    ultradag_coin::MIN_STAKE_SATS / ultradag_coin::SATS_PER_UDAG);
                return;
            }

            // Atomic check-and-insert: hold state read + mempool write together
            // to prevent TOCTOU between balance check and mempool insertion.
            let tx = {
                let state = server_clone.state.read().await;

                // Check if already staked
                let current_stake = state.stake_of(&sender);
                if current_stake >= ultradag_coin::MIN_STAKE_SATS {
                    info!("Auto-stake: already staked {} UDAG, skipping",
                        current_stake / ultradag_coin::SATS_PER_UDAG);
                    return;
                }

                // Check balance including pending mempool costs
                let mut mp = server_clone.mempool.write().await;
                let pending_cost: u64 = mp.best(10_000)
                    .iter()
                    .filter(|t| t.from() == sender)
                    .map(|t| t.total_cost())
                    .fold(0u64, |acc, x| acc.saturating_add(x));
                let total_needed = pending_cost
                    .saturating_add(auto_stake_sats)
                    .saturating_add(ultradag_coin::constants::MIN_FEE_SATS);
                let balance = state.balance(&sender);
                if balance < total_needed {
                    warn!("Auto-stake: balance {} UDAG insufficient for stake of {} UDAG (need {} sats incl. pending+fee, have {} sats)",
                        balance / 100_000_000,
                        auto_stake_udag,
                        total_needed,
                        balance);
                    return;
                }

                // Compute nonce under same lock
                let base_nonce = state.nonce(&sender);
                let nonce = match mp.pending_nonce(&sender) {
                    Some(max_pending) => max_pending.saturating_add(1),
                    None => base_nonce,
                };

                // Build, sign, and insert under same lock scope
                let mut stake_tx = ultradag_coin::StakeTx {
                    from: sender,
                    amount: auto_stake_sats,
                    nonce,
                    pub_key: auto_sk.verifying_key().to_bytes(),
                    signature: ultradag_coin::Signature([0u8; 64]),
                };
                stake_tx.signature = auto_sk.sign(&stake_tx.signable_bytes());
                let tx = ultradag_coin::Transaction::Stake(stake_tx);

                if !mp.insert(tx.clone()) {
                    warn!("Auto-stake: failed to insert stake tx into mempool");
                    return;
                }

                tx
            };

            // Broadcast to peers
            server_clone.peers.broadcast(&ultradag_network::Message::NewTx(tx.clone()), "").await;
            let _ = server_clone.tx_tx.send(tx);

            let current_epoch = {
                let state = server_clone.state.read().await;
                state.current_epoch()
            };
            let next_epoch_round = (current_epoch + 1) * ultradag_coin::constants::EPOCH_LENGTH_ROUNDS;
            info!("Auto-stake: submitted stake of {} UDAG, will be active at next epoch boundary (round {})",
                auto_stake_udag, next_epoch_round);
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

    // Heartbeat: detect and remove dead TCP connections every 30 seconds
    server.start_heartbeat();

    if let Err(e) = server.listen().await {
        error!("Server error: {}", e);
        cancel.store(true, Ordering::Relaxed);
        save_state(&server, &data_dir).await;
    }
}
