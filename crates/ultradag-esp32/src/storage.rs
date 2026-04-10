//! Persistent storage in the ESP-IDF NVS partition.
//!
//! NVS (Non-Volatile Storage) is a key-value store that lives in a
//! dedicated flash partition (see the partition table: `nvs` at 0x9000,
//! 24 KB). It survives reboots and reflashes (as long as the nvs
//! partition itself isn't erased), making it the natural home for
//! device identity and configuration that should NOT live in the
//! firmware image.
//!
//! This module wraps `esp_idf_svc::nvs::EspDefaultNvs` with a tiny typed
//! API for the two things the light client needs to persist:
//!
//!   - `wallet_seed`   : 32-byte Ed25519 seed (generated with hardware
//!     RNG on first boot). Once generated it is never overwritten unless
//!     you explicitly wipe the NVS partition (e.g. via
//!     `espflash erase-parts --partition-table partition-table.bin nvs`).
//!
//!   - `wifi_ssid` / `wifi_password` : WiFi credentials, UTF-8 strings.
//!     If either is missing the caller falls back to the compile-time
//!     values in `config.rs`, so a freshly-flashed chip still works out
//!     of the box.
//!
//! **Security note.** This NVS partition is NOT encrypted by default.
//! Anyone who can read the flash (with `espflash read-flash`) can recover
//! both the Ed25519 seed and the WiFi password in plaintext. If you need
//! at-rest protection, enable ESP-IDF flash encryption — but be aware
//! that flash encryption is a ONE-TIME eFuse burn that permanently locks
//! the chip into only accepting encrypted firmware. Do that only after
//! you're done developing this codebase; see `README.md` for the
//! procedure.

use anyhow::{anyhow, Context, Result};
use esp_idf_svc::nvs::{EspDefaultNvs, EspDefaultNvsPartition};

/// NVS namespace used for all light-client keys. Keeping everything under
/// one namespace makes it easy to wipe just our data without touching
/// other ESP-IDF subsystems that may use NVS (WiFi calibration, PHY
/// tuning, etc.).
const NAMESPACE: &str = "udag";

// ── Key names (< 15 chars, NVS limit) ────────────────────────────────
const KEY_SEED: &str = "wallet_seed";    // 32 bytes
const KEY_SSID: &str = "wifi_ssid";      // UTF-8, ≤ 32 bytes
const KEY_PSK: &str = "wifi_pass";       // UTF-8, ≤ 64 bytes
const KEY_VAL_ROOT: &str = "val_root";   // 32 bytes — blake3 of sorted validator address set

/// Thin wrapper around an NVS handle + convenience getters/setters.
/// Keep a single instance alive for the life of the program — opening
/// and closing the NVS handle is cheap but pointless when the process
/// never exits.
pub struct Storage {
    nvs: EspDefaultNvs,
}

impl Storage {
    /// Open (or create) the `udag` NVS namespace with read-write access.
    /// Takes ownership of a reference to the default NVS partition, which
    /// the caller already has via `EspDefaultNvsPartition::take()`.
    pub fn open(partition: EspDefaultNvsPartition) -> Result<Self> {
        let nvs = EspDefaultNvs::new(partition, NAMESPACE, /*read_write=*/ true)
            .context("EspDefaultNvs::new on namespace 'udag' failed")?;
        Ok(Self { nvs })
    }

    // ── Ed25519 seed ──────────────────────────────────────────────────

    /// Read the persisted 32-byte Ed25519 seed, or `None` if absent.
    /// Returns `Err` only for actual NVS I/O errors; a missing key is
    /// treated as `Ok(None)` because "not set" is a normal case on a
    /// freshly-flashed chip.
    pub fn get_wallet_seed(&self) -> Result<Option<[u8; 32]>> {
        let mut buf = [0u8; 32];
        match self.nvs.get_blob(KEY_SEED, &mut buf) {
            Ok(Some(slice)) if slice.len() == 32 => Ok(Some(buf)),
            Ok(Some(slice)) => Err(anyhow!(
                "NVS wallet_seed has wrong length: {} bytes (expected 32)",
                slice.len()
            )),
            Ok(None) => Ok(None),
            Err(e) => Err(anyhow!("NVS get_blob(wallet_seed) failed: {:?}", e)),
        }
    }

