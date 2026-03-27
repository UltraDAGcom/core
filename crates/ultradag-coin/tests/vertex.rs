/// Module 3: DAG Vertices — Production-grade tests

use ultradag_coin::address::{SecretKey, Signature};
use ultradag_coin::block::header::BlockHeader;
use ultradag_coin::block::Block;
use ultradag_coin::consensus::dag::BlockDag;
use ultradag_coin::consensus::vertex::DagVertex;
use ultradag_coin::tx::CoinbaseTx;

fn make_vertex(uid: u64, round: u64, parents: Vec<[u8; 32]>, sk: &SecretKey) -> DagVertex {
    // Use current time for timestamp to pass validation (within 5 min past, 1 min future)
    let current_timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;
    
    let validator = sk.address();
    let mut block = Block {
        header: BlockHeader {
            version: 1,
            height: uid,
            timestamp: current_timestamp, // Use current time for validation
            prev_hash: parents.first().copied().unwrap_or([0u8; 32]),
            merkle_root: [0u8; 32],
        },
        coinbase: CoinbaseTx { to: validator, amount: 0, height: uid },
        transactions: vec![],
    };
    block.header.merkle_root = block.compute_merkle_root();
    let mut v = DagVertex::new(
        block, parents, round, validator,
        sk.verifying_key().to_bytes(), Signature([0u8; 64]),
    );
    v.signature = sk.sign(&v.signable_bytes());
    v
}

/// Valid signature from known validator is accepted.
/// Mutation: verify_signature always true → negative cases fail.
#[test]
fn valid_vertex_signature_accepted() {
    let sk = SecretKey::from_bytes([1u8; 32]);
    let v = make_vertex(1, 0, vec![], &sk);

    // POSITIVE: valid signature passes
    assert!(v.verify_signature());
    assert_eq!(v.validator, sk.address());
    assert_eq!(v.round, 0);
    assert_eq!(v.pub_key, sk.verifying_key().to_bytes());
}

/// Invalid signatures are rejected for multiple tampering scenarios.
/// Mutation: removing address binding check → wrong key accepted.
#[test]
fn invalid_signature_rejected() {
    let sk = SecretKey::from_bytes([2u8; 32]);
    let other_sk = SecretKey::from_bytes([3u8; 32]);
    let v = make_vertex(2, 1, vec![], &sk);

    // NEGATIVE: tampered round
    let mut bad = v.clone();
    bad.round = 999;
    assert!(!bad.verify_signature(), "tampered round must fail");

    // NEGATIVE: wrong pub_key (address mismatch)
    let mut bad = v.clone();
    bad.pub_key = other_sk.verifying_key().to_bytes();
    assert!(!bad.verify_signature(), "wrong pubkey must fail");

    // NEGATIVE: tampered parents
    let mut bad = v.clone();
    bad.parent_hashes = vec![[0xff; 32]];
    assert!(!bad.verify_signature(), "tampered parents must fail");

    // NEGATIVE: impersonated validator address
    let mut bad = v.clone();
    bad.validator = other_sk.address();
    assert!(!bad.verify_signature(), "impersonated address must fail");

    // NEGATIVE: garbage signature
    let mut bad = v.clone();
    bad.signature = Signature([0xBB; 64]);
    assert!(!bad.verify_signature(), "garbage signature must fail");

    // NEGATIVE: all-zero pubkey
    let mut bad = v.clone();
    bad.pub_key = [0u8; 32];
    assert!(!bad.verify_signature(), "zero pubkey must fail");
}

/// Equivocation: second vertex by same validator in same round is rejected.
/// Mutation: try_insert not checking equivocation → second insert succeeds.
#[test]
fn equivocation_second_vertex_rejected() {
    let sk = SecretKey::from_bytes([4u8; 32]);
    let mut dag = BlockDag::new();

    let v1 = make_vertex(10, 5, vec![], &sk);
    let h1 = v1.hash();

    // POSITIVE: first insert succeeds
    assert!(matches!(dag.try_insert(v1), Ok(true)));
    assert!(dag.get(&h1).is_some());
    assert_eq!(dag.vertices_in_round(5).len(), 1);

    // NEGATIVE: second vertex, same validator, same round → equivocation
    let v2 = make_vertex(11, 5, vec![], &sk);
    let h2 = v2.hash();
    assert_ne!(h1, h2, "different content should produce different hash");
    let result = dag.try_insert(v2);
    assert!(result.is_err(), "equivocating vertex must be rejected");
    assert!(dag.get(&h2).is_none(), "rejected vertex must not be in DAG");
    assert_eq!(dag.vertices_in_round(5).len(), 1, "still exactly 1 vertex in round 5");

    // POSITIVE: different validator, same round is fine
    let other_sk = SecretKey::from_bytes([5u8; 32]);
    let v3 = make_vertex(12, 5, vec![], &other_sk);
    assert!(matches!(dag.try_insert(v3), Ok(true)));
    assert_eq!(dag.vertices_in_round(5).len(), 2);

    // NEGATIVE: after equivocation, validator is marked Byzantine — rejected in all rounds
    let v4 = make_vertex(13, 6, vec![h1], &sk);
    assert!(matches!(dag.try_insert(v4), Ok(false)), "Byzantine validator should be rejected");
}

