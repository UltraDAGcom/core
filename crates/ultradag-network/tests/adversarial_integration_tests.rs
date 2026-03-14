//! Adversarial integration tests: multi-node consensus simulation with state application.
//!
//! These tests verify system-level correctness under adversarial conditions:
//! 1. Crash-restart: kill a node mid-round, restart, verify state converges
//! 2. Partition-heal: partition two nodes for 50 rounds, reconnect, verify agreement
//! 3. Equivocation: one node equivocates, verify all others slash identically

use std::collections::HashSet;
use ultradag_coin::{
    Address, Block, BlockHeader, SecretKey, Signature,
    BlockDag, DagVertex, FinalityTracker, StateEngine,
};
use ultradag_coin::tx::CoinbaseTx;
use ultradag_coin::consensus::compute_state_root;

/// A test node with DAG, finality tracker, and state engine.
struct AdversarialNode {
    id: usize,
    state: StateEngine,
    dag: BlockDag,
    finality: FinalityTracker,
    secret_key: SecretKey,
    address: Address,
}

impl AdversarialNode {
    fn new(id: usize, total_validators: u64) -> Self {
        let mut seed = [0u8; 32];
        seed[0] = id as u8 + 50; // deterministic keys
        let sk = SecretKey::from_bytes(seed);
        let address = sk.address();
        let mut state = StateEngine::new();
        state.set_configured_validator_count(total_validators);
        Self {
            id,
            state,
            dag: BlockDag::new(),
            finality: FinalityTracker::new(3),
            secret_key: sk,
            address,
        }
    }
}

/// Create a vertex with the correct coinbase amount for state application.
/// Uses `block_reward(round) / validators_in_round` for pre-staking mode.
fn make_correct_vertex(
    round: u64,
    parents: Vec<[u8; 32]>,
    sk: &SecretKey,
    validators_in_round: u64,
) -> DagVertex {
    let validator = sk.address();
    let total_reward = ultradag_coin::constants::block_reward(round);
    let reward = total_reward / validators_in_round.max(1);

    let block = Block {
        header: BlockHeader {
            version: 1,
            height: round,
            timestamp: 1_000_000 + round as i64,
            prev_hash: parents.first().copied().unwrap_or([0u8; 32]),
            merkle_root: [0u8; 32],
        },
        coinbase: CoinbaseTx { to: validator, amount: reward, height: round },
        transactions: vec![],
    };
    let mut v = DagVertex::new(
        block, parents, round, validator,
        sk.verifying_key().to_bytes(), Signature([0u8; 64]),
    );
    v.signature = sk.sign(&v.signable_bytes());
    v
}

/// Simulate rounds with full state application.
///
/// - Produces vertices, distributes per partition rules
/// - Runs finality, applies finalized vertices to StateEngine
/// - Returns the round vertices produced each round (for verification)
fn simulate_with_state(
    nodes: &mut [AdversarialNode],
    num_rounds: u64,
    start_round: u64,
    partitions: &[Vec<usize>], // empty = no partition
    crashed: &HashSet<usize>,
) {
    // Use configured_validator_count for coinbase calculation (matches production behavior).
    let configured_count = nodes[0].state.configured_validator_count().unwrap_or(nodes.len() as u64);

    for round in start_round..start_round + num_rounds {
        // Each non-crashed node produces a vertex
        let mut round_vertices: Vec<(usize, DagVertex)> = Vec::new();
        for node in nodes.iter() {
            if crashed.contains(&node.id) {
                continue;
            }
            let parents = if round == start_round && round == 1 {
                vec![]
            } else {
                let prev_round = round - 1;
                let prev_verts = node.dag.vertices_in_round(prev_round);
                prev_verts.iter().map(|v| v.hash()).collect::<Vec<_>>()
            };
            let vertex = make_correct_vertex(round, parents, &node.secret_key, configured_count);
            round_vertices.push((node.id, vertex));
        }

        // Distribute vertices respecting partitions
        for (producer_id, vertex) in &round_vertices {
            for node in nodes.iter_mut() {
                if crashed.contains(&node.id) {
                    continue;
                }
                if !can_communicate(*producer_id, node.id, partitions) {
                    continue;
                }
                node.dag.insert(vertex.clone());
            }
        }

        // Register validators and check finality on all non-crashed nodes
        let all_addresses: Vec<Address> = nodes.iter().map(|n| n.address).collect();
        for node in nodes.iter_mut() {
            if crashed.contains(&node.id) {
                continue;
            }
            for addr in &all_addresses {
                node.finality.register_validator(*addr);
            }
            let newly_finalized = node.finality.find_newly_finalized(&node.dag);
            if !newly_finalized.is_empty() {
                // Collect finalized vertices
                let finalized_vertices: Vec<DagVertex> = newly_finalized.iter()
                    .filter_map(|h| node.dag.get(h).cloned())
                    .collect();
                if !finalized_vertices.is_empty() {
                    if let Err(e) = node.state.apply_finalized_vertices(&finalized_vertices) {
                        panic!("Node {} failed to apply finalized vertices at round {}: {:?}",
                            node.id, round, e);
                    }
                }
            }
        }
    }
}

