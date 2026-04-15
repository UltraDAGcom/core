//! Test that phantom validator registrations do not break finality
//! when configured_validators is set.

use ultradag_coin::address::SecretKey;
use ultradag_coin::block::header::BlockHeader;
use ultradag_coin::block::Block;
use ultradag_coin::consensus::dag::BlockDag;
use ultradag_coin::consensus::finality::FinalityTracker;
use ultradag_coin::consensus::vertex::DagVertex;
use ultradag_coin::tx::CoinbaseTx;
use ultradag_coin::address::Signature;

fn make_vertex(
    nonce: u64,
    round: u64,
    parents: Vec<[u8; 32]>,
    sk: &SecretKey,
) -> DagVertex {
    let validator = sk.address();
    let block = Block {
        header: BlockHeader {
            version: 1,
            height: 0,
            timestamp: 1_000_000 + nonce as i64,
            prev_hash: parents.first().copied().unwrap_or([0u8; 32]),
            merkle_root: [0u8; 32],
        },
        coinbase: CoinbaseTx {
            to: validator,
            amount: 0,
            height: 0,
        },
        transactions: vec![],
    };
    let mut v = DagVertex::new(
        block,
        parents,
        round,
        validator,
        sk.verifying_key().to_bytes(),
        Signature([0u8; 64]),
    );
    v.signature = sk.sign(&v.signable_bytes());
    v
}

/// Run 4 validators through `rounds` rounds, producing one vertex each per round.
/// Returns the DAG and the hashes of all vertices.
fn build_dag(
    sks: &[SecretKey],
    rounds: u64,
    dag: &mut BlockDag,
) -> Vec<[u8; 32]> {
    let mut all_hashes = Vec::new();
    let mut nonce = 0u64;

    for round in 0..rounds {
        let tips = dag.tips();
        let parents = if tips.is_empty() { vec![] } else { tips };

        for sk in sks {
            nonce += 1;
            let v = make_vertex(nonce, round, parents.clone(), sk);
            let h = v.hash();
            dag.insert(v);
            all_hashes.push(h);
        }
    }

    all_hashes
}

#[test]
fn phantom_validator_does_not_break_finality_with_configured_count() {
    let sks: Vec<SecretKey> = (0..4).map(|_| SecretKey::generate()).collect();

    let mut dag = BlockDag::new();
    let mut finality = FinalityTracker::new(3);

    // Set configured validators to 4 — this is the fix
    finality.set_configured_validators(4);

    // Register the 4 real validators
    for sk in &sks {
        finality.register_validator(sk.address());
    }

    // Run 20 rounds with 4 validators
    build_dag(&sks, 20, &mut dag);

    // Run finality passes
    let mut total_finalized = 0;
    loop {
        let newly = finality.find_newly_finalized(&dag);
        if newly.is_empty() {
            break;
        }
        total_finalized += newly.len();
    }

    // With 4 validators over 20 rounds, many vertices should be finalized
    assert!(total_finalized > 0, "should finalize vertices before phantom registration");
    let finalized_before = finality.finalized_count();

    // Now simulate a phantom validator (5th address registered from stale data)
    let phantom_sk = SecretKey::generate();
    finality.register_validator(phantom_sk.address());

    // Verify the validator count is now 5
    assert_eq!(finality.validator_count(), 5);

    // But the quorum threshold should still be based on configured_count=4
    // ceil(2*4/3) = ceil(8/3) = 3
    assert_eq!(finality.finality_threshold(), 3, "threshold should stay at 3 despite phantom");

    // Continue producing vertices with the 4 real validators (rounds 20-25)
    let mut nonce = 1000u64;
    for round in 20..25 {
        let tips = dag.tips();
        for sk in &sks {
            nonce += 1;
            let v = make_vertex(nonce, round, tips.clone(), sk);
            dag.insert(v);
        }
    }

    // Finality should continue working
    let mut new_finalized = 0;
    loop {
        let newly = finality.find_newly_finalized(&dag);
        if newly.is_empty() {
            break;
        }
        new_finalized += newly.len();
    }

    assert!(new_finalized > 0, "finality must continue after phantom registration");
    assert!(
        finality.finalized_count() > finalized_before,
        "more vertices should be finalized after additional rounds"
    );
}

