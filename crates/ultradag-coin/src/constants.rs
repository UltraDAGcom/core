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

/// Nominal block reward: 1 UDAG per round. This is the curve basis — the actual
/// minted amount per round is 88% of this (the sum of all bucket percentages
/// below), with the remaining 12% matching the IDO_GENESIS_PREMINE.
///
/// Emission timeline (at 5s/round), nominal curve:
/// - Epoch 0: 1 UDAG/round × 10.5M rounds = 10,500,000 UDAG nominal
/// - Epoch 1: 0.5 UDAG/round × 10.5M rounds = 5,250,000 UDAG nominal
/// - Epoch 2: 0.25 UDAG/round × 10.5M rounds = 2,625,000 UDAG nominal
/// - Geometric series sum: INITIAL_REWARD × HALVING_INTERVAL × 2 = 21M nominal
/// - Actual emitted = 88% of nominal curve = 18.48M UDAG
/// - Plus IDO genesis pre-mine = 2.52M UDAG
/// - Total supply cap = 21M UDAG
/// - Full emission over ~106 years
///
/// Per-round split (sums to 88% — the remaining 12% is not minted, matching
/// the genesis IDO pre-mine so total supply converges to exactly 21M):
/// - 44% validators/stakers (proportional to effective stake)
/// - 10% council (equal split among seated members)
/// - 16% treasury (controlled by Council of 21 via TreasurySpend proposals)
/// -  5% founder (liquid balance, can spend/stake/delegate normally)
/// -  8% ecosystem (liquid balance at ecosystem multisig address)
/// -  5% reserve (liquid balance at reserve multisig address)
///
/// Genesis pre-mine:
/// - 2,520,000 UDAG to IDO distributor address (private round + Uniswap seed)
pub const INITIAL_REWARD_SATS: u64 = COIN;

/// Reward halves every 10,500,000 rounds (~1.66 years at 5s rounds).
/// Geometric series of the nominal curve: reward × interval × 2 = 21M UDAG.
/// Actual emitted = 88% of that = 18.48M, plus 2.52M IDO pre-mine = 21M cap.
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

/// Validator emission share: percentage of each round's block reward distributed to
/// validators/stakers proportional to effective stake. Explicitly configured (not
/// computed as residual) so governance can tune individual buckets independently.
/// Governable via ParameterChange proposals (param: "validator_emission_percent", bounds: 30-80%).
pub const VALIDATOR_EMISSION_PERCENT: u64 = 44;

/// Founder emission share: percentage of each round's block reward credited to the founder address.
/// The founder starts with 0 balance and earns through emission like everyone else.
/// 5% of each round's reward is credited as liquid balance (can spend/stake/delegate normally).
/// Governable via ParameterChange proposals (param: "founder_emission_percent", bounds: 0-10%).
pub const FOUNDER_EMISSION_PERCENT: u64 = 5;

/// Treasury emission share: percentage of each round's block reward credited to the treasury.
/// The treasury starts at 0 and grows through emission, controlled by Council of 21 via
/// TreasurySpend proposals. 16% of each round's reward is added to treasury_balance.
/// Governable via ParameterChange proposals (param: "treasury_emission_percent", bounds: 0-25%).
pub const TREASURY_EMISSION_PERCENT: u64 = 16;

/// Ecosystem emission share: percentage of each round's block reward credited to
/// the ecosystem multisig address. Used for airdrops, grants, early-adopter rewards.
/// The ecosystem address is a normal account; credits behave like liquid balance.
/// Governable via ParameterChange proposals (param: "ecosystem_emission_percent", bounds: 0-20%).
pub const ECOSYSTEM_EMISSION_PERCENT: u64 = 8;

/// Reserve emission share: percentage of each round's block reward credited to the
/// reserve multisig address. Strategic reserve for future use, community-governed.
/// The reserve address is a normal account; credits behave like liquid balance.
/// Governable via ParameterChange proposals (param: "reserve_emission_percent", bounds: 0-15%).
pub const RESERVE_EMISSION_PERCENT: u64 = 5;