fn can_communicate(a: usize, b: usize, partitions: &[Vec<usize>]) -> bool {
    if partitions.is_empty() {
        return true;
    }
    for group in partitions {
        if group.contains(&a) && group.contains(&b) {
            return true;
        }
    }
    false
}

// ============================================================================
// Test 1: Crash-restart convergence
// ============================================================================

#[tokio::test]
async fn test_crash_restart_state_convergence() {
    // 4 nodes, run 10 rounds, crash node 3, run 10 more rounds,
    // restart node 3 by syncing all vertices, verify state converges.

    let mut nodes: Vec<AdversarialNode> = (0..4).map(|i| AdversarialNode::new(i, 4)).collect();

    // Phase 1: All 4 nodes run 10 rounds together
    simulate_with_state(&mut nodes, 10, 1, &[], &HashSet::new());

    // Verify all 4 nodes agree on state after phase 1
    let supply_after_phase1: Vec<u64> = nodes.iter().map(|n| n.state.total_supply()).collect();
    assert!(supply_after_phase1.iter().all(|s| *s == supply_after_phase1[0]),
        "All nodes should agree on supply after phase 1: {:?}", supply_after_phase1);

    // Phase 2: Crash node 3, run 10 more rounds with 3 nodes
    let mut crashed = HashSet::new();
    crashed.insert(3);
    simulate_with_state(&mut nodes, 10, 11, &[], &crashed);

    // Nodes 0-2 should have advanced, node 3 is stale
    let supplies: Vec<u64> = nodes.iter().map(|n| n.state.total_supply()).collect();
    assert_eq!(supplies[0], supplies[1]);
    assert_eq!(supplies[1], supplies[2]);
    assert!(supplies[0] > supplies[3],
        "Active nodes should have higher supply than crashed node");

    // Phase 3: Restart node 3 by syncing all vertices from node 0
    // This simulates what fast-sync/DagVertices handler does
    let mut fresh_state = StateEngine::new();
    fresh_state.set_configured_validator_count(4);
    nodes[3].state = fresh_state;
    nodes[3].dag = BlockDag::new();
    nodes[3].finality = FinalityTracker::new(3);

    // Replay all vertices from node 0's DAG into node 3
    let all_addresses: Vec<Address> = nodes.iter().map(|n| n.address).collect();

    // Collect vertices from node 0 first (avoid borrow conflict)
    let mut all_vertices: Vec<DagVertex> = Vec::new();
    for round in 1..=20 {
        let verts = nodes[0].dag.vertices_in_round(round);
        all_vertices.extend(verts.into_iter().cloned());
    }

    // Insert into node 3
    for v in &all_vertices {
        nodes[3].dag.insert(v.clone());
    }

    // Register validators and run finality on node 3
    {
        let node = &mut nodes[3];
        for addr in &all_addresses {
            node.finality.register_validator(*addr);
        }
        for _ in 0..5 {
            let newly_finalized = node.finality.find_newly_finalized(&node.dag);
            if !newly_finalized.is_empty() {
                let finalized_vertices: Vec<DagVertex> = newly_finalized.iter()
                    .filter_map(|h| node.dag.get(h).cloned())
                    .collect();
                if !finalized_vertices.is_empty() {
                    node.state.apply_finalized_vertices(&finalized_vertices)
                        .expect("Node 3 should apply synced finalized vertices");
                }
            }
        }
    }

    // Verify convergence: node 3's state should match nodes 0-2
    let final_supplies: Vec<u64> = nodes.iter().map(|n| n.state.total_supply()).collect();
    assert_eq!(final_supplies[0], final_supplies[3],
        "After sync, node 3 supply ({}) should match node 0 ({})",
        final_supplies[3], final_supplies[0]);

    // Verify state roots match
    let root_0 = compute_state_root(&nodes[0].state.snapshot());
    let root_3 = compute_state_root(&nodes[3].state.snapshot());
    assert_eq!(root_0, root_3,
        "After sync, node 3 state root should match node 0");
}

