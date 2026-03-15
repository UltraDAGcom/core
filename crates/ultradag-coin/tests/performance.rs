//! Performance benchmark tests for DAG-BFT finality checking.
//! With incremental descendant tracking, finality is O(1) per vertex.

use ultradag_coin::*;

fn make_vertex(
    proposer_sk: &SecretKey,
    round: u64,
    height: u64,
    txs: Vec<Transaction>,
    parents: Vec<[u8; 32]>,
) -> DagVertex {
    let proposer = proposer_sk.address();
    let total_fees: u64 = txs.iter().map(|tx| tx.fee()).sum();
    let coinbase = CoinbaseTx {
        to: proposer,
        amount: total_fees,
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
        if parents.is_empty() { vec![] } else { parents },
        round,
        proposer,
        proposer_sk.verifying_key().to_bytes(),
        Signature([0u8; 64]),
    );
    vertex.signature = proposer_sk.sign(&vertex.signable_bytes());
    vertex
}

/// Test 09: Finality check performance with 1000 vertices
/// With incremental descendant tracking: must complete in < 50ms
#[test]
fn test_09_finality_check_performance_1000_vertices() {
    let sks: Vec<_> = (0..4).map(|_| SecretKey::generate()).collect();
    let mut dag = BlockDag::new();
    let mut ft = FinalityTracker::new(3);

    for sk in &sks {
        ft.register_validator(sk.address());
    }

    // Build 250 rounds × 4 validators = 1000 vertices
    for round in 0u64..250 {
        for (i, sk) in sks.iter().enumerate() {
            let tips = dag.tips();
            let parents = if tips.is_empty() { vec![] } else { tips };
            let v = make_vertex(sk, round, round * 4 + i as u64, vec![], parents);
            dag.insert(v);
        }
    }
    assert_eq!(dag.len(), 1000);

    // Measure finality check performance
    let start = std::time::Instant::now();
    let mut total = 0;
    loop {
        let newly = ft.find_newly_finalized(&dag);
        if newly.is_empty() { break; }
        total += newly.len();
    }
    let elapsed = start.elapsed();

    assert!(total > 0, "Should have finalized some vertices");

    println!("1000 vertices: finalized {} in {}ms", total, elapsed.as_millis());

    // With incremental descendant tracking, must complete in < 50ms
    assert!(
        elapsed.as_millis() < 50,
        "Finality at 1K vertices took {}ms (limit 50ms with O(1) descendant tracking)", elapsed.as_millis()
    );
}

/// Test 10: Finality check performance with 10000 vertices
/// With incremental descendant tracking: must complete in < 500ms
#[test]
fn test_10_finality_check_performance_10000_vertices() {
    let sks: Vec<_> = (0..4).map(|_| SecretKey::generate()).collect();
    let mut dag = BlockDag::new();
    let mut ft = FinalityTracker::new(3);

    for sk in &sks {
        ft.register_validator(sk.address());
    }

    // Build 2500 rounds × 4 validators = 10000 vertices
    for round in 0u64..2500 {
        for (i, sk) in sks.iter().enumerate() {
            let tips = dag.tips();
            let parents = if tips.is_empty() { vec![] } else { tips };
            let v = make_vertex(sk, round, round * 4 + i as u64, vec![], parents);
            dag.insert(v);
        }
    }
    assert_eq!(dag.len(), 10000);

    // Measure finality check performance
    let start = std::time::Instant::now();
    let mut total = 0;
    loop {
        let newly = ft.find_newly_finalized(&dag);
        if newly.is_empty() { break; }
        total += newly.len();
    }
    let elapsed = start.elapsed();

    assert!(total > 0, "Should have finalized some vertices");

    println!("10000 vertices: finalized {} in {}ms", total, elapsed.as_millis());

    // With incremental descendant tracking, must complete in < 500ms
    assert!(
        elapsed.as_millis() < 500,
        "Finality at 10K vertices took {}ms (limit 500ms with O(1) descendant tracking)", elapsed.as_millis()
    );
}