/// Genesis pre-mine to the IDO distributor address: 2,520,000 UDAG (12% of total supply).
///
/// This is the ONLY pre-mined allocation in the system. All other buckets are
/// distributed through per-round emission. The IDO distributor is a multisig
/// responsible for:
///   1. Distributing tokens to private-round participants.
///   2. Seeding Uniswap liquidity via the UDAG bridge (bridge/src/UDAGToken.sol).
///
/// The 12% pre-mine is offset by the 12% gap in per-round emission: buckets
/// sum to 88% of `block_reward()`, so total supply converges to exactly 21M UDAG.
pub const IDO_GENESIS_PREMINE_SATS: u64 = 2_520_000 * COIN;

/// Compile-time assertion: the six emission buckets sum to exactly 88% of block_reward.
/// The remaining 12% corresponds to the IDO genesis pre-mine, maintaining the 21M cap.
const _: () = assert!(
    VALIDATOR_EMISSION_PERCENT
        + COUNCIL_EMISSION_PERCENT
        + TREASURY_EMISSION_PERCENT
        + FOUNDER_EMISSION_PERCENT
        + ECOSYSTEM_EMISSION_PERCENT
        + RESERVE_EMISSION_PERCENT
        == 88,
    "Emission bucket percentages must sum to 88 (12% reserved for IDO pre-mine)"
);

/// Founder / developer address — 5% of per-round emission lands here.
/// bech32m: `udag1nqcz7h7xe9kh2fvjnqflwjc7zxnvdyy309lt5t` (@founder, 2026-04-10).
///
/// This is the canonical protocol-level founder address, baked into the binary
/// so every node agrees on where founder emission is credited. The private key
/// is held off-network by the founder (hardware wallet); nodes only need the
/// public 20-byte address to route credits.
///
/// Environment variable `ULTRADAG_DEV_ADDRESS` can override this for local
/// development and alternate networks, but the default below is the canonical
/// protocol value for both mainnet and testnet.
pub const DEV_ADDRESS_BYTES: [u8; 20] = [
    0x98, 0x30, 0x2f, 0x5f, 0xc6, 0xc9, 0x6d, 0x75, 0x25, 0x92,
    0x98, 0x13, 0xf7, 0x4b, 0x1e, 0x11, 0xa6, 0xc6, 0x90, 0x91,
];

/// Testnet seed for the legacy `dev_keypair()` convenience signer.
///
/// This seed is **unrelated** to the canonical `DEV_ADDRESS_BYTES` above — it
/// exists only to provide a deterministic Ed25519 keypair for testnet and SDK
/// integration tests that need *some* stable signing identity. Transactions
/// signed by this key land at the seed-derived address, NOT at the founder
/// address, so `dev_keypair().address() != dev_address()` by design.
///
/// Production code (node, RPC, consensus) never calls `dev_keypair()` — only
/// test fixtures and SDK determinism checks do.
#[cfg(not(feature = "mainnet"))]
pub const DEV_ADDRESS_SEED: [u8; 32] = [
    0x8a, 0x3d, 0x7e, 0x2f, 0x91, 0xc4, 0xb5, 0x6e,
    0x1a, 0xf8, 0x3c, 0x0d, 0x57, 0xe9, 0x4b, 0x2a,
    0x6f, 0x1c, 0x8e, 0x3d, 0x95, 0x7a, 0x4f, 0x0b,
    0x62, 0xd5, 0x8c, 0x1e, 0xa7, 0x3b, 0x9f, 0x4c,
];

/// Mainnet: the legacy dev seed is unused — left as `[0u8; 32]` for compilation
/// compatibility. `dev_keypair()` is not available on mainnet builds.
#[cfg(feature = "mainnet")]
pub const DEV_ADDRESS_SEED: [u8; 32] = [0u8; 32];

/// Compile-time assertion: dev address seed must not be the old insecure test placeholder.
const _: () = assert!(
    DEV_ADDRESS_SEED[0] != 0x75 || DEV_ADDRESS_SEED[1] != 0x6c,
    "DEV_ADDRESS_SEED uses old insecure placeholder. Use new testnet seed or set mainnet key."
);