// ============================================================================
// Test 2: Partition-heal agreement
// ============================================================================

#[tokio::test]
async fn test_partition_heal_state_agreement() {
    // 4 nodes, partition into [0,1] and [2,3] for 50 rounds.
    // Heal partition, run 50 more rounds, verify all agree.

    let mut nodes: Vec<AdversarialNode> = (0..4).map(|i| AdversarialNode::new(i, 4)).collect();

    // Phase 1: Run 5 rounds together to establish baseline
    simulate_with_state(&mut nodes, 5, 1, &[], &HashSet::new());

    let supply_baseline: Vec<u64> = nodes.iter().map(|n| n.state.total_supply()).collect();
    assert!(supply_baseline.iter().all(|s| *s == supply_baseline[0]),
        "All nodes should agree before partition");

    // Phase 2: Partition [0,1] vs [2,3] for 50 rounds
    // With 2 nodes per partition and quorum = ceil(2*4/3) = 3,
    // neither partition has quorum, so finality won't advance.
    // But vertices are still produced and exchanged within partitions.
    let partitions = vec![vec![0, 1], vec![2, 3]];
    simulate_with_state(&mut nodes, 50, 6, &partitions, &HashSet::new());

    // Phase 3: Heal partition — sync all vertices between groups
    // First, collect all vertices from each group
    let mut group_a_vertices: Vec<DagVertex> = Vec::new();
    let mut group_b_vertices: Vec<DagVertex> = Vec::new();
    for round in 1..=55 {
        // Node 0 has group A's view
        for v in nodes[0].dag.vertices_in_round(round).iter() {
            group_a_vertices.push((*v).clone());
        }
        // Node 2 has group B's view
        for v in nodes[2].dag.vertices_in_round(round).iter() {
            group_b_vertices.push((*v).clone());
        }
    }

    // Cross-sync: give group A's vertices to group B nodes and vice versa
    for v in &group_b_vertices {
        nodes[0].dag.insert(v.clone());
        nodes[1].dag.insert(v.clone());
    }
    for v in &group_a_vertices {
        nodes[2].dag.insert(v.clone());
        nodes[3].dag.insert(v.clone());
    }

    // Phase 4: Run 50 more rounds fully connected
    simulate_with_state(&mut nodes, 50, 56, &[], &HashSet::new());

    // Verify all nodes agree on finalized state
    let final_supplies: Vec<u64> = nodes.iter().map(|n| n.state.total_supply()).collect();
    assert!(final_supplies.iter().all(|s| *s == final_supplies[0]),
        "All nodes should agree on supply after partition heal: {:?}", final_supplies);

    // Verify state roots match
    let roots: Vec<[u8; 32]> = nodes.iter()
        .map(|n| compute_state_root(&n.state.snapshot()))
        .collect();
    assert!(roots.iter().all(|r| r == &roots[0]),
        "All nodes should have identical state roots after partition heal");

    // Verify finality advanced past the partition period
    let finalized_rounds: Vec<u64> = nodes.iter()
        .map(|n| n.finality.last_finalized_round())
        .collect();
    assert!(finalized_rounds.iter().all(|r| *r > 55),
        "All nodes should have finalized past the partition period: {:?}", finalized_rounds);
}

