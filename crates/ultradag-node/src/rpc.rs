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

use ultradag_coin::{Address, SecretKey, Signature, Transaction, TransferTx, StakeTx, UnstakeTx, MIN_STAKE_SATS};
use ultradag_coin::governance::{CreateProposalTx, VoteTx, ProposalType};
use ultradag_network::{Message, NodeServer};
use crate::rate_limit::{RateLimiter, limits};

type BoxBody = Full<Bytes>;

/// Max transactions to scan in mempool for pending cost calculation
const MAX_MEMPOOL_SCAN: usize = 10_000;

/// Max request body size (1MB)
const MAX_REQUEST_SIZE: usize = 1_048_576;

fn json_response(status: StatusCode, body: &impl Serialize) -> Response<BoxBody> {
    let json = serde_json::to_string_pretty(body).unwrap();
    Response::builder()
        .status(status)
        .header("Content-Type", "application/json")
        .header("Access-Control-Allow-Origin", "*")
        .header("Access-Control-Allow-Methods", "GET, POST, OPTIONS")
        .header("Access-Control-Allow-Headers", "Content-Type")
        .body(Full::new(Bytes::from(json)))
        .unwrap()
}

fn error_response(status: StatusCode, msg: &str) -> Response<BoxBody> {
    json_response(status, &serde_json::json!({"error": msg}))
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
}

