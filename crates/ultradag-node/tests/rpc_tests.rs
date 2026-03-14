//! Integration tests for RPC endpoints and helper functions.
//!
//! Tests cover:
//! - is_trusted_proxy() logic for X-Forwarded-For trust
//! - Actual HTTP RPC endpoints via a minimal node setup
//! - Rate limiting integration

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::sync::Arc;
use std::sync::atomic::{AtomicU16, Ordering};

// ─── is_trusted_proxy tests ─────────────────────────────────────────────────

#[test]
fn test_trusted_proxy_loopback_v4() {
    let ip = IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1));
    assert!(ultradag_node::is_trusted_proxy(ip));
}

#[test]
fn test_trusted_proxy_loopback_v6() {
    let ip = IpAddr::V6(Ipv6Addr::LOCALHOST);
    assert!(ultradag_node::is_trusted_proxy(ip));
}

#[test]
fn test_trusted_proxy_private_10() {
    let ip = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1));
    assert!(ultradag_node::is_trusted_proxy(ip));
}

#[test]
fn test_trusted_proxy_private_172() {
    let ip = IpAddr::V4(Ipv4Addr::new(172, 16, 0, 1));
    assert!(ultradag_node::is_trusted_proxy(ip));
    // 172.31.x.x is still private
    let ip2 = IpAddr::V4(Ipv4Addr::new(172, 31, 255, 255));
    assert!(ultradag_node::is_trusted_proxy(ip2));
    // 172.32.x.x is NOT private
    let ip3 = IpAddr::V4(Ipv4Addr::new(172, 32, 0, 1));
    assert!(!ultradag_node::is_trusted_proxy(ip3));
}

#[test]
fn test_trusted_proxy_private_192() {
    let ip = IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1));
    assert!(ultradag_node::is_trusted_proxy(ip));
}

#[test]
fn test_trusted_proxy_fly_internal_fdaa() {
    // Fly.io internal network: fdaa::/16
    let ip = IpAddr::V6(Ipv6Addr::new(0xfdaa, 0, 0, 0, 0, 0, 0, 1));
    assert!(ultradag_node::is_trusted_proxy(ip));
}

#[test]
fn test_trusted_proxy_unique_local_fc00() {
    // General unique-local: fc00::/7
    let ip = IpAddr::V6(Ipv6Addr::new(0xfc00, 0, 0, 0, 0, 0, 0, 1));
    assert!(ultradag_node::is_trusted_proxy(ip));
    let ip2 = IpAddr::V6(Ipv6Addr::new(0xfd00, 0, 0, 0, 0, 0, 0, 1));
    assert!(ultradag_node::is_trusted_proxy(ip2));
}

#[test]
fn test_untrusted_public_ipv4() {
    let ip = IpAddr::V4(Ipv4Addr::new(8, 8, 8, 8));
    assert!(!ultradag_node::is_trusted_proxy(ip));
    let ip2 = IpAddr::V4(Ipv4Addr::new(1, 2, 3, 4));
    assert!(!ultradag_node::is_trusted_proxy(ip2));
}

#[test]
fn test_untrusted_public_ipv6() {
    let ip = IpAddr::V6(Ipv6Addr::new(0x2001, 0xdb8, 0, 0, 0, 0, 0, 1));
    assert!(!ultradag_node::is_trusted_proxy(ip));
}

#[test]
fn test_trusted_proxy_ipv4_mapped_loopback() {
    // ::ffff:127.0.0.1
    let ip = IpAddr::V6(Ipv6Addr::new(0, 0, 0, 0, 0, 0xffff, 0x7f00, 0x0001));
    assert!(ultradag_node::is_trusted_proxy(ip));
}

#[test]
fn test_trusted_proxy_ipv4_mapped_private() {
    // ::ffff:10.0.0.1
    let ip = IpAddr::V6(Ipv6Addr::new(0, 0, 0, 0, 0, 0xffff, 0x0a00, 0x0001));
    assert!(ultradag_node::is_trusted_proxy(ip));
}

#[test]
fn test_untrusted_ipv4_mapped_public() {
    // ::ffff:8.8.8.8
    let ip = IpAddr::V6(Ipv6Addr::new(0, 0, 0, 0, 0, 0xffff, 0x0808, 0x0808));
    assert!(!ultradag_node::is_trusted_proxy(ip));
}

// ─── RPC server integration tests ───────────────────────────────────────────

use ultradag_network::NodeServer;

/// Atomic counter to assign unique ports to each test.
static PORT_COUNTER: AtomicU16 = AtomicU16::new(40_100);

/// Start a minimal RPC server on a unique port and return the port.
async fn start_test_rpc() -> u16 {
    let port = PORT_COUNTER.fetch_add(1, Ordering::SeqCst);
    let server = Arc::new(NodeServer::new(port + 10000));

    let server_clone = server.clone();
    tokio::spawn(async move {
        ultradag_node::rpc::start_rpc(server_clone, port).await;
    });

    // Give the server a moment to bind and start accepting
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    port
}

/// Create a reqwest client that does not use system proxy settings.
fn http_client() -> reqwest::Client {
    reqwest::Client::builder()
        .no_proxy()
        .build()
        .unwrap()
}

#[tokio::test]
async fn test_rpc_health_returns_200() {
    let port = start_test_rpc().await;
    let client = http_client();
    let resp = client.get(format!("http://127.0.0.1:{}/health", port))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["status"], "ok");
}