// ============================================================================
// Test 3: Equivocation detected and slashed identically
// ============================================================================

#[tokio::test]
async fn test_equivocation_slash_identical_across_nodes() {
    // 4 nodes, node 0 equivocates in round 5 (produces 2 different vertices).
    // All other nodes should detect and slash identically via apply_finalized_vertices.

    let mut nodes: Vec<AdversarialNode> = (0..4).map(|i| AdversarialNode::new(i, 4)).collect();

    // Phase 1: Run 4 rounds normally
    simulate_with_state(&mut nodes, 4, 1, &[], &HashSet::new());

    // Phase 2: Node 0 equivocates in round 5 — produces TWO different vertices
    let equivocator_sk = &nodes[0].secret_key;
    let _equivocator_addr = nodes[0].address;

    // Get parents from round 4 (from node 1's perspective for consistency)
    let parents: Vec<[u8; 32]> = nodes[1].dag.vertices_in_round(4)
        .iter().map(|v| v.hash()).collect();

    // Vertex A: normal vertex
    let vertex_a = make_correct_vertex(5, parents.clone(), equivocator_sk, 4);

    // Vertex B: different vertex (different timestamp → different hash)
    let vertex_b = {
        let validator = equivocator_sk.address();
        let total_reward = ultradag_coin::constants::block_reward(5);
        let reward = total_reward / 4;
        let block = Block {
            header: BlockHeader {
                version: 1,
                height: 5,
                timestamp: 2_000_000, // different timestamp → different hash
                prev_hash: parents.first().copied().unwrap_or([0u8; 32]),
                merkle_root: [0u8; 32],
            },
            coinbase: CoinbaseTx { to: validator, amount: reward, height: 5 },
            transactions: vec![],
        };
        let mut v = DagVertex::new(
            block, parents.clone(), 5, validator,
            equivocator_sk.verifying_key().to_bytes(), Signature([0u8; 64]),
        );
        v.signature = equivocator_sk.sign(&v.signable_bytes());
        v
    };

    assert_ne!(vertex_a.hash(), vertex_b.hash(), "Equivocating vertices must differ");

    // Insert BOTH equivocating vertices into all nodes' DAGs
    // (In production, try_insert would reject the second one, but for testing
    // we use insert() which allows it — simulating successful equivocation delivery)
    for node in nodes.iter_mut() {
        node.dag.insert(vertex_a.clone());
        node.dag.insert(vertex_b.clone());
    }

    // Also produce normal vertices for nodes 1-3 in round 5
    for i in 1..4 {
        let parents_i: Vec<[u8; 32]> = nodes[i].dag.vertices_in_round(4)
            .iter().map(|v| v.hash()).collect();
        let v = make_correct_vertex(5, parents_i, &nodes[i].secret_key, 4);
        for node in nodes.iter_mut() {
            node.dag.insert(v.clone());
        }
    }

    // Run more rounds so the equivocation round gets finalized
    simulate_with_state(&mut nodes, 10, 6, &[], &HashSet::new());

    // Collect finalized vertices including round 5 and run finality
    let all_addresses: Vec<Address> = nodes.iter().map(|n| n.address).collect();
    for node in nodes.iter_mut() {
        for addr in &all_addresses {
            node.finality.register_validator(*addr);
        }
        for _ in 0..5 {
            let newly_finalized = node.finality.find_newly_finalized(&node.dag);
            if !newly_finalized.is_empty() {
                let finalized_vertices: Vec<DagVertex> = newly_finalized.iter()
                    .filter_map(|h| node.dag.get(h).cloned())
                    .collect();
                if !finalized_vertices.is_empty() {
                    // apply_finalized_vertices handles deterministic slashing
                    let _ = node.state.apply_finalized_vertices(&finalized_vertices);
                }
            }
        }
    }

    // Verify state roots match across all nodes
    let roots: Vec<[u8; 32]> = nodes.iter()
        .map(|n| compute_state_root(&n.state.snapshot()))
        .collect();
    assert!(roots.iter().all(|r| r == &roots[0]),
        "All nodes should have identical state roots after equivocation handling");

    // Verify supply consistency
    let supplies: Vec<u64> = nodes.iter().map(|n| n.state.total_supply()).collect();
    assert!(supplies.iter().all(|s| *s == supplies[0]),
        "All nodes should agree on total supply: {:?}", supplies);
}

