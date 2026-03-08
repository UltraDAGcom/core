use mockito::{Matcher, Server};
use ultradag_sdk::UltraDagClient;

fn make_client(server: &Server) -> UltraDagClient {
    UltraDagClient::new(&server.url())
}

// ---------------------------------------------------------------------------
// GET endpoints
// ---------------------------------------------------------------------------

#[test]
fn test_health() {
    let mut server = Server::new();
    let mock = server
        .mock("GET", "/health")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"status":"ok"}"#)
        .create();

    let client = make_client(&server);
    let resp = client.health().unwrap();
    assert_eq!(resp.status, "ok");
    mock.assert();
}

#[test]
fn test_status() {
    let mut server = Server::new();
    let body = serde_json::json!({
        "last_finalized_round": 42,
        "peer_count": 3,
        "mempool_size": 10,
        "total_supply": 2100000000000000u64,
        "account_count": 5,
        "dag_vertices": 200,
        "dag_round": 50,
        "dag_tips": 4,
        "finalized_count": 180,
        "validator_count": 4,
        "total_staked": 1000000000000u64,
        "active_stakers": 4,
        "bootstrap_connected": true
    });
    let mock = server
        .mock("GET", "/status")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(body.to_string())
        .create();

    let client = make_client(&server);
    let resp = client.status().unwrap();
    assert_eq!(resp.last_finalized_round, Some(42));
    assert_eq!(resp.peer_count, 3);
    assert_eq!(resp.dag_round, 50);
    assert!(resp.bootstrap_connected);
    mock.assert();
}

#[test]
fn test_balance() {
    let mut server = Server::new();
    let addr = "a".repeat(64);
    let body = serde_json::json!({
        "address": addr,
        "balance": 500000000u64,
        "nonce": 3,
        "balance_tdag": 5.0
    });
    let mock = server
        .mock("GET", format!("/balance/{addr}").as_str())
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(body.to_string())
        .create();

    let client = make_client(&server);
    let resp = client.balance(&addr).unwrap();
    assert_eq!(resp.balance, 500000000);
    assert_eq!(resp.nonce, 3);
    mock.assert();
}

#[test]
fn test_balance_invalid_address() {
    let server = Server::new();
    let client = make_client(&server);
    let err = client.balance("tooshort").unwrap_err();
    assert!(matches!(err, ultradag_sdk::UltraDagError::InvalidAddress(_)));
}

#[test]
fn test_round() {
    let mut server = Server::new();
    let body = serde_json::json!([
        {
            "round": 10,
            "hash": "abcd1234",
            "validator": "val1",
            "reward": 5000000000u64,
            "tx_count": 2,
            "parent_count": 3
        }
    ]);
    let mock = server
        .mock("GET", "/round/10")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(body.to_string())
        .create();

    let client = make_client(&server);
    let resp = client.round(10).unwrap();
    assert_eq!(resp.len(), 1);
    assert_eq!(resp[0].round, 10);
    assert_eq!(resp[0].tx_count, 2);
    mock.assert();
}

#[test]
fn test_mempool() {
    let mut server = Server::new();
    let mock = server
        .mock("GET", "/mempool")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body("[]")
        .create();

    let client = make_client(&server);
    let resp = client.mempool().unwrap();
    assert!(resp.is_empty());
    mock.assert();
}

#[test]
fn test_keygen() {
    let mut server = Server::new();
    let body = serde_json::json!({
        "secret_key": "ab".repeat(32),
        "address": "cd".repeat(32)
    });
    let mock = server
        .mock("GET", "/keygen")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(body.to_string())
        .create();

    let client = make_client(&server);
    let resp = client.keygen().unwrap();
    assert_eq!(resp.secret_key.len(), 64);
    assert_eq!(resp.address.len(), 64);
    mock.assert();
}

