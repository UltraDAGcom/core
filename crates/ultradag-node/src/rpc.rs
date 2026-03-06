use std::sync::Arc;

use http_body_util::{BodyExt, Full};
use hyper::body::{Bytes, Incoming};
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{Method, Request, Response, StatusCode};
use hyper_util::rt::TokioIo;
use serde::{Deserialize, Serialize};
use tokio::net::TcpListener;
use tracing::{error, info};

use ultradag_coin::{Address, SecretKey, Signature, Transaction};
use ultradag_network::{Message, NodeServer};

type BoxBody = Full<Bytes>;

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

#[derive(Serialize)]
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
struct SendTxRequest {
    from_secret: String,
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

    let path = req.uri().path().to_string();
    let segments: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();

    let response = match (req.method(), segments.as_slice()) {
        (&Method::GET, ["status"]) => {
            let state = server.state.read().await;
            let mp = server.mempool.read().await;
            let peers = server.peers.connected_count().await;
            let dag = server.dag.read().await;
            let fin = server.finality.read().await;
            json_response(
                StatusCode::OK,
                &StatusResponse {
                    last_finalized_round: state.last_finalized_round(),
                    peer_count: peers,
                    mempool_size: mp.len(),
                    total_supply: state.total_supply(),
                    account_count: state.account_count(),
                    dag_vertices: dag.len(),
                    dag_round: dag.current_round(),
                    dag_tips: dag.tips().len(),
                    finalized_count: fin.finalized_count(),
                    validator_count: fin.validator_count(),
                },
            )
        }

        (&Method::GET, ["balance", addr_hex]) => {
            let Some(addr) = Address::from_hex(addr_hex) else {
                return Ok(error_response(StatusCode::BAD_REQUEST, "invalid address hex (need 64 chars)"));
            };
            let state = server.state.read().await;
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
            let dag = server.dag.read().await;
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
            let body = req.collect().await?.to_bytes();
            let Ok(send_req) = serde_json::from_slice::<SendTxRequest>(&body) else {
                return Ok(error_response(StatusCode::BAD_REQUEST, "invalid JSON body, need: {from_secret, to, amount, fee}"));
            };

            // Parse secret key (64 hex chars = 32 bytes)
            if send_req.from_secret.len() != 64 {
                return Ok(error_response(StatusCode::BAD_REQUEST, "from_secret must be 64 hex chars (32 bytes)"));
            }
            let mut sk_bytes = [0u8; 32];
            for (i, chunk) in send_req.from_secret.as_bytes().chunks(2).enumerate() {
                let Ok(s) = std::str::from_utf8(chunk) else {
                    return Ok(error_response(StatusCode::BAD_REQUEST, "invalid hex in from_secret"));
                };
                let Ok(b) = u8::from_str_radix(s, 16) else {
                    return Ok(error_response(StatusCode::BAD_REQUEST, "invalid hex in from_secret"));
                };
                sk_bytes[i] = b;
            }
            let sk = SecretKey::from_bytes(sk_bytes);
            let sender = sk.address();

            // Parse recipient
            let Some(to) = Address::from_hex(&send_req.to) else {
                return Ok(error_response(StatusCode::BAD_REQUEST, "invalid to address hex"));
            };

            // Get nonce
            let nonce = {
                let state = server.state.read().await;
                state.nonce(&sender)
            };

            // Build and sign transaction
            let mut tx = Transaction {
                from: sender,
                to,
                amount: send_req.amount,
                fee: send_req.fee,
                nonce,
                pub_key: sk.verifying_key().to_bytes(),
                signature: Signature([0u8; 64]),
            };
            tx.signature = sk.sign(&tx.signable_bytes());

            // Validate against state before accepting
            {
                let state = server.state.read().await;
                let balance = state.balance(&sender);
                if balance < tx.total_cost() {
                    return Ok(error_response(
                        StatusCode::BAD_REQUEST,
                        &format!(
                            "insufficient balance: need {} sats, have {} sats ({:.4} UDAG)",
                            tx.total_cost(),
                            balance,
                            balance as f64 / 100_000_000.0,
                        ),
                    ));
                }
                let expected_nonce = state.nonce(&sender);
                if tx.nonce != expected_nonce {
                    return Ok(error_response(
                        StatusCode::BAD_REQUEST,
                        &format!("invalid nonce: expected {}, got {}", expected_nonce, tx.nonce),
                    ));
                }
            }

            let tx_hash = tx.hash();

            // Insert into mempool
            let inserted = {
                let mut mp = server.mempool.write().await;
                mp.insert(tx.clone())
            };

            if !inserted {
                return Ok(error_response(StatusCode::CONFLICT, "duplicate transaction"));
            }

            // Broadcast to peers
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
            let mp = server.mempool.read().await;
            let txs: Vec<serde_json::Value> = mp.best(100).iter().map(|tx| {
                serde_json::json!({
                    "hash": hex_encode(&tx.hash()),
                    "from": tx.from.to_hex(),
                    "to": tx.to.to_hex(),
                    "amount": tx.amount,
                    "fee": tx.fee,
                    "nonce": tx.nonce,
                })
            }).collect();
            json_response(StatusCode::OK, &txs)
        }

        (&Method::POST, ["faucet"]) => {
            let body = req.collect().await?.to_bytes();
            let Ok(faucet_req) = serde_json::from_slice::<FaucetRequest>(&body) else {
                return Ok(error_response(StatusCode::BAD_REQUEST, "invalid JSON body, need: {address, amount}"));
            };
            let Some(addr) = Address::from_hex(&faucet_req.address) else {
                return Ok(error_response(StatusCode::BAD_REQUEST, "invalid address hex"));
            };
            {
                let mut state = server.state.write().await;
                state.faucet_credit(&addr, faucet_req.amount);
            }
            json_response(
                StatusCode::OK,
                &serde_json::json!({
                    "address": faucet_req.address,
                    "credited": faucet_req.amount,
                    "credited_udag": faucet_req.amount as f64 / 100_000_000.0,
                }),
            )
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

        _ => error_response(StatusCode::NOT_FOUND, "not found"),
    };

    Ok(response)
}

pub async fn start_rpc(server: Arc<NodeServer>, rpc_port: u16) {
    let listener = match TcpListener::bind(format!("0.0.0.0:{}", rpc_port)).await {
        Ok(l) => l,
        Err(e) => {
            error!("RPC bind failed on port {}: {}", rpc_port, e);
            return;
        }
    };

    info!("RPC server listening on http://0.0.0.0:{}", rpc_port);

    loop {
        let (stream, _) = match listener.accept().await {
            Ok(s) => s,
            Err(e) => {
                error!("RPC accept error: {}", e);
                continue;
            }
        };

        let server_clone = server.clone();
        tokio::spawn(async move {
            let io = TokioIo::new(stream);
            let svc = service_fn(move |req| {
                let s = server_clone.clone();
                async move { handle_request(req, s).await }
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