#[tokio::test]
async fn test_rpc_status_returns_valid_json() {
    let port = start_test_rpc().await;
    let client = http_client();
    let resp = client.get(format!("http://127.0.0.1:{}/status", port))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    // Status should contain standard fields
    assert!(body.get("dag_round").is_some(), "missing dag_round");
    assert!(body.get("total_supply").is_some(), "missing total_supply");
    assert!(body.get("peer_count").is_some(), "missing peer_count");
    assert!(body.get("mempool_size").is_some(), "missing mempool_size");
}

#[tokio::test]
async fn test_rpc_keygen_returns_valid_keypair() {
    let port = start_test_rpc().await;
    let client = http_client();
    let resp = client.get(format!("http://127.0.0.1:{}/keygen", port))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    let secret = body["secret_key"].as_str().unwrap();
    let address = body["address"].as_str().unwrap();
    // Secret key should be 64 hex chars (32 bytes)
    assert_eq!(secret.len(), 64, "secret key should be 64 hex chars");
    assert!(secret.chars().all(|c| c.is_ascii_hexdigit()), "secret key should be hex");
    // Address should be 64 hex chars (32 bytes)
    assert_eq!(address.len(), 64, "address should be 64 hex chars");
    assert!(address.chars().all(|c| c.is_ascii_hexdigit()), "address should be hex");
}

#[tokio::test]
async fn test_rpc_balance_invalid_address_returns_error() {
    let port = start_test_rpc().await;
    let client = http_client();
    // Too short address
    let resp = client.get(format!("http://127.0.0.1:{}/balance/notahexaddr", port))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 400);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert!(body.get("error").is_some(), "should return error field");
}

#[tokio::test]
async fn test_rpc_balance_valid_unknown_address() {
    let port = start_test_rpc().await;
    let client = http_client();
    // Valid 64-char hex address that doesn't exist
    let addr = "0000000000000000000000000000000000000000000000000000000000000001";
    let resp = client.get(format!("http://127.0.0.1:{}/balance/{}", port, addr))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["balance"], 0);
}

#[tokio::test]
async fn test_rpc_tx_invalid_body_returns_error() {
    let port = start_test_rpc().await;
    let client = http_client();
    let resp = client
        .post(format!("http://127.0.0.1:{}/tx", port))
        .header("Content-Type", "application/json")
        .body("{\"invalid\": true}")
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 400);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert!(body.get("error").is_some(), "should return error on invalid tx body");
}

#[tokio::test]
async fn test_rpc_tx_empty_body_returns_error() {
    let port = start_test_rpc().await;
    let client = http_client();
    let resp = client
        .post(format!("http://127.0.0.1:{}/tx", port))
        .header("Content-Type", "application/json")
        .body("")
        .send()
        .await
        .unwrap();
    // Should return 400 for empty/invalid body
    assert!(resp.status() == 400 || resp.status() == 413);
}

#[tokio::test]
async fn test_rpc_unknown_endpoint_returns_404() {
    let port = start_test_rpc().await;
    let client = http_client();
    let resp = client.get(format!("http://127.0.0.1:{}/nonexistent", port))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 404);
}

#[tokio::test]
async fn test_rpc_rate_limiting() {
    let port = start_test_rpc().await;
    let client = http_client();

    // The global rate limit is 100/min. Send 105 requests rapidly.
    let mut success_count = 0u32;
    let mut rate_limited_count = 0u32;

    for _ in 0..105 {
        let resp = client
            .get(format!("http://127.0.0.1:{}/health", port))
            .send()
            .await
            .unwrap();
        if resp.status() == 200 {
            success_count += 1;
        } else if resp.status() == 429 {
            rate_limited_count += 1;
        }
    }

    // Should have allowed exactly 100 and rate-limited some
    assert!(success_count <= 100, "should not allow more than 100 requests in window");
    assert!(rate_limited_count >= 5, "should rate-limit excess requests, got {} limited", rate_limited_count);
}

#[tokio::test]
async fn test_rpc_cors_headers() {
    let port = start_test_rpc().await;
    let client = http_client();
    let resp = client.get(format!("http://127.0.0.1:{}/health", port))
        .send()
        .await
        .unwrap();
    let headers = resp.headers();
    assert_eq!(
        headers.get("access-control-allow-origin").unwrap().to_str().unwrap(),
        "*"
    );
}

#[tokio::test]
async fn test_rpc_validators_endpoint() {
    let port = start_test_rpc().await;
    let client = http_client();
    let resp = client.get(format!("http://127.0.0.1:{}/validators", port))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    // Should return a validators array (may be empty on fresh node)
    assert!(body.get("validators").is_some(), "should have validators field");
}

#[tokio::test]
async fn test_rpc_mempool_endpoint() {
    let port = start_test_rpc().await;
    let client = http_client();
    let resp = client.get(format!("http://127.0.0.1:{}/mempool", port))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    // Mempool returns a JSON array directly (empty on fresh node)
    assert!(body.is_array(), "mempool should return an array");
    assert_eq!(body.as_array().unwrap().len(), 0, "fresh node mempool should be empty");
}

#[tokio::test]
async fn test_rpc_peers_endpoint() {
    let port = start_test_rpc().await;
    let client = http_client();
    let resp = client.get(format!("http://127.0.0.1:{}/peers", port))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert!(body.get("peers").is_some(), "should have peers field");
}
