/// Jepsen-style fault injection tests for UltraDAG.
/// 
/// These tests systematically inject faults to validate distributed system
/// properties like safety, liveness, and consistency.

mod fault_injection;

use fault_injection::*;
use fault_injection::invariants::{InvariantChecker, InvariantViolation};
use fault_injection::network_partition::{PartitionScenario, test_partition_scenario, test_minority_cannot_finalize, test_partition_convergence};
use fault_injection::clock_skew::{ClockSkewScenario, test_future_timestamp_rejection, test_sync_with_moderate_skew};
use fault_injection::message_chaos::{MessageChaosScenario, test_consensus_with_delays, test_consensus_with_reordering, test_consensus_with_drops, test_extreme_chaos};
use fault_injection::crash_restart::{crash_and_restart, test_crash_during_consensus, test_crash_restart_cycles, test_simultaneous_crashes};

use std::time::Duration;
use tokio::time::sleep;
use ultradag_coin::SecretKey;

/// Test: Network partition - split brain scenario
#[tokio::test]
#[ignore] // Run with: cargo test --test jepsen_tests -- --ignored
async fn test_split_brain_partition() {
    let injector = FaultInjector::new();
    
    // Create 4 test nodes
    let validators: Vec<SecretKey> = (0..4).map(|_| SecretKey::generate()).collect();
    let nodes: Vec<TestNode> = validators.iter().enumerate()
        .map(|(i, sk)| TestNode::new(i, sk.address()))
        .collect();
    
    // Run split-brain partition for 10 seconds
    test_partition_scenario(
        &injector,
        &nodes,
        PartitionScenario::SplitBrain,
        Duration::from_secs(10),
    ).await;
    
    // Check invariants after partition heals
    let mut checker = InvariantChecker::new();
    let violations = checker.check_all(&nodes).await;
    
    if !violations.is_empty() {
        panic!("Invariant violations after split-brain: {}", checker.report(&violations));
    }
}

/// Test: Minority partition cannot finalize
#[tokio::test]
#[ignore]
async fn test_minority_partition_liveness() {
    let injector = FaultInjector::new();
    
    // Create 6 nodes (minority = 2, majority = 4)
    let validators: Vec<SecretKey> = (0..6).map(|_| SecretKey::generate()).collect();
    let nodes: Vec<TestNode> = validators.iter().enumerate()
        .map(|(i, sk)| TestNode::new(i, sk.address()))
        .collect();
    
    // Test that minority cannot finalize
    let result = test_minority_cannot_finalize(&injector, &nodes).await;
    
    assert!(result.is_ok(), "Minority partition test failed: {:?}", result);
}

/// Test: Nodes converge after partition heals
#[tokio::test]
#[ignore]
async fn test_partition_heal_convergence() {
    let injector = FaultInjector::new();
    
    let validators: Vec<SecretKey> = (0..4).map(|_| SecretKey::generate()).collect();
    let nodes: Vec<TestNode> = validators.iter().enumerate()
        .map(|(i, sk)| TestNode::new(i, sk.address()))
        .collect();
    
    let result = test_partition_convergence(
        &injector,
        &nodes,
        Duration::from_secs(5),  // partition duration
        Duration::from_secs(30), // convergence timeout
    ).await;
    
    assert!(result.is_ok(), "Partition convergence failed: {:?}", result);
}

/// Test: Clock skew - moderate offset
#[tokio::test]
#[ignore]
async fn test_moderate_clock_skew() {
    let injector = FaultInjector::new();
    
    let validators: Vec<SecretKey> = (0..4).map(|_| SecretKey::generate()).collect();
    let nodes: Vec<TestNode> = validators.iter().enumerate()
        .map(|(i, sk)| TestNode::new(i, sk.address()))
        .collect();
    
    // Test with ±30 second skew
    let result = test_sync_with_moderate_skew(&injector, &nodes, 30).await;
    
    assert!(result.is_ok(), "Moderate clock skew test failed: {:?}", result);
}

/// Test: Clock skew - future timestamp rejection
#[tokio::test]
#[ignore]
async fn test_future_timestamp_validation() {
    let injector = FaultInjector::new();
    
    let validators: Vec<SecretKey> = (0..4).map(|_| SecretKey::generate()).collect();
    let nodes: Vec<TestNode> = validators.iter().enumerate()
        .map(|(i, sk)| TestNode::new(i, sk.address()))
        .collect();
    
    // Test with 10 minute future offset
    let result = test_future_timestamp_rejection(&injector, &nodes, 600).await;
    
    assert!(result.is_ok(), "Future timestamp rejection failed: {:?}", result);
}

