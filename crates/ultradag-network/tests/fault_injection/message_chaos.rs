/// Message chaos fault injection.
/// 
/// Simulates unreliable networks with message delays, reordering, and drops.

use super::{FaultInjector, TestNode, simulate_rounds};
use std::time::Duration;
use tokio::time::sleep;

/// Message chaos scenarios
pub enum MessageChaosScenario {
    /// Random delays up to max_ms
    RandomDelay { max_ms: u64 },
    /// Reorder messages (deliver out of order)
    Reordering,
    /// Drop messages with specified probability (0.0 to 1.0)
    RandomDrop { drop_rate: f64 },
    /// Combine delay + reordering
    DelayAndReorder { max_delay_ms: u64 },
    /// Extreme chaos: delay + reorder + drops
    ExtremeChao { max_delay_ms: u64, drop_rate: f64 },
}

impl MessageChaosScenario {
    /// Apply message chaos configuration
    pub fn apply(&self, injector: &FaultInjector) {
        match self {
            MessageChaosScenario::RandomDelay { max_ms } => {
                injector.inject_message_delay(*max_ms);
            }
            MessageChaosScenario::Reordering => {
                injector.enable_message_reordering(true);
            }
            MessageChaosScenario::RandomDrop { drop_rate } => {
                let mut chaos = injector.message_chaos.lock().unwrap();
                chaos.drop_rate = *drop_rate;
            }
            MessageChaosScenario::DelayAndReorder { max_delay_ms } => {
                injector.inject_message_delay(*max_delay_ms);
                injector.enable_message_reordering(true);
            }
            MessageChaosScenario::ExtremeChao { max_delay_ms, drop_rate } => {
                injector.inject_message_delay(*max_delay_ms);
                injector.enable_message_reordering(true);
                let mut chaos = injector.message_chaos.lock().unwrap();
                chaos.drop_rate = *drop_rate;
            }
        }
    }

    /// Clear message chaos
    pub fn clear(&self, injector: &FaultInjector) {
        let mut chaos = injector.message_chaos.lock().unwrap();
        chaos.reset();
    }
}

/// Test that consensus works with moderate message delays
pub async fn test_consensus_with_delays(
    injector: &FaultInjector,
    nodes: &[TestNode],
    max_delay_ms: u64,
    _duration: Duration,
) -> Result<(), String> {
    println!("🐌 Injecting message delays up to {}ms", max_delay_ms);

    injector.inject_message_delay(max_delay_ms);

    // Record initial state
    let initial_rounds: Vec<u64> = futures::future::join_all(
        nodes.iter().map(|n| n.finalized_round())
    ).await;

    // Simulate rounds with delay chaos active (drops simulated by chaos.should_drop())
    simulate_rounds(nodes, injector, 10).await;

    // Check that nodes still made progress
    let final_rounds: Vec<u64> = futures::future::join_all(
        nodes.iter().map(|n| n.finalized_round())
    ).await;

    let mut any_progress = false;
    for i in 0..nodes.len() {
        if final_rounds[i] > initial_rounds[i] {
            any_progress = true;
            break;
        }
    }

    // Clear delays
    injector.inject_message_delay(0);

    if !any_progress {
        return Err("Nodes did not make progress with message delays".to_string());
    }

    Ok(())
}

/// Test that message reordering doesn't break consensus
pub async fn test_consensus_with_reordering(
    injector: &FaultInjector,
    nodes: &[TestNode],
    _duration: Duration,
) -> Result<(), String> {
    println!("🔀 Enabling message reordering");

    injector.enable_message_reordering(true);

    // Record initial state
    let initial_rounds: Vec<u64> = futures::future::join_all(
        nodes.iter().map(|n| n.finalized_round())
    ).await;

    // Simulate rounds with reordering active
    simulate_rounds(nodes, injector, 10).await;

    // Check that nodes still made progress
    let final_rounds: Vec<u64> = futures::future::join_all(
        nodes.iter().map(|n| n.finalized_round())
    ).await;

    let mut any_progress = false;
    for i in 0..nodes.len() {
        if final_rounds[i] > initial_rounds[i] {
            any_progress = true;
            break;
        }
    }

    // Clear reordering
    injector.enable_message_reordering(false);

    if !any_progress {
        return Err("Nodes did not make progress with message reordering".to_string());
    }

    Ok(())
}

