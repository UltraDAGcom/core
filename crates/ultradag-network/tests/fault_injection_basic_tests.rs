/// Basic fault injection tests that don't require full node setup.
/// These tests verify the fault injection infrastructure itself.

mod fault_injection;

use fault_injection::*;
use fault_injection::invariants::{InvariantChecker, InvariantViolation};
use fault_injection::network_partition::PartitionScenario;
use fault_injection::clock_skew::ClockSkewScenario;
use fault_injection::message_chaos::MessageChaosScenario;

use std::time::Duration;
use tokio::time::sleep;
use ultradag_coin::SecretKey;

#[tokio::test]
async fn test_fault_injector_creation() {
    let injector = FaultInjector::new();
    
    // Verify initial state
    assert!(injector.can_communicate(0, 1), "Nodes should communicate initially");
    assert!(!injector.is_crashed(0), "Nodes should not be crashed initially");
}

#[tokio::test]
async fn test_network_partition_basic() {
    let injector = FaultInjector::new();
    
    // Create partition: [0, 1] vs [2, 3]
    injector.partition(vec![vec![0, 1], vec![2, 3]]);
    
    // Nodes in same group can communicate
    assert!(injector.can_communicate(0, 1));
    assert!(injector.can_communicate(2, 3));
    
    // Nodes in different groups cannot communicate
    assert!(!injector.can_communicate(0, 2));
    assert!(!injector.can_communicate(0, 3));
    assert!(!injector.can_communicate(1, 2));
    assert!(!injector.can_communicate(1, 3));
    
    // Heal partition
    injector.heal_partitions();
    
    // All nodes can communicate again
    assert!(injector.can_communicate(0, 2));
    assert!(injector.can_communicate(1, 3));
}

#[tokio::test]
async fn test_partition_scenarios() {
    let _injector = FaultInjector::new();
    
    // Test split-brain
    let scenario = PartitionScenario::SplitBrain;
    let groups = scenario.generate_groups(4);
    assert_eq!(groups.len(), 2);
    assert_eq!(groups[0].len(), 2);
    assert_eq!(groups[1].len(), 2);
    
    // Test isolate one
    let scenario = PartitionScenario::IsolateOne(1);
    let groups = scenario.generate_groups(4);
    assert_eq!(groups.len(), 2);
    assert!(groups[0].contains(&1) || groups[1].contains(&1));
    
    // Test minority partition
    let scenario = PartitionScenario::MinorityPartition;
    let groups = scenario.generate_groups(6);
    assert_eq!(groups.len(), 2);
    assert_eq!(groups[0].len(), 2); // 1/3
    assert_eq!(groups[1].len(), 4); // 2/3
}

#[tokio::test]
async fn test_clock_skew_basic() {
    let injector = FaultInjector::new();
    
    // Set clock offset
    injector.set_clock_offset(0, 300); // 5 minutes ahead
    
    let node_time = injector.node_time(0);
    let real_time = chrono::Utc::now().timestamp();
    
    // Node 0 should be ~300 seconds ahead
    let diff = node_time - real_time;
    assert!((diff - 300).abs() <= 2, "Clock offset should be ~300s, got {}", diff);
    
    // Clear offset
    injector.set_clock_offset(0, 0);
    let node_time = injector.node_time(0);
    let real_time = chrono::Utc::now().timestamp();
    assert!((node_time - real_time).abs() <= 1, "Clock should be back to normal");
}

#[tokio::test]
async fn test_clock_skew_scenarios() {
    let injector = FaultInjector::new();
    
    // Test single node ahead
    let scenario = ClockSkewScenario::SingleNodeAhead {
        node_id: 0,
        seconds: 600,
    };
    scenario.apply(&injector, 4);
    
    let time0 = injector.node_time(0);
    let time1 = injector.node_time(1);
    assert!((time0 - time1 - 600).abs() <= 2);
    
    // Clear
    scenario.clear(&injector, 4);
    
    // Test gradual drift
    let scenario = ClockSkewScenario::GradualDrift {
        max_drift_secs: 120,
    };
    scenario.apply(&injector, 4);
    
    // Verify nodes have different offsets
    let time0 = injector.node_time(0);
    let time3 = injector.node_time(3);
    assert_ne!(time0, time3, "Nodes should have different times with gradual drift");
}