/// Return the developer/founder address.
///
/// DETERMINISM: The address is resolved ONCE at first call and cached via OnceLock.
/// All nodes compute the same address, and the result never changes during the
/// process lifetime. This prevents state divergence if env vars were
/// hypothetically modified mid-run.
///
/// Resolution order:
/// - `ULTRADAG_DEV_ADDRESS` env var (hex or bech32m) — optional override
/// - Hardcoded `DEV_ADDRESS_BYTES` constant (canonical default)
///
/// Nodes only need the ADDRESS (public) to route emission — never the private key.
pub fn dev_address() -> crate::address::Address {
    static CACHED: std::sync::OnceLock<crate::address::Address> = std::sync::OnceLock::new();
    *CACHED.get_or_init(|| {
        if let Ok(s) = std::env::var("ULTRADAG_DEV_ADDRESS") {
            if let Some(addr) = crate::address::Address::parse(&s) {
                return addr;
            }
        }
        crate::address::Address(DEV_ADDRESS_BYTES)
    })
}

/// Get the testnet convenience keypair (testnet builds only).
///
/// **Important:** this keypair is derived from `DEV_ADDRESS_SEED`, which is
/// unrelated to the canonical `DEV_ADDRESS_BYTES` / `dev_address()`. The address
/// returned by `dev_keypair().address()` is NOT the founder address — it's a
/// stable test-only identity for SDK integration tests and local signing.
///
/// Production node code never calls this. Mainnet builds don't expose it.
#[cfg(not(feature = "mainnet"))]
pub fn dev_keypair() -> crate::address::SecretKey {
    if let Ok(key_hex) = std::env::var("ULTRADAG_DEV_KEY") {
        if key_hex.len() == 64 {
            let mut bytes = [0u8; 32];
            for (i, chunk) in key_hex.as_bytes().chunks(2).enumerate() {
                if let Ok(hex_str) = std::str::from_utf8(chunk) {
                    if let Ok(b) = u8::from_str_radix(hex_str, 16) {
                        bytes[i] = b;
                    }
                }
            }
            return crate::address::SecretKey::from_bytes(bytes);
        }
    }
    crate::address::SecretKey::from_bytes(DEV_ADDRESS_SEED)
}

/// Maximum number of active validators (top stakers by amount).
/// Increased from 21 to 100 for better decentralization while maintaining
/// tractable DAG-BFT consensus (O(n²) message complexity).
pub const MAX_ACTIVE_VALIDATORS: usize = 100;

/// Minimum number of active validators required for BFT consensus.
/// BFT requires at least 7 validators to tolerate 2 Byzantine faults (3f+1 where f=2).
/// With fewer than 7 validators, the system cannot guarantee safety.
pub const MIN_ACTIVE_VALIDATORS: usize = 7;

/// Epoch length in rounds. Validator set recalculated at epoch boundaries.
/// Matches halving interval for clean alignment.
pub const EPOCH_LENGTH_ROUNDS: u64 = 210_000;

/// Sentinel value for `current_epoch` indicating "never initialized".
/// On the first vertex, `epoch_of(round)` will differ from this sentinel,
/// triggering the initial `recalculate_active_set()`.
///
/// SAFETY: This is safe because `epoch_of(round) = round / EPOCH_LENGTH_ROUNDS` (210,000),
/// so the maximum possible epoch is `u64::MAX / 210,000 ≈ 8.78×10^13` — never `u64::MAX`.
/// `EPOCH_LENGTH_ROUNDS` is a compile-time constant, not governable, so this invariant holds forever.
pub const EPOCH_UNINITIALIZED: u64 = u64::MAX;

/// Observer reward percentage: staked-but-not-active addresses earn 50% of normal.
/// Increased from 20% to provide better rewards for small validators who are
/// staked but not in the top-100 active set, helping them grow through passive accumulation.
pub const OBSERVER_REWARD_PERCENT: u64 = 50;

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
/// - total_supply: genesis total (testnet: faucet only, mainnet: 0)
/// - prev_checkpoint_hash: [0u8; 32] (genesis has no predecessor)
///
/// CRITICAL: This must be updated if genesis state changes.
/// Run `cargo test test_compute_genesis_hash -- --nocapture` to recompute.
/// Testnet: genesis hash verification skipped (dev key may be overridden via ULTRADAG_DEV_KEY).
/// Mainnet hash below is the security-critical one.
#[cfg(not(feature = "mainnet"))]
pub const GENESIS_CHECKPOINT_HASH: [u8; 32] = [0u8; 32];

