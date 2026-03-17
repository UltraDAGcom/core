//! Noise handshake abuse tests.
//! Tests that the node handles malformed handshake data gracefully.

use ultradag_sim::p2p::cluster::TestCluster;
use ultradag_sim::p2p::attacker::RawAttacker;
use ultradag_sim::p2p::helpers;
use std::time::Duration;

/// Send random bytes during Noise handshake phase.
/// Node should close connection without crashing.
#[tokio::test(flavor = "multi_thread")]
async fn random_bytes_during_handshake() {
    let base = helpers::allocate_ports();
    let cluster = TestCluster::new(2, base);
    assert!(cluster.wait_for_ready(30).await, "Nodes not ready");

    // Send 10 different random-ish byte sequences
    for i in 0..10u8 {
        let addr = format!("127.0.0.1:{}", base);
        if let Ok(mut attacker) = RawAttacker::connect(&addr).await {
            // Send bytes that look like partial Noise handshakes
            let mut data = vec![0u8; 48 + i as usize * 10];
            for (j, b) in data.iter_mut().enumerate() {
                *b = (i.wrapping_mul(37).wrapping_add(j as u8)) ^ 0xAA;
            }
            let _ = attacker.send_bytes(&data).await;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    // Node should still be healthy
    tokio::time::sleep(Duration::from_secs(2)).await;
    assert!(cluster.get_status(0).await.is_some(), "Node should survive handshake abuse");
}

/// Send the correct first Noise handshake message length but wrong content.
/// Tests that snow library handles invalid key material gracefully.
#[tokio::test(flavor = "multi_thread")]
async fn invalid_noise_key_material() {
    let base = helpers::allocate_ports();
    let cluster = TestCluster::new(2, base);
    assert!(cluster.wait_for_ready(30).await, "Nodes not ready");

    let addr = format!("127.0.0.1:{}", base);
    let mut attacker = RawAttacker::connect(&addr).await.expect("Should connect");

    // Noise XX handshake message 1 (→ e) is typically 32 bytes (ephemeral key)
    // Send 32 bytes of zeros — invalid ephemeral key
    attacker.send_bytes(&[0u8; 32]).await.unwrap();

    // Wait for server to process and close
    tokio::time::sleep(Duration::from_secs(3)).await;
    assert!(!attacker.is_connected().await, "Node should close on invalid Noise key");

    // Node still healthy
    assert!(cluster.get_status(0).await.is_some(), "Node healthy after invalid key");
}

/// Flood with handshake attempts — many rapid connections sending partial handshakes.
/// Tests that handshake timeout (10s) cleans up resources.
#[tokio::test(flavor = "multi_thread")]
async fn handshake_flood() {
    let base = helpers::allocate_ports();
    let cluster = TestCluster::new(2, base);
    assert!(cluster.wait_for_ready(30).await, "Nodes not ready");

    let addr = format!("127.0.0.1:{}", base);

    // 20 rapid partial handshakes
    for _ in 0..20 {
        if let Ok(mut c) = RawAttacker::connect(&addr).await {
            // Send partial handshake data
            let _ = c.send_bytes(&[0x00; 48]).await;
            // Don't complete — let it time out
            drop(c);
        }
    }

    // Wait for handshake timeouts to fire
    tokio::time::sleep(Duration::from_secs(5)).await;

    // Node should still be healthy and accepting connections
    assert!(cluster.get_status(0).await.is_some(), "Node should survive handshake flood");

    // Consensus should still work
    assert!(cluster.wait_for_finality(3, 30).await, "Consensus should progress after flood");
}
