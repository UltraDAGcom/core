//! Trust-on-first-use (TOFU) validator-set check.
//!
//! ## What this is (and what it isn't)
//!
//! A **true** cryptographic light client verifies finality by fetching
//! a signed checkpoint and checking that ≥ 2/3 of the active validator
//! set's signatures attest to a specific state root. The current
//! ultradag-node HTTP RPC does not expose a `/checkpoint` endpoint, so
//! we can't do that today from the ESP32 side. Fixing that requires
//! adding a new Rust handler that serializes the node's internal
//! `Checkpoint` struct (including per-validator signatures) and
//! re-deploying the testnet nodes. Flagged as a TODO in `README.md`.
//!
//! Until that lands, this module implements the **strongest proxy
//! available with the current RPC surface**: TOFU of the validator
//! address set. On first boot we snapshot the output of
//! `GET /validators`, blake3-hash the sorted address list, and save
//! the 32-byte root to NVS. On every subsequent boot (and optionally
//! on an interval) we refetch `/validators`, recompute the root, and
//! compare against the stored one.
//!
//! ## What this detects
//!
//!   - A compromised RPC provider silently swapping to a different
//!     testnet or mainnet (different validator set → different root).
//!   - Validator churn — legitimate validators leaving or joining the
//!     active set. In a small testnet this can fire during normal
//!     operation; treat the warning accordingly.
//!
//! ## What this does NOT detect
//!
//!   - The same validator set lying about BALANCES, NONCES, or TXs in
//!     individual `/balance` or `/tx/submit` responses. We still have
//!     to trust the node for those reads — only a real finality
//!     endpoint can fix that.
//!   - A compromised node that was compromised BEFORE our first boot
//!     (that's what TOFU means — we trust what we see first).
//!
//! So this is defense-in-depth, not a replacement for a signed
//! checkpoint. It turns a silent takeover of the RPC endpoint into a
//! loud one.

use anyhow::{Context, Result};

use crate::client::UltraDagClient;
use crate::storage::Storage;

/// Outcome of one validator-set check. Useful for the caller to decide
/// whether to keep polling, alarm, or self-quarantine.
#[derive(Debug)]
pub enum ValidatorCheckResult {
    /// First boot on this chip — we saved a fresh root and trusted it.
    /// Includes the number of validators observed so the caller can log.
    FirstBoot { validator_count: usize, root_hex: String },
    /// Stored root matches the live set — normal, everything's fine.
    Match { validator_count: usize, root_hex: String },
    /// Stored root does NOT match the live set — something changed.
    /// This could be legitimate churn or a compromise; the caller
    /// should flag it loudly and let a human decide.
    Mismatch {
        stored_root_hex: String,
        live_root_hex: String,
        live_validator_count: usize,
    },
}

/// Fetch the active validator set and compare against the TOFU root in
/// NVS. On first boot, seeds the root. On subsequent boots, returns
/// `Match` or `Mismatch`.
pub fn check_validator_set(
    client: &UltraDagClient,
    storage: &mut Storage,
) -> Result<ValidatorCheckResult> {
    let resp = client
        .get_validators()
        .context("finality: GET /validators failed")?;

    let live_root = compute_validator_set_root(&resp.validators);
    let live_root_hex = hex::encode(live_root);

    match storage.get_validator_root().context("finality: NVS read failed")? {
        None => {
            // First boot on this chip (or first time the validator check
            // runs after an NVS wipe). Trust what we see and persist.
            if resp.count == 0 {
                // Empty set is a surprising but valid testnet state:
                // `/validators` reports *stakers*, and on a fresh testnet
                // nobody has staked yet even though the network has a
                // pre-configured producer set. Log loudly so the user
                // knows the TOFU root is degenerate.
                log::warn!(
                    "finality: /validators returned an EMPTY set — no one has staked."
                );
                log::warn!(
                    "finality: this is normal on a pre-stake testnet, but means the TOFU"
                );
                log::warn!(
                    "finality: check is effectively a no-op until at least one account stakes."
                );
            }
            storage
                .set_validator_root(&live_root)
                .context("finality: NVS write failed")?;
            Ok(ValidatorCheckResult::FirstBoot {
                validator_count: resp.count,
                root_hex: live_root_hex,
            })
        }
        Some(stored) if stored == live_root => Ok(ValidatorCheckResult::Match {
            validator_count: resp.count,
            root_hex: live_root_hex,
        }),
        Some(stored) => Ok(ValidatorCheckResult::Mismatch {
            stored_root_hex: hex::encode(stored),
            live_root_hex,
            live_validator_count: resp.count,
        }),
    }
}

/// Compute the 32-byte TOFU root from a list of validators. Sorts the
/// address strings lexicographically so the hash is stable regardless
/// of the order the node returns them in — without this, a node that
/// returns validators in a different order on every call would
/// generate a spurious mismatch on every check.
fn compute_validator_set_root(validators: &[crate::client::ValidatorInfo]) -> [u8; 32] {
    let mut sorted: Vec<&str> = validators.iter().map(|v| v.address.as_str()).collect();
    sorted.sort_unstable();

    let mut hasher = blake3::Hasher::new();
    // Domain separator + set size so "0 validators" doesn't collide
    // with "empty hash".
    hasher.update(b"ultradag-esp32-validator-set-v1");
    hasher.update(&(sorted.len() as u32).to_le_bytes());
    for addr in sorted {
        hasher.update(&(addr.len() as u32).to_le_bytes());
        hasher.update(addr.as_bytes());
    }
    *hasher.finalize().as_bytes()
}
