/// Network partition fault injection.
/// 
/// Simulates split-brain scenarios where groups of nodes cannot communicate.

use super::{FaultInjector, TestNode};
use std::time::Duration;
use tokio::time::sleep;

/// Partition scenario configurations
pub enum PartitionScenario {
    /// Split network into two equal halves
    SplitBrain,
    /// Isolate a single node from the rest
    IsolateOne(usize),
    /// Create a minority partition (1/3 vs 2/3)
    MinorityPartition,
    /// Complete network failure (all nodes isolated)
    CompleteIsolation,
}

impl PartitionScenario {
    /// Generate partition groups for N nodes
    pub fn generate_groups(&self, num_nodes: usize) -> Vec<Vec<usize>> {
        match self {
            PartitionScenario::SplitBrain => {
                let mid = num_nodes / 2;
                vec![
                    (0..mid).collect(),
                    (mid..num_nodes).collect(),
                ]
            }
            PartitionScenario::IsolateOne(node_id) => {
                let mut others: Vec<usize> = (0..num_nodes).filter(|&n| n != *node_id).collect();
                vec![
                    vec![*node_id],
                    others,
                ]
            }
            PartitionScenario::MinorityPartition => {
                let minority_size = num_nodes / 3;
                vec![
                    (0..minority_size).collect(),
                    (minority_size..num_nodes).collect(),
                ]
            }
            PartitionScenario::CompleteIsolation => {
                (0..num_nodes).map(|n| vec![n]).collect()
            }
        }
    }
}

/// Run a partition test scenario
pub async fn test_partition_scenario(
    injector: &FaultInjector,
    nodes: &[TestNode],
    scenario: PartitionScenario,
    duration: Duration,
) {
    let groups = scenario.generate_groups(nodes.len());
    
    println!("🔪 Creating partition: {:?}", groups);
    injector.partition(groups);
    
    // Let the partition run
    sleep(duration).await;
    
    println!("🔧 Healing partition");
    injector.heal_partitions();
    
    // Allow time for nodes to reconnect and sync
    sleep(Duration::from_secs(5)).await;
}

/// Test that minority partition cannot make progress
pub async fn test_minority_cannot_finalize(
    injector: &FaultInjector,
    nodes: &[TestNode],
) -> Result<(), String> {
    let minority_size = nodes.len() / 3;
    let groups = vec![
        (0..minority_size).collect(),
        (minority_size..nodes.len()).collect(),
    ];
    
    // Record finalized rounds before partition
    let mut before_rounds = Vec::new();
    for node in nodes {
        before_rounds.push(node.finalized_round().await);
    }
    
    // Create partition
    injector.partition(groups);
    sleep(Duration::from_secs(10)).await;
    
    // Check that minority nodes did NOT make progress
    for i in 0..minority_size {
        let round_after = nodes[i].finalized_round().await;
        if round_after > before_rounds[i] {
            return Err(format!(
                "Minority node {} finalized new rounds during partition (before: {}, after: {})",
                i, before_rounds[i], round_after
            ));
        }
    }
    
    // Check that majority nodes DID make progress
    let mut majority_progressed = false;
    for i in minority_size..nodes.len() {
        let round_after = nodes[i].finalized_round().await;
        if round_after > before_rounds[i] {
            majority_progressed = true;
            break;
        }
    }
    
    injector.heal_partitions();
    
    if !majority_progressed {
        return Err("Majority partition did not make progress".to_string());
    }
    
    Ok(())
}

/// Test that nodes converge after partition heals
pub async fn test_partition_convergence(
    injector: &FaultInjector,
    nodes: &[TestNode],
    partition_duration: Duration,
    convergence_timeout: Duration,
) -> Result<(), String> {
    // Create split-brain partition
    let mid = nodes.len() / 2;
    let groups = vec![
        (0..mid).collect(),
        (mid..nodes.len()).collect(),
    ];
    
    injector.partition(groups);
    sleep(partition_duration).await;
    
    // Heal partition
    injector.heal_partitions();
    
    // Wait for convergence
    let start = tokio::time::Instant::now();
    loop {
        if start.elapsed() > convergence_timeout {
            return Err("Nodes did not converge within timeout".to_string());
        }
        
        // Check if all nodes have the same finalized round
        let rounds: Vec<u64> = futures::future::join_all(
            nodes.iter().map(|n| n.finalized_round())
        ).await;
        
        let min_round = *rounds.iter().min().unwrap();
        let max_round = *rounds.iter().max().unwrap();
        
        if max_round - min_round <= 2 {
            // Nodes are within 2 rounds of each other - close enough
            return Ok(());
        }
        
        sleep(Duration::from_millis(500)).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_split_brain_groups() {
        let scenario = PartitionScenario::SplitBrain;
        let groups = scenario.generate_groups(4);
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0], vec![0, 1]);
        assert_eq!(groups[1], vec![2, 3]);
    }

    #[test]
    fn test_isolate_one_groups() {
        let scenario = PartitionScenario::IsolateOne(2);
        let groups = scenario.generate_groups(4);
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0], vec![2]);
        assert_eq!(groups[1], vec![0, 1, 3]);
    }

    #[test]
    fn test_minority_partition_groups() {
        let scenario = PartitionScenario::MinorityPartition;
        let groups = scenario.generate_groups(6);
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0], vec![0, 1]); // 1/3
        assert_eq!(groups[1], vec![2, 3, 4, 5]); // 2/3
    }
}
