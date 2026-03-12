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
use ultradag_coin::{Address, Block, BlockHeader, SecretKey, Signature, BlockDag, DagVertex, FinalityTracker, StateEngine};
use ultradag_coin::tx::CoinbaseTx;

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
    pub secret_key: SecretKey,
}

impl TestNode {
    pub fn new(id: usize, _validator_address: Address) -> Self {
        // Generate a deterministic secret key from id
        let mut seed = [0u8; 32];
        seed[0] = id as u8 + 100;
        let sk = SecretKey::from_bytes(seed);
        Self::new_with_key(id, sk)
    }

    pub fn new_with_key(id: usize, sk: SecretKey) -> Self {
        let validator_address = sk.address();
        let state = Arc::new(RwLock::new(StateEngine::new()));
        let dag = Arc::new(RwLock::new(BlockDag::new()));
        let finality = Arc::new(RwLock::new(FinalityTracker::new(3)));

        Self {
            id,
            state,
            dag,
            finality,
            validator_address,
            secret_key: sk,
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

/// Create a signed DagVertex for a given node.
fn make_vertex(uid: u64, round: u64, parents: Vec<[u8; 32]>, sk: &SecretKey) -> DagVertex {
    let validator = sk.address();
    let block = Block {
        header: BlockHeader {
            version: 1,
            height: uid,
            timestamp: 1_000_000 + uid as i64,
            prev_hash: parents.first().copied().unwrap_or([0u8; 32]),
            merkle_root: [0u8; 32],
        },
        coinbase: CoinbaseTx { to: validator, amount: 5_000_000_000, height: uid },
        transactions: vec![],
    };
    let mut v = DagVertex::new(
        block, parents, round, validator,
        sk.verifying_key().to_bytes(), Signature([0u8; 64]),
    );
    v.signature = sk.sign(&v.signable_bytes());
    v
}

/// Simulate multiple rounds of consensus across nodes, respecting fault injection.
///
/// Each round:
/// 1. Each non-crashed node produces a vertex referencing previous round's vertices
/// 2. Vertices are inserted into all reachable nodes (respecting partitions)
/// 3. Finality is checked on all nodes
pub async fn simulate_rounds(
    nodes: &[TestNode],
    injector: &FaultInjector,
    num_rounds: u64,
) {
    let mut vertex_uid = 1u64;

    for round in 1..=num_rounds {
        // Collect vertices produced this round
        let mut round_vertices: Vec<(usize, DagVertex)> = Vec::new();

        for node in nodes.iter() {
            if injector.is_crashed(node.id) {
                continue;
            }

            // Get parents from previous round on this node's DAG
            let parents = {
                let dag = node.dag.read().await;
                if round == 1 {
                    vec![]
                } else {
                    let prev_verts = dag.vertices_in_round(round - 1);
                    prev_verts.iter().map(|v| v.hash()).collect::<Vec<_>>()
                }
            };

            let vertex = make_vertex(vertex_uid, round, parents, &node.secret_key);
            vertex_uid += 1;
            round_vertices.push((node.id, vertex));
        }

        // Distribute vertices to all reachable nodes
        for (producer_id, vertex) in &round_vertices {
            for node in nodes.iter() {
                if injector.is_crashed(node.id) {
                    continue;
                }
                // Check partition: can producer reach this node?
                if !injector.can_communicate(*producer_id, node.id) {
                    continue;
                }
                // Check message drops
                {
                    let chaos = injector.message_chaos.lock().unwrap();
                    if chaos.should_drop() {
                        continue;
                    }
                }

                let mut dag = node.dag.write().await;
                dag.insert(vertex.clone());
            }
        }

        // Register validators and check finality on all non-crashed nodes
        for node in nodes.iter() {
            if injector.is_crashed(node.id) {
                continue;
            }

            // Register all known validators
            {
                let mut ft = node.finality.write().await;
                for other in nodes.iter() {
                    ft.register_validator(other.validator_address);
                }
            }

            // Check finality
            let dag = node.dag.read().await;
            let mut ft = node.finality.write().await;
            let newly_finalized = ft.find_newly_finalized(&dag);
            drop(newly_finalized); // We just need finality to advance
        }
    }
}
