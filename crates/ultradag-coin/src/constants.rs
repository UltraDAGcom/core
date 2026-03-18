/// Total supply: 21,000,000 UDAG (stored as smallest unit = 1 sat = 0.00000001 UDAG)
pub const MAX_SUPPLY_SATS: u64 = 21_000_000 * COIN;

/// 1 UDAG = 100,000,000 satoshis (also exported as SATS_PER_UDAG for clarity)
pub const COIN: u64 = 100_000_000;

/// Alias for COIN — clearer when used in display/conversion contexts.
pub const SATS_PER_UDAG: u64 = COIN;

/// Convert sats to UDAG as a float for display purposes.
pub fn sats_to_udag(sats: u64) -> f64 {
    sats as f64 / SATS_PER_UDAG as f64
}

/// Initial block reward: 1 UDAG per round (split among validators)
pub const INITIAL_REWARD_SATS: u64 = COIN;

/// Reward halves every 10,500,000 rounds (~1.66 years at 5s rounds)
/// Chosen so that reward × interval × 2 = MAX_SUPPLY (21M UDAG).
pub const HALVING_INTERVAL: u64 = 10_500_000;

/// Genesis timestamp
pub const GENESIS_TIMESTAMP: i64 = 1741132800; // 2025-03-05T00:00:00Z

/// Maximum allowed timestamp drift into the future (in seconds).
/// Vertices with timestamps more than this far ahead of local time are rejected.
/// Set to 300 seconds (5 minutes) to tolerate clock skew while preventing
/// far-future timestamp attacks that could manipulate round timing.
pub const MAX_FUTURE_TIMESTAMP: i64 = 300;

/// Maximum transactions per vertex
pub const MAX_TXS_PER_BLOCK: usize = 10_000;

/// Minimum transaction fee to prevent mempool spam.
/// 10,000 sats = 0.0001 UDAG. Cost to fill 10K-tx mempool: 1 UDAG.
pub const MIN_FEE_SATS: u64 = 10_000;

/// Maximum memo size in bytes for transaction data payloads.
/// 256 bytes is sufficient for IoT sensor data (temperature, humidity, pressure, GPS, timestamp)
/// while preventing DAG bloat from oversized memos.
pub const MAX_MEMO_BYTES: usize = 256;

/// # Mainnet Key Lifecycle Requirements
///
/// - **Offline key generation only** — Never generate keys on a network-facing machine.
///   Use an air-gapped device or hardware wallet for all mainnet keypairs.
/// - **Hardware wallet integration recommended** — Store validator and dev allocation keys
///   in hardware wallets (Ledger, Trezor) for production use.
/// - **No private keys should ever transit the network** — The `/tx/submit` endpoint is
///   the mainnet transaction path. All signing happens client-side via SDKs.
/// - **`/keygen`, `/tx`, `/stake`, `/unstake`, `/faucet`, `/proposal`, `/vote`** are
///   testnet-only endpoints that accept secret keys in the request body. They return
///   HTTP 410 GONE when `--testnet false` (mainnet mode).
///
/// # Network Identifier
///
/// Included in all signatures to prevent cross-network replay attacks.
/// Mainnet and testnet use different NETWORK_IDs, making signatures cryptographically
/// incompatible across networks. A transaction signed for testnet cannot be replayed
/// on mainnet (and vice versa) because the signable_bytes() include this prefix.
///
/// On mainnet builds (`--features mainnet`), this is `b"ultradag-mainnet-v1"` instead,
/// making cross-network signature replay cryptographically impossible.
#[cfg(not(feature = "mainnet"))]
pub const NETWORK_ID: &[u8] = b"ultradag-testnet-v1";

/// Network identifier for mainnet builds. Cryptographically incompatible with testnet —
/// cross-network signature replay is impossible.
#[cfg(feature = "mainnet")]
pub const NETWORK_ID: &[u8] = b"ultradag-mainnet-v1";

/// Developer allocation: 5% of total supply allocated at genesis.
/// Funds protocol development. Visible and auditable from round 0.
/// Total: 1,050,000 UDAG (5% of 21,000,000 UDAG max supply).
pub const DEV_ALLOCATION_SATS: u64 = 1_050_000 * COIN;

/// DAO Treasury allocation: 10% of total supply allocated at genesis.
/// Controlled by Council of 21 via TreasurySpend proposals.
/// Total: 2,100,000 UDAG (10% of 21,000,000 UDAG max supply).
pub const TREASURY_ALLOCATION_SATS: u64 = 2_100_000 * COIN;