/// Cached /status response — serves last good data when locks are contended.
static STATUS_CACHE: std::sync::OnceLock<tokio::sync::Mutex<Option<StatusResponse>>> = std::sync::OnceLock::new();
fn status_cache() -> &'static tokio::sync::Mutex<Option<StatusResponse>> {
    STATUS_CACHE.get_or_init(|| tokio::sync::Mutex::new(None))
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
    balance_tdag: f64,
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
    proposer_secret: String,
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
    voter_secret: String,
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
            .status(200)
            .header("Access-Control-Allow-Origin", "*")
            .header("Access-Control-Allow-Methods", "GET, POST, OPTIONS")
            .header("Access-Control-Allow-Headers", "Content-Type")
            .body(Full::new(Bytes::new()))
            .unwrap());
    }

    let method = req.method();
    let path: Vec<&str> = req.uri().path().trim_matches('/').split('/').collect();

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

        (&Method::GET, ["status"]) => {
            // Use try_read() for non-blocking best-effort reads.
            // Falls back to cached response when locks are contended
            // (validator loop holds write locks for ~100ms every 5s).
            let state_data = server.state.try_read().ok().map(|s|
                (s.last_finalized_round(), s.total_supply(), s.account_count(), s.total_staked(), s.active_stakers().len())
            );
            let mempool_size = server.mempool.try_read().ok().map(|m| m.len());
            let dag_data = server.dag.try_read().ok().map(|d|
                (d.len(), d.current_round(), d.tips().len())
            );
            let fin_data = server.finality.try_read().ok().map(|f|
                (f.finalized_count(), f.validator_count())
            );

            // If any lock failed, serve cached response
            if state_data.is_none() || dag_data.is_none() || fin_data.is_none() {
                let cache = status_cache().lock().await;
                if let Some(cached) = cache.as_ref() {
                    return Ok(json_response(StatusCode::OK, cached));
                }
                // No cache yet — return 503
                return Ok(error_response(StatusCode::SERVICE_UNAVAILABLE, "node busy, try again"));
            }

            let (last_finalized_round, total_supply, account_count, total_staked, active_stakers_len) = state_data.unwrap();
            let (dag_vertices, dag_round, dag_tips_len) = dag_data.unwrap();
            let (finalized_count, validator_count) = fin_data.unwrap();
            let peers = server.peers.connected_count().await;
            let connected_addrs = server.peers.connected_listen_addrs().await;
            let bootstrap_connected = ultradag_network::TESTNET_BOOTSTRAP_NODES
                .iter()
                .any(|bn| connected_addrs.iter().any(|ca| ca == *bn));

            let status = StatusResponse {
                last_finalized_round,
                peer_count: peers,
                mempool_size: mempool_size.unwrap_or(0),
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
            };

            // Update cache
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
                    balance_tdag: balance as f64 / 100_000_000.0,
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
                return Ok(error_response(StatusCode::BAD_REQUEST, "invalid JSON body, need: {secret_key, to, amount, fee}"));
            };

            // Parse secret key (64 hex chars = 32 bytes)
            if send_req.secret_key.len() != 64 {
                return Ok(error_response(StatusCode::BAD_REQUEST, "secret_key must be 64 hex chars (32 bytes)"));
            }
            // Reject null bytes and other invalid characters
            if send_req.secret_key.contains('\0') || !send_req.secret_key.chars().all(|c| c.is_ascii_hexdigit()) {
                return Ok(error_response(StatusCode::BAD_REQUEST, "invalid hex in secret_key"));
            }
            let mut sk_bytes = [0u8; 32];
            for (i, chunk) in send_req.secret_key.as_bytes().chunks(2).enumerate() {
                let Ok(s) = std::str::from_utf8(chunk) else {
                    return Ok(error_response(StatusCode::BAD_REQUEST, "invalid hex in secret_key"));
                };
                let Ok(b) = u8::from_str_radix(s, 16) else {
                    return Ok(error_response(StatusCode::BAD_REQUEST, "invalid hex in secret_key"));
                };
                sk_bytes[i] = b;
            }
            let sk = SecretKey::from_bytes(sk_bytes);
            let sender = sk.address();

            // Parse recipient
            let Some(to) = Address::from_hex(&send_req.to) else {
                return Ok(error_response(StatusCode::BAD_REQUEST, "invalid to address hex"));
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
                let base_nonce = state.nonce(&sender);
                let nonce = match mp.pending_nonce(&sender) {
                    Some(max_pending) => max_pending + 1,
                    None => base_nonce,
                };

                // Validate balance including pending cost
                let balance = state.balance(&sender);
                let pending_cost: u64 = mp.best(MAX_MEMPOOL_SCAN)
                    .iter()
                    .filter(|t| t.from() == sender)
                    .map(|t| t.total_cost())
                    .sum();
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
                            balance as f64 / 100_000_000.0,
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
            let Some(to) = Address::from_hex(&faucet_req.address) else {
                return Ok(error_response(StatusCode::BAD_REQUEST, "invalid address hex"));
            };

            // Use the deterministic faucet keypair (same on every node)
            let faucet_sk = ultradag_coin::faucet_keypair();
            let faucet_addr = faucet_sk.address();
            let fee = ultradag_coin::constants::MIN_FEE_SATS; // must meet minimum fee

            let (tx, tx_hash, nonce) = {
                let state = read_lock_or_503!(server.state);
                let mut mp = write_lock_or_503!(server.mempool);

                let base_nonce = state.nonce(&faucet_addr);
                let nonce = match mp.pending_nonce(&faucet_addr) {
                    Some(max_pending) => max_pending + 1,
                    None => base_nonce,
                };

                let balance = state.balance(&faucet_addr);
                let pending_cost: u64 = mp.best(MAX_MEMPOOL_SCAN)
                    .iter()
                    .filter(|t| t.from() == faucet_addr)
                    .map(|t| t.total_cost())
                    .sum();
                let total_needed = pending_cost.saturating_add(faucet_req.amount);
                if balance < total_needed {
                    return Ok(error_response(
                        StatusCode::BAD_REQUEST,
                        &format!(
                            "faucet insufficient balance: need {} sats, have {} sats ({:.4} UDAG)",
                            total_needed, balance, balance as f64 / 100_000_000.0,
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
                    "amount_udag": faucet_req.amount as f64 / 100_000_000.0,
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

            if stake_req.secret_key.len() != 64 {
                return Ok(error_response(StatusCode::BAD_REQUEST, "secret_key must be 64 hex chars"));
            }
            // Reject null bytes and other invalid characters
            if stake_req.secret_key.contains('\0') || !stake_req.secret_key.chars().all(|c| c.is_ascii_hexdigit()) {
                return Ok(error_response(StatusCode::BAD_REQUEST, "invalid hex in secret_key"));
            }
            let mut sk_bytes = [0u8; 32];
            for (i, chunk) in stake_req.secret_key.as_bytes().chunks(2).enumerate() {
                let Ok(s) = std::str::from_utf8(chunk) else {
                    return Ok(error_response(StatusCode::BAD_REQUEST, "invalid hex"));
                };
                let Ok(b) = u8::from_str_radix(s, 16) else {
                    return Ok(error_response(StatusCode::BAD_REQUEST, "invalid hex"));
                };
                sk_bytes[i] = b;
            }
            let sk = SecretKey::from_bytes(sk_bytes);
            let sender = sk.address();

            // Build stake transaction and add to mempool (will be included in next vertex)
            let (tx, tx_hash, nonce) = {
                let state = read_lock_or_503!(server.state);
                let mut mp = write_lock_or_503!(server.mempool);

                let base_nonce = state.nonce(&sender);
                let nonce = match mp.pending_nonce(&sender) {
                    Some(max_pending) => max_pending + 1,
                    None => base_nonce,
                };

                let balance = state.balance(&sender);
                let pending_cost: u64 = mp.best(MAX_MEMPOOL_SCAN)
                    .iter()
                    .filter(|t| t.from() == sender)
                    .map(|t| t.total_cost())
                    .sum();
                let total_needed = pending_cost.saturating_add(stake_req.amount);

                if stake_req.amount < MIN_STAKE_SATS {
                    return Ok(error_response(StatusCode::BAD_REQUEST,
                        &format!("minimum stake is {} sats ({} UDAG)", MIN_STAKE_SATS, MIN_STAKE_SATS / 100_000_000)));
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
                "amount_udag": stake_req.amount as f64 / 100_000_000.0,
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

            if unstake_req.secret_key.len() != 64 {
                return Ok(error_response(StatusCode::BAD_REQUEST, "secret_key must be 64 hex chars"));
            }
            // Reject null bytes and other invalid characters
            if unstake_req.secret_key.contains('\0') || !unstake_req.secret_key.chars().all(|c| c.is_ascii_hexdigit()) {
                return Ok(error_response(StatusCode::BAD_REQUEST, "invalid hex in secret_key"));
            }
            let mut sk_bytes = [0u8; 32];
            for (i, chunk) in unstake_req.secret_key.as_bytes().chunks(2).enumerate() {
                let Ok(s) = std::str::from_utf8(chunk) else {
                    return Ok(error_response(StatusCode::BAD_REQUEST, "invalid hex"));
                };
                let Ok(b) = u8::from_str_radix(s, 16) else {
                    return Ok(error_response(StatusCode::BAD_REQUEST, "invalid hex"));
                };
                sk_bytes[i] = b;
            }
            let sk = SecretKey::from_bytes(sk_bytes);
            let sender = sk.address();

            let current_round = {
                let dag = read_lock_or_503!(server.dag);
                dag.current_round()
            };

            // Build unstake transaction and add to mempool
            let (tx, tx_hash, nonce) = {
                let state = read_lock_or_503!(server.state);
                let mut mp = write_lock_or_503!(server.mempool);

                let base_nonce = state.nonce(&sender);
                let nonce = match mp.pending_nonce(&sender) {
                    Some(max_pending) => max_pending + 1,
                    None => base_nonce,
                };

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
            let is_active = staked >= MIN_STAKE_SATS && unlock_at.is_none();
            json_response(StatusCode::OK, &StakeInfoResponse {
                address: addr.to_hex(),
                staked,
                staked_udag: staked as f64 / 100_000_000.0,
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
                    staked_udag: staked as f64 / 100_000_000.0,
                }
            }).collect();
            json_response(StatusCode::OK, &serde_json::json!({
                "count": validators.len(),
                "total_staked": state.total_staked(),
                "validators": validators,
            }))
        }

        (&Method::GET, ["keygen"]) => {
            let sk = SecretKey::generate();
            let addr = sk.address();
            json_response(
                StatusCode::OK,
                &serde_json::json!({
                    "secret_key": hex_encode(&sk.to_bytes()),
                    "address": addr.to_hex(),
                }),
            )
        }

        // ====== Governance POST endpoints ======

        (&Method::POST, ["proposal"]) => {
            let body = req.collect().await?.to_bytes();
            if body.len() > MAX_REQUEST_SIZE {
                return Ok(error_response(StatusCode::PAYLOAD_TOO_LARGE, "request body too large"));
            }
            let Ok(prop_req) = serde_json::from_slice::<ProposalRequest>(&body) else {
                return Ok(error_response(StatusCode::BAD_REQUEST,
                    "invalid JSON: need {proposer_secret, title, description, proposal_type}"));
            };

            let sk = match parse_secret_key(&prop_req.proposer_secret) {
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

                let base_nonce = state.nonce(&sender);
                let nonce = match mp.pending_nonce(&sender) {
                    Some(max_pending) => max_pending + 1,
                    None => base_nonce,
                };

                let balance = state.balance(&sender);
                if balance < fee {
                    return Ok(error_response(StatusCode::BAD_REQUEST,
                        &format!("insufficient balance for fee: need {}, have {}", fee, balance)));
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
            let body = req.collect().await?.to_bytes();
            if body.len() > MAX_REQUEST_SIZE {
                return Ok(error_response(StatusCode::PAYLOAD_TOO_LARGE, "request body too large"));
            }
            let Ok(vote_req) = serde_json::from_slice::<VoteRequest>(&body) else {
                return Ok(error_response(StatusCode::BAD_REQUEST,
                    "invalid JSON: need {voter_secret, proposal_id, vote}"));
            };

            let sk = match parse_secret_key(&vote_req.voter_secret) {
                Ok(sk) => sk,
                Err(e) => return Ok(error_response(StatusCode::BAD_REQUEST, e)),
            };
            let sender = sk.address();
            let fee = vote_req.fee.unwrap_or(ultradag_coin::constants::MIN_FEE_SATS);

            let (tx, tx_hash) = {
                let state = read_lock_or_503!(server.state);
                let mut mp = write_lock_or_503!(server.mempool);

                let base_nonce = state.nonce(&sender);
                let nonce = match mp.pending_nonce(&sender) {
                    Some(max_pending) => max_pending + 1,
                    None => base_nonce,
                };

                let balance = state.balance(&sender);
                if balance < fee {
                    return Ok(error_response(StatusCode::BAD_REQUEST,
                        &format!("insufficient balance for fee: need {}, have {}", fee, balance)));
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
            json_response(
                StatusCode::OK,
                &serde_json::json!({
                    "min_stake_to_propose": ultradag_coin::constants::MIN_STAKE_TO_PROPOSE,
                    "min_stake_to_propose_udag": ultradag_coin::constants::MIN_STAKE_TO_PROPOSE as f64 / 100_000_000.0,
                    "voting_period_rounds": ultradag_coin::constants::GOVERNANCE_VOTING_PERIOD_ROUNDS,
                    "quorum_percent": (ultradag_coin::constants::GOVERNANCE_QUORUM_NUMERATOR as f64 / ultradag_coin::constants::GOVERNANCE_QUORUM_DENOMINATOR as f64) * 100.0,
                    "approval_percent": (ultradag_coin::constants::GOVERNANCE_APPROVAL_NUMERATOR as f64 / ultradag_coin::constants::GOVERNANCE_APPROVAL_DENOMINATOR as f64) * 100.0,
                    "execution_delay_rounds": ultradag_coin::constants::GOVERNANCE_EXECUTION_DELAY_ROUNDS,
                    "max_active_proposals": ultradag_coin::constants::MAX_ACTIVE_PROPOSALS,
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
                    "votes_for_udag": p.votes_for as f64 / 100_000_000.0,
                    "votes_against": p.votes_against,
                    "votes_against_udag": p.votes_against as f64 / 100_000_000.0,
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
            json_response(StatusCode::OK, &serde_json::json!({
                "id": p.id,
                "proposer": p.proposer.to_hex(),
                "title": p.title,
                "description": p.description,
                "proposal_type": p.proposal_type,
                "voting_starts": p.voting_starts,
                "voting_ends": p.voting_ends,
                "votes_for": p.votes_for,
                "votes_for_udag": p.votes_for as f64 / 100_000_000.0,
                "votes_against": p.votes_against,
                "votes_against_udag": p.votes_against as f64 / 100_000_000.0,
                "status": p.status,
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
                "proposal_id": id,
                "voter": addr.to_hex(),
                "vote": vote,
            }))
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
