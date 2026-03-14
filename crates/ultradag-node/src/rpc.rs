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
const RPC_LOCK_TIMEOUT: Duration = Duration::from_secs(10);

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

use ultradag_coin::{Address, Mempool, SecretKey, Signature, StateEngine, Transaction, TransferTx, StakeTx, UnstakeTx, MIN_STAKE_SATS};
use ultradag_coin::governance::{CreateProposalTx, VoteTx, ProposalType};
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
fn pending_cost(mp: &Mempool, sender: &Address) -> u64 {
    mp.best(MAX_MEMPOOL_SCAN)
        .iter()
        .filter(|t| t.from() == *sender)
        .map(|t| t.total_cost())
        .fold(0u64, |acc, x| acc.saturating_add(x))
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
    bootstrap_connected: bool,
    // System resource metrics
    #[serde(skip_serializing_if = "Option::is_none")]
    memory_usage_bytes: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    cpu_usage_percent: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    uptime_seconds: Option<u64>,
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
    balance: u64,
    nonce: u64,
    balance_udag: f64,
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
}

#[derive(Serialize)]
struct ValidatorInfo {
    address: String,
    staked: u64,
    staked_udag: f64,
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
            drop(state);

            let mempool_size = server.mempool.try_read().map(|m| m.len()).unwrap_or(0);
            let peers = server.peers.connected_count().await;

            let dag = read_or_cache!(server.dag);
            let dag_vertices = dag.len();
            let dag_round = dag.current_round();
            let dag_tips_len = dag.tips().len();
            drop(dag);

