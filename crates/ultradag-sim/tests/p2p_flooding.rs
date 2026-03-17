//! Resource exhaustion and flooding attack tests.

use ultradag_sim::p2p::cluster::TestCluster;
use ultradag_sim::p2p::attacker::RawAttacker;
use ultradag_sim::p2p::helpers;
use std::time::Duration;

/// Open many concurrent connections. Node should survive.
#[tokio::test(flavor = "multi_thread")]
async fn many_concurrent_connections() {
    let base = helpers::allocate_ports();
    let cluster = TestCluster::new(2, base);
    assert!(cluster.wait_for_ready(30).await, "Nodes not ready");

    let addr = format!("127.0.0.1:{}", base);

    // Open 30 connections rapidly
    let mut connections = Vec::new();
    for _ in 0..30 {
        if let Ok(c) = RawAttacker::connect(&addr).await {
            connections.push(c);
        }
    }

    // Send garbage on each to trigger handshake failure
    for c in &mut connections {
        let _ = c.send_bytes(b"garbage").await;
    }

    tokio::time::sleep(Duration::from_secs(5)).await;

    // Node should still be healthy
    assert!(cluster.get_status(0).await.is_some(),
        "Node should survive 30 concurrent connections");

    // Consensus should still work
    assert!(cluster.wait_for_finality(3, 30).await,
        "Consensus should still progress after connection flood");
}

/// Flood the node with TCP connections, each sending a byte then closing.
/// Tests connection cleanup under rapid churn.
#[tokio::test(flavor = "multi_thread")]
async fn rapid_connection_churn() {
    let base = helpers::allocate_ports();
    let cluster = TestCluster::new(2, base);
    assert!(cluster.wait_for_ready(30).await, "Nodes not ready");

    let addr = format!("127.0.0.1:{}", base);

    // 50 rapid connect-send-close cycles
    for _ in 0..50 {
        if let Ok(mut c) = RawAttacker::connect(&addr).await {
            let _ = c.send_bytes(&[0xFF]).await;
            drop(c);
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }

    tokio::time::sleep(Duration::from_secs(3)).await;
    assert!(cluster.get_status(0).await.is_some(),
        "Node should survive rapid connection churn");
}
