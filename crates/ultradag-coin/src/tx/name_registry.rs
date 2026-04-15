use serde::{Deserialize, Serialize};

use crate::address::{Address, Signature};
use crate::constants::COIN;

// ────────────────────────────────────────────────────────────
// Constants
// ────────────────────────────────────────────────────────────

/// Minimum name length (inclusive).
pub const MIN_NAME_LENGTH: usize = 3;

/// Maximum name length (inclusive).
pub const MAX_NAME_LENGTH: usize = 20;

/// Rounds per year (~365.25 days at 5s rounds).
pub const ROUNDS_PER_YEAR: u64 = 6_307_200;

/// Grace period after expiration (30 days in rounds).
///
/// Applies ONLY to premium (3-5 char) names, which are rented annually.
/// Free-tier names (6+ chars) are perpetual — see `is_perpetual_name`.
pub const NAME_GRACE_PERIOD_ROUNDS: u64 = 518_400;

/// Sentinel expiry value used for perpetual (free-tier) names.
/// Never decremented; `process_name_expiry` skips entries bearing this value.
pub const PERPETUAL_EXPIRY: u64 = u64::MAX;

/// Maximum years for a single registration or renewal.
pub const MAX_REGISTRATION_YEARS: u8 = 5;

/// Maximum number of external addresses in a profile.
pub const MAX_PROFILE_EXTERNAL_ADDRESSES: usize = 10;

/// Maximum number of metadata entries in a profile.
pub const MAX_PROFILE_METADATA: usize = 10;

/// Maximum length for profile metadata keys.
pub const MAX_PROFILE_KEY_BYTES: usize = 32;

/// Maximum length for profile metadata values.
pub const MAX_PROFILE_VALUE_BYTES: usize = 256;

/// Maximum number of pockets per name.
/// Pockets let a name owner expose multiple labeled addresses under the same
/// name (e.g. `@alice.savings`, `@alice.business`). 10 is plenty for a
/// personal wallet and keeps per-name storage bounded.
pub const MAX_POCKETS: usize = 10;

/// Maximum pocket label length in bytes. Same charset as names
/// (`[a-z0-9-]`, no leading/trailing hyphen), so 32 is comfortable.
pub const MAX_POCKET_LABEL_BYTES: usize = 32;

/// Reserved names that cannot be registered.
const RESERVED_NAMES: &[&str] = &[
    "admin", "system", "null", "bridge", "treasury", "ultradag",
    "validator", "council", "governance", "faucet", "genesis",
    "root", "mod", "moderator", "support", "help", "info",
];

// ────────────────────────────────────────────────────────────
// Pricing
// ────────────────────────────────────────────────────────────

/// Compute the annual registration/renewal fee for a name based on length.
/// Standard names (6+ chars) are FREE AND PERMANENT — no fee, no expiry.
/// Premium short names (3-5 chars) are rented annually; fee paid to treasury.
/// All fees go to the DAO treasury.
pub fn name_annual_fee(name: &str) -> u64 {
    match name.len() {
        3 => 1_000 * COIN,        // 1,000 UDAG/yr — premium 3-char, rented
        4 => 500 * COIN,          //   500 UDAG/yr — premium 4-char, rented
        5 => 100 * COIN,          //   100 UDAG/yr — premium 5-char, rented
        6..=20 => 0,              // FREE and PERMANENT — standard names, own forever
        _ => u64::MAX,            // Invalid length — will be rejected by validation
    }
}

/// Whether a name belongs to the perpetual (free) tier.
///
/// Perpetual names never expire. The consensus rules for these names:
/// - Registration sets `expiry = PERPETUAL_EXPIRY` (u64::MAX).
/// - `resolve_name` skips the grace-period check entirely.
/// - `process_name_expiry` skips entries for these names even if they
///   were migrated from an older pre-perpetual format with a finite expiry.
/// - `apply_renew_name_tx` rejects renewal attempts (nothing to renew).
///
/// This makes the decision purely a function of name length, so old on-disk
/// state needs no migration: any previously-registered 6+ char name is
/// automatically promoted to perpetual at the next resolve.
pub fn is_perpetual_name(name: &str) -> bool {
    name_annual_fee(name) == 0
}

// ────────────────────────────────────────────────────────────
// Validation
// ────────────────────────────────────────────────────────────

