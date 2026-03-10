/// Jepsen-style fault injection testing framework for UltraDAG.
/// 
/// This module provides systematic fault injection capabilities:
/// - Network partitions (split-brain scenarios)
/// - Clock skew (time drift between nodes)
/// - Message reordering and delays
/// - Crash-restart cycles with state recovery
/// - Combined fault scenarios
///
/// Invariants checked:
/// - No double-spending
/// - Finality safety (finalized vertices never revert)
/// - Liveness (progress under f < N/3 faults)
/// - Consistency (all nodes agree on finalized state)

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use ultradag_coin::{Address, BlockDag, DagVertex, FinalityTracker, StateEngine};

pub mod network_partition;
pub mod clock_skew;
pub mod message_chaos;
pub mod crash_restart;
pub mod invariants;

/// Fault injection controller for coordinating multiple fault types.
pub struct FaultInjector {
    /// Network partition state (which nodes can communicate)
    pub partitions: Arc<Mutex<NetworkPartitions>>,
    /// Clock skew per node (offset from real time)
    pub clock_offsets: Arc<Mutex<HashMap<usize, i64>>>,
    /// Message delay/reorder queue
    pub message_chaos: Arc<Mutex<MessageChaos>>,
    /// Crash state tracking
    pub crashed_nodes: Arc<Mutex<HashSet<usize>>>,
    /// Start time for test
    pub start_time: Instant,
}

impl FaultInjector {
    pub fn new() -> Self {
        Self {
            partitions: Arc::new(Mutex::new(NetworkPartitions::new())),
            clock_offsets: Arc::new(Mutex::new(HashMap::new())),
            message_chaos: Arc::new(Mutex::new(MessageChaos::new())),
            crashed_nodes: Arc::new(Mutex::new(HashSet::new())),
            start_time: Instant::now(),
        }
    }

    /// Create a network partition (split nodes into groups that can't communicate)
    pub fn partition(&self, groups: Vec<Vec<usize>>) {
        let mut partitions = self.partitions.lock().unwrap();
        partitions.set_partition(groups);
    }

    /// Heal all network partitions
    pub fn heal_partitions(&self) {
        let mut partitions = self.partitions.lock().unwrap();
        partitions.clear();
    }

    /// Check if two nodes can communicate
    pub fn can_communicate(&self, node_a: usize, node_b: usize) -> bool {
        let partitions = self.partitions.lock().unwrap();
        partitions.can_communicate(node_a, node_b)
    }

    /// Set clock offset for a node (in seconds)
    pub fn set_clock_offset(&self, node: usize, offset_secs: i64) {
        let mut offsets = self.clock_offsets.lock().unwrap();
        offsets.insert(node, offset_secs);
    }

    /// Get current time for a node (with clock skew applied)
    pub fn node_time(&self, node: usize) -> i64 {
        let offsets = self.clock_offsets.lock().unwrap();
        let offset = offsets.get(&node).copied().unwrap_or(0);
        chrono::Utc::now().timestamp() + offset
    }

    /// Inject message delay (messages delayed by random amount up to max_delay)
    pub fn inject_message_delay(&self, max_delay_ms: u64) {
        let mut chaos = self.message_chaos.lock().unwrap();
        chaos.max_delay_ms = max_delay_ms;
    }

    /// Enable message reordering
    pub fn enable_message_reordering(&self, enabled: bool) {
        let mut chaos = self.message_chaos.lock().unwrap();
        chaos.reorder_enabled = enabled;
    }

    /// Crash a node
    pub fn crash_node(&self, node: usize) {
        let mut crashed = self.crashed_nodes.lock().unwrap();
        crashed.insert(node);
    }

    /// Restart a crashed node
    pub fn restart_node(&self, node: usize) {
        let mut crashed = self.crashed_nodes.lock().unwrap();
        crashed.remove(&node);
    }

