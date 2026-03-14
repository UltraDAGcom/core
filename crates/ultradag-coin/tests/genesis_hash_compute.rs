#[test]
fn print_genesis_hash() {
    let state = ultradag_coin::StateEngine::new_with_genesis();
    let snapshot = state.snapshot();
    let state_root = ultradag_coin::consensus::checkpoint::compute_state_root(&snapshot);

    let genesis_checkpoint = ultradag_coin::consensus::checkpoint::Checkpoint {
        round: 0,
        state_root,
        dag_tip: [0u8; 32],
        total_supply: state.total_supply(),
        prev_checkpoint_hash: [0u8; 32],
        signatures: vec![],
    };

    let hash = ultradag_coin::consensus::checkpoint::compute_checkpoint_hash(&genesis_checkpoint);

    let hex: String = hash.iter().map(|b| format!("0x{:02x}", b)).collect::<Vec<_>>().join(", ");
    println!("\nGENESIS_CHECKPOINT_HASH = [{}]", hex);

    // Verify consistency
    let hash2 = ultradag_coin::consensus::checkpoint::compute_checkpoint_hash(&genesis_checkpoint);
    assert_eq!(hash, hash2, "Genesis hash must be deterministic");
}