/// Mainnet genesis checkpoint hash — computed 2026-04-10 for the April 2026
/// 7-bucket tokenomics update (hard fork from the original zero-pre-mine model).
///
/// Genesis state:
/// - total_supply = 2,520,000 UDAG (12% IDO pre-mine only, no faucet on mainnet)
/// - IDO distributor credited `IDO_GENESIS_PREMINE_SATS` at `IDO_ADDRESS_BYTES`
/// - Founder, ecosystem, reserve, treasury, council all start at 0
/// - Per-round emission: 44/10/16/5/8/5 buckets (see `INITIAL_REWARD_SATS` docs)
///
/// All four protocol addresses (founder, IDO, ecosystem, reserve) are hardcoded
/// constants in this file, so the genesis hash is fully deterministic from the
/// source tree — no environment variables required.
///
/// To recompute (if you change any genesis-affecting constant):
///   cargo test --features mainnet test_compute_genesis_hash -- --nocapture
/// then paste the printed hash here and rebuild.
#[cfg(feature = "mainnet")]
pub const GENESIS_CHECKPOINT_HASH: [u8; 32] = [
    0xf2, 0x15, 0x7d, 0x73, 0x68, 0x2d, 0x6b, 0x7b,
    0x04, 0x7c, 0x5b, 0x25, 0xee, 0xbb, 0x1a, 0xcd,
    0xac, 0x9d, 0x03, 0x97, 0x95, 0x99, 0x1d, 0x09,
    0x3a, 0xeb, 0x25, 0xf7, 0xed, 0xcd, 0xb3, 0x7a,
];

/// Compile-time assertion: GENESIS_CHECKPOINT_HASH must not be the placeholder on mainnet.
/// This is the primary defense — prevents building a mainnet binary with [0u8; 32].
/// The runtime check below is a secondary defense for extra safety.
#[cfg(feature = "mainnet")]
const _GENESIS_HASH_GUARD: () = {
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
    round % EPOCH_LENGTH_ROUNDS == 0
}

/// Deterministic seed for the testnet faucet keypair.
/// Same on every node so all nodes recognize the faucet address.
/// 
/// # Security Notice
/// 
/// This is TESTNET ONLY. The faucet is disabled on mainnet.
/// Uses a less guessable seed than the previous [0xFA; 32].
/// 
/// For mainnet: faucet functionality is completely disabled.
#[cfg(not(feature = "mainnet"))]
pub const FAUCET_SEED: [u8; 32] = [
    0x2b, 0x5e, 0x8f, 0x1a, 0x93, 0xc7, 0x4d, 0x6b,
    0x0f, 0xe2, 0xa8, 0x35, 0x7c, 0x1d, 0x9e, 0x4f,
    0x8a, 0x3c, 0x6b, 0x0d, 0x5f, 0xe1, 0xa9, 0x27,
    0x4c, 0x8d, 0x1f, 0x6a, 0x3e, 0x9b, 0x5c, 0x0e,
];

/// Faucet genesis pre-fund: 1,000,000 UDAG in sats (testnet only).
#[cfg(not(feature = "mainnet"))]
pub const FAUCET_PREFUND_SATS: u64 = 1_000_000 * COIN;

/// Return the testnet faucet keypair.
/// 
/// # Panics
/// 
/// Panics on mainnet builds - faucet is disabled.
#[cfg(not(feature = "mainnet"))]
pub fn faucet_keypair() -> crate::address::SecretKey {
    crate::address::SecretKey::from_bytes(FAUCET_SEED)
}

/// Mainnet: faucet is disabled.
/// This function exists only to satisfy compilation - it panics if called.
#[cfg(feature = "mainnet")]
pub fn faucet_keypair() -> crate::address::SecretKey {
    panic!("MAINNET SECURITY: Faucet is disabled on mainnet. faucet_keypair() must never be called.");
}