/// Developer allocation address seed for TESTNET.
/// This seed is: "ultradag-dev-addr-testnet-v1\0\0\0\0" encoded as bytes.
/// MAINNET REQUIREMENT: Replace with offline-generated keypair before mainnet launch.
/// The private key for this testnet seed is derivable — acceptable for testnet only.
/// For mainnet: generate offline, store in hardware wallet, never commit private key.
pub const DEV_ADDRESS_SEED: [u8; 32] = [
    0x75, 0x6c, 0x74, 0x72, 0x61, 0x64, 0x61, 0x67,
    0x2d, 0x64, 0x65, 0x76, 0x2d, 0x61, 0x64, 0x64,
    0x72, 0x2d, 0x74, 0x65, 0x73, 0x74, 0x6e, 0x65,
    0x74, 0x2d, 0x76, 0x31, 0x00, 0x00, 0x00, 0x00,
];

/// Compile-time assertion: dev address seed must not be the old test placeholder.
const _: () = assert!(
    DEV_ADDRESS_SEED[0] != 0xDE,
    "DEV_ADDRESS_SEED is still the test placeholder. Replace before any launch."
);

/// Return the deterministic developer address.
pub fn dev_address() -> crate::address::Address {
    crate::address::SecretKey::from_bytes(DEV_ADDRESS_SEED).address()
}

/// Maximum number of active validators (top stakers by amount).
/// Odd number for clean BFT quorum (ceil(2*21/3) = 14).
pub const MAX_ACTIVE_VALIDATORS: usize = 21;

/// Minimum number of active validators required for BFT consensus.
/// BFT requires at least 4 validators to tolerate 1 Byzantine fault (3f+1 where f=1).
/// With fewer than 4 validators, the system cannot guarantee safety.
pub const MIN_ACTIVE_VALIDATORS: usize = 4;

/// Epoch length in rounds. Validator set recalculated at epoch boundaries.
/// Matches halving interval for clean alignment.
pub const EPOCH_LENGTH_ROUNDS: u64 = 210_000;

/// Observer reward percentage: staked-but-not-active addresses earn 20% of normal.
pub const OBSERVER_REWARD_PERCENT: u64 = 20;

// ===== COUNCIL OF 21 CONSTANTS =====

/// Council of 21: Panama Foundation membership requirement.
///
/// No stake requirement for council members. Council seats are earned
/// through Foundation membership and expertise, not purchased with tokens.
/// Council members earn emission rewards instead.
///
/// In production, this would be verified against foundation records.
/// Verified via governance (CouncilMembership proposals) rather than external records.
pub const COUNCIL_FOUNDATION_MEMBERSHIP_REQUIRED: bool = true;

/// Council of 21: Maximum number of council members (fixed at 21).
/// This matches MAX_ACTIVE_VALIDATORS but provides semantic clarity.
pub const COUNCIL_MAX_MEMBERS: usize = 21;

/// Council emission share: percentage of each block reward distributed to council members.
/// 10% of each vertex reward is split equally among seated council members.
/// Governable via ParameterChange proposals (param: "council_emission_percent").
pub const COUNCIL_EMISSION_PERCENT: u64 = 10;

/// How often to produce a checkpoint (in finalized rounds).
/// Checkpoints enable fast-sync for new nodes.
pub const CHECKPOINT_INTERVAL: u64 = 100;

/// Genesis checkpoint hash - the blake3 hash of the genesis checkpoint.
/// This is the trust anchor for checkpoint chain verification.
/// Any checkpoint chain must link back to this hash to be valid.
///
/// This is computed as blake3(serialize(genesis_checkpoint)) where genesis_checkpoint has:
/// - round: 0
/// - state_root: computed from genesis state
/// - dag_tip: [0u8; 32] (no vertices yet)
/// - total_supply: genesis total (testnet includes faucet, mainnet does not)
/// - prev_checkpoint_hash: [0u8; 32] (genesis has no predecessor)
///
/// CRITICAL: This must be updated if genesis state changes.
/// The testnet and mainnet hashes differ because mainnet excludes faucet funds.
/// Run `cargo test --features mainnet test_compute_mainnet_genesis_hash` to recompute.
#[cfg(not(feature = "mainnet"))]
pub const GENESIS_CHECKPOINT_HASH: [u8; 32] = [
    0x8b, 0x11, 0x13, 0x6b, 0xfe, 0x2e, 0x19, 0xe1,
    0x54, 0xa2, 0x07, 0xd5, 0x6f, 0x49, 0x5d, 0x45,
    0xd8, 0x29, 0xef, 0x73, 0x2f, 0x55, 0x36, 0xf2,
    0x59, 0xf5, 0xdf, 0x61, 0x1b, 0x1d, 0x41, 0x05,
]; // Testnet: canonical state root v1 + configured_validator_count in state root

