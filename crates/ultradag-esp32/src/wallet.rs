//! Per-chip wallet identity.
//!
//! The Ed25519 seed lives in the `udag` NVS namespace (see `storage.rs`).
//! On first boot the NVS value is absent, so we generate a fresh 32-byte
//! seed from ESP-IDF's hardware RNG, persist it, and use it. Every
//! subsequent boot reads the same seed back, so the chip has a stable
//! on-chain identity without the key ever appearing in the firmware
//! image or in the git repo.
//!
//! Migration from the earlier hardcoded-seed version:
//! the first boot after this change will SEE that NVS is empty, generate
//! a fresh seed, and persist it. The chip's address will therefore
//! change between "old hardcoded demo" and "new random per-chip". If you
//! want the old demo seed back for continuity, erase NVS and set the
//! seed manually (the signing logic is unchanged).

use anyhow::{Context, Result};
use ed25519_dalek::SigningKey;

use crate::sign::address_from_pubkey;
use crate::storage::{random_seed, Storage};

/// Load the wallet's Ed25519 signing key, creating one on first boot.
///
/// Behavior:
///
///   1. Try to read `wallet_seed` from NVS.
///   2. If present and 32 bytes → build the SigningKey from it. Done.
///   3. If absent → generate 32 bytes from the hardware RNG
///      (`esp_fill_random`), persist to NVS, then build the SigningKey.
///   4. If NVS read errors out (e.g. the partition is corrupted), bail
///      with `Err`. We deliberately do NOT fall through to an ephemeral
///      in-memory key — that would silently rotate the wallet on every
///      boot, which is a much worse UX than a clean error.
///
/// The `Storage` parameter is `&mut` because the first-boot path needs
/// to write the freshly-generated seed.
pub fn load_or_create(storage: &mut Storage) -> Result<SigningKey> {
    if let Some(seed) = storage.get_wallet_seed().context("wallet: NVS read failed")? {
        log::info!("wallet: loaded Ed25519 seed from NVS");
        return Ok(SigningKey::from_bytes(&seed));
    }

    log::info!("wallet: no seed in NVS — generating a fresh one from hardware RNG");
    let seed = random_seed();
    storage
        .set_wallet_seed(&seed)
        .context("wallet: persisting new seed to NVS failed")?;
    log::info!("wallet: new seed persisted — this chip now has a stable identity");
    Ok(SigningKey::from_bytes(&seed))
}

/// The 20-byte on-chain address for a given signing key, derived via
/// the same `blake3(ed25519_pubkey)[..20]` rule the Rust node uses.
pub fn address_of(sk: &SigningKey) -> [u8; 20] {
    let pk: [u8; 32] = sk.verifying_key().to_bytes();
    address_from_pubkey(&pk)
}

/// Hex-encoded address for log messages and RPC path construction.
pub fn address_hex_of(sk: &SigningKey) -> String {
    hex::encode(address_of(sk))
}
