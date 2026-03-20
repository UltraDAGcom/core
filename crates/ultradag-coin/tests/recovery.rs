//! Recovery and mathematical verification tests.

use ultradag_coin::*;

fn make_vertex(
    proposer_sk: &SecretKey,
    round: u64,
    height: u64,
    txs: Vec<Transaction>,
    parents: Vec<[u8; 32]>,
) -> DagVertex {
    let proposer = proposer_sk.address();
    let coinbase = CoinbaseTx {
        to: proposer,
        amount: 0,
        height,
    };
    let block = Block {
        header: BlockHeader {
            version: 1,
            height,
            timestamp: 1_000_000 + round as i64,
            prev_hash: parents.first().copied().unwrap_or([0u8; 32]),
            merkle_root: [0u8; 32],
        },
        coinbase,
        transactions: txs,
    };
    let mut vertex = DagVertex::new(
        block,
        if parents.is_empty() { vec![[0u8; 32]] } else { parents },
        round,
        proposer,
        proposer_sk.verifying_key().to_bytes(),
        Signature([0u8; 64]),
    );
    vertex.signature = proposer_sk.sign(&vertex.signable_bytes());
    vertex
}

/// Test 11: State behind DAG recovery
#[test]
fn test_11_state_behind_dag_recovery() {
    let sk = SecretKey::generate();
    let tmp = std::env::temp_dir().join("ultradag_test_recovery");
    std::fs::create_dir_all(&tmp).unwrap();

    // Build a DAG with 20 rounds
    let mut dag = BlockDag::new();
    let mut all_vertices = Vec::new();
    for r in 0..20 {
        let v = make_vertex(&sk, r, r, vec![], vec![]);
        dag.insert(v.clone());
        all_vertices.push(v);
    }

    // Apply all 20 rounds to state
    let mut state_full = StateEngine::new();
    for v in &all_vertices {
        state_full.apply_vertex(v).unwrap();
    }
    let expected_balance = state_full.balance(&sk.address());
    let expected_supply = state_full.total_supply();

    // Save state at round 10 (simulate crash)
    let mut state_partial = StateEngine::new();
    for v in &all_vertices[0..11] {
        state_partial.apply_vertex(v).unwrap();
    }
    state_partial.save(&tmp.join("state.redb")).unwrap();

    // Save full DAG
    dag.save(&tmp.join("dag.bin")).unwrap();

    // Load the round-10 state
    let mut loaded_state = StateEngine::load(&tmp.join("state.redb")).unwrap();
    assert_eq!(loaded_state.last_finalized_round(), Some(10));

    // Re-apply rounds 11-20 from the persisted DAG
    for v in &all_vertices[11..20] {
        loaded_state.apply_vertex(v).unwrap();
    }

    // Verify final state matches the expected balances
    assert_eq!(loaded_state.balance(&sk.address()), expected_balance);
    assert_eq!(loaded_state.total_supply(), expected_supply);
    assert_eq!(loaded_state.last_finalized_round(), Some(19));

    std::fs::remove_dir_all(&tmp).ok();
}

/// Test 12: Coinbase reward sum mathematical verification
/// The raw block_reward() schedule sums to ~21M UDAG, but StateEngine enforces
/// MAX_SUPPLY_SATS cap. Faucet prefund comes from the cap, reducing mining rewards.
#[test]
fn test_12_coinbase_reward_sum_equals_21m() {
    // Compute total supply from block rewards mathematically:
    // Each halving period has HALVING_INTERVAL rounds at the same reward.
    // reward_per_round halves each period until it reaches 0 (after 64 halvings).
    let mut total_reward: u64 = 0;
    for halving in 0..64 {
        let reward = block_reward(halving * HALVING_INTERVAL);
        if reward == 0 {
            break;
        }
        total_reward = total_reward.saturating_add(HALVING_INTERVAL.saturating_mul(reward));
    }

    println!("Total reward from schedule (uncapped): {} sats = {} UDAG",
        total_reward, total_reward / COIN);
    println!("MAX_SUPPLY_SATS: {} sats = {} UDAG",
        MAX_SUPPLY_SATS, MAX_SUPPLY_SATS / COIN);

    // The raw schedule should sum to ~21M (within 1% due to halving rounding)
    let diff = total_reward.abs_diff(MAX_SUPPLY_SATS);

    let max_allowed_diff = MAX_SUPPLY_SATS / 100; // 1% tolerance
    assert!(
        diff < max_allowed_diff,
        "Raw reward schedule deviates too much from MAX_SUPPLY: {} vs {} (diff: {} sats)",
        total_reward, MAX_SUPPLY_SATS, diff
    );

    // Verify that after 64 halvings, reward is 0
    assert_eq!(block_reward(64 * HALVING_INTERVAL), 0);
    assert_eq!(block_reward(64 * HALVING_INTERVAL + 1), 0);
}
