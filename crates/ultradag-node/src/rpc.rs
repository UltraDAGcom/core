use std::sync::Arc;
use std::net::SocketAddr;
use std::time::Duration;

use http_body_util::{BodyExt, Full};
use hyper::body::{Bytes, Incoming};
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{Method, Request, Response, StatusCode};
use hyper_util::rt::TokioIo;
use serde::{Deserialize, Serialize};
use tokio::net::TcpListener;
use tracing::{error, info, warn};

/// Timeout for RPC lock acquisition — prevents blocking when P2P sync holds write locks.
/// Set to 60 seconds to handle slow sync operations without spurious 503 errors.
const RPC_LOCK_TIMEOUT: Duration = Duration::from_secs(60);

/// Acquire a read lock with timeout. Returns 503 if the lock can't be acquired.
macro_rules! read_lock_or_503 {
    ($lock:expr) => {
        match tokio::time::timeout(RPC_LOCK_TIMEOUT, $lock.read()).await {
            Ok(guard) => guard,
            Err(_) => return Ok(error_response(StatusCode::SERVICE_UNAVAILABLE, "node busy, try again")),
        }
    };
}

/// Acquire a write lock with timeout. Returns 503 if the lock can't be acquired.
macro_rules! write_lock_or_503 {
    ($lock:expr) => {
        match tokio::time::timeout(RPC_LOCK_TIMEOUT, $lock.write()).await {
            Ok(guard) => guard,
            Err(_) => return Ok(error_response(StatusCode::SERVICE_UNAVAILABLE, "node busy, try again")),
        }
    };
}

use ultradag_coin::{Address, Mempool, SecretKey, Signature, StateEngine, Transaction, TransferTx, StakeTx, UnstakeTx, DelegateTx, UndelegateTx, SetCommissionTx, BridgeDepositTx, MIN_STAKE_SATS};
use ultradag_coin::governance::{CreateProposalTx, VoteTx, ProposalType, CouncilAction, CouncilSeatCategory};
use ultradag_network::{Message, NodeServer};
use crate::rate_limit::{RateLimiter, limits};

type BoxBody = Full<Bytes>;

/// Max transactions to scan in mempool for pending cost calculation
const MAX_MEMPOOL_SCAN: usize = 10_000;

/// Max request body size (1MB)
const MAX_REQUEST_SIZE: usize = 1_048_576;

fn json_response(status: StatusCode, json: &impl Serialize) -> Response<BoxBody> {
    let json = serde_json::to_string_pretty(json).unwrap_or_else(|e| {
        format!("{{\"error\": \"serialization failed: {}\"}}", e)
    });
    Response::builder()
        .status(status)
        .header("Content-Type", "application/json")
        .header("Access-Control-Allow-Origin", "*")
        .header("Access-Control-Allow-Methods", "GET, POST, OPTIONS")
        .header("Access-Control-Allow-Headers", "Content-Type")
        .header("X-Content-Type-Options", "nosniff")
        .header("Cache-Control", "no-store")
        .body(Full::new(Bytes::from(json)))
        .unwrap_or_else(|e| {
            tracing::error!("Failed to build JSON response: {}", e);
            Response::new(Full::new(Bytes::from("{\"error\": \"response build failed\"}")))
        })
}

fn error_response(status: StatusCode, msg: &str) -> Response<BoxBody> {
    json_response(status, &serde_json::json!({"error": msg}))
}

/// Check if the TCP peer address is a trusted proxy that we should accept
/// forwarded headers from. Trusted proxies include loopback, private networks
/// (RFC 1918 / RFC 4193), and Fly.io's internal network (fdaa::/16).
/// An attacker connecting directly from a public IP can spoof X-Forwarded-For
/// headers to bypass rate limits, so we only trust these headers from known proxies.
pub fn is_trusted_proxy(ip: std::net::IpAddr) -> bool {
    match ip {
        std::net::IpAddr::V4(v4) => {
            v4.is_loopback()          // 127.0.0.0/8
            || v4.is_private()        // 10.0.0.0/8, 172.16.0.0/12, 192.168.0.0/16
            || v4.is_link_local()     // 169.254.0.0/16
        }
        std::net::IpAddr::V6(v6) => {
            v6.is_loopback()          // ::1
            // Fly.io internal network: fdaa::/16
            || v6.segments()[0] == 0xfdaa
            // General unique-local addresses: fc00::/7
            || (v6.segments()[0] & 0xfe00) == 0xfc00
            // IPv4-mapped loopback/private (e.g., ::ffff:127.0.0.1)
            || v6.to_ipv4_mapped().map(|v4| v4.is_loopback() || v4.is_private()).unwrap_or(false)
        }
    }
}

/// Parse a 64-hex-char secret key string into a SecretKey.
fn parse_secret_key(hex: &str) -> Result<SecretKey, &'static str> {
    if hex.len() != 64 {
        return Err("secret key must be 64 hex chars (32 bytes)");
    }
    if hex.contains('\0') || !hex.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err("invalid hex in secret key");
    }
    let mut bytes = [0u8; 32];
    for (i, chunk) in hex.as_bytes().chunks(2).enumerate() {
        let s = std::str::from_utf8(chunk).map_err(|_| "invalid hex")?;
        bytes[i] = u8::from_str_radix(s, 16).map_err(|_| "invalid hex")?;
    }
    Ok(SecretKey::from_bytes(bytes))
}

/// Compute the next nonce for a sender, accounting for pending mempool transactions.
fn next_nonce(state: &StateEngine, mp: &Mempool, sender: &Address) -> u64 {
    let base_nonce = state.nonce(sender);
    match mp.pending_nonce(sender) {
        Some(max_pending) => max_pending.saturating_add(1),
        None => base_nonce,
    }
}

/// Calculate total cost of pending transactions from a sender in the mempool.
/// Uses the per-sender index for O(K) lookup instead of O(N) full scan.
fn pending_cost(mp: &Mempool, sender: &Address) -> u64 {
    mp.pending_cost_for(sender)
}

#[derive(Serialize, Clone)]
struct StatusResponse {
    last_finalized_round: Option<u64>,
    peer_count: usize,
    mempool_size: usize,
    total_supply: u64,
    account_count: usize,
    dag_vertices: usize,
    dag_round: u64,
    dag_tips: usize,
    finalized_count: usize,
    validator_count: usize,
    total_staked: u64,
    active_stakers: usize,
    treasury_balance: u64,
    bootstrap_connected: bool,
    // System resource metrics
    #[serde(skip_serializing_if = "Option::is_none")]
    memory_usage_bytes: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    cpu_usage_percent: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    uptime_seconds: Option<u64>,
    // DAG memory statistics
    #[serde(skip_serializing_if = "Option::is_none")]
    dag_memory_stats: Option<DagMemoryStats>,
}

/// DAG memory statistics for monitoring.
#[derive(Serialize, Clone)]
struct DagMemoryStats {
    vertex_count: usize,
    equivocation_vertex_count: usize,
    children_map_count: usize,
    total_children_entries: usize,
    tips_count: usize,
    rounds_count: usize,
    descendant_validators_count: usize,
    total_descendant_bitmap_bits: usize,
    validator_index_count: usize,
    validator_round_vertex_count: usize,
    byzantine_validators_count: usize,
    equivocation_evidence_count: usize,
    evidence_store_validators: usize,
    evidence_store_entries: usize,
    pruning_floor: u64,
    current_round: u64,
}

/// Cached /status response — serves last good data when locks are contended.
static STATUS_CACHE: std::sync::OnceLock<tokio::sync::Mutex<Option<StatusResponse>>> = std::sync::OnceLock::new();
fn status_cache() -> &'static tokio::sync::Mutex<Option<StatusResponse>> {
    STATUS_CACHE.get_or_init(|| tokio::sync::Mutex::new(None))
}

/// Get current process memory usage in bytes (cached 30s to avoid subprocess spam).
fn get_memory_usage() -> Option<u64> {
    use std::time::Instant;

    static MEMORY_CACHE: tokio::sync::Mutex<(Option<u64>, Option<Instant>)> =
        tokio::sync::Mutex::const_new((None, None));

    // Try non-blocking acquire to avoid stalling RPC on contention
    let mut cache = match MEMORY_CACHE.try_lock() {
        Ok(guard) => guard,
        Err(_) => return None,
    };

    if let Some(cached_time) = cache.1 {
        if cached_time.elapsed() < Duration::from_secs(30) {
            return cache.0;
        }
    }

    let memory = read_process_memory();
    *cache = (memory, Some(Instant::now()));
    memory
}

/// Read process RSS from OS-specific source.
fn read_process_memory() -> Option<u64> {
    #[cfg(target_os = "linux")]
    {
        std::fs::read_to_string("/proc/self/status")
            .ok()
            .and_then(|content| {
                content.lines()
                    .find(|line| line.starts_with("VmRSS:"))
                    .and_then(|line| {
                        line.split_whitespace()
                            .nth(1)
                            .and_then(|kb| kb.parse::<u64>().ok())
                            .map(|kb| kb * 1024)
                    })
            })
    }
    #[cfg(target_os = "macos")]
    {
        use std::process::Command;
        Command::new("ps")
            .args(["-o", "rss=", "-p", &std::process::id().to_string()])
            .output()
            .ok()
            .and_then(|output| {
                String::from_utf8(output.stdout)
                    .ok()
                    .and_then(|s| s.trim().parse::<u64>().ok())
                    .map(|kb| kb * 1024)
            })
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        None
    }
}

/// Get current process CPU usage percentage (best-effort, returns None on failure)
fn get_cpu_usage() -> Option<f32> {
    // CPU usage requires sampling over time, which is complex for a single call
    // For now, return None - can be enhanced with a background sampling thread
    None
}

/// Get process uptime in seconds.
fn get_uptime() -> Option<u64> {
    static PROCESS_START: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
    let start = PROCESS_START.get_or_init(std::time::Instant::now);
    Some(start.elapsed().as_secs())
}

/// Unused — kept for reference.
#[allow(dead_code)]
fn get_system_uptime() -> Option<u64> {
    #[cfg(target_os = "linux")]
    {
        std::fs::read_to_string("/proc/uptime")
            .ok()
            .and_then(|content| {
                content.split_whitespace()
                    .next()
                    .and_then(|s| s.parse::<f64>().ok())
                    .map(|seconds| seconds as u64)
            })
    }
    #[cfg(target_os = "macos")]
    {
        use std::process::Command;
        Command::new("sysctl")
            .args(["-n", "kern.boottime"])
            .output()
            .ok()
            .and_then(|output| {
                String::from_utf8(output.stdout)
                    .ok()
                    .and_then(|s| {
                        s.split("sec = ")
                            .nth(1)
                            .and_then(|s| s.split(',').next())
                            .and_then(|s| s.trim().parse::<u64>().ok())
                            .map(|boot_time| {
                                std::time::SystemTime::now()
                                    .duration_since(std::time::UNIX_EPOCH)
                                    .unwrap_or_default()
                                    .as_secs()
                                    .saturating_sub(boot_time)
                            })
                    })
            })
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        None
    }
}

#[derive(Serialize)]
struct BootstrapNodeStatus {
    addr: String,
    connected: bool,
}

#[derive(Serialize)]
struct PeersResponse {
    connected: usize,
    peers: Vec<String>,
    bootstrap_nodes: Vec<BootstrapNodeStatus>,
}

#[derive(Serialize)]
struct BalanceResponse {
    address: String,
    address_bech32: String,
    balance: u64,
    nonce: u64,
    balance_udag: f64,
    staked: u64,
    staked_udag: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    unlock_at_round: Option<u64>,
    is_council_member: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    delegated: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    delegated_udag: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    delegated_to: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    delegated_to_bech32: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    delegation_unlock_at_round: Option<u64>,
}

#[derive(Serialize)]
struct VertexResponse {
    round: u64,
    hash: String,
    validator: String,
    reward: u64,
    tx_count: usize,
    parent_count: usize,
}

#[derive(Deserialize)]
struct FaucetRequest {
    address: String,
    amount: u64,
}

#[derive(Deserialize)]
struct StakeRequest {
    secret_key: String,
    amount: u64,
}

#[derive(Deserialize)]
struct UnstakeRequest {
    secret_key: String,
}

#[derive(Deserialize)]
struct ProposalRequest {
    #[serde(alias = "proposer_secret")]
    secret_key: String,
    title: String,
    description: String,
    proposal_type: String,
    #[serde(default)]
    parameter_name: Option<String>,
    #[serde(default)]
    parameter_value: Option<String>,
    /// Council membership: "add" or "remove"
    #[serde(default)]
    council_action: Option<String>,
    /// Council membership: target address (hex)
    #[serde(default)]
    council_address: Option<String>,
    /// Council membership: seat category (technical, business, legal, academic, community, foundation)
    #[serde(default)]
    council_category: Option<String>,
    /// TreasurySpend: recipient address (hex)
    #[serde(default)]
    treasury_recipient: Option<String>,
    /// TreasurySpend: amount in sats
    #[serde(default)]
    treasury_amount: Option<u64>,
    #[serde(default)]
    fee: Option<u64>,
}

#[derive(Deserialize)]
struct VoteRequest {
    #[serde(alias = "voter_secret")]
    secret_key: String,
    proposal_id: u64,
    vote: bool,
    #[serde(default)]
    fee: Option<u64>,
}

#[derive(Serialize)]
struct StakeInfoResponse {
    address: String,
    staked: u64,
    staked_udag: f64,
    unlock_at_round: Option<u64>,
    is_active_validator: bool,
    effective_stake: u64,
    effective_stake_udag: f64,
    commission_percent: u8,
    delegator_count: usize,
    total_delegated: u64,
    total_delegated_udag: f64,
}

#[derive(Serialize)]
struct ValidatorInfo {
    address: String,
    address_bech32: String,
    staked: u64,
    staked_udag: f64,
    effective_stake: u64,
    effective_stake_udag: f64,
    commission_percent: u8,
    delegator_count: usize,
}

#[derive(Deserialize)]
struct DelegateRequest {
    secret_key: String,
    validator: String,
    amount: u64,
}

#[derive(Deserialize)]
struct UndelegateRequest {
    secret_key: String,
}

// ========================================
// CLIENT-SIDE SIGNING REQUEST TYPES
// For mainnet: transactions must be pre-signed
// ========================================

/// Pre-signed transaction submission (mainnet-compatible)
#[derive(Deserialize)]
struct SubmitTxRequest {
    /// Hex-encoded serialized transaction (postcard format)
    tx_hex: String,
}

/// Pre-signed stake transaction submission
#[derive(Deserialize)]
struct SubmitStakeRequest {
    /// Hex-encoded serialized StakeTx
    tx_hex: String,
}

/// Pre-signed unstake transaction submission
#[derive(Deserialize)]
struct SubmitUnstakeRequest {
    /// Hex-encoded serialized UnstakeTx
    tx_hex: String,
}

/// Pre-signed proposal creation
#[derive(Deserialize)]
struct SubmitProposalRequest {
    /// Hex-encoded serialized CreateProposalTx
    tx_hex: String,
}

/// Pre-signed vote submission
#[derive(Deserialize)]
struct SubmitVoteRequest {
    /// Hex-encoded serialized VoteTx
    tx_hex: String,
}

/// Pre-signed delegation submission
#[derive(Deserialize)]
struct SubmitDelegateRequest {
    /// Hex-encoded serialized DelegateTx
    tx_hex: String,
}

/// Pre-signed undelegation submission
#[derive(Deserialize)]
struct SubmitUndelegateRequest {
    /// Hex-encoded serialized UndelegateTx
    tx_hex: String,
}