/// Validate a name string. Returns Ok(()) or Err with reason.
pub fn validate_name(name: &str) -> Result<(), &'static str> {
    if name.len() < MIN_NAME_LENGTH {
        return Err("name too short (minimum 3 characters)");
    }
    if name.len() > MAX_NAME_LENGTH {
        return Err("name too long (maximum 20 characters)");
    }
    if name.starts_with('-') || name.ends_with('-') {
        return Err("name cannot start or end with a hyphen");
    }
    if name.contains("--") {
        return Err("name cannot contain consecutive hyphens");
    }
    if !name.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-') {
        return Err("name can only contain lowercase letters, numbers, and hyphens");
    }
    if RESERVED_NAMES.contains(&name) {
        return Err("name is reserved");
    }
    Ok(())
}

// ────────────────────────────────────────────────────────────
// Data structures
// ────────────────────────────────────────────────────────────

/// Validate a pocket label. Uses the same charset rules as names (minus the
/// reserved word check — labels are scoped under a parent name so they can
/// be called `admin.alice` without polluting the name registry).
pub fn validate_pocket_label(label: &str) -> Result<(), &'static str> {
    if label.is_empty() {
        return Err("pocket label cannot be empty");
    }
    if label.len() > MAX_POCKET_LABEL_BYTES {
        return Err("pocket label too long (max 32 bytes)");
    }
    if label.starts_with('-') || label.ends_with('-') {
        return Err("pocket label cannot start or end with a hyphen");
    }
    if label.contains("--") {
        return Err("pocket label cannot contain consecutive hyphens");
    }
    if !label.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-') {
        return Err("pocket label can only contain lowercase letters, numbers, and hyphens");
    }
    Ok(())
}

/// Derive a pocket address deterministically from the parent address + label.
///
/// `pocket_addr = blake3("ultradag_pocket" || parent_addr(20) || label_utf8)[..20]`
///
/// This is a pure function — no randomness, no signing keys. The same
/// parent + label always produce the same address. The parent SmartAccount's
/// authorized keys can sign for all derived pocket addresses (via the
/// pocket-to-parent delegation check in verify_smart_op / verify_smart_transfer).
pub fn derive_pocket_address(parent: &Address, label: &str) -> Address {
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"ultradag_pocket");
    hasher.update(&parent.0);
    hasher.update(label.as_bytes());
    let hash = hasher.finalize();
    let mut addr = [0u8; 20];
    addr.copy_from_slice(&hash.as_bytes()[..20]);
    Address(addr)
}

/// Optional profile data associated with a name.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NameProfile {
    /// Cross-chain addresses: ("eth", "0xabc..."), ("btc", "bc1..."), etc.
    pub external_addresses: Vec<(String, String)>,
    /// Public metadata: ("website", "https://..."), ("avatar", "https://..."), etc.
    pub metadata: Vec<(String, String)>,
    /// Legacy field — pockets are now on SmartAccountConfig, not here.
    /// Kept with serde(default) so old serialized profiles deserialize cleanly.
    #[serde(default)]
    pub _pockets_legacy: Vec<serde_json::Value>,
}

impl Default for NameProfile {
    fn default() -> Self {
        Self {
            external_addresses: Vec::new(),
            metadata: Vec::new(),
            _pockets_legacy: Vec::new(),
        }
    }
}

// ────────────────────────────────────────────────────────────
// Transaction types
// ────────────────────────────────────────────────────────────

/// Register a name for your address. Fee: tiered by name length (paid to treasury).
/// When `fee_payer` is present, the fee is paid by the fee_payer (sponsored registration).
/// The sender (`from`) still owns the name — the fee_payer just pays.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterNameTx {
    pub from: Address,
    pub name: String,
    pub duration_years: u8,
    /// Fee must be >= name_annual_fee(name) * duration_years.
    pub fee: u64,
    pub nonce: u64,
    pub pub_key: [u8; 32],
    pub signature: Signature,
    /// Optional: third-party fee payer. When present, fee is debited from fee_payer
    /// instead of from. Enables relay-sponsored name registration for new users.
    #[serde(default)]
    pub fee_payer: Option<crate::tx::smart_account::FeePayer>,
}

impl RegisterNameTx {
    pub fn signable_bytes(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(crate::constants::NETWORK_ID);
        buf.extend_from_slice(b"name_register");
        buf.extend_from_slice(&self.from.0);
        buf.extend_from_slice(&(self.name.len() as u32).to_le_bytes());
        buf.extend_from_slice(self.name.as_bytes());
        buf.extend_from_slice(&[self.duration_years]);
        buf.extend_from_slice(&self.fee.to_le_bytes());
        buf.extend_from_slice(&self.nonce.to_le_bytes());
        buf
    }