    /// Check if a node is crashed
    pub fn is_crashed(&self, node: usize) -> bool {
        let crashed = self.crashed_nodes.lock().unwrap();
        crashed.contains(&node)
    }

    /// Reset all faults
    pub fn reset(&self) {
        self.heal_partitions();
        self.clock_offsets.lock().unwrap().clear();
        self.message_chaos.lock().unwrap().reset();
        self.crashed_nodes.lock().unwrap().clear();
    }
}

/// Network partition state
pub struct NetworkPartitions {
    /// Groups of nodes that can communicate with each other
    groups: Vec<Vec<usize>>,
}

impl NetworkPartitions {
    pub fn new() -> Self {
        Self { groups: vec![] }
    }

    pub fn set_partition(&mut self, groups: Vec<Vec<usize>>) {
        self.groups = groups;
    }

    pub fn clear(&mut self) {
        self.groups.clear();
    }

    pub fn can_communicate(&self, node_a: usize, node_b: usize) -> bool {
        if self.groups.is_empty() {
            return true; // No partition
        }

        // Check if both nodes are in the same group
        for group in &self.groups {
            if group.contains(&node_a) && group.contains(&node_b) {
                return true;
            }
        }
        false
    }
}

/// Message chaos injection (delays, reordering, drops)
pub struct MessageChaos {
    pub max_delay_ms: u64,
    pub reorder_enabled: bool,
    pub drop_rate: f64, // 0.0 to 1.0
}

impl MessageChaos {
    pub fn new() -> Self {
        Self {
            max_delay_ms: 0,
            reorder_enabled: false,
            drop_rate: 0.0,
        }
    }

    pub fn reset(&mut self) {
        self.max_delay_ms = 0;
        self.reorder_enabled = false;
        self.drop_rate = 0.0;
    }

    /// Calculate delay for a message (0 if no delay injection)
    pub fn calculate_delay(&self) -> Duration {
        if self.max_delay_ms == 0 {
            return Duration::from_millis(0);
        }
        let delay_ms = rand::random::<u64>() % self.max_delay_ms;
        Duration::from_millis(delay_ms)
    }

    /// Should this message be dropped?
    pub fn should_drop(&self) -> bool {
        if self.drop_rate == 0.0 {
            return false;
        }
        rand::random::<f64>() < self.drop_rate
    }
}

/// Test node state for fault injection testing
pub struct TestNode {
    pub id: usize,
    pub state: Arc<RwLock<StateEngine>>,
    pub dag: Arc<RwLock<BlockDag>>,
    pub finality: Arc<RwLock<FinalityTracker>>,
    pub validator_address: Address,
}

impl TestNode {
    pub fn new(id: usize, validator_address: Address) -> Self {
        let state = Arc::new(RwLock::new(StateEngine::new()));
        let dag = Arc::new(RwLock::new(BlockDag::new()));
        let finality = Arc::new(RwLock::new(FinalityTracker::new(3)));
        
        Self {
            id,
            state,
            dag,
            finality,
            validator_address,
        }
    }

    /// Simulate crash by dropping all state (will need to recover from disk)
    pub async fn crash(&mut self) {
        // In a real crash, state is lost but disk persists
        // We simulate by creating new in-memory state
        self.state = Arc::new(RwLock::new(StateEngine::new()));
        self.dag = Arc::new(RwLock::new(BlockDag::new()));
        self.finality = Arc::new(RwLock::new(FinalityTracker::new(3)));
    }

    /// Get current finalized round
    pub async fn finalized_round(&self) -> u64 {
        let finality = self.finality.read().await;
        finality.last_finalized_round()
    }

    /// Get total supply
    pub async fn total_supply(&self) -> u64 {
        let state = self.state.read().await;
        state.total_supply()
    }

    /// Get balance for an address
    pub async fn balance(&self, addr: &Address) -> u64 {
        let state = self.state.read().await;
        state.balance(addr)
    }
}