#[test]
fn test_peers() {
    let mut server = Server::new();
    let body = serde_json::json!({
        "connected": 2,
        "peers": ["1.2.3.4:9333", "5.6.7.8:9333"],
        "bootstrap_nodes": [
            {"address": "1.2.3.4:9333", "connected": true}
        ]
    });
    let mock = server
        .mock("GET", "/peers")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(body.to_string())
        .create();

    let client = make_client(&server);
    let resp = client.peers().unwrap();
    assert_eq!(resp.connected, 2);
    assert_eq!(resp.peers.len(), 2);
    assert_eq!(resp.bootstrap_nodes.len(), 1);
    assert!(resp.bootstrap_nodes[0].connected);
    mock.assert();
}

#[test]
fn test_validators() {
    let mut server = Server::new();
    let body = serde_json::json!({
        "count": 1,
        "total_staked": 1000000000000u64,
        "validators": [
            {"address": "a".repeat(64), "stake": 1000000000000u64}
        ]
    });
    let mock = server
        .mock("GET", "/validators")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(body.to_string())
        .create();

    let client = make_client(&server);
    let resp = client.validators().unwrap();
    assert_eq!(resp.count, 1);
    assert_eq!(resp.validators[0].stake, 1000000000000);
    mock.assert();
}

#[test]
fn test_stake_info() {
    let mut server = Server::new();
    let addr = "b".repeat(64);
    let body = serde_json::json!({
        "address": addr,
        "staked": 500000000000u64,
        "staked_udag": 5000.0,
        "unlock_at_round": null,
        "is_active_validator": true
    });
    let mock = server
        .mock("GET", format!("/stake/{addr}").as_str())
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(body.to_string())
        .create();

    let client = make_client(&server);
    let resp = client.stake_info(&addr).unwrap();
    assert_eq!(resp.staked, 500000000000);
    assert!(resp.is_active_validator);
    assert!(resp.unlock_at_round.is_none());
    mock.assert();
}

#[test]
fn test_governance_config() {
    let mut server = Server::new();
    let body = serde_json::json!({"min_stake": 10000});
    let mock = server
        .mock("GET", "/governance/config")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(body.to_string())
        .create();

    let client = make_client(&server);
    let resp = client.governance_config().unwrap();
    assert_eq!(resp["min_stake"], 10000);
    mock.assert();
}

#[test]
fn test_proposals() {
    let mut server = Server::new();
    let body = serde_json::json!({"count": 0, "proposals": []});
    let mock = server
        .mock("GET", "/proposals")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(body.to_string())
        .create();

    let client = make_client(&server);
    let resp = client.proposals().unwrap();
    assert_eq!(resp.count, 0);
    mock.assert();
}

#[test]
fn test_proposal() {
    let mut server = Server::new();
    let body = serde_json::json!({"id": 1, "title": "Test"});
    let mock = server
        .mock("GET", "/proposal/1")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(body.to_string())
        .create();

    let client = make_client(&server);
    let resp = client.proposal(1).unwrap();
    assert_eq!(resp["id"], 1);
    mock.assert();
}

#[test]
fn test_vote_info() {
    let mut server = Server::new();
    let addr = "c".repeat(64);
    let body = serde_json::json!({"voted": true});
    let mock = server
        .mock("GET", format!("/vote/1/{addr}").as_str())
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(body.to_string())
        .create();

    let client = make_client(&server);
    let resp = client.vote_info(1, &addr).unwrap();
    assert_eq!(resp["voted"], true);
    mock.assert();
}

// ---------------------------------------------------------------------------
// POST endpoints
// ---------------------------------------------------------------------------

#[test]
fn test_send_tx() {
    let mut server = Server::new();
    let body = serde_json::json!({
        "hash": "deadbeef",
        "from": "f".repeat(64),
        "to": "e".repeat(64),
        "amount": 100000000u64,
        "fee": 100000u64,
        "nonce": 1
    });
    let mock = server
        .mock("POST", "/tx")
        .match_header("content-type", "application/json")
        .match_body(Matcher::Any)
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(body.to_string())
        .create();

    let client = make_client(&server);
    let resp = client
        .send_tx(&"ab".repeat(16), &"e".repeat(64), 100000000, 100000)
        .unwrap();
    assert_eq!(resp.amount, 100000000);
    assert_eq!(resp.nonce, 1);
    mock.assert();
}