    pub fn hash(&self) -> [u8; 32] { *blake3::hash(&self.signable_bytes()).as_bytes() }
    pub fn total_cost(&self) -> u64 { self.fee }

    pub fn verify_signature(&self) -> bool {
        // The name owner (`from`) must always sign — even on sponsored
        // registrations. Without this, anyone with a funded fee_payer could
        // register arbitrary names to arbitrary victim addresses, since
        // `from` is a free-form field bound only by this signature
        // (GHSA-hf8w-rcvm-rgqr).
        //
        // The fee_payer's signature (when present) is additionally verified
        // in StateEngine::apply_register_name_tx — it authorizes the fee
        // debit, not the name assignment.
        let expected_addr = Address::from_pubkey(&self.pub_key);
        if expected_addr != self.from { return false; }
        let Ok(vk) = ed25519_dalek::VerifyingKey::from_bytes(&self.pub_key) else { return false; };
        let sig = ed25519_dalek::Signature::from_bytes(&self.signature.0);
        vk.verify_strict(&self.signable_bytes(), &sig).is_ok()
    }
}

/// Renew an existing name registration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenewNameTx {
    pub from: Address,
    pub name: String,
    pub additional_years: u8,
    pub fee: u64,
    pub nonce: u64,
    pub pub_key: [u8; 32],
    pub signature: Signature,
}

impl RenewNameTx {
    pub fn signable_bytes(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(crate::constants::NETWORK_ID);
        buf.extend_from_slice(b"name_renew");
        buf.extend_from_slice(&self.from.0);
        buf.extend_from_slice(&(self.name.len() as u32).to_le_bytes());
        buf.extend_from_slice(self.name.as_bytes());
        buf.extend_from_slice(&[self.additional_years]);
        buf.extend_from_slice(&self.fee.to_le_bytes());
        buf.extend_from_slice(&self.nonce.to_le_bytes());
        buf
    }

    pub fn hash(&self) -> [u8; 32] { *blake3::hash(&self.signable_bytes()).as_bytes() }
    pub fn total_cost(&self) -> u64 { self.fee }

    pub fn verify_signature(&self) -> bool {
        let expected_addr = Address::from_pubkey(&self.pub_key);
        if expected_addr != self.from { return false; }
        let Ok(vk) = ed25519_dalek::VerifyingKey::from_bytes(&self.pub_key) else { return false; };
        let sig = ed25519_dalek::Signature::from_bytes(&self.signature.0);
        vk.verify_strict(&self.signable_bytes(), &sig).is_ok()
    }
}

/// Transfer a name to a different address.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransferNameTx {
    pub from: Address,
    pub name: String,
    pub new_owner: Address,
    pub fee: u64,
    pub nonce: u64,
    pub pub_key: [u8; 32],
    pub signature: Signature,
}

impl TransferNameTx {
    pub fn signable_bytes(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(crate::constants::NETWORK_ID);
        buf.extend_from_slice(b"name_transfer");
        buf.extend_from_slice(&self.from.0);
        buf.extend_from_slice(&(self.name.len() as u32).to_le_bytes());
        buf.extend_from_slice(self.name.as_bytes());
        buf.extend_from_slice(&self.new_owner.0);
        buf.extend_from_slice(&self.fee.to_le_bytes());
        buf.extend_from_slice(&self.nonce.to_le_bytes());
        buf
    }

    pub fn hash(&self) -> [u8; 32] { *blake3::hash(&self.signable_bytes()).as_bytes() }
    pub fn total_cost(&self) -> u64 { self.fee }

    pub fn verify_signature(&self) -> bool {
        let expected_addr = Address::from_pubkey(&self.pub_key);
        if expected_addr != self.from { return false; }
        let Ok(vk) = ed25519_dalek::VerifyingKey::from_bytes(&self.pub_key) else { return false; };
        let sig = ed25519_dalek::Signature::from_bytes(&self.signature.0);
        vk.verify_strict(&self.signable_bytes(), &sig).is_ok()
    }
}

/// Update cross-chain profile data for a name.
/// Pockets are no longer managed here — use SmartOpType::CreatePocket instead.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateProfileTx {
    pub from: Address,
    pub name: String,
    pub external_addresses: Vec<(String, String)>,
    pub metadata: Vec<(String, String)>,
    pub fee: u64,
    pub nonce: u64,
    pub pub_key: [u8; 32],
    pub signature: Signature,
}