#[test]
fn permissionless_mode_refuses_finality() {
    // SECURITY (GHSA-rprp-wjrh-hx7g): in fully permissionless mode (no
    // configured_validators, no allowlist), finality must fail closed.
    // An attacker minting fresh keys and producing signed vertices cannot
    // be distinguished from honest validators, so any threshold derived
    // from "validators we've seen" is sybil-gameable. The tracker now
    // returns threshold=usize::MAX in this mode — operators must set
    // --validators or --validator-key.
    let sks: Vec<SecretKey> = (0..4).map(|_| SecretKey::generate()).collect();

    let mut dag = BlockDag::new();
    let mut finality = FinalityTracker::new(3);
    // NO set_configured_validators, NO set_allowed_validators — permissionless.

    for sk in &sks {
        finality.register_validator(sk.address());
    }

    build_dag(&sks, 20, &mut dag);

    let mut total = 0;
    loop {
        let newly = finality.find_newly_finalized(&dag);
        if newly.is_empty() { break; }
        total += newly.len();
    }

    assert_eq!(finality.finality_threshold(), usize::MAX);
    assert_eq!(total, 0, "permissionless mode must not finalize anything");
}

#[test]
fn producer_backed_phantom_cannot_stall_finality() {
    // Regression test for GHSA-rprp-wjrh-hx7g (Sumitshah00, 2026-04-13).
    //
    // BEFORE FIX: with 4 honest validators + unconfigured mode, an attacker
    // that produces 3 signed vertices from 3 fresh keys inflated the
    // adaptive threshold to ceil(2*7/3)=5. Only 4 honest producers remained,
    // so finality stalled forever at round < attack_round.
    //
    // AFTER FIX: operators must declare topology. With set_configured_validators(4),
    // the upper bound is pinned at 4, phantoms cannot raise the threshold, and
    // honest-only rounds finalize cleanly past the attack round.
    let honest: Vec<SecretKey> = (0..4).map(|_| SecretKey::generate()).collect();
    let mut dag = BlockDag::new();
    let mut finality = FinalityTracker::new(3);
    finality.set_configured_validators(4);
    for sk in &honest {
        finality.register_validator(sk.address());
    }

    build_dag(&honest, 20, &mut dag);
    loop {
        if finality.find_newly_finalized(&dag).is_empty() { break; }
    }
    let finalized_before = finality.last_finalized_round();
    assert!(finalized_before > 0, "baseline rounds should finalize");

    // Attack: 3 phantom producers each sign exactly one vertex in round 20.
    let phantoms: Vec<SecretKey> = (0..3).map(|_| SecretKey::generate()).collect();
    let attack_round = 20u64;
    let attack_parents = dag.tips();
    let mut nonce = 10_000u64;
    for sk in &phantoms {
        nonce += 1;
        dag.insert(make_vertex(nonce, attack_round, attack_parents.clone(), sk));
        finality.register_validator(sk.address());
    }

    // Threshold stays pinned to configured count (4) -> ceil(8/3) = 3.
    assert_eq!(finality.finality_threshold(), 3,
        "configured topology must pin threshold despite phantom producers");

    // Honest-only rounds after the attack.
    for round in (attack_round + 1)..(attack_round + 11) {
        let tips = dag.tips();
        for sk in &honest {
            nonce += 1;
            dag.insert(make_vertex(nonce, round, tips.clone(), sk));
        }
    }
    loop {
        if finality.find_newly_finalized(&dag).is_empty() { break; }
    }

    assert!(
        finality.last_finalized_round() > attack_round,
        "finality must progress past attack_round={} (got {})",
        attack_round,
        finality.last_finalized_round()
    );
}