/// Mainnet genesis checkpoint hash — computed from genesis WITHOUT faucet.
/// To compute: `cargo test test_compute_genesis_hash -- --nocapture`
/// Then replace this constant with the printed hash.
/// IMPORTANT: This MUST be set to the real hash before mainnet launch.
#[cfg(feature = "mainnet")]
pub const GENESIS_CHECKPOINT_HASH: [u8; 32] = [0u8; 32]; // PLACEHOLDER — see test_compute_genesis_hash

/// Compile-time assertion: GENESIS_CHECKPOINT_HASH must not be the placeholder on mainnet.
/// This is the primary defense — prevents building a mainnet binary with [0u8; 32].
/// The runtime check below is a secondary defense for extra safety.
#[cfg(feature = "mainnet")]
const _GENESIS_HASH_GUARD: () = {
    // Check first 4 bytes are not all zero (sufficient to detect placeholder).
    // Full const array comparison requires nightly, so we check a prefix.
    assert!(
        GENESIS_CHECKPOINT_HASH[0] != 0
            || GENESIS_CHECKPOINT_HASH[1] != 0
            || GENESIS_CHECKPOINT_HASH[2] != 0
            || GENESIS_CHECKPOINT_HASH[3] != 0,
        "GENESIS_CHECKPOINT_HASH is placeholder [0u8; 32]. \
         Compute mainnet hash with: cargo test test_compute_genesis_hash -- --nocapture"
    );
};

/// Runtime check: panics at startup if mainnet builds have the placeholder hash.
/// Secondary defense — the compile-time assertion above should catch this first.
#[cfg(feature = "mainnet")]
pub fn verify_genesis_checkpoint_hash() {
    assert_ne!(
        GENESIS_CHECKPOINT_HASH, [0u8; 32],
        "FATAL: GENESIS_CHECKPOINT_HASH is placeholder [0u8; 32]. \
         Compute mainnet hash with: cargo test test_compute_genesis_hash -- --nocapture"
    );
}

/// Testnet: no-op (testnet hash is already correct).
#[cfg(not(feature = "mainnet"))]
pub fn verify_genesis_checkpoint_hash() {}

/// Compute the epoch number for a given round.
pub fn epoch_of(round: u64) -> u64 {
    round / EPOCH_LENGTH_ROUNDS
}

/// Check if a round is an epoch boundary (start of new epoch).
pub fn is_epoch_boundary(round: u64) -> bool {
    round.is_multiple_of(EPOCH_LENGTH_ROUNDS)
}

/// Deterministic seed for the testnet faucet keypair.
/// Same on every node so all nodes recognize the faucet address.
/// MAINNET: Remove faucet entirely. This assertion prevents shipping the test seed.
pub const FAUCET_SEED: [u8; 32] = [0xFA; 32];

/// Compile-time assertion: faucet seed must not ship with mainnet builds.
/// Enable `--features mainnet` to trigger this check.
#[cfg(feature = "mainnet")]
const _FAUCET_GUARD: () = assert!(
    FAUCET_SEED[0] != 0xFA || FAUCET_SEED[1] != 0xFA || FAUCET_SEED[16] != 0xFA,
    "FAUCET_SEED is the test placeholder [0xFA; 32]. Remove faucet before mainnet launch."
);

/// Faucet genesis pre-fund: 1,000,000 UDAG in sats.
pub const FAUCET_PREFUND_SATS: u64 = 1_000_000 * COIN;

/// Return the deterministic faucet keypair (same on every node).
pub fn faucet_keypair() -> crate::address::SecretKey {
    crate::address::SecretKey::from_bytes(FAUCET_SEED)
}

/// Calculate round reward for a given round height.
pub fn block_reward(height: u64) -> u64 {
    let halvings = height / HALVING_INTERVAL;
    if halvings >= 64 {
        return 0;
    }
    INITIAL_REWARD_SATS >> halvings
}

// ========================================
// GOVERNANCE CONSTANTS
// ========================================

/// Minimum stake required to submit a governance proposal.
/// Prevents spam. Set low for testnet community building.
pub const MIN_STAKE_TO_PROPOSE: u64 = 10_000 * COIN; // 10,000 UDAG (same as MIN_STAKE_SATS)

/// Voting period in rounds. At 2.5s/round ≈ 3.5 days.
/// Long enough for community participation, short enough to ship.
pub const GOVERNANCE_VOTING_PERIOD_ROUNDS: u64 = 120_960;