impl UpdateProfileTx {
    pub fn signable_bytes(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(crate::constants::NETWORK_ID);
        buf.extend_from_slice(b"name_update_profile");
        buf.extend_from_slice(&self.from.0);
        buf.extend_from_slice(&(self.name.len() as u32).to_le_bytes());
        buf.extend_from_slice(self.name.as_bytes());
        buf.extend_from_slice(&(self.external_addresses.len() as u32).to_le_bytes());
        for (chain, addr) in &self.external_addresses {
            buf.extend_from_slice(&(chain.len() as u32).to_le_bytes());
            buf.extend_from_slice(chain.as_bytes());
            buf.extend_from_slice(&(addr.len() as u32).to_le_bytes());
            buf.extend_from_slice(addr.as_bytes());
        }
        buf.extend_from_slice(&(self.metadata.len() as u32).to_le_bytes());
        for (key, val) in &self.metadata {
            buf.extend_from_slice(&(key.len() as u32).to_le_bytes());
            buf.extend_from_slice(key.as_bytes());
            buf.extend_from_slice(&(val.len() as u32).to_le_bytes());
            buf.extend_from_slice(val.as_bytes());
        }
        buf.extend_from_slice(&self.fee.to_le_bytes());
        buf.extend_from_slice(&self.nonce.to_le_bytes());
        buf
    }

    pub fn hash(&self) -> [u8; 32] { *blake3::hash(&self.signable_bytes()).as_bytes() }
    pub fn total_cost(&self) -> u64 { self.fee }

    pub fn verify_signature(&self) -> bool {
        let expected_addr = Address::from_pubkey(&self.pub_key);
        if expected_addr != self.from { return false; }
        let Ok(vk) = ed25519_dalek::VerifyingKey::from_bytes(&self.pub_key) else { return false; };
        let sig = ed25519_dalek::Signature::from_bytes(&self.signature.0);
        vk.verify_strict(&self.signable_bytes(), &sig).is_ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_name_valid() {
        assert!(validate_name("alice").is_ok());
        assert!(validate_name("john29").is_ok());
        assert!(validate_name("coffee-shop").is_ok());
        assert!(validate_name("abc").is_ok());
        assert!(validate_name("a1b2c3d4e5f6g7h8i9j0").is_ok()); // 20 chars
    }

    #[test]
    fn test_validate_name_invalid() {
        assert_eq!(validate_name("ab"), Err("name too short (minimum 3 characters)"));
        assert_eq!(validate_name("A"), Err("name too short (minimum 3 characters)"));
        assert!(validate_name("ABC").is_err()); // uppercase
        assert!(validate_name("-test").is_err()); // starts with hyphen
        assert!(validate_name("test-").is_err()); // ends with hyphen
        assert!(validate_name("te--st").is_err()); // consecutive hyphens
        assert!(validate_name("te st").is_err()); // space
        assert!(validate_name("admin").is_err()); // reserved
        assert!(validate_name("treasury").is_err()); // reserved
        let long = "a".repeat(21);
        assert!(validate_name(&long).is_err()); // too long
    }

    #[test]
    fn test_name_annual_fee_tiers() {
        assert_eq!(name_annual_fee("abc"), 1_000 * COIN);      // 3 chars — premium
        assert_eq!(name_annual_fee("john"), 500 * COIN);        // 4 chars — premium
        assert_eq!(name_annual_fee("alice"), 100 * COIN);       // 5 chars — premium
        assert_eq!(name_annual_fee("john29"), 0);                // 6 chars — FREE
        assert_eq!(name_annual_fee("coffeeshop"), 0);            // 10 chars — FREE
        assert_eq!(name_annual_fee("coffeeshopnyc"), 0);         // 13 chars — FREE
    }

    #[test]
    fn test_register_name_tx_sign_verify() {
        use crate::address::SecretKey;
        let sk = SecretKey::from_bytes([0x42; 32]);
        let mut tx = RegisterNameTx {
            from: sk.address(),
            name: "alice".to_string(),
            duration_years: 1,
            fee: 100 * COIN,
            nonce: 0,
            pub_key: sk.verifying_key().to_bytes(),
            signature: Signature([0u8; 64]),
            fee_payer: None,
        };
        tx.signature = sk.sign(&tx.signable_bytes());
        assert!(tx.verify_signature());
    }
}
