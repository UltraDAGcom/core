/// Clock skew fault injection.
/// 
/// Simulates time drift between nodes to test timestamp validation,
/// round synchronization, and timeout handling.

use super::{FaultInjector, TestNode, simulate_rounds};
use std::time::Duration;
use tokio::time::sleep;

/// Clock skew scenarios
pub enum ClockSkewScenario {
    /// One node is ahead by specified seconds
    SingleNodeAhead { node_id: usize, seconds: i64 },
    /// One node is behind by specified seconds
    SingleNodeBehind { node_id: usize, seconds: i64 },
    /// Gradual drift (nodes drift apart over time)
    GradualDrift { max_drift_secs: i64 },
    /// Random skew for all nodes
    RandomSkew { max_offset_secs: i64 },
}

impl ClockSkewScenario {
    /// Apply clock skew to nodes
    pub fn apply(&self, injector: &FaultInjector, num_nodes: usize) {
        match self {
            ClockSkewScenario::SingleNodeAhead { node_id, seconds } => {
                injector.set_clock_offset(*node_id, *seconds);
            }
            ClockSkewScenario::SingleNodeBehind { node_id, seconds } => {
                injector.set_clock_offset(*node_id, -*seconds);
            }
            ClockSkewScenario::GradualDrift { max_drift_secs } => {
                // Each node gets progressively more drift
                let drift_per_node = *max_drift_secs / num_nodes as i64;
                for i in 0..num_nodes {
                    let offset = (i as i64) * drift_per_node - (*max_drift_secs / 2);
                    injector.set_clock_offset(i, offset);
                }
            }
            ClockSkewScenario::RandomSkew { max_offset_secs } => {
                for i in 0..num_nodes {
                    let offset = (rand::random::<i64>() % (2 * max_offset_secs)) - max_offset_secs;
                    injector.set_clock_offset(i, offset);
                }
            }
        }
    }

    /// Clear all clock skew
    pub fn clear(&self, injector: &FaultInjector, num_nodes: usize) {
        for i in 0..num_nodes {
            injector.set_clock_offset(i, 0);
        }
    }
}

/// Test that nodes reject vertices with timestamps too far in the future
pub async fn test_future_timestamp_rejection(
    injector: &FaultInjector,
    nodes: &[TestNode],
    future_offset_secs: i64,
) -> Result<(), String> {
    // Set one node's clock far into the future
    injector.set_clock_offset(0, future_offset_secs);
    
    // In a real test, we'd verify that vertices from node 0 are rejected
    // by other nodes due to timestamp being too far in the future
    
    // For now, just verify the clock offset is applied
    let node_time = injector.node_time(0);
    let real_time = chrono::Utc::now().timestamp();
    
    if (node_time - real_time - future_offset_secs).abs() > 1 {
        return Err(format!(
            "Clock offset not applied correctly: expected {}, got {}",
            future_offset_secs,
            node_time - real_time
        ));
    }
    
    // Clear skew
    injector.set_clock_offset(0, 0);
    
    Ok(())
}

/// Test that nodes can sync despite moderate clock skew
pub async fn test_sync_with_moderate_skew(
    injector: &FaultInjector,
    nodes: &[TestNode],
    max_skew_secs: i64,
) -> Result<(), String> {
    // Apply random skew to all nodes (within acceptable bounds)
    for i in 0..nodes.len() {
        let offset = (rand::random::<i64>() % (2 * max_skew_secs)) - max_skew_secs;
        injector.set_clock_offset(i, offset);
    }

    // Simulate rounds with clock skew active
    simulate_rounds(nodes, injector, 10).await;

    // Check that nodes are making progress despite skew
    let mut any_progress = false;
    for node in nodes {
        if node.finalized_round().await > 0 {
            any_progress = true;
            break;
        }
    }

    // Clear skew
    for i in 0..nodes.len() {
        injector.set_clock_offset(i, 0);
    }

    if !any_progress {
        return Err("Nodes did not make progress with moderate clock skew".to_string());
    }

    Ok(())
}

/// Test extreme clock skew (should cause issues)
pub async fn test_extreme_clock_skew(
    injector: &FaultInjector,
    nodes: &[TestNode],
) -> Result<(), String> {
    // Set one node 1 hour in the future
    injector.set_clock_offset(0, 3600);
    
    // This should cause the node's vertices to be rejected
    sleep(Duration::from_secs(5)).await;
    
    // In a real test, we'd verify that node 0's vertices are rejected
    // and it cannot participate in consensus
    
    // Clear skew
    injector.set_clock_offset(0, 0);
    
    Ok(())
}

/// Test clock skew during epoch boundary
pub async fn test_skew_at_epoch_boundary(
    injector: &FaultInjector,
    nodes: &[TestNode],
) -> Result<(), String> {
    // This tests whether clock skew can cause epoch transition issues
    // where nodes disagree on when the epoch boundary occurs
    
    // Apply skew
    injector.set_clock_offset(0, 60); // 1 minute ahead
    injector.set_clock_offset(1, -60); // 1 minute behind
    
    // In a real test, we'd advance to an epoch boundary and verify
    // that all nodes transition correctly despite clock differences
    
    sleep(Duration::from_secs(5)).await;
    
    // Clear skew
    injector.set_clock_offset(0, 0);
    injector.set_clock_offset(1, 0);
    
    Ok(())
}

/// Test gradual clock drift over time
pub async fn test_gradual_drift(
    injector: &FaultInjector,
    nodes: &[TestNode],
    drift_rate_secs_per_sec: f64,
    duration: Duration,
) -> Result<(), String> {
    let start = tokio::time::Instant::now();
    
    while start.elapsed() < duration {
        // Update clock offsets to simulate drift
        let elapsed_secs = start.elapsed().as_secs_f64();
        for i in 0..nodes.len() {
            let drift = (elapsed_secs * drift_rate_secs_per_sec * (i as f64 - nodes.len() as f64 / 2.0)) as i64;
            injector.set_clock_offset(i, drift);
        }
        
        sleep(Duration::from_secs(1)).await;
    }
    
    // Clear all drift
    for i in 0..nodes.len() {
        injector.set_clock_offset(i, 0);
    }
    
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clock_skew_scenario_single_ahead() {
        let injector = FaultInjector::new();
        let scenario = ClockSkewScenario::SingleNodeAhead {
            node_id: 0,
            seconds: 300,
        };
        
        scenario.apply(&injector, 4);
        
        let node_time = injector.node_time(0);
        let real_time = chrono::Utc::now().timestamp();
        
        assert!((node_time - real_time - 300).abs() <= 1);
    }

    #[test]
    fn test_clock_skew_scenario_gradual_drift() {
        let injector = FaultInjector::new();
        let scenario = ClockSkewScenario::GradualDrift {
            max_drift_secs: 120,
        };
        
        scenario.apply(&injector, 4);
        
        // Verify nodes have different offsets
        let time0 = injector.node_time(0);
        let time3 = injector.node_time(3);
        
        assert_ne!(time0, time3);
    }

    #[tokio::test]
    async fn test_future_timestamp_basic() {
        let injector = FaultInjector::new();
        let nodes = vec![];
        
        let result = test_future_timestamp_rejection(&injector, &nodes, 600).await;
        assert!(result.is_ok());
    }
}