/// Quorum: minimum fraction of total staked supply that must vote.
/// Numerator/denominator form to avoid floats.
/// 10% quorum — achievable on a small network at launch.
pub const GOVERNANCE_QUORUM_NUMERATOR: u64 = 10;
pub const GOVERNANCE_QUORUM_DENOMINATOR: u64 = 100;

/// Approval threshold: fraction of votes_for / (votes_for + votes_against).
/// 66% supermajority required.
pub const GOVERNANCE_APPROVAL_NUMERATOR: u64 = 66;
pub const GOVERNANCE_APPROVAL_DENOMINATOR: u64 = 100;

/// Execution delay after a proposal passes, in rounds.
/// Safety buffer before parameter changes take effect.
/// ~1.4 hours at 2.5s/round.
pub const GOVERNANCE_EXECUTION_DELAY_ROUNDS: u64 = 2_016;

/// Minimum active validators for DAO governance execution.
/// Below this threshold, ParameterChange proposals cannot execute (they remain
/// in PassedPending until the network is healthy enough). TextProposals are
/// unaffected — the community can signal at any validator count.
/// This prevents a small group from changing protocol parameters before the
/// network is sufficiently decentralized.
pub const MIN_DAO_VALIDATORS: usize = 8;

/// Maximum proposals active simultaneously (prevents state bloat).
pub const MAX_ACTIVE_PROPOSALS: usize = 20;

/// Maximum number of rounds a vertex can be ahead of the current DAG round.
/// Rejects vertices more than this many rounds in the future.
pub const MAX_FUTURE_ROUNDS: u64 = 10;

/// Percentage of stake burned on equivocation (slashing).
pub const SLASH_PERCENTAGE: u64 = 50;

/// Minimum delegation amount: 100 UDAG.
/// Keeps delegations meaningful and reduces state bloat from dust delegations.
pub const MIN_DELEGATION_SATS: u64 = 100 * COIN;

/// Default commission percentage for validators on delegated rewards.
pub const DEFAULT_COMMISSION_PERCENT: u8 = 10;

/// Maximum commission percentage a validator can charge on delegated rewards.
pub const MAX_COMMISSION_PERCENT: u8 = 100;

/// Transaction time-to-live in mempool (in seconds).
/// Transactions older than this are evicted to prevent stale execution.
/// 1 hour = 3600 seconds.
pub const MEMPOOL_TX_TTL_SECS: u64 = 3600;

/// Maximum title length in bytes.
pub const PROPOSAL_TITLE_MAX_BYTES: usize = 128;

/// Maximum description length in bytes.
pub const PROPOSAL_DESCRIPTION_MAX_BYTES: usize = 4096;

/// Rounds to retain terminal proposals (Executed/Rejected/Failed/Cancelled) before pruning.
/// ~14 hours at 5s rounds. Votes for pruned proposals are also removed.
pub const PROPOSAL_RETENTION_ROUNDS: u64 = 10_000;

/// Interval (in finalized rounds) between state bloat pruning passes.
/// Dust accounts and old proposals are pruned every this many rounds.
pub const STATE_PRUNING_INTERVAL: u64 = 1_000;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn block_reward_at_height_zero() {
        assert_eq!(block_reward(0), INITIAL_REWARD_SATS);
    }

    #[test]
    fn block_reward_first_halving() {
        assert_eq!(block_reward(HALVING_INTERVAL), INITIAL_REWARD_SATS / 2);
    }

    #[test]
    fn block_reward_second_halving() {
        assert_eq!(block_reward(2 * HALVING_INTERVAL), INITIAL_REWARD_SATS / 4);
    }

    #[test]
    fn block_reward_just_before_halving() {
        assert_eq!(block_reward(HALVING_INTERVAL - 1), INITIAL_REWARD_SATS);
    }

    #[test]
    fn block_reward_zero_after_64_halvings() {
        assert_eq!(block_reward(64 * HALVING_INTERVAL), 0);
        assert_eq!(block_reward(64 * HALVING_INTERVAL + 1), 0);
        assert_eq!(block_reward(u64::MAX), 0);
    }

    #[test]
    fn block_reward_decreases_monotonically() {
        let mut prev = block_reward(0);
        for i in 1..64 {
            let reward = block_reward(i * HALVING_INTERVAL);
            assert!(reward <= prev, "reward should decrease at halving {i}");
            prev = reward;
        }
    }

    #[test]
    fn constants_sanity() {
        assert_eq!(COIN, 100_000_000);
        assert_eq!(MAX_SUPPLY_SATS, 21_000_000 * COIN);
        const { assert!(MAX_TXS_PER_BLOCK > 0) };
    }
}
