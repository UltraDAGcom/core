/// Computes and prints the genesis checkpoint hash for the current build configuration.
/// For mainnet hash: `cargo test --features mainnet test_compute_genesis_hash -- --nocapture`
/// For testnet hash: `cargo test test_compute_genesis_hash -- --nocapture`
#[test]
fn test_compute_genesis_hash() {
    // For mainnet: ULTRADAG_DEV_KEY must be set in the environment.
    // If not set, use a deterministic test key for CI/development.
    #[cfg(feature = "mainnet")]
    if std::env::var("ULTRADAG_DEV_KEY").is_err() {
        std::env::set_var("ULTRADAG_DEV_KEY", "0000000000000000000000000000000000000000000000000000000000000001");
    }
    
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

    let computed = ultradag_coin::consensus::checkpoint::compute_checkpoint_hash(&genesis_checkpoint);
    let hex: String = computed.iter().map(|b| format!("0x{:02x}", b)).collect::<Vec<_>>().join(", ");

    #[cfg(feature = "mainnet")]
    eprintln!("MAINNET GENESIS_CHECKPOINT_HASH = [{}]", hex);
    #[cfg(not(feature = "mainnet"))]
    eprintln!("TESTNET GENESIS_CHECKPOINT_HASH = [{}]", hex);

    eprintln!("Genesis total_supply = {} sats ({} UDAG)", state.total_supply(),
        state.total_supply() / ultradag_coin::SATS_PER_UDAG);

    // Founder address is hardcoded in constants.rs as DEV_ADDRESS_BYTES.
    // (We no longer derive it from dev_keypair() — the seed-derived keypair is
    // a separate test-only signing identity.)
    let dev_addr = ultradag_coin::constants::dev_address();
    eprintln!("Founder address = {} ({})", dev_addr.to_hex(), dev_addr);
}

#[test]
fn genesis_hash_matches_constant() {
    // For mainnet: ULTRADAG_DEV_KEY must be set in the environment.
    #[cfg(feature = "mainnet")]
    if std::env::var("ULTRADAG_DEV_KEY").is_err() {
        std::env::set_var("ULTRADAG_DEV_KEY", "0000000000000000000000000000000000000000000000000000000000000001");
    }

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

    let computed = ultradag_coin::consensus::checkpoint::compute_checkpoint_hash(&genesis_checkpoint);

    // Verify determinism
    let computed2 = ultradag_coin::consensus::checkpoint::compute_checkpoint_hash(&genesis_checkpoint);
    assert_eq!(computed, computed2, "Genesis hash must be deterministic");

    // Skip match check when testnet uses [0u8; 32] (dev key is configurable via env var)
    if ultradag_coin::constants::GENESIS_CHECKPOINT_HASH == [0u8; 32] {
        eprintln!("Skipping genesis hash match (testnet placeholder [0u8; 32])");
        return;
    }

    // Verify the hardcoded constant matches the computed value.
    // If this fails, GENESIS_CHECKPOINT_HASH in constants.rs is stale.
    // Update the constant with the printed value below.
    let hex: String = computed.iter().map(|b| format!("0x{:02x}", b)).collect::<Vec<_>>().join(", ");
    assert_eq!(
        computed,
        ultradag_coin::constants::GENESIS_CHECKPOINT_HASH,
        "GENESIS_CHECKPOINT_HASH in constants.rs is stale! Computed: [{}]",
        hex
    );
}