/// Helper struct for deserializing raw transaction bytes
#[derive(Deserialize)]
struct RawTxHex {
    tx_hex: String,
}

#[derive(Deserialize)]
struct SetCommissionRequest {
    secret_key: String,
    commission_percent: u8,
}

#[derive(Deserialize)]
struct BridgeDepositRequest {
    secret_key: String,
    recipient: String,
    amount: u64,
    fee: u64,
    destination_chain_id: u64,
}

#[derive(Deserialize)]
struct SendTxRequest {
    secret_key: String,
    to: String,
    amount: u64,
    fee: u64,
    #[serde(default)]
    memo: Option<String>,
}

#[derive(Serialize)]
struct TxResponse {
    hash: String,
    from: String,
    to: String,
    amount: u64,
    fee: u64,
    nonce: u64,
}

async fn handle_request(
    req: Request<Incoming>,
    server: Arc<NodeServer>,
    rate_limiter: Arc<RateLimiter>,
    client_ip: std::net::IpAddr,
) -> Result<Response<BoxBody>, hyper::Error> {
    // Handle CORS preflight
    if req.method() == Method::OPTIONS {
        return Ok(Response::builder()
            .status(StatusCode::NO_CONTENT)
            .header("Access-Control-Allow-Origin", "*")
            .header("Access-Control-Allow-Methods", "GET, POST, OPTIONS")
            .header("Access-Control-Allow-Headers", "Content-Type")
            .header("Access-Control-Max-Age", "3600")
            .body(Full::new(Bytes::new()))
            .unwrap_or_else(|e| {
                tracing::error!("Failed to build CORS response: {}", e);
                Response::new(Full::new(Bytes::new()))
            }));
    }

    let method = req.method();
    let path: Vec<&str> = req.uri().path().trim_matches('/').split('/').collect();

    // Behind reverse proxies (e.g. Fly.io), peer_addr is the proxy IP, not the real client.
    // Only trust proxy headers when the TCP peer is a known proxy (loopback, private network,
    // or Fly.io internal fdaa::/16). If the TCP peer is a public IP, an attacker could spoof
    // these headers to bypass rate limits.
    let client_ip = if is_trusted_proxy(client_ip) {
        req.headers()
            .get("fly-client-ip")
            .or_else(|| req.headers().get("x-forwarded-for"))
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.split(',').next()) // X-Forwarded-For may be comma-separated
            .and_then(|s| s.trim().parse::<std::net::IpAddr>().ok())
            .unwrap_or(client_ip)
    } else {
        client_ip
    };

    // Check global rate limit
    if !rate_limiter.check_rate_limit(client_ip, limits::GLOBAL) {
        return Ok(error_response(
            StatusCode::TOO_MANY_REQUESTS,
            "rate limit exceeded: too many requests",
        ));
    }

    let response = match (method, path.as_slice()) {
        // Lock-free health check for Fly.io proxy — never blocks on DAG/state locks
        (&Method::GET, ["health"]) => {
            json_response(StatusCode::OK, &serde_json::json!({"status": "ok"}))
        }

        // Prometheus-compatible metrics endpoint
        (&Method::GET, ["metrics"]) => {
            // Collect metrics without blocking
            let dag_round = server.dag.try_read()
                .map(|d| d.current_round())
                .unwrap_or(0);
            let dag_vertices = server.dag.try_read()
                .map(|d| d.len())
                .unwrap_or(0);
            let pruning_floor = server.dag.try_read()
                .map(|d| d.pruning_floor())
                .unwrap_or(0);
            
            let finality_lag = server.dag.try_read()
                .and_then(|d| {
                    server.finality.try_read()
                        .map(|f| d.current_round().saturating_sub(f.last_finalized_round()))
                })
                .unwrap_or(0);
            
            let validator_count = server.finality.try_read()
                .map(|f| f.validator_count())
                .unwrap_or(0);
            
            let (total_supply, account_count, total_staked, active_validators, mempool_size) = 
                match (server.state.try_read(), server.mempool.try_read()) {
                    (Ok(state), Ok(mempool)) => (
                        state.total_supply(),
                        state.account_count(),
                        state.total_staked(),
                        state.active_validators().len(),
                        mempool.len(),
                    ),
                    _ => (0, 0, 0, 0, 0),
                };

            let peer_count = server.peers.peer_count().await;
            let ban_count = server.peers.ban_count().await;

            // Format as Prometheus metrics
            let checkpoint_metrics = server.checkpoint_metrics.export_prometheus();
            let metrics = format!(
                "# HELP ultradag_current_round Current DAG round number
# TYPE ultradag_current_round gauge
ultradag_current_round {dag_round}

# HELP ultradag_vertex_count Total vertices in DAG
# TYPE ultradag_vertex_count gauge
ultradag_vertex_count {dag_vertices}

# HELP ultradag_pruning_floor Pruning floor round
# TYPE ultradag_pruning_floor gauge
ultradag_pruning_floor {pruning_floor}

# HELP ultradag_finality_lag Rounds behind finality
# TYPE ultradag_finality_lag gauge
ultradag_finality_lag {finality_lag}

# HELP ultradag_validator_count Number of registered validators
# TYPE ultradag_validator_count gauge
ultradag_validator_count {validator_count}

# HELP ultradag_active_validators Number of active validators
# TYPE ultradag_active_validators gauge
ultradag_active_validators {active_validators}

# HELP ultradag_total_supply Total UDAG supply in sats
# TYPE ultradag_total_supply gauge
ultradag_total_supply {total_supply}

# HELP ultradag_account_count Number of accounts
# TYPE ultradag_account_count gauge
ultradag_account_count {account_count}

# HELP ultradag_total_staked Total staked UDAG in sats
# TYPE ultradag_total_staked gauge
ultradag_total_staked {total_staked}

# HELP ultradag_mempool_size Transactions in mempool
# TYPE ultradag_mempool_size gauge
ultradag_mempool_size {mempool_size}

# HELP ultradag_peer_count Connected P2P peers
# TYPE ultradag_peer_count gauge
ultradag_peer_count {peer_count}

# HELP ultradag_banned_ips Banned IP addresses
# TYPE ultradag_banned_ips gauge
ultradag_banned_ips {ban_count}

{checkpoint_metrics}");

            Response::builder()
                .status(StatusCode::OK)
                .header("Content-Type", "text/plain; version=0.0.4")
                .header("Access-Control-Allow-Origin", "*")
                .body(Full::new(Bytes::from(metrics)))
                .unwrap_or_else(|e| {
                    error!("Failed to build metrics response: {}", e);
                    Response::new(Full::new(Bytes::from("")))
                })
        }

        // Detailed health diagnostics with comprehensive system status
        (&Method::GET, ["health", "detailed"]) => {
            // Use try_read to avoid blocking - return partial diagnostics if locks are contended
            let dag_status = match server.dag.try_read() {
                Ok(dag) => serde_json::json!({
                    "available": true,
                    "current_round": dag.current_round(),
                    "vertex_count": dag.len(),
                    "tips_count": dag.tips().len(),
                    "pruning_floor": dag.pruning_floor(),
                }),
                Err(_) => serde_json::json!({
                    "available": false,
                    "reason": "DAG lock contended"
                })
            };

            let finality_status = match server.finality.try_read() {
                Ok(fin) => {
                    let last_fin = fin.last_finalized_round();
                    let dag_round = server.dag.try_read().map(|d| d.current_round()).unwrap_or(0);
                    let finality_lag = dag_round.saturating_sub(last_fin);
                    serde_json::json!({
                        "available": true,
                        "last_finalized_round": last_fin,
                        "finality_lag": finality_lag,
                        "validator_count": fin.validator_count(),
                    })
                }
                Err(_) => serde_json::json!({
                    "available": false,
                    "reason": "Finality lock contended"
                })
            };

            let state_status = match server.state.try_read() {
                Ok(state) => serde_json::json!({
                    "available": true,
                    "total_supply": state.total_supply(),
                    "account_count": state.account_count(),
                    "total_staked": state.total_staked(),
                    "active_validators": state.active_validators().len(),
                    "next_proposal_id": state.next_proposal_id(),
                }),
                Err(_) => serde_json::json!({
                    "available": false,
                    "reason": "State lock contended"
                })
            };

            let mempool_status = match server.mempool.try_read() {
                Ok(mp) => serde_json::json!({
                    "available": true,
                    "transaction_count": mp.len(),
                }),
                Err(_) => serde_json::json!({
                    "available": false,
                    "reason": "Mempool lock contended"
                })
            };

            let network_status = serde_json::json!({
                "peer_count": server.peers.connected_count().await,
                "sync_complete": server.sync_complete.load(std::sync::atomic::Ordering::Relaxed),
            });

            let checkpoint_status = {
                let metrics = server.checkpoint_metrics.export_json();
                serde_json::json!({
                    "last_checkpoint_round": metrics["health"]["last_checkpoint_round"],
                    "checkpoint_age_seconds": metrics["health"]["last_checkpoint_age_seconds"],
                    "pending_checkpoints": metrics["health"]["pending_checkpoints"],
                    "disk_count": metrics["pruning"]["checkpoint_disk_count"],
                })
            };

            // Determine overall health status
            let all_available = dag_status["available"].as_bool().unwrap_or(false)
                && finality_status["available"].as_bool().unwrap_or(false)
                && state_status["available"].as_bool().unwrap_or(false)
                && mempool_status["available"].as_bool().unwrap_or(false);

            let finality_lag = finality_status["finality_lag"].as_u64().unwrap_or(u64::MAX);
            let peer_count = network_status["peer_count"].as_u64().unwrap_or(0);
            
            let (overall_status, warnings) = if !all_available {
                ("degraded", vec!["Some components have contended locks"])
            } else if finality_lag > 100 {
                ("unhealthy", vec!["High finality lag (>100 rounds)"])
            } else if peer_count == 0 {
                ("warning", vec!["No connected peers"])
            } else if finality_lag > 10 {
                ("warning", vec!["Elevated finality lag (>10 rounds)"])
            } else {
                ("healthy", vec![])
            };

            json_response(StatusCode::OK, &serde_json::json!({
                "status": overall_status,
                "warnings": warnings,
                "timestamp": std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs(),
                "components": {
                    "dag": dag_status,
                    "finality": finality_status,
                    "state": state_status,
                    "mempool": mempool_status,
                    "network": network_status,
                    "checkpoints": checkpoint_status,
                }
            }))
        }

        (&Method::GET, ["status"]) => {
            // Short timeout (500ms) — fast enough for dashboard polling,
            // long enough to catch gaps between validator write locks.
            // Falls back to cached response if timeout expires.
            const STATUS_TIMEOUT: Duration = Duration::from_millis(500);

            macro_rules! read_or_cache {
                ($lock:expr) => {
                    match tokio::time::timeout(STATUS_TIMEOUT, $lock.read()).await {
                        Ok(guard) => guard,
                        Err(_) => {
                            let cache = status_cache().lock().await;
                            if let Some(cached) = cache.as_ref() {
                                return Ok(json_response(StatusCode::OK, cached));
                            }
                            return Ok(error_response(StatusCode::SERVICE_UNAVAILABLE, "node busy, try again"));
                        }
                    }
                };
            }

            let state = read_or_cache!(server.state);
            let last_finalized_round = state.last_finalized_round();
            let total_supply = state.total_supply();
            let account_count = state.account_count();
            let total_staked = state.total_staked();
            let active_stakers_len = state.active_stakers().len();
            let treasury_balance = state.treasury_balance();
            drop(state);

            let mempool_size = server.mempool.try_read().map(|m| m.len()).unwrap_or(0);
            let peers = server.peers.connected_count().await;

            let dag = read_or_cache!(server.dag);
            let dag_vertices = dag.len();
            let dag_round = dag.current_round();
            let dag_tips_len = dag.tips().len();
            let dag_stats = dag.dag_memory_stats();
            drop(dag);

            let fin = read_lock_or_503!(server.finality);
            let finalized_count = fin.finalized_count();
            let validator_count = fin.validator_count();
            drop(fin);

            let connected_addrs = server.peers.connected_listen_addrs().await;
            let bootstrap_connected = ultradag_network::TESTNET_BOOTSTRAP_NODES
                .iter()
                .any(|bn| connected_addrs.iter().any(|ca| ca == *bn));

            // Gather system resource metrics (best-effort, don't fail if unavailable)
            let memory_usage_bytes = get_memory_usage();
            let cpu_usage_percent = get_cpu_usage();
            let uptime_seconds = get_uptime();

            let status = StatusResponse {
                last_finalized_round,
                peer_count: peers,
                mempool_size,
                total_supply,
                account_count,
                dag_vertices,
                dag_round,
                dag_tips: dag_tips_len,
                finalized_count,
                validator_count,
                total_staked,
                active_stakers: active_stakers_len,
                treasury_balance,
                bootstrap_connected,
                memory_usage_bytes,
                cpu_usage_percent,
                uptime_seconds,
                dag_memory_stats: Some(DagMemoryStats {
                    vertex_count: dag_stats.vertex_count,
                    equivocation_vertex_count: dag_stats.equivocation_vertex_count,
                    children_map_count: dag_stats.children_map_count,
                    total_children_entries: dag_stats.total_children_entries,
                    tips_count: dag_stats.tips_count,
                    rounds_count: dag_stats.rounds_count,
                    descendant_validators_count: dag_stats.descendant_validators_count,
                    total_descendant_bitmap_bits: dag_stats.total_descendant_bitmap_bits,
                    validator_index_count: dag_stats.validator_index_count,
                    validator_round_vertex_count: dag_stats.validator_round_vertex_count,
                    byzantine_validators_count: dag_stats.byzantine_validators_count,
                    equivocation_evidence_count: dag_stats.equivocation_evidence_count,
                    evidence_store_validators: dag_stats.evidence_store_validators,
                    evidence_store_entries: dag_stats.evidence_store_entries,
                    pruning_floor: dag_stats.pruning_floor,
                    current_round: dag_stats.current_round,
                }),
            };

            *status_cache().lock().await = Some(status.clone());
            json_response(StatusCode::OK, &status)
        }

        (&Method::GET, ["balance", addr_hex]) => {
            let Some(addr) = Address::parse(addr_hex) else {
                return Ok(error_response(StatusCode::BAD_REQUEST, "invalid address: expected 40-char hex or bech32m (udag1.../tudg1...)"));
            };
            let state = read_lock_or_503!(server.state);
            let balance = state.balance(&addr);
            let nonce = state.nonce(&addr);
            let staked = state.stake_of(&addr);
            let unlock_at = state.stake_account(&addr).and_then(|s| s.unlock_at_round);
            let is_council = state.is_council_member(&addr);
            let delegation = state.delegation_account(&addr);
            json_response(
                StatusCode::OK,
                &BalanceResponse {
                    address: addr.to_hex(),
                    address_bech32: addr.to_bech32(),
                    balance,
                    nonce,
                    balance_udag: balance as f64 / ultradag_coin::SATS_PER_UDAG as f64,
                    staked,
                    staked_udag: staked as f64 / ultradag_coin::SATS_PER_UDAG as f64,
                    unlock_at_round: unlock_at,
                    is_council_member: is_council,
                    delegated: delegation.map(|d| d.delegated),
                    delegated_udag: delegation.map(|d| d.delegated as f64 / ultradag_coin::SATS_PER_UDAG as f64),
                    delegated_to: delegation.map(|d| d.validator.to_hex()),
                    delegated_to_bech32: delegation.map(|d| d.validator.to_bech32()),
                    delegation_unlock_at_round: delegation.and_then(|d| d.unlock_at_round),
                },
            )
        }

        (&Method::GET, ["round", round_str]) => {
            let Ok(round) = round_str.parse::<u64>() else {
                return Ok(error_response(StatusCode::BAD_REQUEST, "invalid round"));
            };
            let dag = read_lock_or_503!(server.dag);
            let vertices = dag.vertices_in_round(round);
            if vertices.is_empty() {
                return Ok(error_response(StatusCode::NOT_FOUND, "no vertices in round"));
            }
            let responses: Vec<VertexResponse> = vertices.iter().map(|v| VertexResponse {
                round: v.round,
                hash: hex_encode(&v.hash()),
                validator: v.validator.to_hex(),
                reward: v.block.coinbase.amount,
                tx_count: v.block.transactions.len(),
                parent_count: v.parent_hashes.len(),
            }).collect();
            json_response(StatusCode::OK, &responses)
        }

        (&Method::POST, ["tx"]) => {
            // TESTNET ONLY: accepts secret_key in body. Mainnet: use /tx/submit with pre-signed tx.
            if !server.testnet_mode {
                return Ok(error_response(StatusCode::GONE,
                    "POST /tx disabled: private keys must not transit over the network. Use /tx/submit with a pre-signed transaction (client-side signing)."));
            }
            // Check endpoint-specific rate limit
            if !rate_limiter.check_rate_limit(client_ip, limits::TX) {
                return Ok(error_response(
                    StatusCode::TOO_MANY_REQUESTS,
                    "rate limit exceeded: too many tx requests",
                ));
            }

            let body = req.collect().await?.to_bytes();
            
            // Enforce request size limit
            if body.len() > MAX_REQUEST_SIZE {
                return Ok(error_response(
                    StatusCode::PAYLOAD_TOO_LARGE,
                    "request body too large (max 1MB)",
                ));
            }
            let Ok(send_req) = serde_json::from_slice::<SendTxRequest>(&body) else {
                return Ok(error_response(StatusCode::BAD_REQUEST, "invalid JSON body, need: {secret_key, to, amount, fee, memo?}"));
            };

            // Parse and validate secret key
            let sk = match parse_secret_key(&send_req.secret_key) {
                Ok(sk) => sk,
                Err(e) => return Ok(error_response(StatusCode::BAD_REQUEST, e)),
            };
            let sender = sk.address();

            // Parse recipient
            let Some(to) = Address::parse(&send_req.to) else {
                return Ok(error_response(StatusCode::BAD_REQUEST, "invalid to address: expected 40-char hex or bech32m (udag1.../tudg1...)"));
            };

            // Parse and validate memo (if provided)
            let memo = if let Some(memo_str) = send_req.memo {
                // 0x prefix: require valid hex (don't silently fall back to UTF-8)
                let memo_bytes = if let Some(hex_str) = memo_str.strip_prefix("0x") {
                    match hex::decode(hex_str) {
                        Ok(bytes) => Some(bytes),
                        Err(_) => return Ok(error_response(StatusCode::BAD_REQUEST, "invalid hex in memo (after 0x prefix)")),
                    }
                } else if memo_str.chars().all(|c| c.is_ascii_hexdigit()) && memo_str.len() % 2 == 0 {
                    // Raw hex (even length, all hex digits)
                    hex::decode(&memo_str).map_err(|_| ()).ok()
                } else {
                    // UTF-8 string
                    Some(memo_str.into_bytes())
                };

                let Some(bytes) = memo_bytes else {
                    return Ok(error_response(StatusCode::BAD_REQUEST, "invalid memo: must be UTF-8 string or hex"));
                };

                if bytes.len() > ultradag_coin::constants::MAX_MEMO_BYTES {
                    return Ok(error_response(
                        StatusCode::BAD_REQUEST,
                        &format!("memo too large: {} bytes (max {})", bytes.len(), ultradag_coin::constants::MAX_MEMO_BYTES),
                    ));
                }

                Some(bytes)
            } else {
                None
            };

            // Validate amount is non-zero
            if send_req.amount == 0 {
                return Ok(error_response(
                    StatusCode::BAD_REQUEST,
                    "amount must be greater than 0",
                ));
            }

            // Validate minimum fee
            if send_req.fee < ultradag_coin::constants::MIN_FEE_SATS {
                return Ok(error_response(
                    StatusCode::BAD_REQUEST,
                    &format!("fee too low: minimum {} sats (0.0001 UDAG)", ultradag_coin::constants::MIN_FEE_SATS),
                ));
            }

            // Atomic nonce assignment + validation + mempool insertion.
            // Hold mempool write lock for the entire sequence to prevent
            // concurrent requests from getting the same nonce.
            let (tx, tx_hash, nonce) = {
                let state = read_lock_or_503!(server.state);
                let mut mp = write_lock_or_503!(server.mempool);

                // Compute nonce: highest pending + 1, or state nonce if no pending
                let nonce = next_nonce(&state, &mp, &sender);

                // Validate balance including pending cost
                let balance = state.balance(&sender);
                let pending_cost = pending_cost(&mp, &sender);
                let tx_cost = send_req.amount.saturating_add(send_req.fee);
                let total_needed = pending_cost.saturating_add(tx_cost);
                if balance < total_needed {
                    return Ok(error_response(
                        StatusCode::BAD_REQUEST,
                        &format!(
                            "insufficient balance: need {} sats (incl. {} pending), have {} sats ({:.4} UDAG)",
                            total_needed,
                            pending_cost,
                            balance,
                            balance as f64 / ultradag_coin::SATS_PER_UDAG as f64,
                        ),
                    ));
                }

                // Build and sign transfer transaction
                let mut transfer = TransferTx {
                    from: sender,
                    to,
                    amount: send_req.amount,
                    fee: send_req.fee,
                    nonce,
                    pub_key: sk.verifying_key().to_bytes(),
                    signature: Signature([0u8; 64]),
                    memo,
                };
                transfer.signature = sk.sign(&transfer.signable_bytes());
                let tx = Transaction::Transfer(transfer);
                let tx_hash = tx.hash();

                // Insert into mempool while still holding the lock
                if !mp.insert(tx.clone()) {
                    return Ok(error_response(StatusCode::CONFLICT, "duplicate transaction"));
                }

                (tx, tx_hash, nonce)
            };

            // Broadcast to peers (outside lock)
            server.peers.broadcast(&Message::NewTx(tx.clone()), "").await;
            let _ = server.tx_tx.send(tx);

            json_response(
                StatusCode::OK,
                &TxResponse {
                    hash: hex_encode(&tx_hash),
                    from: sender.to_hex(),
                    to: send_req.to.clone(),
                    amount: send_req.amount,
                    fee: send_req.fee,
                    nonce,
                },
            )
        }

        (&Method::GET, ["fee-estimate"]) => {
            let state = read_lock_or_503!(server.state);
            let mp = read_lock_or_503!(server.mempool);
            let mempool_size = mp.len();
            let mempool_capacity: usize = 10_000;
            let base_fee = state.governance_params().min_fee_sats;
            let dynamic_fee = state.dynamic_min_fee(mempool_size);
            let usage_percent = if mempool_capacity > 0 {
                (mempool_size * 100) / mempool_capacity
            } else {
                0
            };
            let congestion = if usage_percent >= 80 {
                "high"
            } else if usage_percent >= 50 {
                "medium"
            } else {
                "low"
            };
            json_response(
                StatusCode::OK,
                &serde_json::json!({
                    "min_fee_sats": base_fee,
                    "recommended_fee_sats": dynamic_fee,
                    "mempool_size": mempool_size,
                    "mempool_capacity": mempool_capacity,
                    "congestion": congestion,
                }),
            )
        }

        (&Method::GET, ["mempool"]) => {
            let mp = read_lock_or_503!(server.mempool);
            let txs: Vec<serde_json::Value> = mp.best(100).iter().map(|tx| {
                match tx {
                    Transaction::Transfer(t) => serde_json::json!({
                        "type": "transfer",
                        "hash": hex_encode(&tx.hash()),
                        "from": t.from.to_hex(),
                        "to": t.to.to_hex(),
                        "amount": t.amount,
                        "fee": t.fee,
                        "nonce": t.nonce,
                    }),
                    Transaction::Stake(t) => serde_json::json!({
                        "type": "stake",
                        "hash": hex_encode(&tx.hash()),
                        "from": t.from.to_hex(),
                        "amount": t.amount,
                        "nonce": t.nonce,
                    }),
                    Transaction::Unstake(t) => serde_json::json!({
                        "type": "unstake",
                        "hash": hex_encode(&tx.hash()),
                        "from": t.from.to_hex(),
                        "nonce": t.nonce,
                    }),
                    Transaction::CreateProposal(t) => serde_json::json!({
                        "type": "create_proposal",
                        "hash": hex_encode(&tx.hash()),
                        "from": t.from.to_hex(),
                        "proposal_id": t.proposal_id,
                        "title": t.title,
                        "fee": t.fee,
                        "nonce": t.nonce,
                    }),
                    Transaction::Vote(t) => serde_json::json!({
                        "type": "vote",
                        "hash": hex_encode(&tx.hash()),
                        "from": t.from.to_hex(),
                        "proposal_id": t.proposal_id,
                        "vote": t.vote,
                        "fee": t.fee,
                        "nonce": t.nonce,
                    }),
                    Transaction::Delegate(t) => serde_json::json!({
                        "type": "delegate",
                        "hash": hex_encode(&tx.hash()),
                        "from": t.from.to_hex(),
                        "validator": t.validator.to_hex(),
                        "amount": t.amount,
                        "nonce": t.nonce,
                    }),
                    Transaction::Undelegate(t) => serde_json::json!({
                        "type": "undelegate",
                        "hash": hex_encode(&tx.hash()),
                        "from": t.from.to_hex(),
                        "nonce": t.nonce,
                    }),
                    Transaction::SetCommission(t) => serde_json::json!({
                        "type": "set_commission",
                        "hash": hex_encode(&tx.hash()),
                        "from": t.from.to_hex(),
                        "commission_percent": t.commission_percent,
                        "nonce": t.nonce,
                    }),
                    Transaction::BridgeDeposit(t) => serde_json::json!({
                        "type": "bridge_lock",
                        "hash": hex_encode(&tx.hash()),
                        "from": t.from.to_hex(),
                        "recipient": hex_encode(&t.recipient),
                        "amount": t.amount,
                        "fee": t.fee,
                        "nonce": t.nonce,
                    }),
                    Transaction::BridgeRelease(t) => serde_json::json!({
                        "type": "bridge_release",
                        "hash": hex_encode(&tx.hash()),
                        "from": t.from.to_hex(),
                        "recipient": t.recipient.to_hex(),
                        "amount": t.amount,
                        "source_chain_id": t.source_chain_id,
                        "deposit_nonce": t.deposit_nonce,
                        "nonce": t.nonce,
                    }),
                }
            }).collect();
            json_response(StatusCode::OK, &txs)
        }

        (&Method::POST, ["faucet"]) => {
            // TESTNET ONLY: faucet distributes free tokens for testing.
            // On mainnet builds, faucet_keypair() doesn't exist — return GONE immediately.
            #[cfg(feature = "mainnet")]
            {
                return Ok(error_response(StatusCode::GONE, "faucet disabled on mainnet"));
            }
            #[cfg(not(feature = "mainnet"))]
            if !server.testnet_mode {
                return Ok(error_response(StatusCode::GONE, "faucet disabled on mainnet"));
            }
            #[cfg(not(feature = "mainnet"))] {
            // Check endpoint-specific rate limit (strict for faucet)
            if !rate_limiter.check_rate_limit(client_ip, limits::FAUCET) {
                return Ok(error_response(
                    StatusCode::TOO_MANY_REQUESTS,
                    "rate limit exceeded: faucet limited to 1 request per 10 minutes",
                ));
            }

            let body = req.collect().await?.to_bytes();
            
            if body.len() > MAX_REQUEST_SIZE {
                return Ok(error_response(
                    StatusCode::PAYLOAD_TOO_LARGE,
                    "request body too large (max 1MB)",
                ));
            }
            let Ok(faucet_req) = serde_json::from_slice::<FaucetRequest>(&body) else {
                return Ok(error_response(StatusCode::BAD_REQUEST, "invalid JSON body, need: {address, amount}"));
            };
            let Some(to) = Address::parse(&faucet_req.address) else {
                return Ok(error_response(StatusCode::BAD_REQUEST, "invalid address: expected 40-char hex or bech32m (udag1.../tudg1...)"));
            };

            // Reject zero amount
            if faucet_req.amount == 0 {
                return Ok(error_response(StatusCode::BAD_REQUEST, "amount must be greater than 0"));
            }

            // Cap faucet amount at 100 UDAG per request
            const MAX_FAUCET_SATS: u64 = 100 * ultradag_coin::COIN; // 100 UDAG
            if faucet_req.amount > MAX_FAUCET_SATS {
                return Ok(error_response(
                    StatusCode::BAD_REQUEST,
                    &format!("faucet amount exceeds maximum of 100 UDAG ({} sats)", MAX_FAUCET_SATS),
                ));
            }

            // Use the deterministic faucet keypair (same on every node)
            let faucet_sk = ultradag_coin::faucet_keypair();
            let faucet_addr = faucet_sk.address();
            let fee = ultradag_coin::constants::MIN_FEE_SATS; // must meet minimum fee

            let (tx, tx_hash, nonce) = {
                let state = read_lock_or_503!(server.state);
                let mut mp = write_lock_or_503!(server.mempool);

                let nonce = next_nonce(&state, &mp, &faucet_addr);

                let balance = state.balance(&faucet_addr);
                let pending_cost = pending_cost(&mp, &faucet_addr);
                let total_needed = pending_cost.saturating_add(faucet_req.amount).saturating_add(fee);
                if balance < total_needed {
                    return Ok(error_response(
                        StatusCode::BAD_REQUEST,
                        &format!(
                            "faucet insufficient balance: need {} sats, have {} sats ({:.4} UDAG)",
                            total_needed, balance, balance as f64 / ultradag_coin::SATS_PER_UDAG as f64,
                        ),
                    ));
                }

                let mut transfer = TransferTx {
                    from: faucet_addr,
                    to,
                    amount: faucet_req.amount,
                    fee,
                    nonce,
                    pub_key: faucet_sk.verifying_key().to_bytes(),
                    signature: Signature([0u8; 64]),
                    memo: None,
                };
                transfer.signature = faucet_sk.sign(&transfer.signable_bytes());
                let tx = Transaction::Transfer(transfer);
                let tx_hash = tx.hash();

                if !mp.insert(tx.clone()) {
                    return Ok(error_response(StatusCode::CONFLICT, "duplicate transaction"));
                }

                (tx, tx_hash, nonce)
            };

            // Broadcast to peers
            server.peers.broadcast(&Message::NewTx(tx.clone()), "").await;
            let _ = server.tx_tx.send(tx);

            json_response(
                StatusCode::OK,
                &serde_json::json!({
                    "tx_hash": hex_encode(&tx_hash),
                    "from": faucet_addr.to_hex(),
                    "to": faucet_req.address,
                    "amount": faucet_req.amount,
                    "amount_udag": faucet_req.amount as f64 / ultradag_coin::SATS_PER_UDAG as f64,
                    "nonce": nonce,
                }),
            )
            } // end #[cfg(not(feature = "mainnet"))]
        }

        (&Method::GET, ["peers"]) => {
            let connected = server.peers.connected_count().await;
            let peer_addrs = server.peers.connected_addrs().await;
            let listen_addrs = server.peers.connected_listen_addrs().await;
            let bootstrap_nodes: Vec<BootstrapNodeStatus> = ultradag_network::TESTNET_BOOTSTRAP_NODES
                .iter()
                .map(|bn| BootstrapNodeStatus {
                    addr: bn.to_string(),
                    connected: listen_addrs.iter().any(|ca| ca == *bn),
                })
                .collect();
            json_response(
                StatusCode::OK,
                &PeersResponse {
                    connected,
                    peers: peer_addrs,
                    bootstrap_nodes,
                },
            )
        }

        (&Method::POST, ["stake"]) => {
            // TESTNET ONLY: accepts secret_key in body. Mainnet: use /tx/submit with pre-signed StakeTx.
            if !server.testnet_mode {
                return Ok(error_response(StatusCode::GONE,
                    "POST /stake disabled: private keys must not transit over the network. Use /tx/submit with a pre-signed StakeTx."));
            }
            // Check endpoint-specific rate limit
            if !rate_limiter.check_rate_limit(client_ip, limits::STAKE) {
                return Ok(error_response(
                    StatusCode::TOO_MANY_REQUESTS,
                    "rate limit exceeded: too many stake requests",
                ));
            }

            let body = req.collect().await?.to_bytes();
            
            if body.len() > MAX_REQUEST_SIZE {
                return Ok(error_response(
                    StatusCode::PAYLOAD_TOO_LARGE,
                    "request body too large (max 1MB)",
                ));
            }
            let Ok(stake_req) = serde_json::from_slice::<StakeRequest>(&body) else {
                return Ok(error_response(StatusCode::BAD_REQUEST, "invalid JSON body, need: {secret_key, amount}"));
            };

            let sk = match parse_secret_key(&stake_req.secret_key) {
                Ok(sk) => sk,
                Err(e) => return Ok(error_response(StatusCode::BAD_REQUEST, e)),
            };
            let sender = sk.address();

            // Build stake transaction and add to mempool (will be included in next vertex)
            let (tx, tx_hash, nonce) = {
                let state = read_lock_or_503!(server.state);
                let mut mp = write_lock_or_503!(server.mempool);

                let nonce = next_nonce(&state, &mp, &sender);

                let balance = state.balance(&sender);
                let pending_cost = pending_cost(&mp, &sender);
                // StakeTx has zero fee — don't add MIN_FEE_SATS
                let total_needed = pending_cost.saturating_add(stake_req.amount);

                if stake_req.amount < MIN_STAKE_SATS {
                    return Ok(error_response(StatusCode::BAD_REQUEST,
                        &format!("minimum stake is {} sats ({} UDAG)", MIN_STAKE_SATS, MIN_STAKE_SATS / ultradag_coin::SATS_PER_UDAG)));
                }
                if balance < total_needed {
                    return Ok(error_response(StatusCode::BAD_REQUEST,
                        &format!("insufficient balance: need {} sats (incl. {} pending), have {} sats",
                            total_needed, pending_cost, balance)));
                }

                let mut stake_tx = StakeTx {
                    from: sender,
                    amount: stake_req.amount,
                    nonce,
                    pub_key: sk.verifying_key().to_bytes(),
                    signature: Signature([0u8; 64]),
                };
                stake_tx.signature = sk.sign(&stake_tx.signable_bytes());
                let tx = Transaction::Stake(stake_tx);
                let tx_hash = tx.hash();

                if !mp.insert(tx.clone()) {
                    return Ok(error_response(StatusCode::CONFLICT, "duplicate transaction"));
                }

                (tx, tx_hash, nonce)
            };

            // Broadcast to peers
            server.peers.broadcast(&Message::NewTx(tx.clone()), "").await;
            let _ = server.tx_tx.send(tx);

            json_response(StatusCode::OK, &serde_json::json!({
                "status": "pending",
                "tx_hash": hex_encode(&tx_hash),
                "address": sender.to_hex(),
                "amount": stake_req.amount,
                "amount_udag": stake_req.amount as f64 / ultradag_coin::SATS_PER_UDAG as f64,
                "nonce": nonce,
                "note": "Stake transaction added to mempool. Will be applied when included in a finalized vertex."
            }))
        }

        (&Method::POST, ["unstake"]) => {
            // TESTNET ONLY: accepts secret_key in body. Mainnet: use /tx/submit with pre-signed UnstakeTx.
            if !server.testnet_mode {
                return Ok(error_response(StatusCode::GONE,
                    "POST /unstake disabled: private keys must not transit over the network. Use /tx/submit with a pre-signed UnstakeTx."));
            }
            // Check endpoint-specific rate limit
            if !rate_limiter.check_rate_limit(client_ip, limits::UNSTAKE) {
                return Ok(error_response(
                    StatusCode::TOO_MANY_REQUESTS,
                    "rate limit exceeded: too many unstake requests",
                ));
            }

            let body = req.collect().await?.to_bytes();
            
            if body.len() > MAX_REQUEST_SIZE {
                return Ok(error_response(
                    StatusCode::PAYLOAD_TOO_LARGE,
                    "request body too large (max 1MB)",
                ));
            }
            let Ok(unstake_req) = serde_json::from_slice::<UnstakeRequest>(&body) else {
                return Ok(error_response(StatusCode::BAD_REQUEST, "invalid JSON body, need: {secret_key}"));
            };

            let sk = match parse_secret_key(&unstake_req.secret_key) {
                Ok(sk) => sk,
                Err(e) => return Ok(error_response(StatusCode::BAD_REQUEST, e)),
            };
            let sender = sk.address();

            let current_round = {
                let dag = read_lock_or_503!(server.dag);
                dag.current_round()
            };

            // Build unstake transaction and add to mempool
            let (tx, tx_hash, nonce) = {
                let state = read_lock_or_503!(server.state);
                let mut mp = write_lock_or_503!(server.mempool);

                // Check that address actually has stake
                if state.stake_of(&sender) == 0 {
                    return Ok(error_response(StatusCode::BAD_REQUEST, "not staked"));
                }

                let nonce = next_nonce(&state, &mp, &sender);

                let mut unstake_tx = UnstakeTx {
                    from: sender,
                    nonce,
                    pub_key: sk.verifying_key().to_bytes(),
                    signature: Signature([0u8; 64]),
                };
                unstake_tx.signature = sk.sign(&unstake_tx.signable_bytes());
                let tx = Transaction::Unstake(unstake_tx);
                let tx_hash = tx.hash();

                if !mp.insert(tx.clone()) {
                    return Ok(error_response(StatusCode::CONFLICT, "duplicate transaction"));
                }

                (tx, tx_hash, nonce)
            };

            // Broadcast to peers
            server.peers.broadcast(&Message::NewTx(tx.clone()), "").await;
            let _ = server.tx_tx.send(tx);

            let unlock_at = current_round + ultradag_coin::UNSTAKE_COOLDOWN_ROUNDS;
            json_response(StatusCode::OK, &serde_json::json!({
                "status": "pending",
                "tx_hash": hex_encode(&tx_hash),
                "address": sender.to_hex(),
                "unlock_at_round": unlock_at,
                "nonce": nonce,
                "note": "Unstake transaction added to mempool. Will be applied when included in a finalized vertex."
            }))
        }

        (&Method::GET, ["stake", addr_hex]) => {
            let Some(addr) = Address::parse(addr_hex) else {
                return Ok(error_response(StatusCode::BAD_REQUEST, "invalid address: expected 40-char hex or bech32m (udag1.../tudg1...)"));
            };
            let state = read_lock_or_503!(server.state);
            let staked = state.stake_of(&addr);
            let stake_acct = state.stake_account(&addr);
            let unlock_at = stake_acct.and_then(|s| s.unlock_at_round);
            let commission = stake_acct.map(|s| s.commission_percent).unwrap_or(ultradag_coin::DEFAULT_COMMISSION_PERCENT);
            let is_active = state.is_active_validator(&addr);
            let effective = state.effective_stake_of(&addr);
            let delegators = state.delegators_of(&addr);
            let total_del: u64 = delegators.iter().map(|(_, amt)| *amt).fold(0u64, |acc, x| acc.saturating_add(x));
            json_response(StatusCode::OK, &StakeInfoResponse {
                address: addr.to_hex(),
                staked,
                staked_udag: staked as f64 / ultradag_coin::SATS_PER_UDAG as f64,
                unlock_at_round: unlock_at,
                is_active_validator: is_active,
                effective_stake: effective,
                effective_stake_udag: effective as f64 / ultradag_coin::SATS_PER_UDAG as f64,
                commission_percent: commission,
                delegator_count: delegators.len(),
                total_delegated: total_del,
                total_delegated_udag: total_del as f64 / ultradag_coin::SATS_PER_UDAG as f64,
            })
        }

        (&Method::GET, ["validators"]) => {
            let state = read_lock_or_503!(server.state);
            let stakers = state.active_stakers();
            let validators: Vec<ValidatorInfo> = stakers.iter().map(|addr| {
                let staked = state.stake_of(addr);
                let effective = state.effective_stake_of(addr);
                let commission = state.stake_account(addr).map(|s| s.commission_percent).unwrap_or(ultradag_coin::DEFAULT_COMMISSION_PERCENT);
                let delegator_count = state.delegators_of(addr).len();
                ValidatorInfo {
                    address: addr.to_hex(),
                    address_bech32: addr.to_bech32(),
                    staked,
                    staked_udag: staked as f64 / ultradag_coin::SATS_PER_UDAG as f64,
                    effective_stake: effective,
                    effective_stake_udag: effective as f64 / ultradag_coin::SATS_PER_UDAG as f64,
                    commission_percent: commission,
                    delegator_count,
                }
            }).collect();
            json_response(StatusCode::OK, &serde_json::json!({
                "count": validators.len(),
                "total_staked": state.total_staked(),
                "total_delegated": state.total_delegated(),
                "validators": validators,
            }))
        }

        // ─── Bridge Endpoints ───

        (&Method::GET, ["bridge", "nonce"]) => {
            if !rate_limiter.check_rate_limit(client_ip, limits::BRIDGE) {
                return Ok(error_response(StatusCode::TOO_MANY_REQUESTS, "rate limit exceeded"));
            }
            let state = read_lock_or_503!(server.state);
            json_response(StatusCode::OK, &serde_json::json!({
                "next_nonce": state.get_bridge_nonce(),
            }))
        }

        (&Method::GET, ["bridge", "attestation", nonce_str]) => {
            if !rate_limiter.check_rate_limit(client_ip, limits::BRIDGE) {
                return Ok(error_response(StatusCode::TOO_MANY_REQUESTS, "rate limit exceeded"));
            }
            let Ok(nonce) = nonce_str.parse::<u64>() else {
                return Ok(error_response(StatusCode::BAD_REQUEST, "invalid nonce"));
            };
            let state = read_lock_or_503!(server.state);
            
            // Get attestation
            let Some(attestation) = state.get_bridge_attestation(nonce) else {
                return Ok(error_response(StatusCode::NOT_FOUND, "attestation not found"));
            };
            
            // Get signature count
            let signature_count = state.get_signature_count(nonce);
            let threshold = state.get_bridge_threshold();
            
            // Try to build proof if we have enough signatures
            let proof = state.build_bridge_proof(nonce).ok();
            
            json_response(StatusCode::OK, &serde_json::json!({
                "nonce": attestation.nonce,
                "sender": attestation.sender.to_hex(),
                "sender_bech32": attestation.sender.to_bech32(),
                "recipient": format!("0x{}", hex::encode(attestation.recipient)),
                "amount": attestation.amount,
                "amount_udag": attestation.amount as f64 / ultradag_coin::SATS_PER_UDAG as f64,
                "destination_chain_id": attestation.destination_chain_id,
                "signature_count": signature_count,
                "threshold": threshold,
                "ready": signature_count >= threshold,
                "proof": proof.map(|p| serde_json::json!({
                    "attestation": serde_json::json!({
                        "sender": p.attestation.sender.to_hex(),
                        "recipient": format!("0x{}", hex::encode(p.attestation.recipient)),
                        "amount": p.attestation.amount,
                        "nonce": p.attestation.nonce,
                        "destination_chain_id": p.attestation.destination_chain_id,
                    }),
                    "signature_count": p.signatures.len(),
                })),
            }))
        }

        (&Method::GET, ["bridge", "reserve"]) => {
            if !rate_limiter.check_rate_limit(client_ip, limits::BRIDGE) {
                return Ok(error_response(StatusCode::TOO_MANY_REQUESTS, "rate limit exceeded"));
            }
            let state = read_lock_or_503!(server.state);
            let reserve = state.get_bridge_reserve();
            json_response(StatusCode::OK, &serde_json::json!({
                "reserve_sats": reserve,
                "reserve_udag": reserve as f64 / ultradag_coin::SATS_PER_UDAG as f64,
            }))
        }

        (&Method::POST, ["bridge", "deposit"]) => {
            // TESTNET ONLY: accepts secret_key in body. Mainnet: use /tx/submit with a pre-signed BridgeDepositTx.
            if !server.testnet_mode {
                return Ok(error_response(StatusCode::GONE,
                    "POST /bridge/deposit disabled: private keys must not transit over the network. Use /tx/submit with a pre-signed BridgeDepositTx."));
            }
            if !rate_limiter.check_rate_limit(client_ip, limits::BRIDGE_DEPOSIT) {
                return Ok(error_response(
                    StatusCode::TOO_MANY_REQUESTS,
                    "rate limit exceeded: too many bridge deposit requests",
                ));
            }

            let body = req.collect().await?.to_bytes();
            if body.len() > MAX_REQUEST_SIZE {
                return Ok(error_response(StatusCode::PAYLOAD_TOO_LARGE, "request body too large (max 1MB)"));
            }
            let Ok(bridge_req) = serde_json::from_slice::<BridgeDepositRequest>(&body) else {
                return Ok(error_response(StatusCode::BAD_REQUEST,
                    "invalid JSON body, need: {secret_key, recipient, amount, fee, destination_chain_id}"));
            };

            // Parse and validate secret key
            let sk = match parse_secret_key(&bridge_req.secret_key) {
                Ok(sk) => sk,
                Err(e) => return Ok(error_response(StatusCode::BAD_REQUEST, e)),
            };
            let sender = sk.address();

            // Validate recipient (0x-prefixed 42-char Ethereum address)
            let recipient_hex = bridge_req.recipient.strip_prefix("0x")
                .or_else(|| bridge_req.recipient.strip_prefix("0X"))
                .unwrap_or(&bridge_req.recipient);
            if recipient_hex.len() != 40 || !recipient_hex.chars().all(|c| c.is_ascii_hexdigit()) {
                return Ok(error_response(StatusCode::BAD_REQUEST,
                    "invalid recipient: expected 0x-prefixed 40-char hex Ethereum address"));
            }
            let mut recipient_bytes = [0u8; 20];
            for (i, chunk) in recipient_hex.as_bytes().chunks(2).enumerate() {
                let s = std::str::from_utf8(chunk).unwrap_or("00");
                recipient_bytes[i] = u8::from_str_radix(s, 16).unwrap_or(0);
            }
            if recipient_bytes == [0u8; 20] {
                return Ok(error_response(StatusCode::BAD_REQUEST, "invalid recipient: zero address"));
            }

            // Validate amount
            if bridge_req.amount < ultradag_coin::constants::MIN_BRIDGE_AMOUNT_SATS {
                return Ok(error_response(StatusCode::BAD_REQUEST,
                    &format!("amount too low: minimum {} sats ({} UDAG)",
                        ultradag_coin::constants::MIN_BRIDGE_AMOUNT_SATS,
                        ultradag_coin::constants::MIN_BRIDGE_AMOUNT_SATS / ultradag_coin::SATS_PER_UDAG)));
            }
            if bridge_req.amount > ultradag_coin::constants::MAX_BRIDGE_AMOUNT_SATS {
                return Ok(error_response(StatusCode::BAD_REQUEST,
                    &format!("amount too high: maximum {} sats ({} UDAG)",
                        ultradag_coin::constants::MAX_BRIDGE_AMOUNT_SATS,
                        ultradag_coin::constants::MAX_BRIDGE_AMOUNT_SATS / ultradag_coin::SATS_PER_UDAG)));
            }

            // Validate fee
            if bridge_req.fee < ultradag_coin::constants::MIN_FEE_SATS {
                return Ok(error_response(StatusCode::BAD_REQUEST,
                    &format!("fee too low: minimum {} sats (0.0001 UDAG)", ultradag_coin::constants::MIN_FEE_SATS)));
            }

            // Validate destination chain ID
            if !ultradag_coin::SUPPORTED_BRIDGE_CHAIN_IDS.contains(&bridge_req.destination_chain_id) {
                return Ok(error_response(StatusCode::BAD_REQUEST,
                    &format!("unsupported destination_chain_id: {}. Supported: {:?}",
                        bridge_req.destination_chain_id,
                        ultradag_coin::SUPPORTED_BRIDGE_CHAIN_IDS)));
            }

            // Atomic nonce assignment + validation + mempool insertion
            let (tx, tx_hash, nonce) = {
                let state = read_lock_or_503!(server.state);
                let mut mp = write_lock_or_503!(server.mempool);

                let nonce = next_nonce(&state, &mp, &sender);

                // Validate balance including pending cost
                let balance = state.balance(&sender);
                let pc = pending_cost(&mp, &sender);
                let tx_cost = bridge_req.amount.saturating_add(bridge_req.fee);
                let total_needed = pc.saturating_add(tx_cost);
                if balance < total_needed {
                    return Ok(error_response(StatusCode::BAD_REQUEST,
                        &format!(
                            "insufficient balance: need {} sats (incl. {} pending), have {} sats ({:.4} UDAG)",
                            total_needed, pc, balance,
                            balance as f64 / ultradag_coin::SATS_PER_UDAG as f64,
                        )));
                }

                // Build and sign bridge deposit transaction
                let mut bridge_tx = BridgeDepositTx {
                    from: sender,
                    recipient: recipient_bytes,
                    amount: bridge_req.amount,
                    destination_chain_id: bridge_req.destination_chain_id,
                    nonce,
                    fee: bridge_req.fee,
                    pub_key: sk.verifying_key().to_bytes(),
                    signature: Signature([0u8; 64]),
                };
                bridge_tx.signature = sk.sign(&bridge_tx.signable_bytes());
                let tx = Transaction::BridgeDeposit(bridge_tx);
                let tx_hash = tx.hash();

                // Insert into mempool while still holding the lock
                if !mp.insert(tx.clone()) {
                    return Ok(error_response(StatusCode::CONFLICT, "duplicate transaction"));
                }

                (tx, tx_hash, nonce)
            };

            // Broadcast to peers (outside lock)
            server.peers.broadcast(&Message::NewTx(tx.clone()), "").await;
            let _ = server.tx_tx.send(tx);

            json_response(StatusCode::OK, &serde_json::json!({
                "status": "pending",
                "tx_hash": hex_encode(&tx_hash),
                "from": sender.to_hex(),
                "recipient": bridge_req.recipient,
                "amount": bridge_req.amount,
                "fee": bridge_req.fee,
                "destination_chain_id": bridge_req.destination_chain_id,
                "nonce": nonce,
            }))
        }

        // POST /bridge/release — Submit a bridge release (Arbitrum→Native unlock).
        // Validators submit this to release locked funds from bridge_reserve.
        (&Method::POST, ["bridge", "release"]) => {
            #[cfg(feature = "mainnet")]
            {
                return Ok(error_response(StatusCode::GONE,
                    "/bridge/release disabled on mainnet: use /tx/submit with pre-signed BridgeReleaseTx"));
            }
            #[cfg(not(feature = "mainnet"))]
            if !server.testnet_mode {
                return Ok(error_response(StatusCode::GONE,
                    "/bridge/release disabled on mainnet: use /tx/submit with pre-signed BridgeReleaseTx"));
            }
            #[cfg(not(feature = "mainnet"))] {
            if !rate_limiter.check_rate_limit(client_ip, limits::BRIDGE_DEPOSIT) {
                return Ok(error_response(StatusCode::TOO_MANY_REQUESTS, "rate limit exceeded"));
            }

            let body = req.collect().await?.to_bytes();
            if body.len() > MAX_REQUEST_SIZE {
                return Ok(error_response(StatusCode::PAYLOAD_TOO_LARGE, "request body too large (max 1MB)"));
            }

            #[derive(serde::Deserialize)]
            struct BridgeReleaseRequest {
                secret_key: String,
                recipient: String,
                amount: u64,
                source_chain_id: u64,
                deposit_nonce: u64,
            }

            let release_req: BridgeReleaseRequest = match serde_json::from_slice(&body) {
                Ok(r) => r,
                Err(e) => return Ok(error_response(StatusCode::BAD_REQUEST, &format!("invalid JSON: {}", e))),
            };

            // Parse secret key
            let sk = match parse_secret_key(&release_req.secret_key) {
                Ok(k) => k,
                Err(e) => return Ok(error_response(StatusCode::BAD_REQUEST, e)),
            };
            let sender = sk.address();

            // Parse recipient (native address hex)
            let recipient_hex = release_req.recipient.strip_prefix("0x").unwrap_or(&release_req.recipient);
            if recipient_hex.len() != 40 || !recipient_hex.chars().all(|c| c.is_ascii_hexdigit()) {
                return Ok(error_response(StatusCode::BAD_REQUEST, "invalid recipient: need 40 hex chars"));
            }
            let mut recipient_bytes = [0u8; 20];
            for (i, chunk) in recipient_hex.as_bytes().chunks(2).enumerate() {
                let s = std::str::from_utf8(chunk).unwrap_or("00");
                recipient_bytes[i] = u8::from_str_radix(s, 16).unwrap_or(0);
            }
            let recipient = ultradag_coin::Address(recipient_bytes);

            // Validate amount
            if release_req.amount < ultradag_coin::constants::MIN_BRIDGE_AMOUNT_SATS {
                return Ok(error_response(StatusCode::BAD_REQUEST,
                    &format!("minimum bridge amount is {} sats", ultradag_coin::constants::MIN_BRIDGE_AMOUNT_SATS)));
            }
            if release_req.amount > ultradag_coin::constants::MAX_BRIDGE_AMOUNT_SATS {
                return Ok(error_response(StatusCode::BAD_REQUEST,
                    &format!("exceeds maximum bridge amount: {} sats", ultradag_coin::constants::MAX_BRIDGE_AMOUNT_SATS)));
            }

            // Validate chain ID
            if !ultradag_coin::constants::SUPPORTED_BRIDGE_CHAIN_IDS.contains(&release_req.source_chain_id) {
                return Ok(error_response(StatusCode::BAD_REQUEST,
                    &format!("unsupported source chain ID: {}", release_req.source_chain_id)));
            }

            // Build, sign, and submit BridgeReleaseTx
            let (tx, tx_hash, nonce) = {
                let state = read_lock_or_503!(server.state);
                let mut mp = write_lock_or_503!(server.mempool);

                // Verify sender is active validator
                if !state.is_active_validator(&sender) {
                    return Ok(error_response(StatusCode::FORBIDDEN, "only active validators can submit bridge releases"));
                }

                let nonce = state.nonce(&sender);
                let pending_nonce = mp.pending_nonce(&sender).map(|n| n.saturating_add(1)).unwrap_or(nonce);

                let mut release_tx = ultradag_coin::tx::bridge::BridgeReleaseTx {
                    from: sender,
                    recipient,
                    amount: release_req.amount,
                    source_chain_id: release_req.source_chain_id,
                    deposit_nonce: release_req.deposit_nonce,
                    nonce: pending_nonce,
                    pub_key: sk.verifying_key().to_bytes(),
                    signature: ultradag_coin::Signature([0u8; 64]),
                };
                release_tx.signature = sk.sign(&release_tx.signable_bytes());

                let tx = Transaction::BridgeRelease(release_tx);
                let tx_hash = tx.hash();

                if !mp.insert(tx.clone()) {
                    return Ok(error_response(StatusCode::CONFLICT, "duplicate transaction"));
                }

                (tx, tx_hash, pending_nonce)
            };

            server.peers.broadcast(&Message::NewTx(tx.clone()), "").await;
            let _ = server.tx_tx.send(tx);

            json_response(StatusCode::OK, &serde_json::json!({
                "status": "pending",
                "tx_hash": hex_encode(&tx_hash),
                "from": sender.to_hex(),
                "recipient": recipient.to_hex(),
                "amount": release_req.amount,
                "source_chain_id": release_req.source_chain_id,
                "deposit_nonce": release_req.deposit_nonce,
                "nonce": nonce,
            }))
            }
        }

        (&Method::GET, ["keygen"]) => {
            // TESTNET ONLY: generates keys server-side (server sees private key).
            // Mainnet: use SDK or CLI for offline key generation.
            #[cfg(feature = "mainnet")]
            {
                return Ok(error_response(StatusCode::GONE,
                    "/keygen disabled on mainnet: generate keys locally using the SDK. Server must never see private keys."));
            }
            #[cfg(not(feature = "mainnet"))]
            if !server.testnet_mode {
                return Ok(error_response(StatusCode::GONE,
                    "/keygen disabled on mainnet: generate keys locally using the SDK. Server must never see private keys."));
            }
            #[cfg(not(feature = "mainnet"))]
            {
            if !rate_limiter.check_rate_limit(client_ip, limits::KEYGEN) {
                return Ok(error_response(
                    StatusCode::TOO_MANY_REQUESTS,
                    "rate limit exceeded: too many keygen requests",
                ));
            }
            let sk = SecretKey::generate();
            let addr = sk.address();
            json_response(
                StatusCode::OK,
                &serde_json::json!({
                    "warning": "TESTNET ONLY — never use /keygen for mainnet. The server sees your private key.",
                    "secret_key": hex_encode(&sk.to_bytes()),
                    "address": addr.to_hex(),
                    "address_bech32": addr.to_bech32(),
                }),
            )
            }
        }

        // ====== Governance POST endpoints ======

        (&Method::POST, ["proposal"]) => {
            // TESTNET ONLY: accepts secret_key in body. Mainnet: use /tx/submit with pre-signed CreateProposalTx.
            if !server.testnet_mode {
                return Ok(error_response(StatusCode::GONE,
                    "POST /proposal disabled: private keys must not transit over the network. Use /tx/submit with a pre-signed CreateProposal transaction."));
            }
            // Check endpoint-specific rate limit
            if !rate_limiter.check_rate_limit(client_ip, limits::PROPOSAL) {
                return Ok(error_response(
                    StatusCode::TOO_MANY_REQUESTS,
                    "rate limit exceeded: too many proposal requests",
                ));
            }
            let body = req.collect().await?.to_bytes();
            if body.len() > MAX_REQUEST_SIZE {
                return Ok(error_response(StatusCode::PAYLOAD_TOO_LARGE, "request body too large"));
            }
            let Ok(prop_req) = serde_json::from_slice::<ProposalRequest>(&body) else {
                return Ok(error_response(StatusCode::BAD_REQUEST,
                    "invalid JSON: need {secret_key, title, description, proposal_type}"));
            };

            // Validate title/description lengths before doing any crypto work
            if prop_req.title.len() > ultradag_coin::constants::PROPOSAL_TITLE_MAX_BYTES {
                return Ok(error_response(StatusCode::BAD_REQUEST,
                    &format!("title too long: max {} bytes", ultradag_coin::constants::PROPOSAL_TITLE_MAX_BYTES)));
            }
            if prop_req.description.len() > ultradag_coin::constants::PROPOSAL_DESCRIPTION_MAX_BYTES {
                return Ok(error_response(StatusCode::BAD_REQUEST,
                    &format!("description too long: max {} bytes", ultradag_coin::constants::PROPOSAL_DESCRIPTION_MAX_BYTES)));
            }

            let sk = match parse_secret_key(&prop_req.secret_key) {
                Ok(sk) => sk,
                Err(e) => return Ok(error_response(StatusCode::BAD_REQUEST, e)),
            };
            let sender = sk.address();

            let proposal_type = match prop_req.proposal_type.as_str() {
                "text" => ProposalType::TextProposal,
                "parameter" => {
                    let Some(param) = prop_req.parameter_name else {
                        return Ok(error_response(StatusCode::BAD_REQUEST, "parameter_name required for parameter change"));
                    };
                    let Some(value) = prop_req.parameter_value else {
                        return Ok(error_response(StatusCode::BAD_REQUEST, "parameter_value required for parameter change"));
                    };
                    ProposalType::ParameterChange { param, new_value: value }
                }
                "council_membership" => {
                    let Some(action_str) = prop_req.council_action else {
                        return Ok(error_response(StatusCode::BAD_REQUEST, "council_action required ('add' or 'remove')"));
                    };
                    let action = match action_str.as_str() {
                        "add" => CouncilAction::Add,
                        "remove" => CouncilAction::Remove,
                        _ => return Ok(error_response(StatusCode::BAD_REQUEST, "council_action must be 'add' or 'remove'")),
                    };
                    let Some(addr_hex) = prop_req.council_address else {
                        return Ok(error_response(StatusCode::BAD_REQUEST, "council_address required (hex address of candidate)"));
                    };
                    let Some(address) = Address::parse(&addr_hex) else {
                        return Ok(error_response(StatusCode::BAD_REQUEST, "invalid council_address: expected 40-char hex or bech32m (udag1.../tudg1...)"));
                    };
                    let Some(cat_str) = prop_req.council_category else {
                        return Ok(error_response(StatusCode::BAD_REQUEST, "council_category required (technical, business, legal, academic, community, foundation)"));
                    };
                    let Some(category) = CouncilSeatCategory::parse_name(&cat_str) else {
                        return Ok(error_response(StatusCode::BAD_REQUEST,
                            "invalid council_category: must be technical, business, legal, academic, community, or foundation"));
                    };
                    ProposalType::CouncilMembership { action, address, category }
                }
                "treasury_spend" => {
                    let Some(recipient_hex) = prop_req.treasury_recipient.as_deref() else {
                        return Ok(error_response(StatusCode::BAD_REQUEST, "treasury_recipient required for treasury_spend"));
                    };
                    let Ok(recipient_bytes) = hex::decode(recipient_hex) else {
                        return Ok(error_response(StatusCode::BAD_REQUEST, "invalid hex for treasury_recipient"));
                    };
                    if recipient_bytes.len() != 20 {
                        return Ok(error_response(StatusCode::BAD_REQUEST,
                            "treasury_recipient must be 20 bytes (40 hex chars)"));
                    }
                    let mut addr_bytes = [0u8; 20];
                    addr_bytes.copy_from_slice(&recipient_bytes);
                    let recipient = ultradag_coin::Address(addr_bytes);
                    let Some(amount) = prop_req.treasury_amount else {
                        return Ok(error_response(StatusCode::BAD_REQUEST,
                            "treasury_amount required for treasury_spend (in sats)"));
                    };
                    if amount == 0 {
                        return Ok(error_response(StatusCode::BAD_REQUEST,
                            "treasury_amount must be greater than 0"));
                    }
                    ProposalType::TreasurySpend { recipient, amount }
                }
                _ => return Ok(error_response(StatusCode::BAD_REQUEST,
                    "proposal_type must be 'text', 'parameter', 'council_membership', or 'treasury_spend'")),
            };

            let fee = prop_req.fee.unwrap_or(ultradag_coin::constants::MIN_FEE_SATS);
            if fee < ultradag_coin::constants::MIN_FEE_SATS {
                return Ok(error_response(
                    StatusCode::BAD_REQUEST,
                    &format!("fee too low: minimum {} sats (0.0001 UDAG)", ultradag_coin::constants::MIN_FEE_SATS),
                ));
            }

            let (tx, tx_hash, proposal_id) = {
                let state = read_lock_or_503!(server.state);
                let mut mp = write_lock_or_503!(server.mempool);

                let nonce = next_nonce(&state, &mp, &sender);

                let balance = state.balance(&sender);
                let pending = pending_cost(&mp, &sender);
                let total_needed = pending.saturating_add(fee);
                if balance < total_needed {
                    return Ok(error_response(StatusCode::BAD_REQUEST,
                        &format!("insufficient balance for fee: need {} (fee={}, pending={}), have {}",
                            total_needed, fee, pending, balance)));
                }

                // Only council members can create proposals
                // Exception: self-nominations (CouncilMembership Add where address == sender)
                let is_self_nomination = matches!(
                    &proposal_type,
                    ProposalType::CouncilMembership { action: CouncilAction::Add, address, .. }
                    if *address == sender
                );
                if !state.is_council_member(&sender) && !is_self_nomination {
                    return Ok(error_response(StatusCode::FORBIDDEN,
                        "only Council of 21 members can create proposals (anyone can self-nominate via council_membership)"));
                }

                // Check active proposal count limit
                let active_count = state.proposals().values()
                    .filter(|p| matches!(p.status, ultradag_coin::governance::ProposalStatus::Active))
                    .count() as u64;
                if active_count >= state.governance_params().max_active_proposals {
                    return Ok(error_response(StatusCode::BAD_REQUEST,
                        "too many active proposals, please wait for existing proposals to complete"));
                }

                // Avoid TOCTOU: if another concurrent request already inserted a
                // CreateProposal tx into the mempool with the same ID, increment past it.
                let mut proposal_id = state.next_proposal_id();
                if let Some(max_pending) = mp.max_pending_proposal_id() {
                    if max_pending >= proposal_id {
                        proposal_id = max_pending.saturating_add(1);
                    }
                }

                let mut create_tx = CreateProposalTx {
                    from: sender,
                    proposal_id,
                    title: prop_req.title.clone(),
                    description: prop_req.description.clone(),
                    proposal_type,
                    fee,
                    nonce,
                    pub_key: sk.verifying_key().to_bytes(),
                    signature: Signature([0u8; 64]),
                };
                create_tx.signature = sk.sign(&create_tx.signable_bytes());
                let tx = Transaction::CreateProposal(create_tx);
                let tx_hash = tx.hash();

                if !mp.insert(tx.clone()) {
                    return Ok(error_response(StatusCode::CONFLICT, "duplicate transaction"));
                }

                (tx, tx_hash, proposal_id)
            };

            server.peers.broadcast(&Message::NewTx(tx.clone()), "").await;
            let _ = server.tx_tx.send(tx);

            json_response(StatusCode::OK, &serde_json::json!({
                "status": "pending",
                "tx_hash": hex_encode(&tx_hash),
                "proposal_id": proposal_id,
                "proposer": sender.to_hex(),
                "title": prop_req.title,
                "note": "Proposal transaction added to mempool. Will be created when included in a finalized vertex."
            }))
        }

        (&Method::POST, ["vote"]) => {
            // TESTNET ONLY: accepts secret_key in body. Mainnet: use /tx/submit with pre-signed Vote tx.
            if !server.testnet_mode {
                return Ok(error_response(StatusCode::GONE,
                    "POST /vote disabled: private keys must not transit over the network. Use /tx/submit with a pre-signed Vote transaction."));
            }
            // Check endpoint-specific rate limit
            if !rate_limiter.check_rate_limit(client_ip, limits::VOTE) {
                return Ok(error_response(
                    StatusCode::TOO_MANY_REQUESTS,
                    "rate limit exceeded: too many vote requests",
                ));
            }
            let body = req.collect().await?.to_bytes();
            if body.len() > MAX_REQUEST_SIZE {
                return Ok(error_response(StatusCode::PAYLOAD_TOO_LARGE, "request body too large"));
            }
            let Ok(vote_req) = serde_json::from_slice::<VoteRequest>(&body) else {
                return Ok(error_response(StatusCode::BAD_REQUEST,
                    "invalid JSON: need {secret_key, proposal_id, vote}"));
            };

            let sk = match parse_secret_key(&vote_req.secret_key) {
                Ok(sk) => sk,
                Err(e) => return Ok(error_response(StatusCode::BAD_REQUEST, e)),
            };
            let sender = sk.address();
            let fee = vote_req.fee.unwrap_or(ultradag_coin::constants::MIN_FEE_SATS);
            if fee < ultradag_coin::constants::MIN_FEE_SATS {
                return Ok(error_response(
                    StatusCode::BAD_REQUEST,
                    &format!("fee too low: minimum {} sats (0.0001 UDAG)", ultradag_coin::constants::MIN_FEE_SATS),
                ));
            }

            let (tx, tx_hash) = {
                let state = read_lock_or_503!(server.state);
                let mut mp = write_lock_or_503!(server.mempool);

                // Check that proposal exists and is accepting votes
                let proposal = match state.proposal(vote_req.proposal_id) {
                    Some(p) => p,
                    None => return Ok(error_response(StatusCode::BAD_REQUEST, "proposal not found")),
                };
                let current_round = state.last_finalized_round().unwrap_or(0);
                if !proposal.is_voting_open(current_round) {
                    return Ok(error_response(StatusCode::BAD_REQUEST,
                        &format!("voting is not open for this proposal (status: {:?})", proposal.status)));
                }

                // Only council members can vote
                if !state.is_council_member(&sender) {
                    return Ok(error_response(StatusCode::FORBIDDEN,
                        "only Council of 21 members can vote on proposals"));
                }

                // Check if voter already voted on this proposal
                if state.get_vote(vote_req.proposal_id, &sender).is_some() {
                    return Ok(error_response(StatusCode::BAD_REQUEST, "already voted on this proposal"));
                }

                let nonce = next_nonce(&state, &mp, &sender);

                let balance = state.balance(&sender);
                let pending = pending_cost(&mp, &sender);
                let total_needed = pending.saturating_add(fee);
                if balance < total_needed {
                    return Ok(error_response(StatusCode::BAD_REQUEST,
                        &format!("insufficient balance for fee: need {} (fee={}, pending={}), have {}",
                            total_needed, fee, pending, balance)));
                }

                let mut vote_tx = VoteTx {
                    from: sender,
                    proposal_id: vote_req.proposal_id,
                    vote: vote_req.vote,
                    fee,
                    nonce,
                    pub_key: sk.verifying_key().to_bytes(),
                    signature: Signature([0u8; 64]),
                };
                vote_tx.signature = sk.sign(&vote_tx.signable_bytes());
                let tx = Transaction::Vote(vote_tx);
                let tx_hash = tx.hash();

                if !mp.insert(tx.clone()) {
                    return Ok(error_response(StatusCode::CONFLICT, "duplicate transaction"));
                }

                (tx, tx_hash)
            };

            server.peers.broadcast(&Message::NewTx(tx.clone()), "").await;
            let _ = server.tx_tx.send(tx);

            json_response(StatusCode::OK, &serde_json::json!({
                "status": "pending",
                "tx_hash": hex_encode(&tx_hash),
                "proposal_id": vote_req.proposal_id,
                "voter": sender.to_hex(),
                "vote": if vote_req.vote { "yes" } else { "no" },
                "note": "Vote transaction added to mempool. Will be applied when included in a finalized vertex."
            }))
        }

        // ====== Governance GET endpoints ======

        (&Method::GET, ["governance", "config"]) => {
            let state = read_lock_or_503!(server.state);
            let gp = state.governance_params();
            json_response(
                StatusCode::OK,
                &serde_json::json!({
                    "min_stake_to_propose": gp.min_stake_to_propose,
                    "min_stake_to_propose_udag": gp.min_stake_to_propose as f64 / ultradag_coin::SATS_PER_UDAG as f64,
                    "voting_period_rounds": gp.voting_period_rounds,
                    "quorum_percent": (gp.quorum_numerator as f64 / ultradag_coin::constants::GOVERNANCE_QUORUM_DENOMINATOR as f64) * 100.0,
                    "approval_percent": (gp.approval_numerator as f64 / ultradag_coin::constants::GOVERNANCE_APPROVAL_DENOMINATOR as f64) * 100.0,
                    "execution_delay_rounds": gp.execution_delay_rounds,
                    "max_active_proposals": gp.max_active_proposals,
                    "min_fee_sats": gp.min_fee_sats,
                    "observer_reward_percent": gp.observer_reward_percent,
                    "council_emission_percent": gp.council_emission_percent,
                    "governable_params": ultradag_coin::governance::GovernanceParams::param_names(),
                }),
            )
        }

        (&Method::GET, ["council"]) => {
            let state = read_lock_or_503!(server.state);
            let mut member_pairs: Vec<_> = state.council_members().collect();
            member_pairs.sort_by_key(|(addr, _)| *addr);
            let members: Vec<serde_json::Value> = member_pairs.into_iter()
                .map(|(addr, cat)| serde_json::json!({
                    "address": addr.to_hex(),
                    "address_bech32": addr.to_bech32(),
                    "category": cat.name(),
                }))
                .collect();
            let mut seats_available = serde_json::Map::new();
            for cat in CouncilSeatCategory::all() {
                let filled = state.council_members()
                    .filter(|(_, c)| *c == cat)
                    .count();
                seats_available.insert(
                    cat.name().to_lowercase(),
                    serde_json::json!({
                        "filled": filled,
                        "max": cat.max_seats(),
                        "available": cat.max_seats().saturating_sub(filled),
                    }),
                );
            }
            let (per_member, total_emission) = state.compute_council_emission(
                state.last_finalized_round().unwrap_or(0),
            );
            json_response(StatusCode::OK, &serde_json::json!({
                "member_count": members.len(),
                "max_members": ultradag_coin::constants::COUNCIL_MAX_MEMBERS,
                "emission_percent": state.governance_params().council_emission_percent,
                "per_member_reward_sats": per_member,
                "per_member_reward_udag": per_member as f64 / ultradag_coin::SATS_PER_UDAG as f64,
                "total_emission_sats": total_emission,
                "members": members,
                "seats": seats_available,
            }))
        }

        (&Method::GET, ["proposals"]) => {
            let state = read_lock_or_503!(server.state);
            // Cap response to prevent unbounded data. Sort by ID descending (newest first).
            let mut proposal_ids: Vec<u64> = state.proposals().keys().copied().collect();
            proposal_ids.sort_unstable_by(|a, b| b.cmp(a));
            const MAX_PROPOSALS_RESPONSE: usize = 200;
            proposal_ids.truncate(MAX_PROPOSALS_RESPONSE);
            let proposals: Vec<serde_json::Value> = proposal_ids.iter()
                .filter_map(|id| state.proposals().get(id))
                .map(|p| {
                    let mut pj = serde_json::json!({
                        "id": p.id,
                        "proposer": p.proposer.to_hex(),
                        "title": p.title,
                        "description": p.description,
                        "proposal_type": p.proposal_type,
                        "voting_starts": p.voting_starts,
                        "voting_ends": p.voting_ends,
                        "votes_for": p.votes_for,
                        "votes_against": p.votes_against,
                        "status": p.status,
                    });
                    if let ProposalType::TreasurySpend { ref recipient, amount } = p.proposal_type {
                        pj["treasury_recipient"] = serde_json::json!(recipient.to_hex());
                        pj["treasury_amount_sats"] = serde_json::json!(amount);
                        pj["treasury_amount_udag"] = serde_json::json!(amount as f64 / ultradag_coin::SATS_PER_UDAG as f64);
                    }
                    pj
                })
                .collect();
            json_response(StatusCode::OK, &serde_json::json!({
                "count": proposals.len(),
                "proposals": proposals,
            }))
        }

        (&Method::GET, ["proposal", id_str]) => {
            let Ok(id) = id_str.parse::<u64>() else {
                return Ok(error_response(StatusCode::BAD_REQUEST, "invalid proposal ID"));
            };
            let state = read_lock_or_503!(server.state);
            let Some(p) = state.proposal(id) else {
                return Ok(error_response(StatusCode::NOT_FOUND, "proposal not found"));
            };
            // Include individual voter breakdown with vote weights
            let voters: Vec<serde_json::Value> = state.votes_for_proposal(id)
                .iter()
                .map(|(addr, vote, weight)| {
                    let category = state.council_seat_category(addr)
                        .map(|c| c.name())
                        .unwrap_or("former member");
                    serde_json::json!({
                        "address": addr.to_hex(),
                        "address_bech32": addr.to_bech32(),
                        "vote": if *vote { "yes" } else { "no" },
                        "vote_weight": weight,
                        "category": category,
                    })
                })
                .collect();
            let mut proposal_json = serde_json::json!({
                "id": p.id,
                "proposer": p.proposer.to_hex(),
                "title": p.title,
                "description": p.description,
                "proposal_type": p.proposal_type,
                "voting_starts": p.voting_starts,
                "voting_ends": p.voting_ends,
                "votes_for": p.votes_for,
                "votes_against": p.votes_against,
                "status": p.status,
                "snapshot_council_size": p.snapshot_total_stake,
                "voters": voters,
            });
            // For TreasurySpend proposals, include recipient and amount
            if let ProposalType::TreasurySpend { ref recipient, amount } = p.proposal_type {
                proposal_json["treasury_recipient"] = serde_json::json!(recipient.to_hex());
                proposal_json["treasury_amount_sats"] = serde_json::json!(amount);
                proposal_json["treasury_amount_udag"] = serde_json::json!(amount as f64 / ultradag_coin::SATS_PER_UDAG as f64);
            }
            json_response(StatusCode::OK, &proposal_json)
        }

        (&Method::GET, ["vote", id_str, addr_hex]) => {
            let Ok(id) = id_str.parse::<u64>() else {
                return Ok(error_response(StatusCode::BAD_REQUEST, "invalid proposal ID"));
            };
            let Some(addr) = Address::parse(addr_hex) else {
                return Ok(error_response(StatusCode::BAD_REQUEST, "invalid address: expected 40-char hex or bech32m (udag1.../tudg1...)"));
            };
            
            let state = read_lock_or_503!(server.state);
            let vote = state.get_vote(id, &addr);
            
            json_response(StatusCode::OK, &serde_json::json!({
                "vote": vote,
            }))
        }

        // ====== Transaction status & lookup endpoints ======

        // Look up a transaction by hash: checks mempool (pending) then finalized index.
        (&Method::GET, ["tx", hash_hex]) => {
            let Some(hash) = parse_hash_hex(hash_hex) else {
                return Ok(error_response(StatusCode::BAD_REQUEST, "invalid transaction hash (need 64 hex chars)"));
            };

            // Check mempool first (pending)
            {
                let mp = read_lock_or_503!(server.mempool);
                if mp.contains(&hash) {
                    return Ok(json_response(StatusCode::OK, &serde_json::json!({
                        "status": "pending",
                        "tx_hash": hash_hex,
                    })));
                }
            }

            // Check finalized tx index
            let state = read_lock_or_503!(server.state);
            if let Some(loc) = state.tx_location(&hash) {
                return Ok(json_response(StatusCode::OK, &serde_json::json!({
                    "status": "finalized",
                    "tx_hash": hash_hex,
                    "round": loc.round,
                    "vertex_hash": hex_encode(&loc.vertex_hash),
                    "validator": loc.validator.to_hex(),
                })));
            }

            error_response(StatusCode::NOT_FOUND, "transaction not found (not in mempool or recent finalized history)")
        }

        // Look up a vertex by hash
        (&Method::GET, ["vertex", hash_hex]) => {
            let Some(hash) = parse_hash_hex(hash_hex) else {
                return Ok(error_response(StatusCode::BAD_REQUEST, "invalid vertex hash (need 64 hex chars)"));
            };
            let dag = read_lock_or_503!(server.dag);
            let Some(v) = dag.get(&hash) else {
                return Ok(error_response(StatusCode::NOT_FOUND, "vertex not found (may be pruned)"));
            };
            let txs: Vec<serde_json::Value> = v.block.transactions.iter().map(|tx| {
                serde_json::json!({
                    "hash": hex_encode(&tx.hash()),
                    "type": match tx {
                        Transaction::Transfer(_) => "transfer",
                        Transaction::Stake(_) => "stake",
                        Transaction::Unstake(_) => "unstake",
                        Transaction::CreateProposal(_) => "create_proposal",
                        Transaction::Vote(_) => "vote",
                        Transaction::Delegate(_) => "delegate",
                        Transaction::Undelegate(_) => "undelegate",
                        Transaction::SetCommission(_) => "set_commission",
                        Transaction::BridgeDeposit(_) => "bridge_lock",
                        Transaction::BridgeRelease(_) => "bridge_release",
                    },
                    "from": tx.from().to_hex(),
                    "fee": tx.fee(),
                    "nonce": tx.nonce(),
                })
            }).collect();
            json_response(StatusCode::OK, &serde_json::json!({
                "hash": hash_hex,
                "round": v.round,
                "validator": v.validator.to_hex(),
                "parent_count": v.parent_hashes.len(),
                "coinbase": {
                    "to": v.block.coinbase.to.to_hex(),
                    "amount": v.block.coinbase.amount,
                    "height": v.block.coinbase.height,
                },
                "transactions": txs,
            }))
        }

        // Submit a pre-signed transaction (enables client-side signing / light clients).
        // Accepts a JSON-serialized Transaction or {tx_hex: "..."} format.
        // Verifies signature, validates against state, inserts in mempool, and broadcasts.
        (&Method::POST, ["tx", "submit"]) => {
            if !rate_limiter.check_rate_limit(client_ip, limits::TX) {
                return Ok(error_response(
                    StatusCode::TOO_MANY_REQUESTS,
                    "rate limit exceeded: too many tx requests",
                ));
            }
            let body = req.collect().await?.to_bytes();
            if body.len() > MAX_REQUEST_SIZE {
                return Ok(error_response(StatusCode::PAYLOAD_TOO_LARGE, "request body too large (max 1MB)"));
            }
            
            // Parse request - accept either {tx_hex: "..."} or direct Transaction JSON
            let tx: Transaction = if let Ok(raw) = serde_json::from_slice::<RawTxHex>(&body) {
                // Hex-encoded serialized transaction (postcard format)
                let tx_bytes = match hex::decode(&raw.tx_hex) {
                    Ok(bytes) => bytes,
                    Err(_) => return Ok(error_response(StatusCode::BAD_REQUEST, "invalid hex in tx_hex")),
                };
                match postcard::from_bytes(&tx_bytes) {
                    Ok(tx) => tx,
                    Err(_) => return Ok(error_response(StatusCode::BAD_REQUEST, "failed to deserialize transaction from hex")),
                }
            } else if let Ok(tx_direct) = serde_json::from_slice::<Transaction>(&body) {
                // Direct JSON transaction
                tx_direct
            } else {
                return Ok(error_response(StatusCode::BAD_REQUEST,
                    "invalid JSON: expected a serialized Transaction or {tx_hex: \"...\"}"));
            };

            // Verify Ed25519 signature
            if !tx.verify_signature() {
                return Ok(error_response(StatusCode::BAD_REQUEST, "invalid signature"));
            }

            // Validate transaction-type-specific constraints before touching state/mempool.
            // These mirror the checks in the per-endpoint handlers (POST /tx, /stake, etc.)
            // but are critical here because /tx/submit is the ONLY mainnet tx path.
            match &tx {
                Transaction::Transfer(t) => {
                    if t.amount == 0 {
                        return Ok(error_response(StatusCode::BAD_REQUEST, "transfer amount must be greater than 0"));
                    }
                    if t.fee < ultradag_coin::constants::MIN_FEE_SATS {
                        return Ok(error_response(StatusCode::BAD_REQUEST,
                            &format!("fee too low: minimum {} sats", ultradag_coin::constants::MIN_FEE_SATS)));
                    }
                    if let Some(ref memo) = t.memo {
                        if memo.len() > ultradag_coin::constants::MAX_MEMO_BYTES {
                            return Ok(error_response(StatusCode::BAD_REQUEST,
                                &format!("memo too large: {} bytes (max {})", memo.len(), ultradag_coin::constants::MAX_MEMO_BYTES)));
                        }
                    }
                }
                Transaction::Stake(t) => {
                    if t.amount < ultradag_coin::MIN_STAKE_SATS {
                        return Ok(error_response(StatusCode::BAD_REQUEST,
                            &format!("minimum stake is {} sats ({} UDAG)",
                                ultradag_coin::MIN_STAKE_SATS,
                                ultradag_coin::MIN_STAKE_SATS / ultradag_coin::SATS_PER_UDAG)));
                    }
                }
                Transaction::Delegate(t) => {
                    if t.amount < ultradag_coin::MIN_DELEGATION_SATS {
                        return Ok(error_response(StatusCode::BAD_REQUEST,
                            &format!("minimum delegation is {} sats ({} UDAG)",
                                ultradag_coin::MIN_DELEGATION_SATS,
                                ultradag_coin::MIN_DELEGATION_SATS / ultradag_coin::SATS_PER_UDAG)));
                    }
                    if t.from == t.validator {
                        return Ok(error_response(StatusCode::BAD_REQUEST, "cannot delegate to self"));
                    }
                }
                Transaction::SetCommission(t) => {
                    if t.commission_percent > ultradag_coin::MAX_COMMISSION_PERCENT {
                        return Ok(error_response(StatusCode::BAD_REQUEST,
                            &format!("commission_percent must be 0-{}", ultradag_coin::MAX_COMMISSION_PERCENT)));
                    }
                }
                Transaction::CreateProposal(t) => {
                    if t.fee < ultradag_coin::constants::MIN_FEE_SATS {
                        return Ok(error_response(StatusCode::BAD_REQUEST,
                            &format!("fee too low: minimum {} sats", ultradag_coin::constants::MIN_FEE_SATS)));
                    }
                    if t.title.len() > ultradag_coin::constants::PROPOSAL_TITLE_MAX_BYTES {
                        return Ok(error_response(StatusCode::BAD_REQUEST,
                            &format!("title too long: max {} bytes", ultradag_coin::constants::PROPOSAL_TITLE_MAX_BYTES)));
                    }
                    if t.description.len() > ultradag_coin::constants::PROPOSAL_DESCRIPTION_MAX_BYTES {
                        return Ok(error_response(StatusCode::BAD_REQUEST,
                            &format!("description too long: max {} bytes", ultradag_coin::constants::PROPOSAL_DESCRIPTION_MAX_BYTES)));
                    }
                }
                Transaction::Vote(t) => {
                    if t.fee < ultradag_coin::constants::MIN_FEE_SATS {
                        return Ok(error_response(StatusCode::BAD_REQUEST,
                            &format!("fee too low: minimum {} sats", ultradag_coin::constants::MIN_FEE_SATS)));
                    }
                }
                Transaction::BridgeDeposit(t) => {
                    if t.amount < ultradag_coin::constants::MIN_BRIDGE_AMOUNT_SATS {
                        return Ok(error_response(StatusCode::BAD_REQUEST,
                            &format!("minimum bridge amount is {} sats ({} UDAG)",
                                ultradag_coin::constants::MIN_BRIDGE_AMOUNT_SATS,
                                ultradag_coin::constants::MIN_BRIDGE_AMOUNT_SATS / ultradag_coin::SATS_PER_UDAG)));
                    }
                    if t.fee < ultradag_coin::constants::MIN_FEE_SATS {
                        return Ok(error_response(StatusCode::BAD_REQUEST,
                            &format!("fee too low: minimum {} sats", ultradag_coin::constants::MIN_FEE_SATS)));
                    }
                }
                Transaction::BridgeRelease(t) => {
                    if t.amount < ultradag_coin::constants::MIN_BRIDGE_AMOUNT_SATS {
                        return Ok(error_response(StatusCode::BAD_REQUEST,
                            &format!("minimum bridge amount is {} sats", ultradag_coin::constants::MIN_BRIDGE_AMOUNT_SATS)));
                    }
                    if t.amount > ultradag_coin::constants::MAX_BRIDGE_AMOUNT_SATS {
                        return Ok(error_response(StatusCode::BAD_REQUEST,
                            &format!("bridge release exceeds maximum: {} sats", ultradag_coin::constants::MAX_BRIDGE_AMOUNT_SATS)));
                    }
                }
                // Unstake, Undelegate — no amount/fee fields to validate
                Transaction::Unstake(_) | Transaction::Undelegate(_) => {}
            }

            let tx_hash = tx.hash();

            // Atomic validation + mempool insertion (hold both locks to prevent TOCTOU)
            {
                let state = read_lock_or_503!(server.state);
                let mut mp = write_lock_or_503!(server.mempool);

                let sender = tx.from();
                let balance = state.balance(&sender);
                let total_cost = tx.total_cost();
                let pc = pending_cost(&mp, &sender);
                let total_needed = pc.saturating_add(total_cost);
                if balance < total_needed {
                    return Ok(error_response(StatusCode::BAD_REQUEST,
                        &format!("insufficient balance: need {} sats (incl. {} pending), have {} sats",
                            total_needed, pc, balance)));
                }
                // Check nonce isn't stale and isn't too far ahead.
                // A nonce gap > 100 would waste mempool slots with unexecutable txs.
                let expected_nonce = state.nonce(&sender);
                if tx.nonce() < expected_nonce {
                    return Ok(error_response(StatusCode::BAD_REQUEST,
                        &format!("nonce too low: expected >= {}, got {}", expected_nonce, tx.nonce())));
                }
                if tx.nonce() > expected_nonce.saturating_add(100) {
                    return Ok(error_response(StatusCode::BAD_REQUEST,
                        &format!("nonce too far ahead: expected near {}, got {}", expected_nonce, tx.nonce())));
                }

                // Insert into mempool while still holding the locks
                if !mp.insert(tx.clone()) {
                    return Ok(error_response(StatusCode::CONFLICT, "transaction rejected by mempool (duplicate or fee too low)"));
                }
            }

            // Broadcast
            server.peers.broadcast(&Message::NewTx(tx.clone()), "").await;
            let _ = server.tx_tx.send(tx);

            json_response(StatusCode::OK, &serde_json::json!({
                "status": "pending",
                "tx_hash": hex_encode(&tx_hash),
            }))
        }

        // ====== Delegation endpoints ======

        (&Method::POST, ["delegate"]) => {
            // TESTNET ONLY: accepts secret_key in body. Mainnet: use /tx/submit with pre-signed DelegateTx.
            if !server.testnet_mode {
                return Ok(error_response(StatusCode::GONE,
                    "POST /delegate disabled: private keys must not transit over the network. Use /tx/submit with a pre-signed DelegateTx."));
            }
            if !rate_limiter.check_rate_limit(client_ip, limits::DELEGATE) {
                return Ok(error_response(
                    StatusCode::TOO_MANY_REQUESTS,
                    "rate limit exceeded: too many delegate requests",
                ));
            }

            let body = req.collect().await?.to_bytes();
            if body.len() > MAX_REQUEST_SIZE {
                return Ok(error_response(StatusCode::PAYLOAD_TOO_LARGE, "request body too large (max 1MB)"));
            }
            let Ok(delegate_req) = serde_json::from_slice::<DelegateRequest>(&body) else {
                return Ok(error_response(StatusCode::BAD_REQUEST, "invalid JSON body, need: {secret_key, validator, amount}"));
            };

            let sk = match parse_secret_key(&delegate_req.secret_key) {
                Ok(sk) => sk,
                Err(e) => return Ok(error_response(StatusCode::BAD_REQUEST, e)),
            };
            let sender = sk.address();

            let Some(validator_addr) = Address::parse(&delegate_req.validator) else {
                return Ok(error_response(StatusCode::BAD_REQUEST, "invalid validator address: expected 40-char hex or bech32m (udag1.../tudg1...)"));
            };

            // Validate: cannot delegate to self (Bug #149)
            if sender == validator_addr {
                return Ok(error_response(StatusCode::BAD_REQUEST, "cannot delegate to self"));
            }

            let (tx, tx_hash, nonce) = {
                let state = read_lock_or_503!(server.state);
                let mut mp = write_lock_or_503!(server.mempool);

                // Validate: minimum delegation amount
                if delegate_req.amount < ultradag_coin::MIN_DELEGATION_SATS {
                    return Ok(error_response(StatusCode::BAD_REQUEST,
                        &format!("minimum delegation is {} sats ({} UDAG)",
                            ultradag_coin::MIN_DELEGATION_SATS,
                            ultradag_coin::MIN_DELEGATION_SATS / ultradag_coin::SATS_PER_UDAG)));
                }

                // Validate: validator must be staking
                if state.stake_of(&validator_addr) == 0 {
                    return Ok(error_response(StatusCode::BAD_REQUEST, "target validator is not staking"));
                }

                // Validate: not already delegating
                if state.delegation_account(&sender).is_some() {
                    return Ok(error_response(StatusCode::BAD_REQUEST, "already has an active delegation — undelegate first"));
                }

                let nonce = next_nonce(&state, &mp, &sender);
                let balance = state.balance(&sender);
                let pc = pending_cost(&mp, &sender);
                let total_needed = pc.saturating_add(delegate_req.amount);

                if balance < total_needed {
                    return Ok(error_response(StatusCode::BAD_REQUEST,
                        &format!("insufficient balance: need {} sats (incl. {} pending), have {} sats",
                            total_needed, pc, balance)));
                }

                let mut delegate_tx = DelegateTx {
                    from: sender,
                    validator: validator_addr,
                    amount: delegate_req.amount,
                    nonce,
                    pub_key: sk.verifying_key().to_bytes(),
                    signature: Signature([0u8; 64]),
                };
                delegate_tx.signature = sk.sign(&delegate_tx.signable_bytes());
                let tx = Transaction::Delegate(delegate_tx);
                let tx_hash = tx.hash();

                if !mp.insert(tx.clone()) {
                    return Ok(error_response(StatusCode::CONFLICT, "duplicate transaction"));
                }

                (tx, tx_hash, nonce)
            };

            server.peers.broadcast(&Message::NewTx(tx.clone()), "").await;
            let _ = server.tx_tx.send(tx);

            json_response(StatusCode::OK, &serde_json::json!({
                "status": "pending",
                "tx_hash": hex_encode(&tx_hash),
                "address": sender.to_hex(),
                "validator": validator_addr.to_hex(),
                "amount": delegate_req.amount,
                "amount_udag": delegate_req.amount as f64 / ultradag_coin::SATS_PER_UDAG as f64,
                "nonce": nonce,
                "note": "Delegate transaction added to mempool. Will be applied when included in a finalized vertex."
            }))
        }

        (&Method::POST, ["undelegate"]) => {
            // TESTNET ONLY: accepts secret_key in body. Mainnet: use /tx/submit with pre-signed UndelegateTx.
            if !server.testnet_mode {
                return Ok(error_response(StatusCode::GONE,
                    "POST /undelegate disabled: private keys must not transit over the network. Use /tx/submit with a pre-signed UndelegateTx."));
            }
            if !rate_limiter.check_rate_limit(client_ip, limits::UNDELEGATE) {
                return Ok(error_response(
                    StatusCode::TOO_MANY_REQUESTS,
                    "rate limit exceeded: too many undelegate requests",
                ));
            }

            let body = req.collect().await?.to_bytes();
            if body.len() > MAX_REQUEST_SIZE {
                return Ok(error_response(StatusCode::PAYLOAD_TOO_LARGE, "request body too large (max 1MB)"));
            }
            let Ok(undelegate_req) = serde_json::from_slice::<UndelegateRequest>(&body) else {
                return Ok(error_response(StatusCode::BAD_REQUEST, "invalid JSON body, need: {secret_key}"));
            };

            let sk = match parse_secret_key(&undelegate_req.secret_key) {
                Ok(sk) => sk,
                Err(e) => return Ok(error_response(StatusCode::BAD_REQUEST, e)),
            };
            let sender = sk.address();

            let current_round = {
                let dag = read_lock_or_503!(server.dag);
                dag.current_round()
            };

            let (tx, tx_hash, nonce) = {
                let state = read_lock_or_503!(server.state);
                let mut mp = write_lock_or_503!(server.mempool);

                // Check that address has an active delegation
                match state.delegation_account(&sender) {
                    None => return Ok(error_response(StatusCode::BAD_REQUEST, "no active delegation")),
                    Some(d) if d.unlock_at_round.is_some() => {
                        return Ok(error_response(StatusCode::BAD_REQUEST, "already undelegating — wait for cooldown to complete"));
                    }
                    _ => {}
                }

                let nonce = next_nonce(&state, &mp, &sender);

                let mut undelegate_tx = UndelegateTx {
                    from: sender,
                    nonce,
                    pub_key: sk.verifying_key().to_bytes(),
                    signature: Signature([0u8; 64]),
                };
                undelegate_tx.signature = sk.sign(&undelegate_tx.signable_bytes());
                let tx = Transaction::Undelegate(undelegate_tx);
                let tx_hash = tx.hash();

                if !mp.insert(tx.clone()) {
                    return Ok(error_response(StatusCode::CONFLICT, "duplicate transaction"));
                }

                (tx, tx_hash, nonce)
            };

            server.peers.broadcast(&Message::NewTx(tx.clone()), "").await;
            let _ = server.tx_tx.send(tx);

            let unlock_at = current_round + ultradag_coin::UNSTAKE_COOLDOWN_ROUNDS;
            json_response(StatusCode::OK, &serde_json::json!({
                "status": "pending",
                "tx_hash": hex_encode(&tx_hash),
                "address": sender.to_hex(),
                "unlock_at_round": unlock_at,
                "nonce": nonce,
                "note": "Undelegate transaction added to mempool. Will be applied when included in a finalized vertex."
            }))
        }

        (&Method::POST, ["set-commission"]) => {
            // TESTNET ONLY: accepts secret_key in body. Mainnet: use /tx/submit with pre-signed SetCommissionTx.
            if !server.testnet_mode {
                return Ok(error_response(StatusCode::GONE,
                    "POST /set-commission disabled: private keys must not transit over the network. Use /tx/submit with a pre-signed SetCommissionTx."));
            }
            if !rate_limiter.check_rate_limit(client_ip, limits::SET_COMMISSION) {
                return Ok(error_response(
                    StatusCode::TOO_MANY_REQUESTS,
                    "rate limit exceeded: too many set-commission requests",
                ));
            }

            let body = req.collect().await?.to_bytes();
            if body.len() > MAX_REQUEST_SIZE {
                return Ok(error_response(StatusCode::PAYLOAD_TOO_LARGE, "request body too large (max 1MB)"));
            }
            let Ok(commission_req) = serde_json::from_slice::<SetCommissionRequest>(&body) else {
                return Ok(error_response(StatusCode::BAD_REQUEST, "invalid JSON body, need: {secret_key, commission_percent}"));
            };

            let sk = match parse_secret_key(&commission_req.secret_key) {
                Ok(sk) => sk,
                Err(e) => return Ok(error_response(StatusCode::BAD_REQUEST, e)),
            };
            let sender = sk.address();

            let (tx, tx_hash, nonce) = {
                let state = read_lock_or_503!(server.state);
                let mut mp = write_lock_or_503!(server.mempool);

                // Must be staking to set commission
                if state.stake_of(&sender) == 0 {
                    return Ok(error_response(StatusCode::BAD_REQUEST, "not staked — must be a validator to set commission"));
                }

                if commission_req.commission_percent > ultradag_coin::MAX_COMMISSION_PERCENT {
                    return Ok(error_response(StatusCode::BAD_REQUEST,
                        &format!("commission_percent must be 0-{}", ultradag_coin::MAX_COMMISSION_PERCENT)));
                }

                let nonce = next_nonce(&state, &mp, &sender);

                let mut set_commission_tx = SetCommissionTx {
                    from: sender,
                    commission_percent: commission_req.commission_percent,
                    nonce,
                    pub_key: sk.verifying_key().to_bytes(),
                    signature: Signature([0u8; 64]),
                };
                set_commission_tx.signature = sk.sign(&set_commission_tx.signable_bytes());
                let tx = Transaction::SetCommission(set_commission_tx);
                let tx_hash = tx.hash();

                if !mp.insert(tx.clone()) {
                    return Ok(error_response(StatusCode::CONFLICT, "duplicate transaction"));
                }

                (tx, tx_hash, nonce)
            };

            server.peers.broadcast(&Message::NewTx(tx.clone()), "").await;
            let _ = server.tx_tx.send(tx);

            json_response(StatusCode::OK, &serde_json::json!({
                "status": "pending",
                "tx_hash": hex_encode(&tx_hash),
                "address": sender.to_hex(),
                "commission_percent": commission_req.commission_percent,
                "nonce": nonce,
                "note": "Set-commission transaction added to mempool. Will be applied when included in a finalized vertex."
            }))
        }

        (&Method::GET, ["delegation", addr_hex]) => {
            let Some(addr) = Address::parse(addr_hex) else {
                return Ok(error_response(StatusCode::BAD_REQUEST, "invalid address: expected 40-char hex or bech32m (udag1.../tudg1...)"));
            };
            let state = read_lock_or_503!(server.state);
            match state.delegation_account(&addr) {
                Some(d) => {
                    json_response(StatusCode::OK, &serde_json::json!({
                        "address": addr.to_hex(),
                        "delegated": d.delegated,
                        "delegated_udag": d.delegated as f64 / ultradag_coin::SATS_PER_UDAG as f64,
                        "validator": d.validator.to_hex(),
                        "unlock_at_round": d.unlock_at_round,
                        "is_undelegating": d.unlock_at_round.is_some(),
                    }))
                }
                None => {
                    error_response(StatusCode::NOT_FOUND, "no active delegation for this address")
                }
            }
        }

        (&Method::GET, ["validator", addr_hex, "delegators"]) => {
            let Some(addr) = Address::parse(addr_hex) else {
                return Ok(error_response(StatusCode::BAD_REQUEST, "invalid address: expected 40-char hex or bech32m (udag1.../tudg1...)"));
            };
            let state = read_lock_or_503!(server.state);
            let delegators = state.delegators_of(&addr);
            let total: u64 = delegators.iter().map(|(_, amt)| *amt).fold(0u64, |acc, x| acc.saturating_add(x));
            // Cap delegator list to prevent unbounded response size
            const MAX_DELEGATORS_RESPONSE: usize = 500;
            let delegator_list: Vec<serde_json::Value> = delegators.iter().take(MAX_DELEGATORS_RESPONSE).map(|(delegator_addr, amount)| {
                let deleg = state.delegation_account(delegator_addr);
                serde_json::json!({
                    "address": delegator_addr.to_hex(),
                    "delegated": amount,
                    "delegated_udag": *amount as f64 / ultradag_coin::SATS_PER_UDAG as f64,
                    "is_undelegating": deleg.map(|d| d.unlock_at_round.is_some()).unwrap_or(false),
                })
            }).collect();
            json_response(StatusCode::OK, &serde_json::json!({
                "validator": addr.to_hex(),
                "delegator_count": delegators.len(),
                "total_delegated": total,
                "total_delegated_udag": total as f64 / ultradag_coin::SATS_PER_UDAG as f64,
                "delegators": delegator_list,
            }))
        }

        // ====== Treasury endpoint ======

        (&Method::GET, ["treasury"]) => {
            let state = read_lock_or_503!(server.state);
            let balance = state.treasury_balance();
            let pending_spends: Vec<serde_json::Value> = state.proposals()
                .values()
                .filter(|p| matches!(p.proposal_type, ProposalType::TreasurySpend { .. }))
                .filter(|p| !matches!(p.status, ultradag_coin::governance::ProposalStatus::Executed
                    | ultradag_coin::governance::ProposalStatus::Rejected
                    | ultradag_coin::governance::ProposalStatus::Cancelled
                    | ultradag_coin::governance::ProposalStatus::Failed { .. }))
                .map(|p| {
                    if let ProposalType::TreasurySpend { ref recipient, amount } = p.proposal_type {
                        serde_json::json!({
                            "proposal_id": p.id,
                            "recipient": recipient.to_hex(),
                            "amount_sats": amount,
                            "amount_udag": amount as f64 / ultradag_coin::SATS_PER_UDAG as f64,
                            "status": p.status,
                        })
                    } else {
                        serde_json::json!({})
                    }
                })
                .collect();
            json_response(StatusCode::OK, &serde_json::json!({
                "balance_sats": balance,
                "balance_udag": balance as f64 / ultradag_coin::SATS_PER_UDAG as f64,
                "pending_spends": pending_spends,
            }))
        }

        // ====== Metrics endpoints ======

        (&Method::GET, ["metrics", "json"]) => {
            // JSON format for dashboards
            let metrics = server.checkpoint_metrics.export_json();
            json_response(StatusCode::OK, &metrics)
        }

        _ => error_response(StatusCode::NOT_FOUND, "not found"),
    };

    Ok(response)
}