#[tokio::test]
async fn test_message_chaos_basic() {
    let injector = FaultInjector::new();
    
    // Test delay injection
    injector.inject_message_delay(1000);
    {
        let chaos = injector.message_chaos.lock().unwrap();
        assert_eq!(chaos.max_delay_ms, 1000);
        
        let delay = chaos.calculate_delay();
        assert!(delay.as_millis() <= 1000);
    }
    
    // Test reordering
    injector.enable_message_reordering(true);
    {
        let chaos = injector.message_chaos.lock().unwrap();
        assert!(chaos.reorder_enabled);
    }
    
    // Test drop rate
    {
        let mut chaos = injector.message_chaos.lock().unwrap();
        chaos.drop_rate = 0.0;
        assert!(!chaos.should_drop());
        
        chaos.drop_rate = 1.0;
        assert!(chaos.should_drop());
    }
}

#[tokio::test]
async fn test_message_chaos_scenarios() {
    let injector = FaultInjector::new();
    
    // Test random delay scenario
    let scenario = MessageChaosScenario::RandomDelay { max_ms: 2000 };
    scenario.apply(&injector);
    
    {
        let chaos = injector.message_chaos.lock().unwrap();
        assert_eq!(chaos.max_delay_ms, 2000);
    }
    
    scenario.clear(&injector);
    
    // Test extreme chaos
    let scenario = MessageChaosScenario::ExtremeChao {
        max_delay_ms: 3000,
        drop_rate: 0.15,
    };
    scenario.apply(&injector);
    
    {
        let chaos = injector.message_chaos.lock().unwrap();
        assert_eq!(chaos.max_delay_ms, 3000);
        assert_eq!(chaos.drop_rate, 0.15);
        assert!(chaos.reorder_enabled);
    }
}

#[tokio::test]
async fn test_crash_restart_basic() {
    let injector = FaultInjector::new();
    
    // Crash a node
    injector.crash_node(0);
    assert!(injector.is_crashed(0));
    assert!(!injector.is_crashed(1));
    
    // Restart the node
    injector.restart_node(0);
    assert!(!injector.is_crashed(0));
}

#[tokio::test]
async fn test_multiple_crashes() {
    let injector = FaultInjector::new();
    
    // Crash multiple nodes
    injector.crash_node(0);
    injector.crash_node(2);
    
    assert!(injector.is_crashed(0));
    assert!(!injector.is_crashed(1));
    assert!(injector.is_crashed(2));
    assert!(!injector.is_crashed(3));
    
    // Restart one
    injector.restart_node(0);
    assert!(!injector.is_crashed(0));
    assert!(injector.is_crashed(2));
}

#[tokio::test]
async fn test_fault_injector_reset() {
    let injector = FaultInjector::new();
    
    // Apply various faults
    injector.partition(vec![vec![0, 1], vec![2, 3]]);
    injector.set_clock_offset(0, 300);
    injector.inject_message_delay(1000);
    injector.crash_node(0);
    
    // Reset all faults
    injector.reset();
    
    // Verify everything is cleared
    assert!(injector.can_communicate(0, 2));
    assert!(!injector.is_crashed(0));
    
    let time0 = injector.node_time(0);
    let real_time = chrono::Utc::now().timestamp();
    assert!((time0 - real_time).abs() <= 1);
    
    {
        let chaos = injector.message_chaos.lock().unwrap();
        assert_eq!(chaos.max_delay_ms, 0);
        assert!(!chaos.reorder_enabled);
        assert_eq!(chaos.drop_rate, 0.0);
    }
}

#[tokio::test]
async fn test_test_node_creation() {
    let sk = SecretKey::generate();
    let node = TestNode::new_with_key(0, sk);

    assert_eq!(node.id, 0);
    assert_eq!(node.validator_address, node.secret_key.address());
    
    // Verify initial state
    let round = node.finalized_round().await;
    assert_eq!(round, 0);
    
    let supply = node.total_supply().await;
    assert_eq!(supply, 0);
}

#[tokio::test]
async fn test_test_node_crash() {
    let sk = SecretKey::generate();
    let mut node = TestNode::new(0, sk.address());
    
    // Crash the node
    node.crash().await;
    
    // State should be reset
    let round = node.finalized_round().await;
    assert_eq!(round, 0);
}

#[tokio::test]
async fn test_invariant_checker_creation() {
    let mut checker = InvariantChecker::new();
    
    // Create a test node
    let sk = SecretKey::generate();
    let node = TestNode::new(0, sk.address());
    
    // Check invariants on empty node
    let violations = checker.check_all(&[node]).await;
    assert!(violations.is_empty(), "Empty node should have no violations");
}