// ===== BUCKET ADDRESSES (IDO, ECOSYSTEM, RESERVE) =====
//
// Each bucket has a dedicated recipient address hardcoded at the protocol level
// so every node (testnet and mainnet) agrees on exactly where the genesis
// pre-mine and per-round emission land. These are held by off-chain multisigs;
// nodes only need the public 20-byte address to route credits.
//
// Environment variable overrides (`ULTRADAG_IDO_ADDRESS`, etc.) are supported
// for local development and alternate networks, but the mainnet/testnet
// defaults below are the canonical protocol values.
//
// Source (provided 2026-04-10):
//   @liquidity  udag1rvdfs928eu7trrc33wj2edwctdkt08gdkmhppx
//   @ecosystem  udag17z5yull0zrhrmkvw6337f3hdh3rfs7mgnhmvfz
//   @reserve    udag1rs22h8y2ack0285efhe4g57hm8kr8z7a4gkxp8

/// IDO distributor address — 2,520,000 UDAG pre-mined at genesis.
/// bech32m: `udag1rvdfs928eu7trrc33wj2edwctdkt08gdkmhppx`
pub const IDO_ADDRESS_BYTES: [u8; 20] = [
    0x1b, 0x1a, 0x98, 0x15, 0x47, 0xcf, 0x3c, 0xb1, 0x8f, 0x11,
    0x8b, 0xa4, 0xac, 0xb5, 0xd8, 0x5b, 0x6c, 0xb7, 0x9d, 0x0d,
];

/// Ecosystem multisig address — earns 8% of per-round emission.
/// bech32m: `udag17z5yull0zrhrmkvw6337f3hdh3rfs7mgnhmvfz`
pub const ECOSYSTEM_ADDRESS_BYTES: [u8; 20] = [
    0xf0, 0xa8, 0x4e, 0x7f, 0xef, 0x10, 0xee, 0x3d, 0xd9, 0x8e,
    0xd4, 0x63, 0xe4, 0xc6, 0xed, 0xbc, 0x46, 0x98, 0x7b, 0x68,
];

/// Reserve multisig address — earns 5% of per-round emission.
/// bech32m: `udag1rs22h8y2ack0285efhe4g57hm8kr8z7a4gkxp8`
pub const RESERVE_ADDRESS_BYTES: [u8; 20] = [
    0x1c, 0x14, 0xab, 0x9c, 0x8a, 0xee, 0x2c, 0xf5, 0x1e, 0x99,
    0x4d, 0xf3, 0x54, 0x53, 0xd7, 0xd9, 0xec, 0x33, 0x8b, 0xdd,
];

/// Return the IDO distributor address.
///
/// DETERMINISM: Resolved ONCE at first call and cached via OnceLock, same as
/// `dev_address()`. All nodes compute the same address.
///
/// - Checks `ULTRADAG_IDO_ADDRESS` env var first (40-hex or bech32m, optional override)
/// - Falls back to hardcoded `IDO_ADDRESS_BYTES`
pub fn ido_address() -> crate::address::Address {
    static CACHED: std::sync::OnceLock<crate::address::Address> = std::sync::OnceLock::new();
    *CACHED.get_or_init(|| {
        if let Ok(s) = std::env::var("ULTRADAG_IDO_ADDRESS") {
            if let Some(addr) = crate::address::Address::parse(&s) {
                return addr;
            }
        }
        crate::address::Address(IDO_ADDRESS_BYTES)
    })
}

/// Return the ecosystem multisig address. Same resolution rules as `ido_address()`.
pub fn ecosystem_address() -> crate::address::Address {
    static CACHED: std::sync::OnceLock<crate::address::Address> = std::sync::OnceLock::new();
    *CACHED.get_or_init(|| {
        if let Ok(s) = std::env::var("ULTRADAG_ECOSYSTEM_ADDRESS") {
            if let Some(addr) = crate::address::Address::parse(&s) {
                return addr;
            }
        }
        crate::address::Address(ECOSYSTEM_ADDRESS_BYTES)
    })
}

