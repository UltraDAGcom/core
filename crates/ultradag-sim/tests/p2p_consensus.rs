//! Real multi-node consensus tests on localhost.
//! Spawns actual ultradag-node binary processes and verifies consensus.

use ultradag_sim::p2p::cluster::TestCluster;
use ultradag_sim::p2p::helpers;

/// 3 real nodes on localhost reach consensus and finalize rounds.
#[tokio::test(flavor = "multi_thread")]
async fn three_nodes_reach_consensus() {
    let base = helpers::allocate_ports();
    let cluster = TestCluster::new(3, base);

    eprintln!("[test] Cluster base port: {}, RPC ports: {}-{}", base, base+1000, base+1002);

    // Wait for nodes to start
    assert!(cluster.wait_for_ready(30).await,
        "Nodes should be reachable within 30s (RPC ports {}-{})", base+1000, base+1002);

    // Wait for consensus to progress
    let reached = cluster.wait_for_finality(5, 60).await;

    // Get final status
    let mut rounds = Vec::new();
    for i in 0..3 {
        if let Some(status) = cluster.get_status(i).await {
            let round = status["last_finalized_round"].as_u64().unwrap_or(0);
            rounds.push(round);
        }
    }

    assert!(reached, "3 nodes should finalize past round 5 within 60s. Rounds: {:?}", rounds);

    // All should be within 3 rounds of each other
    if rounds.len() == 3 {
        let max = *rounds.iter().max().unwrap();
        let min = *rounds.iter().min().unwrap();
        assert!(max - min <= 3, "Finalized rounds should be close: {:?}", rounds);
    }
}

/// Verify all nodes agree on the same supply after consensus.
#[tokio::test(flavor = "multi_thread")]
async fn nodes_agree_on_supply() {
    let base = helpers::allocate_ports();
    let cluster = TestCluster::new(3, base);

    assert!(cluster.wait_for_ready(30).await, "Nodes not ready");
    assert!(cluster.wait_for_finality(5, 60).await, "Finality not reached");

    let mut supplies = Vec::new();
    for i in 0..3 {
        if let Some(status) = cluster.get_status(i).await {
            let supply = status["total_supply"].as_u64().unwrap_or(0);
            supplies.push(supply);
        }
    }

    assert_eq!(supplies.len(), 3, "Should get status from all 3 nodes");
    assert!(supplies.windows(2).all(|w| w[0] == w[1]),
        "All nodes should agree on total_supply: {:?}", supplies);
}