#[test]
fn test_faucet() {
    let mut server = Server::new();
    let body = serde_json::json!({"success": true});
    let mock = server
        .mock("POST", "/faucet")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(body.to_string())
        .create();

    let client = make_client(&server);
    let resp = client.faucet(&"d".repeat(64), 1000000000).unwrap();
    assert_eq!(resp["success"], true);
    mock.assert();
}

#[test]
fn test_stake() {
    let mut server = Server::new();
    let body = serde_json::json!({"ok": true});
    let mock = server
        .mock("POST", "/stake")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(body.to_string())
        .create();

    let client = make_client(&server);
    let resp = client.stake(&"ab".repeat(16), 1000000000000).unwrap();
    assert_eq!(resp["ok"], true);
    mock.assert();
}

#[test]
fn test_unstake() {
    let mut server = Server::new();
    let body = serde_json::json!({"ok": true});
    let mock = server
        .mock("POST", "/unstake")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(body.to_string())
        .create();

    let client = make_client(&server);
    let resp = client.unstake(&"ab".repeat(16)).unwrap();
    assert_eq!(resp["ok"], true);
    mock.assert();
}

#[test]
fn test_submit_proposal() {
    let mut server = Server::new();
    let body = serde_json::json!({"id": 1});
    let mock = server
        .mock("POST", "/proposal")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(body.to_string())
        .create();

    let client = make_client(&server);
    let req = ultradag_sdk::ProposalRequest {
        proposer_secret: "ab".repeat(16),
        title: "Test".into(),
        description: "A test proposal".into(),
        proposal_type: "parameter_change".into(),
        parameter_name: Some("round_ms".into()),
        parameter_value: Some("3000".into()),
        fee: None,
    };
    let resp = client.submit_proposal(&req).unwrap();
    assert_eq!(resp["id"], 1);
    mock.assert();
}

#[test]
fn test_vote() {
    let mut server = Server::new();
    let body = serde_json::json!({"ok": true});
    let mock = server
        .mock("POST", "/vote")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(body.to_string())
        .create();

    let client = make_client(&server);
    let req = ultradag_sdk::VoteRequest {
        voter_secret: "ab".repeat(16),
        proposal_id: 1,
        vote: true,
        fee: None,
    };
    let resp = client.vote(&req).unwrap();
    assert_eq!(resp["ok"], true);
    mock.assert();
}

// ---------------------------------------------------------------------------
// Error handling
// ---------------------------------------------------------------------------

#[test]
fn test_api_error_404() {
    let mut server = Server::new();
    let mock = server
        .mock("GET", "/balance/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")
        .with_status(404)
        .with_body("not found")
        .create();

    let client = make_client(&server);
    let addr = "a".repeat(64);
    let err = client.balance(&addr).unwrap_err();
    match err {
        ultradag_sdk::UltraDagError::Api { status, message } => {
            assert_eq!(status, 404);
            assert_eq!(message, "not found");
        }
        other => panic!("expected Api error, got: {other:?}"),
    }
    mock.assert();
}

#[test]
fn test_json_parse_error() {
    let mut server = Server::new();
    let mock = server
        .mock("GET", "/status")
        .with_status(200)
        .with_body("not json at all")
        .create();

    let client = make_client(&server);
    let err = client.status().unwrap_err();
    assert!(matches!(err, ultradag_sdk::UltraDagError::Json(_)));
    mock.assert();
}

#[test]
fn test_connection_error() {
    // No server running on this port
    let client = UltraDagClient::new("http://127.0.0.1:19999");
    let err = client.health().unwrap_err();
    assert!(matches!(err, ultradag_sdk::UltraDagError::Http(_)));
}