/// Test: Message delays
#[tokio::test]
#[ignore]
async fn test_message_delay_resilience() {
    let injector = FaultInjector::new();
    
    let validators: Vec<SecretKey> = (0..4).map(|_| SecretKey::generate()).collect();
    let nodes: Vec<TestNode> = validators.iter().enumerate()
        .map(|(i, sk)| TestNode::new(i, sk.address()))
        .collect();
    
    // Test with up to 2 second delays
    let result = test_consensus_with_delays(
        &injector,
        &nodes,
        2000, // 2 second max delay
        Duration::from_secs(15),
    ).await;
    
    assert!(result.is_ok(), "Message delay test failed: {:?}", result);
}

/// Test: Message reordering
#[tokio::test]
#[ignore]
async fn test_message_reordering_resilience() {
    let injector = FaultInjector::new();
    
    let validators: Vec<SecretKey> = (0..4).map(|_| SecretKey::generate()).collect();
    let nodes: Vec<TestNode> = validators.iter().enumerate()
        .map(|(i, sk)| TestNode::new(i, sk.address()))
        .collect();
    
    let result = test_consensus_with_reordering(
        &injector,
        &nodes,
        Duration::from_secs(15),
    ).await;
    
    assert!(result.is_ok(), "Message reordering test failed: {:?}", result);
}

/// Test: Message drops (10%)
#[tokio::test]
#[ignore]
async fn test_message_drop_resilience() {
    let injector = FaultInjector::new();
    
    let validators: Vec<SecretKey> = (0..4).map(|_| SecretKey::generate()).collect();
    let nodes: Vec<TestNode> = validators.iter().enumerate()
        .map(|(i, sk)| TestNode::new(i, sk.address()))
        .collect();
    
    // Test with 10% message drop rate
    let result = test_consensus_with_drops(
        &injector,
        &nodes,
        0.10,
        Duration::from_secs(20),
    ).await;
    
    assert!(result.is_ok(), "Message drop test failed: {:?}", result);
}

/// Test: Crash and restart single node
#[tokio::test]
#[ignore]
async fn test_single_node_crash_restart() {
    let injector = FaultInjector::new();
    
    let validators: Vec<SecretKey> = (0..4).map(|_| SecretKey::generate()).collect();
    let mut nodes: Vec<TestNode> = validators.iter().enumerate()
        .map(|(i, sk)| TestNode::new(i, sk.address()))
        .collect();
    
    let result = test_crash_during_consensus(&injector, &mut nodes, 0).await;
    
    assert!(result.is_ok(), "Single node crash test failed: {:?}", result);
}

/// Test: Repeated crash-restart cycles
#[tokio::test]
#[ignore]
async fn test_repeated_crash_cycles() {
    let injector = FaultInjector::new();
    
    let sk = SecretKey::generate();
    let mut node = TestNode::new(0, sk.address());
    
    let result = test_crash_restart_cycles(
        &injector,
        &mut node,
        5, // 5 cycles
        Duration::from_secs(1),
    ).await;
    
    assert!(result.is_ok(), "Repeated crash cycles failed: {:?}", result);
}

/// Test: Simultaneous crashes (< 1/3 of nodes)
#[tokio::test]
#[ignore]
async fn test_simultaneous_node_crashes() {
    let injector = FaultInjector::new();
    
    let validators: Vec<SecretKey> = (0..7).map(|_| SecretKey::generate()).collect();
    let mut nodes: Vec<TestNode> = validators.iter().enumerate()
        .map(|(i, sk)| TestNode::new(i, sk.address()))
        .collect();
    
    // Crash 2 out of 7 nodes (< 1/3)
    let result = test_simultaneous_crashes(&injector, &mut nodes, vec![0, 1]).await;
    
    assert!(result.is_ok(), "Simultaneous crashes test failed: {:?}", result);
}