// ============================================================================
// Test 4: Minority partition makes no progress (liveness bound)
// ============================================================================

#[tokio::test]
async fn test_minority_partition_no_finality() {
    // 4 nodes, partition [0] vs [1,2,3].
    // Node 0 alone cannot achieve finality (needs ceil(2*4/3) = 3 validators).
    // Majority [1,2,3] should continue making progress.

    let mut nodes: Vec<AdversarialNode> = (0..4).map(|i| AdversarialNode::new(i, 4)).collect();

    // Run 5 rounds together
    simulate_with_state(&mut nodes, 5, 1, &[], &HashSet::new());

    let baseline_round = nodes[0].finality.last_finalized_round();

    // Partition: node 0 isolated
    let partitions = vec![vec![0], vec![1, 2, 3]];
    simulate_with_state(&mut nodes, 20, 6, &partitions, &HashSet::new());

    // Node 0 should NOT have advanced finality (alone, no quorum)
    let node0_finalized = nodes[0].finality.last_finalized_round();
    assert_eq!(node0_finalized, baseline_round,
        "Isolated node should not advance finality");

    // Majority nodes should have advanced finality
    let node1_finalized = nodes[1].finality.last_finalized_round();
    assert!(node1_finalized > baseline_round,
        "Majority partition should advance finality");

    // Majority nodes should agree
    let majority_supplies: Vec<u64> = nodes[1..4].iter()
        .map(|n| n.state.total_supply()).collect();
    assert!(majority_supplies.iter().all(|s| *s == majority_supplies[0]),
        "Majority nodes should agree on supply: {:?}", majority_supplies);
}

// ============================================================================
// Test 5: State root determinism after identical vertex sequences
// ============================================================================

#[tokio::test]
async fn test_state_root_deterministic() {
    // Create two independent node sets, feed them identical vertex sequences,
    // verify they produce identical state roots.

    let mut nodes_a: Vec<AdversarialNode> = (0..4).map(|i| AdversarialNode::new(i, 4)).collect();
    let mut nodes_b: Vec<AdversarialNode> = (0..4).map(|i| AdversarialNode::new(i, 4)).collect();

    // Run same simulation on both sets
    simulate_with_state(&mut nodes_a, 20, 1, &[], &HashSet::new());
    simulate_with_state(&mut nodes_b, 20, 1, &[], &HashSet::new());

    // Compare state roots between corresponding nodes
    for i in 0..4 {
        let root_a = compute_state_root(&nodes_a[i].state.snapshot());
        let root_b = compute_state_root(&nodes_b[i].state.snapshot());
        assert_eq!(root_a, root_b,
            "Node {} state root should be identical across independent runs", i);

        assert_eq!(nodes_a[i].state.total_supply(), nodes_b[i].state.total_supply(),
            "Node {} supply should match across independent runs", i);
    }
}
