//! RPC input fuzzing tests.
//!
//! Sends random and malformed JSON to every UltraDAG RPC endpoint and verifies
//! the node handles it gracefully: returns proper HTTP error codes, never panics,
//! and never returns 500 Internal Server Error.

use std::time::Duration;
use ultradag_sim::p2p::cluster::TestCluster;
use ultradag_sim::p2p::helpers;

// ---------------------------------------------------------------------------
// Raw HTTP helpers (no reqwest dependency)
// ---------------------------------------------------------------------------

async fn http_post(port: u16, path: &str, body: &str) -> (u16, String) {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let addr = format!("127.0.0.1:{}", port);
    let mut stream = tokio::net::TcpStream::connect(&addr).await.unwrap();
    let request = format!(
        "POST {} HTTP/1.1\r\nHost: 127.0.0.1:{}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        path, port, body.len(), body
    );
    stream.write_all(request.as_bytes()).await.unwrap();
    let mut response = Vec::new();
    tokio::time::timeout(Duration::from_secs(5), stream.read_to_end(&mut response))
        .await
        .ok();
    let response_str = String::from_utf8_lossy(&response).to_string();
    let status = response_str
        .lines()
        .next()
        .and_then(|l| l.split_whitespace().nth(1))
        .and_then(|s| s.parse::<u16>().ok())
        .unwrap_or(0);
    (status, response_str)
}

async fn http_get(port: u16, path: &str) -> (u16, String) {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let addr = format!("127.0.0.1:{}", port);
    let mut stream = tokio::net::TcpStream::connect(&addr).await.unwrap();
    let request = format!(
        "GET {} HTTP/1.1\r\nHost: 127.0.0.1:{}\r\nConnection: close\r\n\r\n",
        path, port
    );
    stream.write_all(request.as_bytes()).await.unwrap();
    let mut response = Vec::new();
    tokio::time::timeout(Duration::from_secs(5), stream.read_to_end(&mut response))
        .await
        .ok();
    let response_str = String::from_utf8_lossy(&response).to_string();
    let status = response_str
        .lines()
        .next()
        .and_then(|l| l.split_whitespace().nth(1))
        .and_then(|s| s.parse::<u16>().ok())
        .unwrap_or(0);
    (status, response_str)
}

async fn http_options(port: u16, path: &str) -> (u16, String) {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let addr = format!("127.0.0.1:{}", port);
    let mut stream = tokio::net::TcpStream::connect(&addr).await.unwrap();
    let request = format!(
        "OPTIONS {} HTTP/1.1\r\nHost: 127.0.0.1:{}\r\nOrigin: https://example.com\r\nAccess-Control-Request-Method: GET\r\nConnection: close\r\n\r\n",
        path, port
    );
    stream.write_all(request.as_bytes()).await.unwrap();
    let mut response = Vec::new();
    tokio::time::timeout(Duration::from_secs(5), stream.read_to_end(&mut response))
        .await
        .ok();
    let response_str = String::from_utf8_lossy(&response).to_string();
    let status = response_str
        .lines()
        .next()
        .and_then(|l| l.split_whitespace().nth(1))
        .and_then(|s| s.parse::<u16>().ok())
        .unwrap_or(0);
    (status, response_str)
}

/// Assert the status code is NOT 500 (Internal Server Error).
/// Any 4xx client error, 2xx success, or other code is acceptable —
/// the key invariant is the node must never crash or return 500.
fn assert_not_500(endpoint: &str, body: &str, status: u16) {
    assert_ne!(
        status, 500,
        "Endpoint {} returned 500 Internal Server Error for input: {}",
        endpoint, body
    );
    // Also verify we got a response at all (status != 0 means we parsed an HTTP response)
    assert_ne!(
        status, 0,
        "Endpoint {} returned no parseable HTTP response for input: {}",
        endpoint, body
    );
}

/// Check that the node is still healthy after a fuzz request.
async fn assert_node_healthy(port: u16, context: &str) {
    let (status, _) = http_get(port, "/health").await;
    assert!(
        status == 200 || status == 204,
        "Node on port {} not healthy after {}: status={}",
        port,
        context,
        status
    );
}