/// RAII guard to decrement connection count when dropped
struct ConnectionGuard {
    rate_limiter: Arc<RateLimiter>,
}

impl ConnectionGuard {
    fn new(rate_limiter: Arc<RateLimiter>) -> Self {
        Self { rate_limiter }
    }
}

impl Drop for ConnectionGuard {
    fn drop(&mut self) {
        self.rate_limiter.remove_connection();
    }
}

pub async fn start_rpc(server: Arc<NodeServer>, rpc_port: u16) {
    let listener = match TcpListener::bind(format!("[::]:{}", rpc_port)).await {
        Ok(l) => l,
        Err(e) => {
            error!("RPC bind failed on port {}: {}", rpc_port, e);
            return;
        }
    };

    info!("RPC server listening on http://0.0.0.0:{}", rpc_port);

    let rate_limiter = Arc::new(RateLimiter::new());
    
    // Spawn cleanup task for rate limiter
    let rate_limiter_cleanup = rate_limiter.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(300));
        loop {
            interval.tick().await;
            rate_limiter_cleanup.cleanup_expired();
        }
    });

    loop {
        let (stream, addr) = match listener.accept().await {
            Ok(s) => s,
            Err(e) => {
                error!("RPC accept error: {}", e);
                continue;
            }
        };

        // Extract client IP
        let client_ip = match addr {
            SocketAddr::V4(v4) => std::net::IpAddr::V4(*v4.ip()),
            SocketAddr::V6(v6) => std::net::IpAddr::V6(*v6.ip()),
        };

        let rate_limiter_clone = rate_limiter.clone();
        
        // Check connection limits
        if let Err(e) = rate_limiter_clone.add_connection() {
            warn!("Connection rejected from {}: {}", client_ip, e);
            continue;
        }

        let server_clone = server.clone();
        tokio::spawn(async move {
            let _guard = ConnectionGuard::new(rate_limiter_clone.clone());
            
            let io = TokioIo::new(stream);
            let svc = service_fn(move |req| {
                let s = server_clone.clone();
                let rl = rate_limiter_clone.clone();
                async move { handle_request(req, s, rl, client_ip).await }
            });
            if let Err(e) = http1::Builder::new().serve_connection(io, svc).await {
                if !e.is_incomplete_message() {
                    error!("RPC connection error: {}", e);
                }
            }
        });
    }
}

fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Parse a 64-hex-char hash string into a [u8; 32].
fn parse_hash_hex(hex: &str) -> Option<[u8; 32]> {
    if hex.len() != 64 || !hex.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let mut bytes = [0u8; 32];
    for (i, chunk) in hex.as_bytes().chunks(2).enumerate() {
        let s = std::str::from_utf8(chunk).ok()?;
        bytes[i] = u8::from_str_radix(s, 16).ok()?;
    }
    Some(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_secret_key_valid() {
        let hex = "aa".repeat(32);
        assert!(parse_secret_key(&hex).is_ok());
    }

    #[test]
    fn test_parse_secret_key_too_short() {
        let hex = "aa".repeat(31);
        assert!(matches!(parse_secret_key(&hex), Err("secret key must be 64 hex chars (32 bytes)")));
    }

    #[test]
    fn test_parse_secret_key_too_long() {
        let hex = "aa".repeat(33);
        assert!(matches!(parse_secret_key(&hex), Err("secret key must be 64 hex chars (32 bytes)")));
    }

    #[test]
    fn test_parse_secret_key_invalid_hex() {
        let hex = "zz".repeat(32);
        assert!(matches!(parse_secret_key(&hex), Err("invalid hex in secret key")));
    }

    #[test]
    fn test_parse_secret_key_null_byte() {
        let mut hex = "aa".repeat(31);
        hex.push('\0');
        hex.push('a');
        assert!(parse_secret_key(&hex).is_err());
    }

    #[test]
    fn test_parse_hash_hex_valid() {
        let hex = "ab".repeat(32);
        let result = parse_hash_hex(&hex);
        assert!(result.is_some());
        assert_eq!(result.unwrap()[0], 0xab);
    }

    #[test]
    fn test_parse_hash_hex_wrong_length() {
        assert!(parse_hash_hex("abcd").is_none());
        assert!(parse_hash_hex(&"ab".repeat(33)).is_none());
    }

    #[test]
    fn test_parse_hash_hex_invalid_chars() {
        let mut hex = "ab".repeat(31);
        hex.push_str("zz");
        assert!(parse_hash_hex(&hex).is_none());
    }

    #[test]
    fn test_hex_encode_roundtrip() {
        let bytes = [0xab, 0xcd, 0xef, 0x01];
        assert_eq!(hex_encode(&bytes), "abcdef01");
    }

    #[test]
    fn test_is_trusted_proxy_public_ipv4_not_trusted() {
        let ip = std::net::IpAddr::V4(std::net::Ipv4Addr::new(8, 8, 8, 8));
        assert!(!is_trusted_proxy(ip));
    }

    #[test]
    fn test_is_trusted_proxy_private_172() {
        let ip = std::net::IpAddr::V4(std::net::Ipv4Addr::new(172, 16, 0, 1));
        assert!(is_trusted_proxy(ip));
    }

    #[test]
    fn test_is_trusted_proxy_flyio_fdaa() {
        let ip = std::net::IpAddr::V6(std::net::Ipv6Addr::new(0xfdaa, 0, 0, 0, 0, 0, 0, 1));
        assert!(is_trusted_proxy(ip));
    }
}
