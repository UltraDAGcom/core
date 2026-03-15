/// Module 6: Deterministic Ordering — Production-grade tests

use ultradag_coin::address::{SecretKey, Signature};
use ultradag_coin::block::header::BlockHeader;
use ultradag_coin::block::Block;
use ultradag_coin::consensus::dag::BlockDag;
use ultradag_coin::consensus::ordering::order_vertices;
use ultradag_coin::consensus::vertex::DagVertex;
use ultradag_coin::tx::CoinbaseTx;

fn make_vertex(uid: u64, round: u64, parents: Vec<[u8; 32]>, sk: &SecretKey) -> DagVertex {
    let validator = sk.address();
    let block = Block {
        header: BlockHeader {
            version: 1, height: uid,
            timestamp: 1_000_000 + uid as i64,
            prev_hash: parents.first().copied().unwrap_or([0u8; 32]),
            merkle_root: [0u8; 32],
        },
        coinbase: CoinbaseTx { to: validator, amount: 0, height: uid },
        transactions: vec![],
    };
    let mut v = DagVertex::new(
        block, parents, round, validator,
        sk.verifying_key().to_bytes(), Signature([0u8; 64]),
    );
    v.signature = sk.sign(&v.signable_bytes());
    v
}

fn make_sks(n: u8) -> Vec<SecretKey> {
    (0..n).map(|i| {
        let mut seed = [0u8; 32];
        seed[0] = i + 50;
        SecretKey::from_bytes(seed)
    }).collect()
}

/// Two independent orderings of the same set produce identical results.
/// Mutation: sort using random tiebreaker → different orderings.
#[test]
fn two_independent_orderings_identical() {
    let sks = make_sks(4);
    let mut dag = BlockDag::new();

    // Build 2 rounds: 4 vertices each
    let mut round0_hashes = vec![];
    for (i, sk) in sks.iter().enumerate() {
        let v = make_vertex(i as u64, 0, vec![], sk);
        round0_hashes.push(v.hash());
        dag.insert(v);
    }

    let mut round1_hashes = vec![];
    for (i, sk) in sks.iter().enumerate() {
        let v = make_vertex(10 + i as u64, 1, round0_hashes.clone(), sk);
        round1_hashes.push(v.hash());
        dag.insert(v);
    }

    let all_hashes: Vec<[u8; 32]> = round0_hashes.iter().chain(round1_hashes.iter()).copied().collect();

    // Order forward
    let order_a = order_vertices(&all_hashes, &dag);
    // Order reversed input
    let reversed: Vec<[u8; 32]> = all_hashes.iter().rev().copied().collect();
    let order_b = order_vertices(&reversed, &dag);

    assert_eq!(order_a.len(), order_b.len());
    assert_eq!(order_a.len(), 8);

    for i in 0..order_a.len() {
        assert_eq!(order_a[i].hash(), order_b[i].hash(),
            "position {i} must match regardless of input order");
    }
}

/// Ordering is stable: same input → same output.
/// Mutation: sort being unstable with random tiebreaker → different each time.
#[test]
fn ordering_stable() {
    let sks = make_sks(3);
    let mut dag = BlockDag::new();

    let mut hashes = vec![];
    for (i, sk) in sks.iter().enumerate() {
        let v = make_vertex(i as u64, 0, vec![], sk);
        hashes.push(v.hash());
        dag.insert(v);
    }

    let o1 = order_vertices(&hashes, &dag);
    let o2 = order_vertices(&hashes, &dag);
    let o3 = order_vertices(&hashes, &dag);

    for i in 0..3 {
        assert_eq!(o1[i].hash(), o2[i].hash());
        assert_eq!(o2[i].hash(), o3[i].hash());
    }
}