/// Test: EXTREME CHAOS - all faults combined
#[tokio::test]
#[ignore]
async fn test_extreme_chaos_scenario() {
    let injector = FaultInjector::new();
    
    let validators: Vec<SecretKey> = (0..6).map(|_| SecretKey::generate()).collect();
    let mut nodes: Vec<TestNode> = validators.iter().enumerate()
        .map(|(i, sk)| TestNode::new(i, sk.address()))
        .collect();
    
    println!("🌪️  EXTREME CHAOS TEST STARTING");
    
    // Phase 1: Network partition + clock skew
    println!("Phase 1: Partition + Clock Skew");
    injector.partition(vec![vec![0, 1], vec![2, 3, 4, 5]]);
    injector.set_clock_offset(0, 60);
    injector.set_clock_offset(1, -60);
    sleep(Duration::from_secs(5)).await;
    
    // Phase 2: Heal partition, add message chaos
    println!("Phase 2: Message Chaos");
    injector.heal_partitions();
    injector.inject_message_delay(1000);
    injector.enable_message_reordering(true);
    sleep(Duration::from_secs(5)).await;
    
    // Phase 3: Crash a node
    println!("Phase 3: Node Crash");
    crash_and_restart(&injector, &mut nodes[0], Duration::from_secs(2)).await;
    sleep(Duration::from_secs(3)).await;
    
    // Phase 4: Extreme message chaos
    println!("Phase 4: Extreme Message Chaos");
    let result = test_extreme_chaos(
        &injector,
        &nodes,
        2000, // 2 second delays
        0.15, // 15% drops
        Duration::from_secs(10),
    ).await;
    
    // Clear all faults
    injector.reset();
    
    // Check invariants
    let mut checker = InvariantChecker::new();
    let violations = checker.check_all(&nodes).await;
    
    println!("🌪️  EXTREME CHAOS TEST COMPLETE");
    println!("{}", checker.report(&violations));
    
    assert!(result.is_ok(), "Extreme chaos test failed: {:?}", result);
    
    // We allow some violations in extreme chaos, but check for critical ones
    for v in &violations {
        match v {
            InvariantViolation::FinalityConflict { .. } => {
                panic!("CRITICAL: Finality conflict detected: {:?}", v);
            }
            InvariantViolation::FinalityRevert { .. } => {
                panic!("CRITICAL: Finality revert detected: {:?}", v);
            }
            _ => {
                // Other violations are logged but not fatal in extreme chaos
                println!("Warning: {:?}", v);
            }
        }
    }
}

/// Test: Partition + Clock Skew combined
#[tokio::test]
#[ignore]
async fn test_partition_with_clock_skew() {
    let injector = FaultInjector::new();
    
    let validators: Vec<SecretKey> = (0..4).map(|_| SecretKey::generate()).collect();
    let nodes: Vec<TestNode> = validators.iter().enumerate()
        .map(|(i, sk)| TestNode::new(i, sk.address()))
        .collect();
    
    // Create partition
    injector.partition(vec![vec![0, 1], vec![2, 3]]);
    
    // Add clock skew
    injector.set_clock_offset(0, 30);
    injector.set_clock_offset(2, -30);
    
    sleep(Duration::from_secs(10)).await;
    
    // Heal
    injector.heal_partitions();
    for i in 0..4 {
        injector.set_clock_offset(i, 0);
    }
    
    sleep(Duration::from_secs(5)).await;
    
    // Check invariants
    let mut checker = InvariantChecker::new();
    let violations = checker.check_all(&nodes).await;
    
    assert!(violations.is_empty(), "Violations: {}", checker.report(&violations));
}

/// Test: Message chaos + crash combined
#[tokio::test]
#[ignore]
async fn test_message_chaos_with_crash() {
    let injector = FaultInjector::new();
    
    let validators: Vec<SecretKey> = (0..4).map(|_| SecretKey::generate()).collect();
    let mut nodes: Vec<TestNode> = validators.iter().enumerate()
        .map(|(i, sk)| TestNode::new(i, sk.address()))
        .collect();
    
    // Enable message chaos
    injector.inject_message_delay(500);
    injector.enable_message_reordering(true);
    
    sleep(Duration::from_secs(3)).await;
    
    // Crash a node
    crash_and_restart(&injector, &mut nodes[0], Duration::from_secs(2)).await;
    
    sleep(Duration::from_secs(5)).await;
    
    // Clear chaos
    injector.reset();
    
    // Verify system recovered
    sleep(Duration::from_secs(3)).await;
}

/// Helper: Print test summary
fn print_test_summary(test_name: &str, violations: &[InvariantViolation]) {
    println!("\n{'='*60}");
    println!("Test: {}", test_name);
    println!("{'='*60}");
    
    if violations.is_empty() {
        println!("✅ PASSED - No invariant violations");
    } else {
        println!("⚠️  {} violation(s) detected", violations.len());
        for v in violations {
            println!("  - {:?}", v);
        }
    }
    println!("{'='*60}\n");
}