#[tokio::test]
async fn test_invariant_checker_supply_consistency() {
    let checker = InvariantChecker::new();
    
    // Create multiple nodes
    let sk1 = SecretKey::generate();
    let sk2 = SecretKey::generate();
    let node1 = TestNode::new(0, sk1.address());
    let node2 = TestNode::new(1, sk2.address());
    
    // Both should have same supply (0)
    let result = checker.check_supply_consistency(&[node1, node2]).await;
    assert!(result.is_ok(), "Supply should be consistent");
}

#[tokio::test]
async fn test_combined_faults() {
    let injector = FaultInjector::new();
    
    // Apply multiple faults simultaneously
    injector.partition(vec![vec![0, 1], vec![2, 3]]);
    injector.set_clock_offset(0, 60);
    injector.inject_message_delay(500);
    injector.crash_node(2);
    
    // Verify all faults are active
    assert!(!injector.can_communicate(0, 2));
    assert!(injector.is_crashed(2));
    
    let time0 = injector.node_time(0);
    let time1 = injector.node_time(1);
    assert!((time0 - time1 - 60).abs() <= 2);
    
    {
        let chaos = injector.message_chaos.lock().unwrap();
        assert_eq!(chaos.max_delay_ms, 500);
    }
    
    // Reset and verify
    injector.reset();
    assert!(injector.can_communicate(0, 2));
    assert!(!injector.is_crashed(2));
}

#[tokio::test]
async fn test_partition_isolation_completeness() {
    let injector = FaultInjector::new();
    
    // Create complete isolation
    let scenario = PartitionScenario::CompleteIsolation;
    let groups = scenario.generate_groups(4);
    
    injector.partition(groups);
    
    // No nodes should be able to communicate
    for i in 0..4 {
        for j in 0..4 {
            if i != j {
                assert!(!injector.can_communicate(i, j),
                    "Nodes {} and {} should not communicate in complete isolation", i, j);
            }
        }
    }
}

#[tokio::test]
async fn test_message_chaos_delay_calculation() {
    let injector = FaultInjector::new();
    injector.inject_message_delay(1000);
    
    // Calculate delays multiple times and verify they're within bounds
    for _ in 0..100 {
        let chaos = injector.message_chaos.lock().unwrap();
        let delay = chaos.calculate_delay();
        assert!(delay.as_millis() <= 1000, "Delay should be <= 1000ms");
    }
}

#[tokio::test]
async fn test_message_drop_probability() {
    let injector = FaultInjector::new();
    
    {
        let mut chaos = injector.message_chaos.lock().unwrap();
        chaos.drop_rate = 0.5; // 50% drop rate
        
        // Test multiple times to verify probabilistic behavior
        let mut dropped = 0;
        let iterations = 1000;
        
        for _ in 0..iterations {
            if chaos.should_drop() {
                dropped += 1;
            }
        }
        
        // Should be roughly 50% (allow 10% variance)
        let drop_percentage = (dropped as f64 / iterations as f64) * 100.0;
        assert!((40.0..=60.0).contains(&drop_percentage),
            "Drop rate should be ~50%, got {}%", drop_percentage);
    }
}

#[tokio::test]
async fn test_fault_injector_concurrent_access() {
    let injector = std::sync::Arc::new(FaultInjector::new());
    
    let mut handles = vec![];
    
    // Spawn multiple tasks modifying faults concurrently
    for i in 0..10 {
        let inj = injector.clone();
        let handle = tokio::spawn(async move {
            inj.set_clock_offset(i, i as i64 * 10);
            sleep(Duration::from_millis(10)).await;
            inj.crash_node(i);
            sleep(Duration::from_millis(10)).await;
            inj.restart_node(i);
        });
        handles.push(handle);
    }
    
    // Wait for all tasks
    for handle in handles {
        handle.await.unwrap();
    }
    
    // Verify no panics occurred and state is consistent
    for i in 0..10 {
        assert!(!injector.is_crashed(i));
    }
}

#[test]
fn test_invariant_violation_formatting() {
    let checker = InvariantChecker::new();
    
    let violation = InvariantViolation::FinalityConflict {
        round: 100,
        node_a: 0,
        node_b: 1,
        hash_a: [1u8; 32],
        hash_b: [2u8; 32],
    };
    
    let report = checker.report(&[violation]);
    assert!(report.contains("Finality conflict"));
    assert!(report.contains("round 100"));
}

#[test]
fn test_empty_violations_report() {
    let checker = InvariantChecker::new();
    let report = checker.report(&[]);
    assert!(report.contains("All invariants satisfied"));
}