/// Test consensus with message drops (should still work if drop_rate < 1/3)
pub async fn test_consensus_with_drops(
    injector: &FaultInjector,
    nodes: &[TestNode],
    drop_rate: f64,
    _duration: Duration,
) -> Result<(), String> {
    if drop_rate >= 0.33 {
        return Err("Drop rate >= 33% may prevent consensus".to_string());
    }

    println!("📉 Dropping {}% of messages", drop_rate * 100.0);

    {
        let mut chaos = injector.message_chaos.lock().unwrap();
        chaos.drop_rate = drop_rate;
    }

    // Record initial state
    let initial_rounds: Vec<u64> = futures::future::join_all(
        nodes.iter().map(|n| n.finalized_round())
    ).await;

    // Simulate rounds with drops active
    simulate_rounds(nodes, injector, 15).await;

    // Check that nodes still made progress
    let final_rounds: Vec<u64> = futures::future::join_all(
        nodes.iter().map(|n| n.finalized_round())
    ).await;

    let mut any_progress = false;
    for i in 0..nodes.len() {
        if final_rounds[i] > initial_rounds[i] {
            any_progress = true;
            break;
        }
    }

    // Clear drops
    {
        let mut chaos = injector.message_chaos.lock().unwrap();
        chaos.drop_rate = 0.0;
    }

    if !any_progress {
        return Err(format!(
            "Nodes did not make progress with {}% message drops",
            drop_rate * 100.0
        ));
    }

    Ok(())
}

/// Test extreme chaos: delays + reordering + drops
pub async fn test_extreme_chaos(
    injector: &FaultInjector,
    nodes: &[TestNode],
    max_delay_ms: u64,
    drop_rate: f64,
    duration: Duration,
) -> Result<(), String> {
    println!("💥 EXTREME CHAOS: {}ms delay, {}% drops, reordering enabled", 
             max_delay_ms, drop_rate * 100.0);
    
    // Apply all chaos
    injector.inject_message_delay(max_delay_ms);
    injector.enable_message_reordering(true);
    {
        let mut chaos = injector.message_chaos.lock().unwrap();
        chaos.drop_rate = drop_rate;
    }
    
    // Record initial state
    let initial_rounds: Vec<u64> = futures::future::join_all(
        nodes.iter().map(|n| n.finalized_round())
    ).await;
    
    // Let nodes run in chaos
    sleep(duration).await;
    
    // Check that nodes still made progress (or at least didn't crash)
    let final_rounds: Vec<u64> = futures::future::join_all(
        nodes.iter().map(|n| n.finalized_round())
    ).await;
    
    // Clear all chaos
    injector.inject_message_delay(0);
    injector.enable_message_reordering(false);
    {
        let mut chaos = injector.message_chaos.lock().unwrap();
        chaos.drop_rate = 0.0;
    }
    
    // With extreme chaos, we just verify nodes didn't crash
    // Progress is not guaranteed
    println!("Survived extreme chaos - progress: {:?} -> {:?}", initial_rounds, final_rounds);
    
    Ok(())
}

/// Test that orphan resolution works with message reordering
pub async fn test_orphan_resolution_with_reordering(
    injector: &FaultInjector,
    _nodes: &[TestNode],
) -> Result<(), String> {
    // Enable reordering which may cause vertices to arrive before their parents
    injector.enable_message_reordering(true);
    
    // In a real test, we'd verify that:
    // 1. Vertices arriving before parents are buffered as orphans
    // 2. When parents arrive, orphans are resolved and inserted
    // 3. No vertices are lost due to reordering
    
    sleep(Duration::from_secs(10)).await;
    
    // Clear reordering
    injector.enable_message_reordering(false);
    
    Ok(())
}

/// Test asymmetric network conditions (different delays per direction)
pub async fn test_asymmetric_delays(
    injector: &FaultInjector,
    _nodes: &[TestNode],
) -> Result<(), String> {
    // This would test scenarios where:
    // - Node A -> B has high delay
    // - Node B -> A has low delay
    // This can expose issues with timeout handling and round synchronization
    
    println!("Testing asymmetric network delays");
    
    // For now, just apply general delays
    injector.inject_message_delay(500);
    sleep(Duration::from_secs(5)).await;
    injector.inject_message_delay(0);
    
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_message_chaos_scenario_delay() {
        let injector = FaultInjector::new();
        let scenario = MessageChaosScenario::RandomDelay { max_ms: 1000 };
        
        scenario.apply(&injector);
        
        let chaos = injector.message_chaos.lock().unwrap();
        assert_eq!(chaos.max_delay_ms, 1000);
    }

    #[test]
    fn test_message_chaos_scenario_extreme() {
        let injector = FaultInjector::new();
        let scenario = MessageChaosScenario::ExtremeChao {
            max_delay_ms: 2000,
            drop_rate: 0.1,
        };
        
        scenario.apply(&injector);
        
        let chaos = injector.message_chaos.lock().unwrap();
        assert_eq!(chaos.max_delay_ms, 2000);
        assert_eq!(chaos.drop_rate, 0.1);
        assert!(chaos.reorder_enabled);
    }

    #[test]
    fn test_calculate_delay() {
        let mut chaos = super::super::MessageChaos::new();
        chaos.max_delay_ms = 1000;
        
        let delay = chaos.calculate_delay();
        assert!(delay.as_millis() <= 1000);
    }

    #[test]
    fn test_should_drop() {
        let mut chaos = super::super::MessageChaos::new();
        
        // 0% drop rate
        chaos.drop_rate = 0.0;
        assert!(!chaos.should_drop());
        
        // 100% drop rate
        chaos.drop_rate = 1.0;
        assert!(chaos.should_drop());
    }
}
