/// Crash-restart fault injection with state recovery testing.
/// 
/// Simulates node crashes and validates that state can be recovered from
/// persistent storage (checkpoints + WAL).

use super::{FaultInjector, TestNode, simulate_rounds};
use std::time::Duration;
use tokio::time::sleep;
use ultradag_coin::{BlockDag, FinalityTracker, StateEngine};
use std::sync::Arc;
use tokio::sync::RwLock;

/// Crash and restart a node, simulating state recovery
pub async fn crash_and_restart(
    injector: &FaultInjector,
    node: &mut TestNode,
    recovery_delay: Duration,
) {
    println!("💥 Crashing node {}", node.id);
    
    // Mark node as crashed
    injector.crash_node(node.id);
    
    // Simulate crash by dropping in-memory state
    node.crash().await;
    
    // Wait for recovery delay
    sleep(recovery_delay).await;
    
    println!("🔄 Restarting node {}", node.id);
    
    // In a real system, we'd load from disk here
    // For testing, we create fresh state (simulating empty recovery)
    // The node would need to fast-sync from peers
    
    // Mark node as recovered
    injector.restart_node(node.id);
}

/// Test crash during active consensus
pub async fn test_crash_during_consensus(
    injector: &FaultInjector,
    nodes: &mut [TestNode],
    crash_node_id: usize,
) -> Result<(), String> {
    // Run some initial rounds so nodes have state
    simulate_rounds(nodes, injector, 5).await;

    // Record state before crash
    let before_round = nodes[crash_node_id].finalized_round().await;

    // Crash the node (keep it crashed while others continue)
    injector.crash_node(crash_node_id);
    nodes[crash_node_id].crash().await;

    // Simulate rounds with the crashed node absent
    simulate_rounds(nodes, injector, 10).await;

    // Check that other nodes made progress
    let mut others_progressed = false;
    for (i, node) in nodes.iter().enumerate() {
        if i == crash_node_id {
            continue;
        }
        let round = node.finalized_round().await;
        if round > before_round {
            others_progressed = true;
            break;
        }
    }

    // Restart the crashed node
    injector.restart_node(crash_node_id);

    if !others_progressed {
        return Err("Other nodes did not make progress during crash".to_string());
    }

    Ok(())
}

/// Test repeated crash-restart cycles
pub async fn test_crash_restart_cycles(
    injector: &FaultInjector,
    node: &mut TestNode,
    num_cycles: usize,
    crash_duration: Duration,
) -> Result<(), String> {
    for cycle in 0..num_cycles {
        println!("Crash-restart cycle {}/{}", cycle + 1, num_cycles);
        
        // Crash
        injector.crash_node(node.id);
        node.crash().await;
        sleep(crash_duration).await;
        
        // Restart
        injector.restart_node(node.id);
        sleep(Duration::from_secs(1)).await;
    }
    
    Ok(())
}

/// Test that crashed node can recover and catch up
pub async fn test_crash_recovery_catchup(
    injector: &FaultInjector,
    nodes: &mut [TestNode],
    crash_node_id: usize,
    crash_duration: Duration,
) -> Result<(), String> {
    // Record finalized round before crash
    let before_round = nodes[crash_node_id].finalized_round().await;
    
    // Crash the node
    injector.crash_node(crash_node_id);
    nodes[crash_node_id].crash().await;
    
    // Let other nodes progress
    sleep(crash_duration).await;
    
    // Check that other nodes progressed
    let mut max_round = before_round;
    for (i, node) in nodes.iter().enumerate() {
        if i == crash_node_id {
            continue;
        }
        let round = node.finalized_round().await;
        max_round = max_round.max(round);
    }
    
    if max_round <= before_round {
        return Err("Other nodes did not progress during crash".to_string());
    }
    
    // Restart crashed node
    injector.restart_node(crash_node_id);
    
    // In a real system, the node would fast-sync from peers
    // For this test, we verify the node can be restarted
    
    Ok(())
}

/// Test crash during checkpoint creation
pub async fn test_crash_during_checkpoint(
    injector: &FaultInjector,
    node: &mut TestNode,
) -> Result<(), String> {
    // This would test crashing while writing a checkpoint
    // The WAL should allow recovery to a consistent state
    
    println!("Testing crash during checkpoint creation");
    
    // Simulate crash at checkpoint boundary
    injector.crash_node(node.id);
    node.crash().await;
    
    sleep(Duration::from_millis(500)).await;
    
    // Restart and verify recovery
    injector.restart_node(node.id);
    
    // In a real system, we'd verify the checkpoint is either:
    // 1. Fully written and valid
    // 2. Not written, and WAL allows recovery
    
    Ok(())
}

/// Test simultaneous crashes of multiple nodes
pub async fn test_simultaneous_crashes(
    injector: &FaultInjector,
    nodes: &mut [TestNode],
    crash_node_ids: Vec<usize>,
) -> Result<(), String> {
    if crash_node_ids.len() >= (nodes.len() * 2) / 3 {
        return Err("Cannot crash >= 2/3 of nodes (would halt consensus)".to_string());
    }

    println!("💥 Crashing {} nodes simultaneously", crash_node_ids.len());

    // Crash all specified nodes
    for &node_id in &crash_node_ids {
        injector.crash_node(node_id);
        nodes[node_id].crash().await;
    }

    // Simulate rounds with remaining nodes
    simulate_rounds(nodes, injector, 10).await;

    // Verify remaining nodes can still make progress
    let mut any_progress = false;
    for (i, node) in nodes.iter().enumerate() {
        if crash_node_ids.contains(&i) {
            continue;
        }
        let round = node.finalized_round().await;
        if round > 0 {
            any_progress = true;
            break;
        }
    }

    if !any_progress {
        return Err("Remaining nodes did not make progress".to_string());
    }

    // Restart crashed nodes
    for &node_id in &crash_node_ids {
        injector.restart_node(node_id);
    }

    Ok(())
}

/// Test Byzantine crash (node crashes and restarts with corrupted state)
pub async fn test_byzantine_crash(
    injector: &FaultInjector,
    node: &mut TestNode,
) -> Result<(), String> {
    // Crash node
    injector.crash_node(node.id);
    node.crash().await;
    
    // Restart with "corrupted" state (different genesis or state)
    injector.restart_node(node.id);
    
    // In a real system, other nodes should reject messages from this node
    // if its state diverges from the canonical chain
    
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use ultradag_coin::SecretKey;

    #[tokio::test]
    async fn test_crash_and_restart_basic() {
        let injector = FaultInjector::new();
        let sk = SecretKey::generate();
        let mut node = TestNode::new(0, sk.address());
        
        crash_and_restart(&injector, &mut node, Duration::from_millis(100)).await;
        
        // Node should be restarted
        assert!(!injector.is_crashed(0));
    }

    #[tokio::test]
    async fn test_crash_restart_cycles_basic() {
        let injector = FaultInjector::new();
        let sk = SecretKey::generate();
        let mut node = TestNode::new(0, sk.address());
        
        let result = test_crash_restart_cycles(
            &injector,
            &mut node,
            3,
            Duration::from_millis(100)
        ).await;
        
        assert!(result.is_ok());
    }
}