    /// Write the 32-byte Ed25519 seed. Overwrites any existing value.
    /// Callers should gate this on `get_wallet_seed()?.is_none()` unless
    /// they explicitly want to rotate the wallet.
    pub fn set_wallet_seed(&mut self, seed: &[u8; 32]) -> Result<()> {
        self.nvs
            .set_blob(KEY_SEED, seed)
            .map_err(|e| anyhow!("NVS set_blob(wallet_seed) failed: {:?}", e))
    }

    // ── WiFi credentials ──────────────────────────────────────────────

    /// Read the persisted WiFi SSID (UTF-8). Returns `None` if unset.
    pub fn get_wifi_ssid(&self) -> Result<Option<String>> {
        read_str(&self.nvs, KEY_SSID)
    }

    /// Read the persisted WiFi password (UTF-8). Returns `None` if unset.
    pub fn get_wifi_password(&self) -> Result<Option<String>> {
        read_str(&self.nvs, KEY_PSK)
    }

    /// Write both WiFi credentials atomically. Used by the "provisioning"
    /// path (which we don't have yet — for now, the credentials in
    /// `config.rs` are copied to NVS on first boot to prime them).
    #[allow(dead_code)]
    pub fn set_wifi_credentials(&mut self, ssid: &str, password: &str) -> Result<()> {
        self.nvs
            .set_str(KEY_SSID, ssid)
            .map_err(|e| anyhow!("NVS set_str(wifi_ssid) failed: {:?}", e))?;
        self.nvs
            .set_str(KEY_PSK, password)
            .map_err(|e| anyhow!("NVS set_str(wifi_pass) failed: {:?}", e))?;
        Ok(())
    }

    // ── Validator-set trust root (TOFU) ───────────────────────────────

    /// Read the persisted 32-byte validator-set root hash.
    /// `blake3(sorted_addresses_concat)`.
    pub fn get_validator_root(&self) -> Result<Option<[u8; 32]>> {
        let mut buf = [0u8; 32];
        match self.nvs.get_blob(KEY_VAL_ROOT, &mut buf) {
            Ok(Some(slice)) if slice.len() == 32 => Ok(Some(buf)),
            Ok(Some(slice)) => Err(anyhow!(
                "NVS val_root has wrong length: {} bytes (expected 32)",
                slice.len()
            )),
            Ok(None) => Ok(None),
            Err(e) => Err(anyhow!("NVS get_blob(val_root) failed: {:?}", e)),
        }
    }

    /// Write the 32-byte validator-set root hash.
    pub fn set_validator_root(&mut self, root: &[u8; 32]) -> Result<()> {
        self.nvs
            .set_blob(KEY_VAL_ROOT, root)
            .map_err(|e| anyhow!("NVS set_blob(val_root) failed: {:?}", e))
    }
}

/// Shared string-read helper. NVS stores strings null-terminated, and the
/// Rust binding requires a user-provided buffer big enough to hold the
/// value + null terminator. We use 96 bytes which comfortably fits the
/// 64-byte max WPA2 passphrase with some slack.
fn read_str(nvs: &EspDefaultNvs, key: &str) -> Result<Option<String>> {
    let mut buf = [0u8; 96];
    match nvs.get_str(key, &mut buf) {
        Ok(Some(s)) => Ok(Some(s.to_string())),
        Ok(None) => Ok(None),
        Err(e) => Err(anyhow!("NVS get_str({}) failed: {:?}", key, e)),
    }
}

/// Fill a 32-byte buffer with cryptographically-strong random bytes from
/// ESP-IDF's hardware RNG. The bootloader enables an early-entropy RNG
/// source by default (see boot log `Enabling RNG early entropy source`),
/// and once WiFi/BT is up the RNG is seeded with radio noise too. By the
/// time we call this in `main()`, the RNG is production-grade. This is
/// the same RNG ESP-IDF mbedTLS uses under the hood.
pub fn random_seed() -> [u8; 32] {
    let mut seed = [0u8; 32];
    unsafe {
        esp_idf_svc::sys::esp_fill_random(
            seed.as_mut_ptr() as *mut core::ffi::c_void,
            seed.len(), // usize on esp-idf-sys bindings; matches C signature size_t
        );
    }
    seed
}

