//! Malformed message attack tests.
//! Sends garbage/invalid data to real nodes and verifies they handle it correctly.

use ultradag_sim::p2p::cluster::TestCluster;
use ultradag_sim::p2p::attacker::RawAttacker;
use ultradag_sim::p2p::helpers;
use std::time::Duration;

/// Send garbage bytes before Noise handshake. Node should close connection.
#[tokio::test(flavor = "multi_thread")]
async fn garbage_before_handshake() {
    let base = helpers::allocate_ports();
    let cluster = TestCluster::new(2, base);
    assert!(cluster.wait_for_ready(30).await, "Nodes not ready");

    let addr = format!("127.0.0.1:{}", base);
    let mut attacker = RawAttacker::connect(&addr).await
        .expect("Should connect to node");

    // Send garbage (not a valid Noise handshake)
    attacker.send_bytes(b"THIS IS GARBAGE\x00\xff\xff\x00").await.unwrap();

    // Node should close the connection (Noise handshake fails)
    tokio::time::sleep(Duration::from_secs(3)).await;
    assert!(!attacker.is_connected().await, "Node should close connection on garbage input");
}

/// Send a huge length prefix (100MB) with tiny payload.
/// Node should reject without allocating 100MB.
#[tokio::test(flavor = "multi_thread")]
async fn oversized_length_prefix() {
    let base = helpers::allocate_ports();
    let cluster = TestCluster::new(2, base);
    assert!(cluster.wait_for_ready(30).await, "Nodes not ready");

    let addr = format!("127.0.0.1:{}", base);
    let mut attacker = RawAttacker::connect(&addr).await
        .expect("Should connect to node");

    // Send a 4-byte length prefix claiming 100MB, then tiny data
    let fake_len: u32 = 100_000_000;
    attacker.send_bytes(&fake_len.to_be_bytes()).await.unwrap();
    attacker.send_bytes(&[0xAA; 10]).await.unwrap();

    tokio::time::sleep(Duration::from_secs(3)).await;
    // Node should either close or the Noise handshake should fail
    // (the "length prefix" is actually interpreted as Noise handshake data)
    assert!(!attacker.is_connected().await, "Node should close on oversized/invalid input");
}

/// Open connection then immediately close. Node should handle gracefully.
#[tokio::test(flavor = "multi_thread")]
async fn connect_and_immediately_close() {
    let base = helpers::allocate_ports();
    let cluster = TestCluster::new(2, base);
    assert!(cluster.wait_for_ready(30).await, "Nodes not ready");

    // Open and close 10 connections rapidly
    for _ in 0..10 {
        let addr = format!("127.0.0.1:{}", base);
        if let Ok(attacker) = RawAttacker::connect(&addr).await {
            drop(attacker); // Immediately close
        }
    }

    // Node should still be healthy
    tokio::time::sleep(Duration::from_secs(2)).await;
    assert!(cluster.get_status(0).await.is_some(), "Node should still be healthy after rapid connect/disconnect");
}

/// Send partial data (incomplete message) then go silent.
/// Node's read timeout (30s) should clean up.
#[tokio::test(flavor = "multi_thread")]
async fn partial_data_then_silence() {
    let base = helpers::allocate_ports();
    let cluster = TestCluster::new(2, base);
    assert!(cluster.wait_for_ready(30).await, "Nodes not ready");

    let addr = format!("127.0.0.1:{}", base);
    let mut attacker = RawAttacker::connect(&addr).await
        .expect("Should connect to node");

    // Send a few bytes (looks like start of Noise handshake) then stop
    attacker.send_bytes(&[0x00, 0x00, 0x00, 0x20]).await.unwrap();
    attacker.send_bytes(&[0x41; 32]).await.unwrap();

    // Wait for handshake timeout (10s) + margin
    tokio::time::sleep(Duration::from_secs(15)).await;
    assert!(!attacker.is_connected().await, "Node should timeout incomplete handshake");
}