            let fin = read_or_cache!(server.finality);
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
                bootstrap_connected,
                memory_usage_bytes,
                cpu_usage_percent,
                uptime_seconds,
            };

            *status_cache().lock().await = Some(status.clone());
            json_response(StatusCode::OK, &status)
        }

        (&Method::GET, ["balance", addr_hex]) => {
            let Some(addr) = Address::from_hex(addr_hex) else {
                return Ok(error_response(StatusCode::BAD_REQUEST, "invalid address hex (need 64 chars)"));
            };
            let state = read_lock_or_503!(server.state);
            let balance = state.balance(&addr);
            let nonce = state.nonce(&addr);
            json_response(
                StatusCode::OK,
                &BalanceResponse {
                    address: addr.to_hex(),
                    balance,
                    nonce,
                    balance_udag: balance as f64 / ultradag_coin::SATS_PER_UDAG as f64,
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
            let Some(to) = Address::from_hex(&send_req.to) else {
                return Ok(error_response(StatusCode::BAD_REQUEST, "invalid to address hex"));
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
                }
            }).collect();
            json_response(StatusCode::OK, &txs)
        }

        (&Method::POST, ["faucet"]) => {
            // Check endpoint-specific rate limit (strict for faucet)
            if !rate_limiter.check_rate_limit(client_ip, limits::FAUCET) {
                return Ok(error_response(
                    StatusCode::TOO_MANY_REQUESTS,
                    "rate limit exceeded: faucet limited to 1 request per 5 seconds",
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
            let Some(to) = Address::from_hex(&faucet_req.address) else {
                return Ok(error_response(StatusCode::BAD_REQUEST, "invalid address hex"));
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
            let Some(addr) = Address::from_hex(addr_hex) else {
                return Ok(error_response(StatusCode::BAD_REQUEST, "invalid address hex"));
            };
            let state = read_lock_or_503!(server.state);
            let staked = state.stake_of(&addr);
            let stake_acct = state.stake_account(&addr);
            let unlock_at = stake_acct.and_then(|s| s.unlock_at_round);
            let is_active = state.is_active_validator(&addr);
            json_response(StatusCode::OK, &StakeInfoResponse {
                address: addr.to_hex(),
                staked,
                staked_udag: staked as f64 / ultradag_coin::SATS_PER_UDAG as f64,
                unlock_at_round: unlock_at,
                is_active_validator: is_active,
            })
        }

        (&Method::GET, ["validators"]) => {
            let state = read_lock_or_503!(server.state);
            let stakers = state.active_stakers();
            let validators: Vec<ValidatorInfo> = stakers.iter().map(|addr| {
                let staked = state.stake_of(addr);
                ValidatorInfo {
                    address: addr.to_hex(),
                    staked,
                    staked_udag: staked as f64 / ultradag_coin::SATS_PER_UDAG as f64,
                }
            }).collect();
            json_response(StatusCode::OK, &serde_json::json!({
                "count": validators.len(),
                "total_staked": state.total_staked(),
                "validators": validators,
            }))
        }

        (&Method::GET, ["keygen"]) => {
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
                }),
            )
        }

        // ====== Governance POST endpoints ======

        (&Method::POST, ["proposal"]) => {
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
                _ => return Ok(error_response(StatusCode::BAD_REQUEST, "proposal_type must be 'text' or 'parameter'")),
            };

            let fee = prop_req.fee.unwrap_or(ultradag_coin::constants::MIN_FEE_SATS);

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

                // Check proposer has sufficient stake
                let proposer_stake = state.stake_of(&sender);
                let min_stake = state.governance_params().min_stake_to_propose;
                if proposer_stake < min_stake {
                    return Ok(error_response(StatusCode::BAD_REQUEST,
                        &format!("insufficient stake to propose: need {} UDAG staked, have {} UDAG staked",
                            min_stake / 100_000_000, proposer_stake / ultradag_coin::SATS_PER_UDAG)));
                }

                // Check active proposal count limit
                let active_count = state.proposals().values()
                    .filter(|p| matches!(p.status, ultradag_coin::governance::ProposalStatus::Active))
                    .count() as u64;
                if active_count >= state.governance_params().max_active_proposals {
                    return Ok(error_response(StatusCode::BAD_REQUEST,
                        "too many active proposals, please wait for existing proposals to complete"));
                }

                let proposal_id = state.next_proposal_id();

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
                    "governable_params": ultradag_coin::governance::GovernanceParams::param_names(),
                }),
            )
        }

        (&Method::GET, ["proposals"]) => {
            let state = read_lock_or_503!(server.state);
            let proposals: Vec<serde_json::Value> = state.proposals()
                .values()
                .map(|p| serde_json::json!({
                    "id": p.id,
                    "proposer": p.proposer.to_hex(),
                    "title": p.title,
                    "description": p.description,
                    "proposal_type": p.proposal_type,
                    "voting_starts": p.voting_starts,
                    "voting_ends": p.voting_ends,
                    "votes_for": p.votes_for,
                    "votes_for_udag": p.votes_for as f64 / ultradag_coin::SATS_PER_UDAG as f64,
                    "votes_against": p.votes_against,
                    "votes_against_udag": p.votes_against as f64 / ultradag_coin::SATS_PER_UDAG as f64,
                    "status": p.status,
                }))
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
                .map(|(addr, vote, weight)| serde_json::json!({
                    "address": addr.to_hex(),
                    "vote": if *vote { "yes" } else { "no" },
                    "weight": weight,
                    "weight_udag": *weight as f64 / ultradag_coin::SATS_PER_UDAG as f64,
                }))
                .collect();
            json_response(StatusCode::OK, &serde_json::json!({
                "id": p.id,
                "proposer": p.proposer.to_hex(),
                "title": p.title,
                "description": p.description,
                "proposal_type": p.proposal_type,
                "voting_starts": p.voting_starts,
                "voting_ends": p.voting_ends,
                "votes_for": p.votes_for,
                "votes_for_udag": p.votes_for as f64 / ultradag_coin::SATS_PER_UDAG as f64,
                "votes_against": p.votes_against,
                "votes_against_udag": p.votes_against as f64 / ultradag_coin::SATS_PER_UDAG as f64,
                "status": p.status,
                "voters": voters,
            }))
        }

        (&Method::GET, ["vote", id_str, addr_hex]) => {
            let Ok(id) = id_str.parse::<u64>() else {
                return Ok(error_response(StatusCode::BAD_REQUEST, "invalid proposal ID"));
            };
            let Some(addr) = Address::from_hex(addr_hex) else {
                return Ok(error_response(StatusCode::BAD_REQUEST, "invalid address hex"));
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
        // Accepts a JSON-serialized Transaction. Verifies signature, validates against
        // state, inserts in mempool, and broadcasts.
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
            let Ok(tx) = serde_json::from_slice::<Transaction>(&body) else {
                return Ok(error_response(StatusCode::BAD_REQUEST,
                    "invalid JSON: expected a serialized Transaction (Transfer, Stake, Unstake, CreateProposal, or Vote)"));
            };

            // Verify Ed25519 signature
            if !tx.verify_signature() {
                return Ok(error_response(StatusCode::BAD_REQUEST, "invalid signature"));
            }

            // Validate against current state
            {
                let state = read_lock_or_503!(server.state);
                let sender = tx.from();
                let balance = state.balance(&sender);
                let total_cost = tx.total_cost();
                if balance < total_cost {
                    return Ok(error_response(StatusCode::BAD_REQUEST,
                        &format!("insufficient balance: need {} sats, have {} sats", total_cost, balance)));
                }
                // Check nonce isn't stale (already confirmed)
                let expected_nonce = state.nonce(&sender);
                if tx.nonce() < expected_nonce {
                    return Ok(error_response(StatusCode::BAD_REQUEST,
                        &format!("nonce too low: expected >= {}, got {}", expected_nonce, tx.nonce())));
                }
                drop(state);
            }

            let tx_hash = tx.hash();

            // Insert into mempool
            {
                let mut mp = write_lock_or_503!(server.mempool);
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

        // ====== Metrics endpoints ======

        (&Method::GET, ["metrics"]) => {
            // Prometheus format
            let metrics = server.checkpoint_metrics.export_prometheus();
            Response::builder()
                .status(StatusCode::OK)
                .header("Content-Type", "text/plain; version=0.0.4")
                .header("Access-Control-Allow-Origin", "*")
                .body(Full::new(Bytes::from(metrics)))
                .unwrap_or_else(|e| {
                    tracing::error!("Failed to build metrics response: {}", e);
                    Response::new(Full::new(Bytes::from("{\"error\": \"metrics build failed\"}")))
                })
        }

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