/// Return the reserve multisig address. Same resolution rules as `ido_address()`.
pub fn reserve_address() -> crate::address::Address {
    static CACHED: std::sync::OnceLock<crate::address::Address> = std::sync::OnceLock::new();
    *CACHED.get_or_init(|| {
        if let Ok(s) = std::env::var("ULTRADAG_RESERVE_ADDRESS") {
            if let Some(addr) = crate::address::Address::parse(&s) {
                return addr;
            }
        }
        crate::address::Address(RESERVE_ADDRESS_BYTES)
    })
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
/// Prevents spam. Set equal to MIN_STAKE_SATS for consistency.
/// 2,000 UDAG — accessible for community participation.
pub const MIN_STAKE_TO_PROPOSE: u64 = 2_000 * COIN; // 2,000 UDAG (same as MIN_STAKE_SATS)

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

/// Minimum bridge lock amount: 1 UDAG.
/// Prevents dust bridge operations that waste relay bandwidth.
pub const MIN_BRIDGE_AMOUNT_SATS: u64 = COIN;

/// Maximum bridge deposit amount: 100,000 UDAG.
/// Matches the Solidity contract's MAX_DEPOSIT (100_000 * 10^8).
pub const MAX_BRIDGE_AMOUNT_SATS: u64 = 100_000 * COIN;

/// Bridge attestation retention: ~5.8 days at 5s/round.
/// When pruned, unclaimed attestations are auto-refunded to the original sender.
pub const BRIDGE_ATTESTATION_RETENTION_ROUNDS: u64 = 100_000;

/// Supported destination chain IDs for bridge operations.
/// Only these chains are valid targets for BridgeDepositTx.
/// - 42161: Arbitrum One (mainnet)
/// - 421614: Arbitrum Sepolia (testnet)
pub const SUPPORTED_BRIDGE_CHAIN_IDS: &[u64] = &[42161, 421614];

/// Default commission percentage for validators on delegated rewards.
pub const DEFAULT_COMMISSION_PERCENT: u8 = 10;

/// Maximum commission percentage a validator can charge on delegated rewards.
pub const MAX_COMMISSION_PERCENT: u8 = 100;

/// Minimum rounds between commission changes for a validator.
/// Prevents sandwich attacks where a validator raises commission right before
/// reward distribution and lowers it immediately after.
/// Set equal to UNSTAKE_COOLDOWN_ROUNDS (2016) so delegators always have time
/// to exit before a commission change takes effect.
pub const COMMISSION_COOLDOWN_ROUNDS: u64 = 2_016;

/// Accounts with balance below this AND nonce == 0 are pruned as economically dead dust.
/// Set to MIN_FEE_SATS — an account that can't pay fees can't do anything.
/// Uses the protocol constant (not governance-adjustable min_fee_sats) so governance
/// can't accidentally prune real accounts by raising min_fee.
pub const DUST_THRESHOLD_SATS: u64 = MIN_FEE_SATS;

/// Maximum serialized size of a DagVertex in bytes.
/// Prevents DoS via oversized vertices (10K transactions x 256-byte memos = ~2.5MB).
/// Set to 1MB — generous for normal use, prevents abuse.
pub const MAX_VERTEX_BYTES: usize = 1_048_576; // 1 MB

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

/// Cooldown period between proposal submissions by the same address.
/// Prevents spam and allows time for community review of failed proposals.
/// 1008 rounds = ~1.4 hours at 5s/round.
pub const PROPOSAL_COOLDOWN_ROUNDS: u64 = 1_008;

/// BFT Safety Minimums for Governance Parameters
/// These constraints prevent governance from changing parameters to values
/// that would compromise consensus safety or enable attacks.

/// Minimum number of active validators for BFT consensus (3f+1 where f=1).
/// Below this threshold, the network cannot guarantee Byzantine fault tolerance.
pub const BFT_MIN_ACTIVE_VALIDATORS: usize = 4;

/// Minimum quorum numerator (represents 10% when denominator is 100).
/// Prevents governance from setting quorum so low that a tiny fraction can pass proposals.
pub const BFT_MIN_QUORUM_NUMERATOR: u64 = 10;

/// Maximum quorum numerator (represents 50% when denominator is 100).
/// Prevents governance from setting quorum so high that proposals cannot pass.
pub const BFT_MAX_QUORUM_NUMERATOR: u64 = 50;

/// Minimum stake required to prevent dust attacks on governance.
/// Below this threshold, attackers could spam proposals cheaply.
/// Set to 500 sats (0.000005 UDAG) — low enough to allow 2,000 UDAG minimum stake
/// while preventing governance spam from zero-stake accounts.
pub const BFT_MIN_STAKE_SATS: u64 = 500;

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