// ---------------------------------------------------------------------------
// Test
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread")]
async fn rpc_fuzz_malformed_inputs() {
    let base = helpers::allocate_ports();
    let cluster = TestCluster::new(2, base);

    eprintln!(
        "[rpc_fuzz] Cluster base port: {}, RPC ports: {}-{}",
        base,
        base + 1000,
        base + 1001
    );

    assert!(
        cluster.wait_for_ready(30).await,
        "Nodes should be reachable within 30s"
    );

    // Give nodes a moment to initialize RPC routes
    tokio::time::sleep(Duration::from_secs(3)).await;

    // Use the first node for all fuzz requests
    let rpc = cluster.nodes[0].rpc_port;

    // -----------------------------------------------------------------
    // POST /tx — malformed inputs
    // -----------------------------------------------------------------
    let mut tx_cases: Vec<(&str, &str)> = vec![
        ("empty body", ""),
        ("not json", "this is not json at all"),
        ("empty object", "{}"),
        ("null", "null"),
        ("array", "[]"),
        ("wrong fields", r#"{"foo":"bar","baz":123}"#),
        (
            "missing required fields",
            r#"{"from_secret":"abc","to":"def"}"#,
        ),
        (
            "amount as string",
            r#"{"from_secret":"aa","to":"bb","amount":"not_a_number","fee":10000}"#,
        ),
        (
            "negative amount",
            r#"{"from_secret":"aa","to":"bb","amount":-100,"fee":10000}"#,
        ),
        (
            "amount zero",
            r#"{"from_secret":"aa","to":"bb","amount":0,"fee":10000}"#,
        ),
        (
            "huge amount",
            r#"{"from_secret":"aa","to":"bb","amount":99999999999999999,"fee":10000}"#,
        ),
        ("number", "42"),
        ("string", "\"hello\""),
        ("deeply nested", r#"{"a":{"b":{"c":{"d":{"e":"f"}}}}}"#),
        (
            "unicode",
            r#"{"from_secret":"\u0000\u0001","to":"\uFFFF","amount":1,"fee":1}"#,
        ),
    ];
    // Dynamic string must outlive the vec
    let very_long = format!(r#"{{"from_secret":"{}"}}"#, "a".repeat(100_000));
    tx_cases.push(("very long string", &very_long));

    for (label, body) in &tx_cases {
        let (status, _) = http_post(rpc, "/tx", body).await;
        assert_not_500("/tx", label, status);
    }
    assert_node_healthy(rpc, "POST /tx fuzz").await;

    // -----------------------------------------------------------------
    // POST /faucet — malformed inputs
    // -----------------------------------------------------------------
    let mut faucet_cases: Vec<(&str, &str)> = vec![
        ("empty body", ""),
        ("not json", "not json"),
        ("empty object", "{}"),
        ("amount zero", r#"{"address":"aa","amount":0}"#),
        (
            "amount negative",
            r#"{"address":"aa","amount":-1}"#,
        ),
        (
            "huge amount",
            r#"{"address":"aa","amount":99999999999999}"#,
        ),
        ("missing address", r#"{"amount":100000000}"#),
        (
            "invalid address not hex",
            r#"{"address":"not_hex_at_all!@#$","amount":100000000}"#,
        ),
        (
            "address too short",
            r#"{"address":"abcd","amount":100000000}"#,
        ),
    ];
    let long_addr_body = format!(
        r#"{{"address":"{}","amount":100000000}}"#,
        "ab".repeat(100)
    );
    faucet_cases.push(("address too long", &long_addr_body));
    faucet_cases.extend_from_slice(&[
        ("null address", r#"{"address":null,"amount":100000000}"#),
        (
            "amount as string",
            r#"{"address":"aa","amount":"lots"}"#,
        ),
        ("amount float", r#"{"address":"aa","amount":1.5}"#),
    ]);

    for (label, body) in &faucet_cases {
        let (status, _) = http_post(rpc, "/faucet", body).await;
        assert_not_500("/faucet", label, status);
    }
    assert_node_healthy(rpc, "POST /faucet fuzz").await;

    // -----------------------------------------------------------------
    // POST /stake — malformed inputs
    // -----------------------------------------------------------------
    let stake_cases: Vec<(&str, &str)> = vec![
        ("empty body", ""),
        ("not json", "garbage"),
        ("empty object", "{}"),
        ("missing secret_key", r#"{"amount":1000000000000}"#),
        (
            "invalid secret_key short",
            r#"{"secret_key":"abc","amount":1000000000000}"#,
        ),
        (
            "invalid secret_key wrong length",
            r#"{"secret_key":"deadbeef","amount":1000000000000}"#,
        ),
        (
            "amount zero",
            r#"{"secret_key":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","amount":0}"#,
        ),
        (
            "amount negative",
            r#"{"secret_key":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","amount":-5}"#,
        ),
        (
            "not hex secret_key",
            r#"{"secret_key":"zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz","amount":1000000000000}"#,
        ),
    ];

    for (label, body) in &stake_cases {
        let (status, _) = http_post(rpc, "/stake", body).await;
        assert_not_500("/stake", label, status);
    }
    assert_node_healthy(rpc, "POST /stake fuzz").await;

    // -----------------------------------------------------------------
    // POST /unstake — malformed inputs
    // -----------------------------------------------------------------
    let unstake_cases: Vec<(&str, &str)> = vec![
        ("empty body", ""),
        ("not json", "xxx"),
        ("empty object", "{}"),
        (
            "invalid secret_key",
            r#"{"secret_key":"tooshort"}"#,
        ),
        (
            "not hex",
            r#"{"secret_key":"zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz"}"#,
        ),
    ];

    for (label, body) in &unstake_cases {
        let (status, _) = http_post(rpc, "/unstake", body).await;
        assert_not_500("/unstake", label, status);
    }
    assert_node_healthy(rpc, "POST /unstake fuzz").await;

    // -----------------------------------------------------------------
    // POST /tx/submit — malformed inputs
    // -----------------------------------------------------------------
    let submit_cases: Vec<(&str, &str)> = vec![
        ("empty body", ""),
        ("not json", "not_json"),
        ("empty object", "{}"),
        ("null", "null"),
        (
            "missing fields",
            r#"{"type":"Transfer"}"#,
        ),
        (
            "invalid signature",
            r#"{"Transfer":{"from":"aa","to":"bb","amount":1,"fee":10000,"nonce":0,"memo":"","pub_key":"0000000000000000000000000000000000000000000000000000000000000000","signature":"00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000"}}"#,
        ),
        (
            "wrong variant",
            r#"{"NotARealVariant":{}}"#,
        ),
        ("number", "12345"),
        ("array of objects", r#"[{"a":1},{"b":2}]"#),
    ];

    for (label, body) in &submit_cases {
        let (status, _) = http_post(rpc, "/tx/submit", body).await;
        assert_not_500("/tx/submit", label, status);
    }
    assert_node_healthy(rpc, "POST /tx/submit fuzz").await;

    // -----------------------------------------------------------------
    // POST /delegate — malformed inputs
    // -----------------------------------------------------------------
    let delegate_cases: Vec<(&str, &str)> = vec![
        ("empty body", ""),
        ("empty object", "{}"),
        (
            "missing validator",
            r#"{"secret_key":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","amount":100000000000}"#,
        ),
        (
            "amount zero",
            r#"{"secret_key":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","validator":"bb","amount":0}"#,
        ),
    ];

    for (label, body) in &delegate_cases {
        let (status, _) = http_post(rpc, "/delegate", body).await;
        assert_not_500("/delegate", label, status);
    }
    assert_node_healthy(rpc, "POST /delegate fuzz").await;

    // -----------------------------------------------------------------
    // POST /undelegate — malformed inputs
    // -----------------------------------------------------------------
    let undelegate_cases: Vec<(&str, &str)> = vec![
        ("empty body", ""),
        ("empty object", "{}"),
        ("invalid key", r#"{"secret_key":"short"}"#),
    ];

    for (label, body) in &undelegate_cases {
        let (status, _) = http_post(rpc, "/undelegate", body).await;
        assert_not_500("/undelegate", label, status);
    }
    assert_node_healthy(rpc, "POST /undelegate fuzz").await;

    // -----------------------------------------------------------------
    // POST /set-commission — malformed inputs
    // -----------------------------------------------------------------
    let commission_cases: Vec<(&str, &str)> = vec![
        ("empty body", ""),
        ("empty object", "{}"),
        (
            "over 100%",
            r#"{"secret_key":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","commission_percent":255}"#,
        ),
        (
            "negative",
            r#"{"secret_key":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","commission_percent":-1}"#,
        ),
    ];

    for (label, body) in &commission_cases {
        let (status, _) = http_post(rpc, "/set-commission", body).await;
        assert_not_500("/set-commission", label, status);
    }
    assert_node_healthy(rpc, "POST /set-commission fuzz").await;

    // -----------------------------------------------------------------
    // POST /proposal — malformed inputs
    // -----------------------------------------------------------------
    let proposal_cases: Vec<(&str, &str)> = vec![
        ("empty body", ""),
        ("empty object", "{}"),
        (
            "missing title",
            r#"{"secret_key":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","description":"desc","proposal_type":"TextProposal","fee":10000}"#,
        ),
        (
            "fee zero",
            r#"{"secret_key":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","title":"t","description":"d","proposal_type":"TextProposal","fee":0}"#,
        ),
    ];

    for (label, body) in &proposal_cases {
        let (status, _) = http_post(rpc, "/proposal", body).await;
        assert_not_500("/proposal", label, status);
    }
    assert_node_healthy(rpc, "POST /proposal fuzz").await;

    // -----------------------------------------------------------------
    // POST /vote — malformed inputs
    // -----------------------------------------------------------------
    let vote_cases: Vec<(&str, &str)> = vec![
        ("empty body", ""),
        ("empty object", "{}"),
        (
            "nonexistent proposal",
            r#"{"secret_key":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","proposal_id":99999,"approve":true,"fee":10000}"#,
        ),
        (
            "fee zero",
            r#"{"secret_key":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","proposal_id":1,"approve":true,"fee":0}"#,
        ),
    ];

    for (label, body) in &vote_cases {
        let (status, _) = http_post(rpc, "/vote", body).await;
        assert_not_500("/vote", label, status);
    }
    assert_node_healthy(rpc, "POST /vote fuzz").await;

    // -----------------------------------------------------------------
    // GET /balance — malformed address inputs
    // -----------------------------------------------------------------
    let long_balance_path = format!("/balance/{}", "ff".repeat(100));
    let balance_paths = vec![
        "/balance/notahex",
        "/balance/xyz!@#$%",
        "/balance/",
        "/balance/0000000000000000000000000000000000000000000000000000000000000000",
        "/balance/abcdef",
        long_balance_path.as_str(),
    ];

    for path in &balance_paths {
        let (status, _) = http_get(rpc, path).await;
        assert_not_500(&format!("GET {}", path), "", status);
    }
    assert_node_healthy(rpc, "GET /balance fuzz").await;

    // -----------------------------------------------------------------
    // GET /round — edge cases
    // -----------------------------------------------------------------
    let round_paths = vec![
        "/round/0",
        "/round/1",
        "/round/99999999",
        "/round/18446744073709551615", // u64::MAX
        "/round/notanumber",
        "/round/-1",
        "/round/",
    ];

    for path in &round_paths {
        let (status, _) = http_get(rpc, path).await;
        assert_not_500(&format!("GET {}", path), "", status);
    }
    assert_node_healthy(rpc, "GET /round fuzz").await;

    // -----------------------------------------------------------------
    // GET /tx/:hash — malformed hash inputs
    // -----------------------------------------------------------------
    let long_tx_path = format!("/tx/{}", "aa".repeat(100));
    let tx_hash_paths = vec![
        "/tx/notahex",
        "/tx/0000000000000000000000000000000000000000000000000000000000000000",
        "/tx/abcdef",
        "/tx/xyz!@#",
        "/tx/",
        long_tx_path.as_str(),
    ];

    for path in &tx_hash_paths {
        let (status, _) = http_get(rpc, path).await;
        assert_not_500(&format!("GET {}", path), "", status);
    }
    assert_node_healthy(rpc, "GET /tx/:hash fuzz").await;

    // -----------------------------------------------------------------
    // GET /vertex/:hash — malformed hash inputs
    // -----------------------------------------------------------------
    let vertex_paths = vec![
        "/vertex/notahex",
        "/vertex/0000000000000000000000000000000000000000000000000000000000000000",
        "/vertex/tooshort",
        "/vertex/",
        "/vertex/ZZZZ",
    ];

    for path in &vertex_paths {
        let (status, _) = http_get(rpc, path).await;
        assert_not_500(&format!("GET {}", path), "", status);
    }
    assert_node_healthy(rpc, "GET /vertex/:hash fuzz").await;

    // -----------------------------------------------------------------
    // GET /proposal/:id — edge cases
    // -----------------------------------------------------------------
    let proposal_paths = vec![
        "/proposal/0",
        "/proposal/99999",
        "/proposal/18446744073709551615",
        "/proposal/notanumber",
        "/proposal/-1",
        "/proposal/",
    ];

    for path in &proposal_paths {
        let (status, _) = http_get(rpc, path).await;
        assert_not_500(&format!("GET {}", path), "", status);
    }
    assert_node_healthy(rpc, "GET /proposal/:id fuzz").await;

    // -----------------------------------------------------------------
    // GET /stake/:address — malformed address inputs
    // -----------------------------------------------------------------
    let stake_paths = vec![
        "/stake/notahex",
        "/stake/0000000000000000000000000000000000000000000000000000000000000000",
        "/stake/",
        "/stake/xyz",
    ];

    for path in &stake_paths {
        let (status, _) = http_get(rpc, path).await;
        assert_not_500(&format!("GET {}", path), "", status);
    }
    assert_node_healthy(rpc, "GET /stake/:address fuzz").await;

    // -----------------------------------------------------------------
    // GET /delegation/:address — malformed address inputs
    // -----------------------------------------------------------------
    let delegation_paths = vec![
        "/delegation/notahex",
        "/delegation/0000000000000000000000000000000000000000000000000000000000000000",
        "/delegation/",
    ];

    for path in &delegation_paths {
        let (status, _) = http_get(rpc, path).await;
        assert_not_500(&format!("GET {}", path), "", status);
    }
    assert_node_healthy(rpc, "GET /delegation/:address fuzz").await;

    // -----------------------------------------------------------------
    // GET /validator/:address/delegators — malformed address
    // -----------------------------------------------------------------
    let delegators_paths = vec![
        "/validator/notahex/delegators",
        "/validator/0000000000000000000000000000000000000000000000000000000000000000/delegators",
    ];

    for path in &delegators_paths {
        let (status, _) = http_get(rpc, path).await;
        assert_not_500(&format!("GET {}", path), "", status);
    }
    assert_node_healthy(rpc, "GET /validator/:address/delegators fuzz").await;

    // -----------------------------------------------------------------
    // GET /vote/:id/:address — edge cases
    // -----------------------------------------------------------------
    let vote_get_paths = vec![
        "/vote/0/0000000000000000000000000000000000000000000000000000000000000000",
        "/vote/99999/notahex",
        "/vote/notanumber/notahex",
    ];

    for path in &vote_get_paths {
        let (status, _) = http_get(rpc, path).await;
        assert_not_500(&format!("GET {}", path), "", status);
    }
    assert_node_healthy(rpc, "GET /vote/:id/:address fuzz").await;

    // -----------------------------------------------------------------
    // OPTIONS /status — CORS preflight
    // -----------------------------------------------------------------
    {
        let (status, response) = http_options(rpc, "/status").await;
        assert_not_500("OPTIONS /status", "", status);
        // CORS preflight should return 200 or 204
        assert!(
            status == 200 || status == 204,
            "OPTIONS /status should return 200 or 204, got {}",
            status
        );
        // Should have CORS headers
        let lower = response.to_lowercase();
        assert!(
            lower.contains("access-control-allow"),
            "OPTIONS /status should include CORS headers"
        );
    }

    // -----------------------------------------------------------------
    // Nonexistent endpoints — should return 404, not 500
    // -----------------------------------------------------------------
    let nonexistent = vec![
        "/nonexistent",
        "/api/v2/status",
        "/../../../etc/passwd",
        "/status/../../secret",
    ];

    for path in &nonexistent {
        let (status, _) = http_get(rpc, path).await;
        assert_not_500(&format!("GET {}", path), "", status);
        assert_ne!(
            status, 200,
            "Nonexistent path {} should not return 200",
            path
        );
    }

    for path in &nonexistent {
        let (status, _) = http_post(rpc, path, "{}").await;
        assert_not_500(&format!("POST {}", path), "{}", status);
    }
    assert_node_healthy(rpc, "nonexistent endpoints fuzz").await;

    // -----------------------------------------------------------------
    // Wrong HTTP method for existing endpoints
    // -----------------------------------------------------------------
    // POST to GET-only endpoints
    {
        let (status, _) = http_post(rpc, "/status", "{}").await;
        assert_not_500("POST /status (wrong method)", "{}", status);
    }
    {
        let (status, _) = http_post(rpc, "/health", "{}").await;
        assert_not_500("POST /health (wrong method)", "{}", status);
    }
    {
        let (status, _) = http_post(rpc, "/validators", "{}").await;
        assert_not_500("POST /validators (wrong method)", "{}", status);
    }
    // GET to POST-only endpoints
    {
        let (status, _) = http_get(rpc, "/tx").await;
        assert_not_500("GET /tx (wrong method)", "", status);
    }
    {
        let (status, _) = http_get(rpc, "/faucet").await;
        assert_not_500("GET /faucet (wrong method)", "", status);
    }
    assert_node_healthy(rpc, "wrong method fuzz").await;

    // -----------------------------------------------------------------
    // Binary / null byte payloads
    // -----------------------------------------------------------------
    {
        let (status, _) = http_post(rpc, "/tx", "\x00\x00\x00\x00").await;
        assert_not_500("/tx null bytes", "\\x00...", status);
    }
    {
        // Send high bytes via raw construction
        let binary_body = String::from_utf8_lossy(&[0x7f, 0x01, 0x02, 0x03, 0x7e, 0x7d]).to_string();
        let (status, _) = http_post(rpc, "/faucet", &binary_body).await;
        assert_not_500("/faucet binary", "binary bytes", status);
    }
    {
        // JSON with embedded null bytes
        let (status, _) = http_post(rpc, "/tx", "{\"from\x00_secret\":\"a\"}").await;
        assert_not_500("/tx embedded null", "embedded null", status);
    }
    assert_node_healthy(rpc, "binary payload fuzz").await;

    // -----------------------------------------------------------------
    // Extremely large payloads (1MB body exceeds max body size)
    // -----------------------------------------------------------------
    {
        let big_body = format!(r#"{{"data":"{}"}}"#, "X".repeat(1_048_576));
        let (status, _) = http_post(rpc, "/tx", &big_body).await;
        assert_not_500("/tx 1MB body", "1MB body", status);
    }
    assert_node_healthy(rpc, "large payload fuzz").await;

    // -----------------------------------------------------------------
    // Final health check on both nodes
    // -----------------------------------------------------------------
    for node in &cluster.nodes {
        let (status, _) = http_get(node.rpc_port, "/health").await;
        assert!(
            status == 200 || status == 204,
            "Node {} not healthy after full fuzz suite: status={}",
            node.index,
            status
        );
    }

    eprintln!("[rpc_fuzz] All fuzz cases passed — no 500s, no crashes.");
}
