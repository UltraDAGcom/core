use ultradag_coin::address::SecretKey;
use ultradag_coin::block::header::BlockHeader;
use ultradag_coin::block::Block;
use ultradag_coin::consensus::{BlockDag, DagVertex};
use ultradag_coin::tx::CoinbaseTx;

fn make_vertex(nonce: u64, round: u64, parents: Vec<[u8; 32]>, sk: &SecretKey) -> DagVertex {
    // Use current time for timestamp to pass validation (within 5 min past, 1 min future)
    let current_timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;
    
    let header = BlockHeader {
        version: 1,
        height: round,
        timestamp: current_timestamp, // Use current time for validation
        prev_hash: [0u8; 32],
        merkle_root: [0u8; 32],
    };
    let coinbase = CoinbaseTx {
        to: sk.address(),
        amount: 0,
        height: round,
    };
    let mut block = Block {
        header,
        coinbase,
        transactions: vec![],
    };
    // Add nonce to make vertices unique (small offset to keep within validation window)
    block.header.timestamp += (nonce % 10) as i64;

    let validator = sk.address();
    let pub_key = sk.verifying_key().to_bytes();

    let mut vertex = DagVertex::new(
        block.clone(),
        parents,
        round,
        validator,
        pub_key,
        ultradag_coin::address::Signature([0u8; 64]), // Placeholder
    );

    // Sign the vertex
    let sig = sk.sign(&vertex.signable_bytes());
    vertex.signature = sig;
    
    vertex
}

#[test]
fn test_equivocation_evidence_survives_pruning() {
    let sk1 = SecretKey::generate();
    let sk2 = SecretKey::generate();
    let mut dag = BlockDag::new();

    // Insert genesis
    let genesis = make_vertex(0, 0, vec![], &sk1);
    dag.insert(genesis.clone());

    // Round 1: sk1 produces valid vertex
    let v1 = make_vertex(1, 1, vec![genesis.hash()], &sk1);
    dag.insert(v1.clone());

    // Round 5: sk2 equivocates (two different vertices, same round)
    let equivocating_v1 = make_vertex(100, 5, vec![v1.hash()], &sk2);
    let equivocating_v2 = make_vertex(101, 5, vec![v1.hash()], &sk2);

    // Insert first equivocating vertex
    assert!(dag.try_insert(equivocating_v1.clone()).is_ok());

    // Insert second equivocating vertex - should detect equivocation
    let result = dag.try_insert(equivocating_v2.clone());
    assert!(result.is_err(), "Should detect equivocation");

    // Verify evidence is stored
    assert!(dag.is_byzantine(&sk2.address()));
    let evidence = dag.get_permanent_evidence(&sk2.address());
    assert!(evidence.is_some(), "Permanent evidence should be stored");
    let evidence = &evidence.unwrap()[0];
    assert_eq!(evidence.validator, sk2.address());
    assert_eq!(evidence.round, 5);

    // Prune old rounds. Use 1504 so new_floor = 1504 - 500 = 1004 > evidence round 5.
    // Wait - we want evidence to SURVIVE, so use a round that keeps evidence.
    // With PRUNING_HORIZON=500, use round 504 so floor = 4 < evidence round 5.
    let pruned_count = dag.prune_old_rounds(504);
    assert!(pruned_count > 0, "Should have pruned some vertices");

    // Verify evidence still exists after pruning
    let evidence_after_prune = dag.get_permanent_evidence(&sk2.address());
    assert!(evidence_after_prune.is_some(), "Evidence should survive pruning");
    let evidence_after = &evidence_after_prune.unwrap()[0];
    assert_eq!(evidence_after.validator, sk2.address());
    assert_eq!(evidence_after.round, 5);
    assert_eq!(evidence_after.vertex_hash_1, equivocating_v1.hash());
    assert_eq!(evidence_after.vertex_hash_2, equivocating_v2.hash());

    // Verify validator is still marked Byzantine
    assert!(dag.is_byzantine(&sk2.address()));
}

#[test]
fn test_evidence_persists_across_save_load() {
    let sk1 = SecretKey::generate();
    let sk2 = SecretKey::generate();
    let mut dag = BlockDag::new();

    // Insert genesis
    let genesis = make_vertex(0, 0, vec![], &sk1);
    dag.insert(genesis.clone());

    // Create equivocation
    let v1 = make_vertex(100, 5, vec![genesis.hash()], &sk2);
    let v2 = make_vertex(101, 5, vec![genesis.hash()], &sk2);
    
    dag.try_insert(v1.clone()).ok();
    let _ = dag.try_insert(v2.clone()); // Triggers equivocation

    // Verify evidence exists
    assert!(dag.get_permanent_evidence(&sk2.address()).is_some());

    // Save and reload
    let temp_dir = std::env::temp_dir();
    let path = temp_dir.join("test_evidence_persistence.json");
    dag.save(&path).unwrap();
    
    let loaded_dag = BlockDag::load(&path).unwrap();
    
    // Verify evidence survived save/load
    let evidence = loaded_dag.get_permanent_evidence(&sk2.address());
    assert!(evidence.is_some(), "Evidence should persist across save/load");
    assert_eq!(evidence.unwrap()[0].validator, sk2.address());
    assert!(loaded_dag.is_byzantine(&sk2.address()));

    std::fs::remove_file(&path).ok();
}

#[test]
fn test_all_evidence_retrieval() {
    let sk1 = SecretKey::generate();
    let sk2 = SecretKey::generate();
    let sk3 = SecretKey::generate();
    let mut dag = BlockDag::new();

    // Insert genesis
    let genesis = make_vertex(0, 0, vec![], &sk1);
    dag.insert(genesis.clone());

    // Create equivocation from sk2
    let v1 = make_vertex(100, 5, vec![genesis.hash()], &sk2);
    let v2 = make_vertex(101, 5, vec![genesis.hash()], &sk2);
    dag.try_insert(v1.clone()).ok();
    let _ = dag.try_insert(v2.clone());

    // Create equivocation from sk3
    let v3 = make_vertex(200, 7, vec![genesis.hash()], &sk3);
    let v4 = make_vertex(201, 7, vec![genesis.hash()], &sk3);
    dag.try_insert(v3.clone()).ok();
    let _ = dag.try_insert(v4.clone());

    // Retrieve all evidence
    let all_evidence = dag.all_evidence();
    assert_eq!(all_evidence.len(), 2, "Should have evidence for 2 validators");

    let validators: Vec<_> = all_evidence.iter().map(|e| e.validator).collect();
    assert!(validators.contains(&sk2.address()));
    assert!(validators.contains(&sk3.address()));
}