/// Ordering respects causal order: parent always before child.
/// Mutation: sort ignoring round → child could appear before parent.
#[test]
fn ordering_respects_causal_order() {
    let sks = make_sks(3);
    let mut dag = BlockDag::new();

    let v0 = make_vertex(0, 0, vec![], &sks[0]);
    let h0 = v0.hash();
    dag.insert(v0);

    let v1 = make_vertex(1, 1, vec![h0], &sks[1]);
    let h1 = v1.hash();
    dag.insert(v1);

    let v2 = make_vertex(2, 2, vec![h1], &sks[2]);
    let h2 = v2.hash();
    dag.insert(v2);

    // Deliberately pass in reverse causal order
    let ordered = order_vertices(&[h2, h1, h0], &dag);
    assert_eq!(ordered.len(), 3);

    // POSITIVE: causal order maintained
    assert_eq!(ordered[0].hash(), h0, "root must be first");
    assert_eq!(ordered[1].hash(), h1, "middle must be second");
    assert_eq!(ordered[2].hash(), h2, "leaf must be last");

    // Verify rounds are non-decreasing
    for window in ordered.windows(2) {
        assert!(window[0].round <= window[1].round,
            "round must be non-decreasing in ordered output");
    }
}

/// Every vertex appears exactly once.
/// Mutation: order_vertices including duplicates → count differs.
#[test]
fn ordering_total_every_vertex_once() {
    let sks = make_sks(4);
    let mut dag = BlockDag::new();

    let mut all_hashes = vec![];
    // 3 rounds, 4 vertices each
    let mut prev = vec![];
    for round in 0..3u64 {
        let mut round_hashes = vec![];
        for (i, sk) in sks.iter().enumerate() {
            let v = make_vertex(round * 10 + i as u64, round, prev.clone(), sk);
            let h = v.hash();
            dag.insert(v);
            round_hashes.push(h);
            all_hashes.push(h);
        }
        prev = round_hashes;
    }

    let ordered = order_vertices(&all_hashes, &dag);
    assert_eq!(ordered.len(), 12, "all 12 vertices must appear");

    // Check uniqueness
    let ordered_hashes: Vec<[u8; 32]> = ordered.iter().map(|v| v.hash()).collect();
    let unique: std::collections::HashSet<[u8; 32]> = ordered_hashes.iter().copied().collect();
    assert_eq!(unique.len(), 12, "all must be unique");

    // Every input hash must appear
    for h in &all_hashes {
        assert!(ordered_hashes.contains(h), "every input hash must appear in output");
    }
}

/// Adding one more vertex produces a strict extension — no reordering of existing.
/// Mutation: sort changing order of earlier elements when new one added → prefix differs.
#[test]
fn adding_vertex_is_strict_extension() {
    let sks = make_sks(3);
    let mut dag = BlockDag::new();

    let v0 = make_vertex(30, 0, vec![], &sks[0]);
    let h0 = v0.hash();
    dag.insert(v0);

    let v1 = make_vertex(31, 0, vec![], &sks[1]);
    let h1 = v1.hash();
    dag.insert(v1);

    // Order with 2 vertices — collect hashes to release borrow
    let order_before: Vec<[u8; 32]> = order_vertices(&[h0, h1], &dag)
        .iter().map(|v| v.hash()).collect();
    assert_eq!(order_before.len(), 2);

    // Add a third vertex in a later round
    let v2 = make_vertex(32, 1, vec![h0, h1], &sks[2]);
    let h2 = v2.hash();
    dag.insert(v2);

    // Order with 3 vertices
    let order_after: Vec<[u8; 32]> = order_vertices(&[h0, h1, h2], &dag)
        .iter().map(|v| v.hash()).collect();
    assert_eq!(order_after.len(), 3);

    // The first two elements must be the same as before (strict extension)
    assert_eq!(order_before[0], order_after[0],
        "first element must not change when appending");
    assert_eq!(order_before[1], order_after[1],
        "second element must not change when appending");

    // New vertex should be last (it's in a later round)
    assert_eq!(order_after[2], h2);
}

/// Empty input returns empty output.
/// Mutation: order_vertices panicking on empty → test catches.
#[test]
fn ordering_empty_input() {
    let dag = BlockDag::new();
    let ordered = order_vertices(&[], &dag);
    assert!(ordered.is_empty());
}

/// Single vertex input returns that vertex.
#[test]
fn ordering_single_vertex() {
    let sk = SecretKey::from_bytes([99u8; 32]);
    let mut dag = BlockDag::new();
    let v = make_vertex(40, 0, vec![], &sk);
    let h = v.hash();
    dag.insert(v);

    let ordered = order_vertices(&[h], &dag);
    assert_eq!(ordered.len(), 1);
    assert_eq!(ordered[0].hash(), h);
    assert_eq!(ordered[0].round, 0);
}