/// Vertex hash is deterministic — same fields always produce same hash.
/// Mutation: hash including random nonce → different hashes.
#[test]
fn vertex_hash_deterministic() {
    let sk = SecretKey::from_bytes([6u8; 32]);

    let make = || {
        let validator = sk.address();
        let block = Block {
            header: BlockHeader {
                version: 1, height: 42,
                timestamp: 1_000_042,
                prev_hash: [0u8; 32],
                merkle_root: [0u8; 32],
            },
            coinbase: CoinbaseTx { to: validator, amount: 0, height: 42 },
            transactions: vec![],
        };
        let mut v = DagVertex::new(
            block, vec![], 7, validator,
            sk.verifying_key().to_bytes(), Signature([0u8; 64]),
        );
        v.signature = sk.sign(&v.signable_bytes());
        v
    };

    let v1 = make();
    let v2 = make();
    assert_eq!(v1.hash(), v2.hash(), "identical fields must produce identical hash");

    // NEGATIVE: different height → different hash
    let v3 = make_vertex(43, 7, vec![], &sk);
    assert_ne!(v1.hash(), v3.hash());
}

/// Signable bytes cover all critical fields.
/// Mutation: signable_bytes omitting round → round tampering not caught.
#[test]
fn signable_bytes_covers_all_fields() {
    let sk = SecretKey::from_bytes([7u8; 32]);
    let parents = vec![[1u8; 32], [2u8; 32]];
    let v = make_vertex(50, 3, parents.clone(), &sk);
    let sb = v.signable_bytes();

    // Should contain: network_id(19) + "vertex"(6) + block_hash(32) + parent_count(4) + parent1(32) + parent2(32) + round(8) + validator(20)
    let nid_len = ultradag_coin::constants::NETWORK_ID.len();
    let disc_len = b"vertex".len();
    let expected_len = nid_len + disc_len + 32 + 4 + 32 * parents.len() + 8 + 20;
    assert_eq!(sb.len(), expected_len);

    // Verify network ID is at the start
    assert_eq!(&sb[0..nid_len], ultradag_coin::constants::NETWORK_ID);

    // Verify type discriminator follows
    assert_eq!(&sb[nid_len..nid_len + disc_len], b"vertex");

    // Verify block hash follows
    let off = nid_len + disc_len;
    assert_eq!(&sb[off..off + 32], &v.block.hash());

    // Verify parent count prefix (4 bytes LE) then parents are included
    let poff = off + 32; // after block hash
    assert_eq!(&sb[poff..poff + 4], &2u32.to_le_bytes());
    assert_eq!(&sb[poff + 4..poff + 36], &parents[0]);
    assert_eq!(&sb[poff + 36..poff + 68], &parents[1]);

    // Verify round bytes (after parent_count + parents)
    let roff = poff + 4 + 32 * parents.len();
    assert_eq!(&sb[roff..roff + 8], &3u64.to_le_bytes());

    // Verify validator address (20 bytes)
    assert_eq!(&sb[roff + 8..roff + 28], &sk.address().0);
}

/// Vertex with valid parent references builds correct DAG topology.
/// Mutation: insert not tracking parent→child edges → children_of fails.
#[test]
fn vertex_parent_references_build_topology() {
    let sk1 = SecretKey::from_bytes([8u8; 32]);
    let sk2 = SecretKey::from_bytes([9u8; 32]);

    let mut dag = BlockDag::new();

    let v1 = make_vertex(60, 0, vec![], &sk1);
    let h1 = v1.hash();
    dag.insert(v1);

    let v2 = make_vertex(61, 0, vec![], &sk2);
    let h2 = v2.hash();
    dag.insert(v2);

    // v3 references both v1 and v2
    let v3 = make_vertex(62, 1, vec![h1, h2], &sk1);
    let h3 = v3.hash();
    dag.insert(v3);

    // POSITIVE: v3's parents are h1 and h2
    let v3_ref = dag.get(&h3).unwrap();
    assert_eq!(v3_ref.parent_hashes.len(), 2);
    assert!(v3_ref.parent_hashes.contains(&h1));
    assert!(v3_ref.parent_hashes.contains(&h2));

    // POSITIVE: v1 and v2 are ancestors of v3
    let ancestors = dag.ancestors(&h3);
    assert!(ancestors.contains(&h1));
    assert!(ancestors.contains(&h2));
    assert_eq!(ancestors.len(), 2);

    // POSITIVE: v3 is descendant of both
    assert!(dag.descendants(&h1).contains(&h3));
    assert!(dag.descendants(&h2).contains(&h3));

    // NEGATIVE: v1 is not ancestor of v2 (they're independent)
    assert!(!dag.ancestors(&h2).contains(&h1));
}

/// Duplicate hash insertion returns Ok(false), not error.
/// Mutation: try_insert treating duplicate as equivocation → error instead of false.
#[test]
fn duplicate_hash_returns_ok_false() {
    let sk = SecretKey::from_bytes([10u8; 32]);
    let mut dag = BlockDag::new();

    let v = make_vertex(70, 0, vec![], &sk);
    let h = v.hash();

    // First insert
    assert!(matches!(dag.try_insert(v.clone()), Ok(true)));
    assert_eq!(dag.len(), 1);

    // Duplicate insert — same hash, same content
    assert!(matches!(dag.try_insert(v), Ok(false)));
    assert_eq!(dag.len(), 1, "duplicate should not increase count");
    assert!(dag.get(&h).is_some());
}
