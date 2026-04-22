use serde::{Deserialize, Serialize};

use crate::address::{Address, Signature};

// ────────────────────────────────────────────────────────────
// Sponsored transactions (fee-payer)
// ────────────────────────────────────────────────────────────

/// A third-party fee payer for sponsored transactions.
/// When present on a transaction, the fee is debited from the fee_payer's address
/// instead of the sender's address. This enables:
/// - Relay-sponsored account creation (new user has no UDAG)
/// - Relay-sponsored name registration
/// - Any third party paying fees on behalf of a user
///
/// The fee_payer must sign the SAME signable_bytes as the primary signer,
/// proving they authorized the fee payment for this specific transaction.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FeePayer {
    /// Address of the fee payer.
    pub address: Address,
    /// Fee payer's Ed25519 public key (must hash to address).
    pub pub_key: [u8; 32],
    /// Fee payer's signature over the transaction's signable_bytes.
    pub signature: Signature,
    /// Fee payer's nonce (incremented on their account).
    pub nonce: u64,
}

impl FeePayer {
    /// Verify the fee payer's Ed25519 signature over the given message.
    pub fn verify(&self, message: &[u8]) -> bool {
        if Address::from_pubkey(&self.pub_key) != self.address {
            return false;
        }
        let Ok(vk) = ed25519_dalek::VerifyingKey::from_bytes(&self.pub_key) else {
            return false;
        };
        let sig = ed25519_dalek::Signature::from_bytes(&self.signature.0);
        vk.verify_strict(message, &sig).is_ok()
    }
}

/// Maximum number of authorized keys per SmartAccount.
pub const MAX_AUTHORIZED_KEYS: usize = 10;

/// Maximum label length in bytes.
pub const MAX_KEY_LABEL_BYTES: usize = 32;

/// Time-lock delay for key removal (same as unstake cooldown: ~2.8 hours at 5s rounds).
pub const KEY_REMOVAL_DELAY_ROUNDS: u64 = 2_016;

// ────────────────────────────────────────────────────────────
// Signature verification
// ────────────────────────────────────────────────────────────

/// Verify a P256 (secp256r1) signature over a message.
/// Accepts both DER-encoded and raw (r || s, 64 bytes) signature formats.
/// Public key can be compressed (33 bytes) or uncompressed (65 bytes) SEC1.
pub fn verify_p256(pubkey: &[u8], sig_bytes: &[u8], message: &[u8]) -> bool {
    use p256::ecdsa::{signature::Verifier, Signature as P256Sig, VerifyingKey};

    let vk = match VerifyingKey::from_sec1_bytes(pubkey) {
        Ok(k) => k,
        Err(_) => return false,
    };

    // Try DER format first (variable length, typically 70-72 bytes)
    if let Ok(sig) = P256Sig::from_der(sig_bytes) {
        return vk.verify(message, &sig).is_ok();
    }

    // Try raw r||s format (exactly 64 bytes)
    if let Ok(sig) = P256Sig::try_from(sig_bytes) {
        return vk.verify(message, &sig).is_ok();
    }

    false
}

/// Verify P256 with a pre-hashed message (for WebAuthn where the message is already
/// the hash that ECDSA should verify against, without the library hashing it again).
/// Verify P256 with a pre-hashed message digest (32 bytes).
/// Used for WebAuthn where the signature is over SHA-256(authenticatorData || clientDataHash)
/// and the p256 crate's `verify()` would double-hash.
pub fn verify_p256_prehashed(pubkey: &[u8], sig_bytes: &[u8], prehash: &[u8]) -> bool {
    use p256::ecdsa::{Signature as P256Sig, VerifyingKey};
    use p256::ecdsa::signature::hazmat::PrehashVerifier;

    let vk = match VerifyingKey::from_sec1_bytes(pubkey) {
        Ok(k) => k,
        Err(_) => return false,
    };

    if let Ok(sig) = P256Sig::try_from(sig_bytes) {
        return vk.verify_prehash(prehash, &sig).is_ok();
    }
    if let Ok(sig) = P256Sig::from_der(sig_bytes) {
        return vk.verify_prehash(prehash, &sig).is_ok();
    }
    false
}

/// Verify an Ed25519 signature over a message.
/// Used by SmartAccount verification dispatch (same as existing verify_strict).
pub fn verify_ed25519(pubkey: &[u8], sig_bytes: &[u8], message: &[u8]) -> bool {
    if pubkey.len() != 32 || sig_bytes.len() != 64 {
        return false;
    }
    let Ok(vk) = ed25519_dalek::VerifyingKey::from_bytes(pubkey.try_into().unwrap()) else {
        return false;
    };
    let sig = ed25519_dalek::Signature::from_bytes(sig_bytes.try_into().unwrap());
    vk.verify_strict(message, &sig).is_ok()
}

/// Verify a signature using the appropriate curve based on key type.
pub fn verify_by_key_type(key_type: KeyType, pubkey: &[u8], sig_bytes: &[u8], message: &[u8]) -> bool {
    match key_type {
        KeyType::Ed25519 => verify_ed25519(pubkey, sig_bytes, message),
        KeyType::P256 => verify_p256(pubkey, sig_bytes, message),
    }
}

// ────────────────────────────────────────────────────────────
// WebAuthn verification
// ────────────────────────────────────────────────────────────

/// WebAuthn signature envelope. Carries the browser's authenticator data
/// and client data JSON alongside the P256 signature.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WebAuthnSignature {
    /// Authenticator data from the WebAuthn response (37+ bytes).
    pub authenticator_data: Vec<u8>,
    /// Client data JSON from the WebAuthn response (contains the challenge).
    pub client_data_json: Vec<u8>,
    /// P256 ECDSA signature (64 bytes raw or DER encoded).
    pub signature: Vec<u8>,
}

/// Verify a WebAuthn P256 signature.
///
/// WebAuthn flow:
/// 1. Client computes challenge = SHA-256(signable_bytes)
/// 2. Browser signs SHA-256(authenticatorData || SHA-256(clientDataJSON))
/// 3. Validator reconstructs the signed message and verifies:
///    - The challenge in clientDataJSON matches SHA-256(signable_bytes)
///    - The P256 signature verifies over SHA-256(authenticatorData || clientDataHash)
pub fn verify_webauthn(
    pubkey: &[u8],
    webauthn: &WebAuthnSignature,
    signable_bytes: &[u8],
) -> bool {
    use sha2::{Sha256, Digest};

    // 1. Compute expected challenge = SHA-256(signable_bytes)
    let expected_challenge = Sha256::digest(signable_bytes);
    let expected_challenge_b64 = base64url_encode(&expected_challenge);

    // 2. Parse clientDataJSON and verify the challenge matches
    // clientDataJSON is a JSON object like: {"type":"webauthn.get","challenge":"<base64url>","origin":"..."}
    let client_data_str = match std::str::from_utf8(&webauthn.client_data_json) {
        Ok(s) => s,
        Err(_) => return false,
    };

    // Extract challenge field from JSON (simple extraction — no full JSON parser dependency)
    let challenge_value = match extract_json_string(client_data_str, "challenge") {
        Some(c) => c,
        None => return false,
    };

    if challenge_value != expected_challenge_b64 {
        tracing::warn!("WebAuthn challenge mismatch: expected={} got={} signable_len={}", expected_challenge_b64, challenge_value, signable_bytes.len());
        return false;
    }

    // 3. Compute clientDataHash = SHA-256(clientDataJSON)
    let client_data_hash = Sha256::digest(&webauthn.client_data_json);

    // 4. Build signed data = authenticatorData || clientDataHash
    // WebAuthn spec: the signature is over this raw concatenation.
    // P256 ECDSA internally applies SHA-256 during verification.
    let mut signed_data = Vec::with_capacity(webauthn.authenticator_data.len() + 32);
    signed_data.extend_from_slice(&webauthn.authenticator_data);
    signed_data.extend_from_slice(&client_data_hash);

    // 5. Try both verification approaches:
    // Method A: standard verify (p256 crate SHA-256s the message internally)
    if verify_p256(pubkey, &webauthn.signature, &signed_data) {
        return true;
    }
    // Method B: prehashed verify (if browser's ECDSA already hashed,
    // we need to pass the raw hash to avoid double-hashing)
    let prehash = Sha256::digest(&signed_data);
    if verify_p256_prehashed(pubkey, &webauthn.signature, &prehash) {
        return true;
    }
    tracing::warn!("WebAuthn P256 verify FAILED both methods: pubkey_len={} sig_len={} signed_data_len={}", pubkey.len(), webauthn.signature.len(), signed_data.len());
    false
}

/// Base64url encode (no padding) — for WebAuthn challenge comparison.
fn base64url_encode(data: &[u8]) -> String {
    let mut result = String::new();
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut i = 0;
    while i < data.len() {
        let b0 = data[i] as usize;
        let b1 = if i + 1 < data.len() { data[i + 1] as usize } else { 0 };
        let b2 = if i + 2 < data.len() { data[i + 2] as usize } else { 0 };

        result.push(ALPHABET[(b0 >> 2)] as char);
        result.push(ALPHABET[((b0 & 0x03) << 4) | (b1 >> 4)] as char);
        if i + 1 < data.len() {
            result.push(ALPHABET[((b1 & 0x0f) << 2) | (b2 >> 6)] as char);
        }
        if i + 2 < data.len() {
            result.push(ALPHABET[(b2 & 0x3f)] as char);
        }
        i += 3;
    }
    result
}

/// Simple JSON string field extraction (avoids pulling in a JSON parser dependency).
fn extract_json_string<'a>(json: &'a str, key: &str) -> Option<&'a str> {
    let pattern = format!("\"{}\"", key);
    let key_pos = json.find(&pattern)?;
    let after_key = &json[key_pos + pattern.len()..];
    // Skip whitespace and colon
    let after_colon = after_key.trim_start().strip_prefix(':')?;
    let after_ws = after_colon.trim_start();
    // Extract quoted string value
    let after_quote = after_ws.strip_prefix('"')?;
    let end_quote = after_quote.find('"')?;
    Some(&after_quote[..end_quote])
}

// ────────────────────────────────────────────────────────────
// Core types
// ────────────────────────────────────────────────────────────

/// Key algorithm type. Ed25519 is the existing UltraDAG key type.
/// P256 is used by WebAuthn/passkeys (secure enclave, FIDO2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum KeyType {
    Ed25519 = 0,
    P256 = 1,
}

/// An authorized key registered to a SmartAccount.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AuthorizedKey {
    /// Unique identifier: blake3(key_type_byte || pubkey)[..8]
    pub key_id: [u8; 8],
    /// Algorithm type.
    pub key_type: KeyType,
    /// Raw public key bytes. 32 bytes for Ed25519, 33 bytes for compressed P256.
    pub pubkey: Vec<u8>,
    /// Human-readable label (max 32 bytes UTF-8), e.g. "iPhone", "YubiKey".
    pub label: String,
    /// Optional per-key daily spending limit in sats. None = no per-key limit.
    pub daily_limit: Option<u64>,
    /// Daily spending tracker: (day_start_round, amount_spent_this_day).
    /// A "day" is 17,280 rounds (~24 hours at 5s/round).
    #[serde(default)]
    pub daily_spent: (u64, u64),
}

impl AuthorizedKey {
    /// Compute a key_id from key type and public key bytes.
    pub fn compute_key_id(key_type: KeyType, pubkey: &[u8]) -> [u8; 8] {
        let mut hasher = blake3::Hasher::new();
        hasher.update(&[key_type as u8]);
        hasher.update(pubkey);
        let hash = hasher.finalize();
        let mut id = [0u8; 8];
        id.copy_from_slice(&hash.as_bytes()[..8]);
        id
    }

    /// Validate the key's fields are well-formed.
    pub fn validate(&self) -> Result<(), &'static str> {
        // Verify key_id matches pubkey
        let expected_id = Self::compute_key_id(self.key_type, &self.pubkey);
        if self.key_id != expected_id {
            return Err("key_id does not match pubkey");
        }
        // Check pubkey length matches key type
        match self.key_type {
            KeyType::Ed25519 => {
                if self.pubkey.len() != 32 {
                    return Err("Ed25519 pubkey must be 32 bytes");
                }
            }
            KeyType::P256 => {
                if self.pubkey.len() != 33 && self.pubkey.len() != 65 {
                    return Err("P256 pubkey must be 33 (compressed) or 65 (uncompressed) bytes");
                }
            }
        }
        // Check label length
        if self.label.len() > MAX_KEY_LABEL_BYTES {
            return Err("key label exceeds 32 bytes");
        }
        Ok(())
    }
}

/// SmartAccount configuration stored per-address in StateEngine.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SmartAccountConfig {
    /// Authorized keys that can sign transactions for this account (max 10).
    pub authorized_keys: Vec<AuthorizedKey>,
    /// Social recovery configuration (guardians + threshold + delay).
    #[serde(default)]
    pub recovery: Option<RecoveryConfig>,
    /// Pending recovery attempt (time-locked key rotation initiated by guardians).
    #[serde(default)]
    pub pending_recovery: Option<PendingRecovery>,
    /// Spending policy (daily limits, vault thresholds, whitelisted recipients).
    #[serde(default)]
    pub policy: Option<SpendingPolicy>,
    /// Pending time-locked policy change, if any.
    #[serde(default)]
    pub pending_policy_change: Option<PendingPolicyChange>,
    /// Pending vault transfers (max 5, time-locked large transfers).
    #[serde(default)]
    pub pending_vault_transfers: Vec<PendingVaultTransfer>,
    /// Pending time-locked key removal, if any.
    pub pending_key_removal: Option<PendingKeyRemoval>,
    /// Round when this SmartAccount was created.
    pub created_at_round: u64,
    /// Pocket labels — derived sub-addresses controlled by this account's keys.
    /// Each label derives a deterministic address via `derive_pocket_address(parent, label)`.
    /// The parent's authorized keys can sign for all pocket addresses.
    #[serde(default)]
    pub pockets: Vec<String>,
}

impl SmartAccountConfig {
    /// Create a new SmartAccount with no keys (receive-only).
    pub fn new(created_at_round: u64) -> Self {
        Self {
            authorized_keys: Vec::new(),
            recovery: None,
            pending_recovery: None,
            policy: None,
            pending_policy_change: None,
            pending_vault_transfers: Vec::new(),
            pending_key_removal: None,
            created_at_round,
            pockets: Vec::new(),
        }
    }

    /// Find an authorized key by key_id.
    pub fn find_key(&self, key_id: &[u8; 8]) -> Option<&AuthorizedKey> {
        self.authorized_keys.iter().find(|k| &k.key_id == key_id)
    }

    /// Check if a key_id is already registered.
    pub fn has_key(&self, key_id: &[u8; 8]) -> bool {
        self.authorized_keys.iter().any(|k| &k.key_id == key_id)
    }
}

/// A pending time-locked key removal.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PendingKeyRemoval {
    /// The key_id being removed.
    pub key_id: [u8; 8],
    /// Round when removal was initiated.
    pub initiated_at_round: u64,
    /// Round when removal can be executed.
    pub executes_at_round: u64,
}

// ────────────────────────────────────────────────────────────
// Recovery types
// ────────────────────────────────────────────────────────────

/// Maximum number of guardians per SmartAccount.
pub const MAX_GUARDIANS: usize = 10;

/// Minimum recovery delay in rounds (~1 hour at 5s rounds).
pub const MIN_RECOVERY_DELAY_ROUNDS: u64 = 720;

/// Default recovery delay in rounds (~2.8 hours at 5s rounds).
pub const DEFAULT_RECOVERY_DELAY_ROUNDS: u64 = 2_016;

/// Maximum recovery delay in rounds (~3.5 days at 5s rounds).
pub const MAX_RECOVERY_DELAY_ROUNDS: u64 = 60_480;

/// Maximum time a pending recovery can stay open before auto-expiry (7 days).
pub const RECOVERY_EXPIRY_ROUNDS: u64 = 120_960;

/// Social recovery configuration.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RecoveryConfig {
    /// Guardian addresses (any UltraDAG address — friend, hardware wallet, service).
    pub guardians: Vec<Address>,
    /// Number of guardians required to approve recovery (e.g., 3 of 5).
    pub threshold: u8,
    /// Time-lock delay after threshold is reached before recovery executes.
    pub delay_rounds: u64,
}

/// A pending recovery attempt initiated by guardians.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PendingRecovery {
    /// The new key to authorize after recovery completes.
    pub new_key: AuthorizedKey,
    /// Guardian addresses that have approved this recovery.
    pub approvals: Vec<Address>,
    /// Round when recovery was initiated (first guardian approval).
    pub initiated_at_round: u64,
    /// Round when recovery can be executed (after time-lock).
    pub executes_at_round: u64,
    /// Whether to revoke all existing keys when recovery completes.
    pub revoke_existing: bool,
}

// ────────────────────────────────────────────────────────────
// Spending policy types
// ────────────────────────────────────────────────────────────

/// Maximum number of whitelisted recipient addresses.
pub const MAX_WHITELISTED_RECIPIENTS: usize = 20;

/// Maximum number of pending vault transfers per account.
pub const MAX_PENDING_VAULT_TRANSFERS: usize = 5;

/// Rounds per "day" for daily limit tracking (~24 hours at 5s/round).
pub const ROUNDS_PER_DAY: u64 = 17_280;

/// Time-lock delay for policy changes (~2.8 hours, same as key removal).
pub const POLICY_CHANGE_DELAY_ROUNDS: u64 = 2_016;

/// Spending policy for a SmartAccount.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SpendingPolicy {
    /// Transfers below this amount execute instantly (no vault delay).
    pub instant_limit: u64,
    /// Transfers above this amount require vault time-lock.
    pub vault_threshold: u64,
    /// How long vault transfers wait before execution.
    pub vault_delay_rounds: u64,
    /// Addresses that bypass all limits and vault delays (max 20).
    pub whitelisted_recipients: Vec<Address>,
    /// Global daily spending cap (None = no daily limit).
    pub daily_limit: Option<u64>,
    /// Daily spending tracker: (day_start_round, total_spent_this_day).
    #[serde(default)]
    pub daily_spent: (u64, u64),
}

/// A pending time-locked large transfer.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PendingVaultTransfer {
    pub to: Address,
    pub amount: u64,
    pub fee: u64,
    pub created_at_round: u64,
    pub executes_at_round: u64,
    /// Unique transfer ID: blake3(from || to || amount || nonce)[..8]
    pub transfer_id: [u8; 8],
    /// Origin address of the funds. May be a pocket whose policy is
    /// enforced on the parent (held here so cancel refunds to the
    /// right surface).
    #[serde(default)]
    pub from: Address,
}

/// A pending time-locked spending policy change.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PendingPolicyChange {
    pub new_policy: SpendingPolicy,
    pub initiated_at_round: u64,
    pub executes_at_round: u64,
}

// ────────────────────────────────────────────────────────────
// Transaction types
// ────────────────────────────────────────────────────────────

/// Register an additional authorized key to a SmartAccount.
/// Must be signed by an existing authorized key (Ed25519 for Phase 1).
/// Fee: MIN_FEE.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddKeyTx {
    /// SmartAccount address.
    pub from: Address,
    /// The new key to authorize.
    pub new_key: AuthorizedKey,
    /// Transaction fee.
    pub fee: u64,
    /// Sender nonce.
    pub nonce: u64,
    /// Ed25519 public key of the signer (must be an authorized key or the legacy key).
    pub pub_key: [u8; 32],
    /// Ed25519 signature.
    pub signature: Signature,
}

impl AddKeyTx {
    pub fn signable_bytes(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(crate::constants::NETWORK_ID);
        buf.extend_from_slice(b"smart_add_key");
        buf.extend_from_slice(&self.from.0);
        buf.extend_from_slice(&self.new_key.key_id);
        buf.extend_from_slice(&[self.new_key.key_type as u8]);
        buf.extend_from_slice(&(self.new_key.pubkey.len() as u32).to_le_bytes());
        buf.extend_from_slice(&self.new_key.pubkey);
        buf.extend_from_slice(&(self.new_key.label.len() as u32).to_le_bytes());
        buf.extend_from_slice(self.new_key.label.as_bytes());
        buf.extend_from_slice(&self.fee.to_le_bytes());
        buf.extend_from_slice(&self.nonce.to_le_bytes());
        buf
    }

    pub fn hash(&self) -> [u8; 32] {
        *blake3::hash(&self.signable_bytes()).as_bytes()
    }

    pub fn verify_signature(&self) -> bool {
        // For Phase 1: Ed25519 only. The pub_key must either:
        // 1. Hash to the `from` address (legacy path), OR
        // 2. Be an authorized key on the SmartAccount (checked in StateEngine)
        //
        // Here we only verify the cryptographic signature is valid.
        // The authorization check (is this key allowed?) happens in StateEngine.
        let Ok(vk) = ed25519_dalek::VerifyingKey::from_bytes(&self.pub_key) else {
            return false;
        };
        let sig = ed25519_dalek::Signature::from_bytes(&self.signature.0);
        vk.verify_strict(&self.signable_bytes(), &sig).is_ok()
    }

    pub fn total_cost(&self) -> u64 {
        self.fee
    }
}

/// Initiate time-locked removal of an authorized key from a SmartAccount.
/// The removal executes after KEY_REMOVAL_DELAY_ROUNDS.
/// Cannot remove the last key — must add a replacement first.
/// Fee-exempt (like unstake).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoveKeyTx {
    /// SmartAccount address.
    pub from: Address,
    /// The key_id to remove.
    pub key_id_to_remove: [u8; 8],
    /// Sender nonce.
    pub nonce: u64,
    /// Ed25519 public key of the signer (must be an authorized key).
    pub pub_key: [u8; 32],
    /// Ed25519 signature.
    pub signature: Signature,
}

impl RemoveKeyTx {
    pub fn signable_bytes(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(crate::constants::NETWORK_ID);
        buf.extend_from_slice(b"smart_remove_key");
        buf.extend_from_slice(&self.from.0);
        buf.extend_from_slice(&self.key_id_to_remove);
        buf.extend_from_slice(&self.nonce.to_le_bytes());
        buf
    }

    pub fn hash(&self) -> [u8; 32] {
        *blake3::hash(&self.signable_bytes()).as_bytes()
    }

    pub fn verify_signature(&self) -> bool {
        // Cryptographic verification only. Authorization checked in StateEngine.
        let Ok(vk) = ed25519_dalek::VerifyingKey::from_bytes(&self.pub_key) else {
            return false;
        };
        let sig = ed25519_dalek::Signature::from_bytes(&self.signature.0);
        vk.verify_strict(&self.signable_bytes(), &sig).is_ok()
    }
}

// ────────────────────────────────────────────────────────────
// SmartTransfer — transfer signed by any SmartAccount key (Ed25519 or P256)
// ────────────────────────────────────────────────────────────

/// A transfer signed by a SmartAccount key. Unlike TransferTx (which is hardcoded to
/// Ed25519 with blake3(pub_key)==from verification), SmartTransferTx uses a key_id
/// to look up the authorized key from state and supports both Ed25519 and P256.
///
/// This is the P256/passkey-compatible transfer path. Legacy TransferTx continues
/// to work for Ed25519-only accounts.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SmartTransferTx {
    /// SmartAccount sender address.
    pub from: Address,
    /// Recipient address.
    pub to: Address,
    /// Transfer amount in sats.
    pub amount: u64,
    /// Transaction fee in sats.
    pub fee: u64,
    /// Sender nonce.
    pub nonce: u64,
    /// Which authorized key signed this transaction (blake3(key_type || pubkey)[..8]).
    pub signing_key_id: [u8; 8],
    /// Signature bytes. 64 bytes for Ed25519, 64-72 bytes for P256 (DER or raw r||s).
    /// For WebAuthn, this field is empty and `webauthn` carries the full envelope.
    pub signature: Vec<u8>,
    /// Optional memo (max 256 bytes).
    #[serde(default)]
    pub memo: Option<Vec<u8>>,
    /// Optional WebAuthn signature envelope. When present, the `signature` field is
    /// ignored and the P256 signature is extracted from the WebAuthn response.
    #[serde(default)]
    pub webauthn: Option<WebAuthnSignature>,
}

impl SmartTransferTx {
    pub fn signable_bytes(&self) -> Vec<u8> {
        let memo_len = self.memo.as_ref().map(|m| m.len()).unwrap_or(0);
        let mut buf = Vec::with_capacity(100 + memo_len);
        buf.extend_from_slice(crate::constants::NETWORK_ID);
        buf.extend_from_slice(b"smart_transfer");
        buf.extend_from_slice(&self.from.0);
        buf.extend_from_slice(&self.to.0);
        buf.extend_from_slice(&self.amount.to_le_bytes());
        buf.extend_from_slice(&self.fee.to_le_bytes());
        buf.extend_from_slice(&self.nonce.to_le_bytes());
        buf.extend_from_slice(&self.signing_key_id);
        if let Some(ref memo) = self.memo {
            buf.extend_from_slice(&(memo.len() as u32).to_le_bytes());
            buf.extend_from_slice(memo);
        }
        buf
    }

    pub fn hash(&self) -> [u8; 32] {
        *blake3::hash(&self.signable_bytes()).as_bytes()
    }

    pub fn total_cost(&self) -> u64 {
        self.amount.saturating_add(self.fee)
    }

    /// Verify the cryptographic signature using the provided key.
    /// Called by StateEngine after looking up the authorized key from state.
    pub fn verify_with_key(&self, key: &AuthorizedKey) -> bool {
        if let Some(ref memo) = self.memo {
            if memo.len() > crate::constants::MAX_MEMO_BYTES {
                return false;
            }
        }
        // WebAuthn path: verify the full WebAuthn assertion chain
        if let Some(ref webauthn) = self.webauthn {
            if key.key_type != KeyType::P256 {
                return false; // WebAuthn only works with P256 keys
            }
            return verify_webauthn(&key.pubkey, webauthn, &self.signable_bytes());
        }
        // Standard path: raw signature
        verify_by_key_type(key.key_type, &key.pubkey, &self.signature, &self.signable_bytes())
    }

    /// Stateless verify_signature — always returns false because SmartTransferTx
    /// requires state to look up the authorized key. The actual verification
    /// happens in StateEngine::verify_smart_transfer().
    pub fn verify_signature(&self) -> bool {
        // SmartTransfer requires state access for key lookup.
        // This method exists only to satisfy the Transaction enum dispatch pattern.
        // The real verification is in StateEngine.
        false
    }
}

// ────────────────────────────────────────────────────────────
// Recovery transaction types
// ────────────────────────────────────────────────────────────

/// Configure social recovery guardians for a SmartAccount.
/// Signed by an existing authorized key. Fee: MIN_FEE.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetRecoveryTx {
    pub from: Address,
    pub guardians: Vec<Address>,
    pub threshold: u8,
    pub delay_rounds: u64,
    pub fee: u64,
    pub nonce: u64,
    pub pub_key: [u8; 32],
    pub signature: Signature,
}

impl SetRecoveryTx {
    pub fn signable_bytes(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(crate::constants::NETWORK_ID);
        buf.extend_from_slice(b"smart_set_recovery");
        buf.extend_from_slice(&self.from.0);
        buf.extend_from_slice(&(self.guardians.len() as u32).to_le_bytes());
        for g in &self.guardians {
            buf.extend_from_slice(&g.0);
        }
        buf.extend_from_slice(&[self.threshold]);
        buf.extend_from_slice(&self.delay_rounds.to_le_bytes());
        buf.extend_from_slice(&self.fee.to_le_bytes());
        buf.extend_from_slice(&self.nonce.to_le_bytes());
        buf
    }

    pub fn hash(&self) -> [u8; 32] {
        *blake3::hash(&self.signable_bytes()).as_bytes()
    }

    pub fn total_cost(&self) -> u64 {
        self.fee
    }

    pub fn verify_signature(&self) -> bool {
        let Ok(vk) = ed25519_dalek::VerifyingKey::from_bytes(&self.pub_key) else {
            return false;
        };
        let sig = ed25519_dalek::Signature::from_bytes(&self.signature.0);
        vk.verify_strict(&self.signable_bytes(), &sig).is_ok()
    }
}

/// Guardian submits approval for account recovery.
/// When threshold guardians submit matching RecoverAccountTx (same new_key + revoke_existing),
/// the time-lock countdown begins.
/// Fee-exempt — guardians should not pay to help someone recover.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecoverAccountTx {
    /// The account being recovered.
    pub target_account: Address,
    /// The guardian submitting this approval.
    pub from: Address,
    /// The new key to authorize after recovery.
    pub new_key: AuthorizedKey,
    /// Whether to revoke all existing keys.
    pub revoke_existing: bool,
    pub nonce: u64,
    pub pub_key: [u8; 32],
    pub signature: Signature,
}

impl RecoverAccountTx {
    pub fn signable_bytes(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(crate::constants::NETWORK_ID);
        buf.extend_from_slice(b"smart_recover_account");
        buf.extend_from_slice(&self.target_account.0);
        buf.extend_from_slice(&self.from.0);
        buf.extend_from_slice(&self.new_key.key_id);
        buf.extend_from_slice(&[self.new_key.key_type as u8]);
        buf.extend_from_slice(&(self.new_key.pubkey.len() as u32).to_le_bytes());
        buf.extend_from_slice(&self.new_key.pubkey);
        buf.extend_from_slice(&[self.revoke_existing as u8]);
        buf.extend_from_slice(&self.nonce.to_le_bytes());
        buf
    }

    pub fn hash(&self) -> [u8; 32] {
        *blake3::hash(&self.signable_bytes()).as_bytes()
    }

    pub fn verify_signature(&self) -> bool {
        let expected_addr = Address::from_pubkey(&self.pub_key);
        if expected_addr != self.from {
            return false;
        }
        let Ok(vk) = ed25519_dalek::VerifyingKey::from_bytes(&self.pub_key) else {
            return false;
        };
        let sig = ed25519_dalek::Signature::from_bytes(&self.signature.0);
        vk.verify_strict(&self.signable_bytes(), &sig).is_ok()
    }
}

/// Cancel a pending recovery attempt. Must be signed by an existing authorized key
/// of the target account (the real owner). Fee-exempt.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CancelRecoveryTx {
    pub from: Address,
    pub nonce: u64,
    pub pub_key: [u8; 32],
    pub signature: Signature,
}

impl CancelRecoveryTx {
    pub fn signable_bytes(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(crate::constants::NETWORK_ID);
        buf.extend_from_slice(b"smart_cancel_recovery");
        buf.extend_from_slice(&self.from.0);
        buf.extend_from_slice(&self.nonce.to_le_bytes());
        buf
    }

    pub fn hash(&self) -> [u8; 32] {
        *blake3::hash(&self.signable_bytes()).as_bytes()
    }

    pub fn verify_signature(&self) -> bool {
        let Ok(vk) = ed25519_dalek::VerifyingKey::from_bytes(&self.pub_key) else {
            return false;
        };
        let sig = ed25519_dalek::Signature::from_bytes(&self.signature.0);
        vk.verify_strict(&self.signable_bytes(), &sig).is_ok()
    }
}

// ────────────────────────────────────────────────────────────
// Spending policy transaction types
// ────────────────────────────────────────────────────────────

/// Configure spending policy for a SmartAccount (time-locked change).
/// The new policy takes effect after POLICY_CHANGE_DELAY_ROUNDS.
/// This prevents an attacker who compromises a key from instantly disabling limits.
/// Fee: MIN_FEE.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetPolicyTx {
    pub from: Address,
    pub instant_limit: u64,
    pub vault_threshold: u64,
    pub vault_delay_rounds: u64,
    pub whitelisted_recipients: Vec<Address>,
    pub daily_limit: Option<u64>,
    pub fee: u64,
    pub nonce: u64,
    pub pub_key: [u8; 32],
    pub signature: Signature,
}

impl SetPolicyTx {
    pub fn signable_bytes(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(crate::constants::NETWORK_ID);
        buf.extend_from_slice(b"smart_set_policy");
        buf.extend_from_slice(&self.from.0);
        buf.extend_from_slice(&self.instant_limit.to_le_bytes());
        buf.extend_from_slice(&self.vault_threshold.to_le_bytes());
        buf.extend_from_slice(&self.vault_delay_rounds.to_le_bytes());
        buf.extend_from_slice(&(self.whitelisted_recipients.len() as u32).to_le_bytes());
        for addr in &self.whitelisted_recipients {
            buf.extend_from_slice(&addr.0);
        }
        match self.daily_limit {
            Some(limit) => { buf.push(1); buf.extend_from_slice(&limit.to_le_bytes()); }
            None => { buf.push(0); }
        }
        buf.extend_from_slice(&self.fee.to_le_bytes());
        buf.extend_from_slice(&self.nonce.to_le_bytes());
        buf
    }

    pub fn hash(&self) -> [u8; 32] { *blake3::hash(&self.signable_bytes()).as_bytes() }
    pub fn total_cost(&self) -> u64 { self.fee }

    pub fn verify_signature(&self) -> bool {
        let Ok(vk) = ed25519_dalek::VerifyingKey::from_bytes(&self.pub_key) else { return false; };
        let sig = ed25519_dalek::Signature::from_bytes(&self.signature.0);
        vk.verify_strict(&self.signable_bytes(), &sig).is_ok()
    }
}

/// Execute a time-locked vault transfer that has passed its delay.
/// Fee-exempt (the fee was already deducted when the vault transfer was created).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecuteVaultTx {
    pub from: Address,
    /// The transfer_id of the pending vault transfer to execute.
    pub transfer_id: [u8; 8],
    pub nonce: u64,
    pub pub_key: [u8; 32],
    pub signature: Signature,
}

impl ExecuteVaultTx {
    pub fn signable_bytes(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(crate::constants::NETWORK_ID);
        buf.extend_from_slice(b"smart_execute_vault");
        buf.extend_from_slice(&self.from.0);
        buf.extend_from_slice(&self.transfer_id);
        buf.extend_from_slice(&self.nonce.to_le_bytes());
        buf
    }

    pub fn hash(&self) -> [u8; 32] { *blake3::hash(&self.signable_bytes()).as_bytes() }

    pub fn verify_signature(&self) -> bool {
        let Ok(vk) = ed25519_dalek::VerifyingKey::from_bytes(&self.pub_key) else { return false; };
        let sig = ed25519_dalek::Signature::from_bytes(&self.signature.0);
        vk.verify_strict(&self.signable_bytes(), &sig).is_ok()
    }
}

/// Cancel a pending vault transfer. Fee-exempt.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CancelVaultTx {
    pub from: Address,
    /// The transfer_id of the pending vault transfer to cancel.
    pub transfer_id: [u8; 8],
    pub nonce: u64,
    pub pub_key: [u8; 32],
    pub signature: Signature,
}

impl CancelVaultTx {
    pub fn signable_bytes(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(crate::constants::NETWORK_ID);
        buf.extend_from_slice(b"smart_cancel_vault");
        buf.extend_from_slice(&self.from.0);
        buf.extend_from_slice(&self.transfer_id);
        buf.extend_from_slice(&self.nonce.to_le_bytes());
        buf
    }

    pub fn hash(&self) -> [u8; 32] { *blake3::hash(&self.signable_bytes()).as_bytes() }

    pub fn verify_signature(&self) -> bool {
        let Ok(vk) = ed25519_dalek::VerifyingKey::from_bytes(&self.pub_key) else { return false; };
        let sig = ed25519_dalek::Signature::from_bytes(&self.signature.0);
        vk.verify_strict(&self.signable_bytes(), &sig).is_ok()
    }
}

/// Compute a vault transfer ID from transaction fields.
pub fn compute_vault_transfer_id(from: &Address, to: &Address, amount: u64, nonce: u64) -> [u8; 8] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(&from.0);
    hasher.update(&to.0);
    hasher.update(&amount.to_le_bytes());
    hasher.update(&nonce.to_le_bytes());
    let hash = hasher.finalize();
    let mut id = [0u8; 8];
    id.copy_from_slice(&hash.as_bytes()[..8]);
    id
}

// ────────────────────────────────────────────────────────────
// SmartOp — generic P256-signed operation for passkey wallets
// ────────────────────────────────────────────────────────────

/// The operation being performed inside a SmartOp.
/// Each variant maps to an existing transaction type but signed with a SmartAccount key.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SmartOpType {
    /// Stake UDAG as validator.
    Stake { amount: u64 },
    /// Begin unstake cooldown.
    Unstake,
    /// Delegate UDAG to a validator.
    Delegate { validator: Address, amount: u64 },
    /// Begin undelegation cooldown.
    Undelegate,
    /// Set validator commission rate.
    SetCommission { commission_percent: u8 },
    /// Create a governance proposal.
    CreateProposal { title: String, description: String, proposal_type_tag: u8, param: String, new_value: u64 },
    /// Vote on a governance proposal.
    Vote { proposal_id: u64, approve: bool },
    /// Register a name (free for 6+ chars).
    RegisterName { name: String, duration_years: u8 },
    /// Renew a name registration.
    RenewName { name: String, additional_years: u8 },
    /// Transfer a name to a new owner.
    TransferName { name: String, new_owner: Address },
    /// Create a streaming payment.
    StreamCreate { recipient: Address, rate_sats_per_round: u64, deposit: u64, #[serde(default)] cliff_rounds: u64 },
    /// Withdraw accrued funds from a stream.
    StreamWithdraw { stream_id: [u8; 32] },
    /// Cancel a stream.
    StreamCancel { stream_id: [u8; 32] },
    /// Add a new authorized key to an existing SmartAccount.
    /// Signed by an already-authorized key on the account — the new key
    /// itself does not sign (it isn't yet authorized and can't). This
    /// enables cross-ecosystem device pairing (e.g. primary on iPhone,
    /// backup YubiKey) where the OS-level passkey sync doesn't apply.
    /// Fee-exempt (account management op).
    AddKey { key_type: KeyType, pubkey: Vec<u8>, label: String },
    /// Update the name profile (external addresses, metadata).
    /// The passkey-friendly counterpart to `UpdateProfileTx` (Ed25519-only).
    UpdateProfile {
        name: String,
        external_addresses: Vec<(String, String)>,
        metadata: Vec<(String, String)>,
    },
    /// Create a derived pocket under this SmartAccount.
    /// The pocket address is deterministically derived from the parent address + label.
    /// No separate keys needed — the parent's authorized keys sign for all pockets.
    /// Fee-exempt (account management op).
    CreatePocket { label: String },
    /// Remove a pocket from this SmartAccount.
    /// Does NOT destroy the pocket address's balance — funds remain accessible
    /// until the parent re-creates the pocket or transfers them first.
    /// Fee-exempt (account management op).
    RemovePocket { label: String },
    /// Initiate time-locked removal of an authorized key.
    /// The passkey-friendly counterpart to `RemoveKeyTx` (Ed25519-only).
    /// Signed by any currently-authorized key; the removal executes after
    /// `KEY_REMOVAL_DELAY_ROUNDS` (same time-lock as RemoveKeyTx). Cannot
    /// remove the last key — add a replacement first.
    /// Fee-exempt (account management op).
    RemoveKey { key_id_to_remove: [u8; 8] },
    /// Configure social recovery guardians.
    /// The passkey-friendly counterpart to `SetRecoveryTx` (Ed25519-only).
    /// Signed by any currently-authorized key. Replaces any existing config
    /// and cancels any pending recovery. Cannot set self as guardian.
    /// Fee-exempt (account management op).
    SetRecovery {
        guardians: Vec<Address>,
        threshold: u8,
        delay_rounds: u64,
    },
    /// Configure spending policy (time-locked change).
    /// The passkey-friendly counterpart to `SetPolicyTx` (Ed25519-only).
    /// Signed by any currently-authorized key. The new policy takes effect
    /// after `POLICY_CHANGE_DELAY_ROUNDS` — this time-lock prevents an
    /// attacker who compromises a key from instantly disabling limits.
    /// Fee-exempt (account management op).
    SetPolicy {
        instant_limit: u64,
        vault_threshold: u64,
        vault_delay_rounds: u64,
        whitelisted_recipients: Vec<Address>,
        daily_limit: Option<u64>,
    },
}

/// A generic SmartAccount operation signed with any authorized key (Ed25519 or P256).
/// This single transaction type replaces the need for Smart* variants of every tx type.
/// The inner `operation` specifies what to do; the outer envelope handles authentication.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SmartOpTx {
    /// SmartAccount sender address.
    pub from: Address,
    /// The operation to perform.
    pub operation: SmartOpType,
    /// Fee in sats (0 for fee-exempt operations like stake/unstake).
    pub fee: u64,
    /// Sender nonce.
    pub nonce: u64,
    /// Which authorized key signed this transaction.
    pub signing_key_id: [u8; 8],
    /// Signature bytes (Ed25519 64 bytes, P256 64-72 bytes, or WebAuthn envelope).
    pub signature: Vec<u8>,
    /// Optional WebAuthn signature envelope.
    #[serde(default)]
    pub webauthn: Option<WebAuthnSignature>,
    /// Optional P256 public key for first-time SmartAccount auto-registration.
    /// When set and no SmartAccount exists, the verifier checks address derivation
    /// (blake3("smart_account_p256" || pubkey)[:20] == from) and auto-creates the account.
    #[serde(default)]
    pub p256_pubkey: Option<Vec<u8>>,
}

impl SmartOpTx {
    pub fn signable_bytes(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(128);
        buf.extend_from_slice(crate::constants::NETWORK_ID);
        buf.extend_from_slice(b"smart_op");
        buf.extend_from_slice(&self.from.0);
        // Serialize operation type for domain separation
        match &self.operation {
            SmartOpType::Stake { amount } => {
                buf.push(0);
                buf.extend_from_slice(&amount.to_le_bytes());
            }
            SmartOpType::Unstake => { buf.push(1); }
            SmartOpType::Delegate { validator, amount } => {
                buf.push(2);
                buf.extend_from_slice(&validator.0);
                buf.extend_from_slice(&amount.to_le_bytes());
            }
            SmartOpType::Undelegate => { buf.push(3); }
            SmartOpType::SetCommission { commission_percent } => {
                buf.push(4);
                buf.push(*commission_percent);
            }
            SmartOpType::CreateProposal { title, description, proposal_type_tag, param, new_value } => {
                buf.push(5);
                buf.extend_from_slice(&(title.len() as u32).to_le_bytes());
                buf.extend_from_slice(title.as_bytes());
                buf.extend_from_slice(&(description.len() as u32).to_le_bytes());
                buf.extend_from_slice(description.as_bytes());
                buf.push(*proposal_type_tag);
                buf.extend_from_slice(&(param.len() as u32).to_le_bytes());
                buf.extend_from_slice(param.as_bytes());
                buf.extend_from_slice(&new_value.to_le_bytes());
            }
            SmartOpType::Vote { proposal_id, approve } => {
                buf.push(6);
                buf.extend_from_slice(&proposal_id.to_le_bytes());
                buf.push(*approve as u8);
            }
            SmartOpType::RegisterName { name, duration_years } => {
                buf.push(7);
                buf.extend_from_slice(&(name.len() as u32).to_le_bytes());
                buf.extend_from_slice(name.as_bytes());
                buf.push(*duration_years);
            }
            SmartOpType::RenewName { name, additional_years } => {
                buf.push(8);
                buf.extend_from_slice(&(name.len() as u32).to_le_bytes());
                buf.extend_from_slice(name.as_bytes());
                buf.push(*additional_years);
            }
            SmartOpType::TransferName { name, new_owner } => {
                buf.push(9);
                buf.extend_from_slice(&(name.len() as u32).to_le_bytes());
                buf.extend_from_slice(name.as_bytes());
                buf.extend_from_slice(&new_owner.0);
            }
            SmartOpType::StreamCreate { recipient, rate_sats_per_round, deposit, cliff_rounds } => {
                buf.push(10);
                buf.extend_from_slice(&recipient.0);
                buf.extend_from_slice(&rate_sats_per_round.to_le_bytes());
                buf.extend_from_slice(&deposit.to_le_bytes());
                buf.extend_from_slice(&cliff_rounds.to_le_bytes());
            }
            SmartOpType::StreamWithdraw { stream_id } => {
                buf.push(11);
                buf.extend_from_slice(stream_id);
            }
            SmartOpType::StreamCancel { stream_id } => {
                buf.push(12);
                buf.extend_from_slice(stream_id);
            }
            SmartOpType::AddKey { key_type, pubkey, label } => {
                buf.push(13);
                buf.push(*key_type as u8);
                buf.extend_from_slice(&(pubkey.len() as u32).to_le_bytes());
                buf.extend_from_slice(pubkey);
                buf.extend_from_slice(&(label.len() as u32).to_le_bytes());
                buf.extend_from_slice(label.as_bytes());
            }
            SmartOpType::UpdateProfile { name, external_addresses, metadata } => {
                buf.push(14);
                buf.extend_from_slice(&(name.len() as u32).to_le_bytes());
                buf.extend_from_slice(name.as_bytes());
                buf.extend_from_slice(&(external_addresses.len() as u32).to_le_bytes());
                for (chain, addr) in external_addresses {
                    buf.extend_from_slice(&(chain.len() as u32).to_le_bytes());
                    buf.extend_from_slice(chain.as_bytes());
                    buf.extend_from_slice(&(addr.len() as u32).to_le_bytes());
                    buf.extend_from_slice(addr.as_bytes());
                }
                buf.extend_from_slice(&(metadata.len() as u32).to_le_bytes());
                for (key, val) in metadata {
                    buf.extend_from_slice(&(key.len() as u32).to_le_bytes());
                    buf.extend_from_slice(key.as_bytes());
                    buf.extend_from_slice(&(val.len() as u32).to_le_bytes());
                    buf.extend_from_slice(val.as_bytes());
                }
            }
            SmartOpType::CreatePocket { label } => {
                buf.push(15);
                buf.extend_from_slice(&(label.len() as u32).to_le_bytes());
                buf.extend_from_slice(label.as_bytes());
            }
            SmartOpType::RemovePocket { label } => {
                buf.push(16);
                buf.extend_from_slice(&(label.len() as u32).to_le_bytes());
                buf.extend_from_slice(label.as_bytes());
            }
            SmartOpType::RemoveKey { key_id_to_remove } => {
                buf.push(17);
                buf.extend_from_slice(key_id_to_remove);
            }
            SmartOpType::SetRecovery { guardians, threshold, delay_rounds } => {
                buf.push(18);
                buf.extend_from_slice(&(guardians.len() as u32).to_le_bytes());
                for g in guardians {
                    buf.extend_from_slice(&g.0);
                }
                buf.push(*threshold);
                buf.extend_from_slice(&delay_rounds.to_le_bytes());
            }
            SmartOpType::SetPolicy { instant_limit, vault_threshold, vault_delay_rounds, whitelisted_recipients, daily_limit } => {
                buf.push(19);
                buf.extend_from_slice(&instant_limit.to_le_bytes());
                buf.extend_from_slice(&vault_threshold.to_le_bytes());
                buf.extend_from_slice(&vault_delay_rounds.to_le_bytes());
                buf.extend_from_slice(&(whitelisted_recipients.len() as u32).to_le_bytes());
                for addr in whitelisted_recipients {
                    buf.extend_from_slice(&addr.0);
                }
                match daily_limit {
                    Some(limit) => { buf.push(1); buf.extend_from_slice(&limit.to_le_bytes()); }
                    None => { buf.push(0); }
                }
            }
        }
        buf.extend_from_slice(&self.fee.to_le_bytes());
        buf.extend_from_slice(&self.nonce.to_le_bytes());
        buf.extend_from_slice(&self.signing_key_id);
        buf
    }

    pub fn hash(&self) -> [u8; 32] {
        *blake3::hash(&self.signable_bytes()).as_bytes()
    }

    pub fn total_cost(&self) -> u64 {
        match &self.operation {
            SmartOpType::Stake { amount } => *amount,
            SmartOpType::Delegate { amount, .. } => *amount,
            SmartOpType::StreamCreate { deposit, .. } => deposit.saturating_add(self.fee),
            _ => self.fee,
        }
    }

    /// Verify using an authorized key (called by StateEngine).
    pub fn verify_with_key(&self, key: &AuthorizedKey) -> bool {
        if let Some(ref webauthn) = self.webauthn {
            if key.key_type != KeyType::P256 { return false; }
            return verify_webauthn(&key.pubkey, webauthn, &self.signable_bytes());
        }
        verify_by_key_type(key.key_type, &key.pubkey, &self.signature, &self.signable_bytes())
    }

    /// Stateless verify — returns false (needs state for key lookup).
    pub fn verify_signature(&self) -> bool { false }

    /// Check if this operation is fee-exempt.
    pub fn is_fee_exempt(&self) -> bool {
        matches!(self.operation,
            SmartOpType::Stake { .. } | SmartOpType::Unstake
            | SmartOpType::Delegate { .. } | SmartOpType::Undelegate
            | SmartOpType::SetCommission { .. }
            | SmartOpType::RegisterName { .. } // Free for 6+ chars
            | SmartOpType::AddKey { .. }       // Account management op
            | SmartOpType::RemoveKey { .. }    // Account management op
            | SmartOpType::CreatePocket { .. } // Account management op
            | SmartOpType::RemovePocket { .. } // Account management op
            | SmartOpType::SetRecovery { .. }  // Account management op
            | SmartOpType::SetPolicy { .. }    // Account management op
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::address::SecretKey;

    #[test]
    fn test_key_id_computation() {
        let sk = SecretKey::from_bytes([0x42; 32]);
        let pubkey = sk.verifying_key().to_bytes();
        let key_id = AuthorizedKey::compute_key_id(KeyType::Ed25519, &pubkey);
        // Deterministic
        let key_id2 = AuthorizedKey::compute_key_id(KeyType::Ed25519, &pubkey);
        assert_eq!(key_id, key_id2);
        // Different key type produces different id
        let key_id3 = AuthorizedKey::compute_key_id(KeyType::P256, &pubkey);
        assert_ne!(key_id, key_id3);
    }

    #[test]
    fn test_authorized_key_validation() {
        let sk = SecretKey::from_bytes([0x42; 32]);
        let pubkey = sk.verifying_key().to_bytes();
        let key_id = AuthorizedKey::compute_key_id(KeyType::Ed25519, &pubkey);

        let key = AuthorizedKey {
            key_id,
            key_type: KeyType::Ed25519,
            pubkey: pubkey.to_vec(),
            label: "test".to_string(),
            daily_limit: None,
            daily_spent: (0, 0),
        };
        assert!(key.validate().is_ok());

        // Wrong key_id
        let bad_key = AuthorizedKey {
            key_id: [0xFF; 8],
            ..key.clone()
        };
        assert_eq!(bad_key.validate(), Err("key_id does not match pubkey"));

        // Wrong pubkey length for Ed25519
        let bad_len = AuthorizedKey {
            pubkey: vec![0u8; 33],
            key_id: AuthorizedKey::compute_key_id(KeyType::Ed25519, &[0u8; 33]),
            ..key.clone()
        };
        assert_eq!(bad_len.validate(), Err("Ed25519 pubkey must be 32 bytes"));

        // Label too long
        let bad_label = AuthorizedKey {
            label: "a".repeat(33),
            ..key.clone()
        };
        assert_eq!(bad_label.validate(), Err("key label exceeds 32 bytes"));
    }

    #[test]
    fn test_add_key_tx_signable_bytes_deterministic() {
        let sk = SecretKey::from_bytes([0x42; 32]);
        let new_sk = SecretKey::from_bytes([0x43; 32]);
        let new_pubkey = new_sk.verifying_key().to_bytes();
        let key_id = AuthorizedKey::compute_key_id(KeyType::Ed25519, &new_pubkey);

        let tx = AddKeyTx {
            from: sk.address(),
            new_key: AuthorizedKey {
                key_id,
                key_type: KeyType::Ed25519,
                pubkey: new_pubkey.to_vec(),
                label: "laptop".to_string(),
                daily_limit: None,
                daily_spent: (0, 0),
            },
            fee: 10_000,
            nonce: 0,
            pub_key: sk.verifying_key().to_bytes(),
            signature: Signature([0u8; 64]),
        };

        let bytes1 = tx.signable_bytes();
        let bytes2 = tx.signable_bytes();
        assert_eq!(bytes1, bytes2);
        assert!(bytes1.starts_with(crate::constants::NETWORK_ID));
    }

    #[test]
    fn test_add_key_tx_sign_and_verify() {
        let sk = SecretKey::from_bytes([0x42; 32]);
        let new_sk = SecretKey::from_bytes([0x43; 32]);
        let new_pubkey = new_sk.verifying_key().to_bytes();
        let key_id = AuthorizedKey::compute_key_id(KeyType::Ed25519, &new_pubkey);

        let mut tx = AddKeyTx {
            from: sk.address(),
            new_key: AuthorizedKey {
                key_id,
                key_type: KeyType::Ed25519,
                pubkey: new_pubkey.to_vec(),
                label: "laptop".to_string(),
                daily_limit: None,
                daily_spent: (0, 0),
            },
            fee: 10_000,
            nonce: 0,
            pub_key: sk.verifying_key().to_bytes(),
            signature: Signature([0u8; 64]),
        };
        tx.signature = sk.sign(&tx.signable_bytes());

        assert!(tx.verify_signature());
    }

    #[test]
    fn test_remove_key_tx_sign_and_verify() {
        let sk = SecretKey::from_bytes([0x42; 32]);
        let mut tx = RemoveKeyTx {
            from: sk.address(),
            key_id_to_remove: [0xAA; 8],
            nonce: 0,
            pub_key: sk.verifying_key().to_bytes(),
            signature: Signature([0u8; 64]),
        };
        tx.signature = sk.sign(&tx.signable_bytes());

        assert!(tx.verify_signature());
    }

    #[test]
    fn test_smart_account_config_find_key() {
        let sk = SecretKey::from_bytes([0x42; 32]);
        let pubkey = sk.verifying_key().to_bytes();
        let key_id = AuthorizedKey::compute_key_id(KeyType::Ed25519, &pubkey);

        let mut config = SmartAccountConfig::new(100);
        assert!(config.find_key(&key_id).is_none());

        config.authorized_keys.push(AuthorizedKey {
            key_id,
            key_type: KeyType::Ed25519,
            pubkey: pubkey.to_vec(),
            label: "phone".to_string(),
            daily_limit: None,
            daily_spent: (0, 0),
        });

        assert!(config.find_key(&key_id).is_some());
        assert!(config.has_key(&key_id));
        assert!(!config.has_key(&[0xFF; 8]));
    }

    // ────────────────────────────────────────────────────────────
    // P256 signature verification tests
    // ────────────────────────────────────────────────────────────

    #[test]
    fn test_p256_sign_and_verify() {
        use p256::ecdsa::{SigningKey, signature::Signer};

        // Generate P256 keypair
        let signing_key = SigningKey::from_bytes(&[0x42; 32].into()).unwrap();
        let verifying_key = signing_key.verifying_key();
        let pubkey_bytes = verifying_key.to_sec1_bytes();

        // Sign a message
        let message = b"test message for P256 verification";
        let signature: p256::ecdsa::Signature = signing_key.sign(message);

        // Verify with our function
        assert!(verify_p256(&pubkey_bytes, &signature.to_bytes(), message));

        // Wrong message should fail
        assert!(!verify_p256(&pubkey_bytes, &signature.to_bytes(), b"wrong message"));

        // Wrong key should fail
        let other_key = SigningKey::from_bytes(&[0x43; 32].into()).unwrap();
        let other_pubkey = other_key.verifying_key().to_sec1_bytes();
        assert!(!verify_p256(&other_pubkey, &signature.to_bytes(), message));
    }

    #[test]
    fn test_p256_der_and_raw_formats() {
        use p256::ecdsa::{SigningKey, signature::Signer};

        let signing_key = SigningKey::from_bytes(&[0x44; 32].into()).unwrap();
        let verifying_key = signing_key.verifying_key();
        let pubkey_bytes = verifying_key.to_sec1_bytes();

        let message = b"test DER vs raw signature formats";
        let signature: p256::ecdsa::Signature = signing_key.sign(message);

        // Raw r||s format (64 bytes)
        let raw_bytes = signature.to_bytes();
        assert_eq!(raw_bytes.len(), 64);
        assert!(verify_p256(&pubkey_bytes, &raw_bytes, message));

        // DER format (variable length, typically 70-72 bytes)
        let der_bytes = signature.to_der();
        assert!(verify_p256(&pubkey_bytes, der_bytes.as_bytes(), message));
    }

    #[test]
    fn test_p256_compressed_and_uncompressed_pubkey() {
        use p256::ecdsa::{SigningKey, signature::Signer};
        use p256::EncodedPoint;

        let signing_key = SigningKey::from_bytes(&[0x45; 32].into()).unwrap();
        let verifying_key = signing_key.verifying_key();

        let message = b"test compressed vs uncompressed pubkey";
        let signature: p256::ecdsa::Signature = signing_key.sign(message);
        let sig_bytes = signature.to_bytes();

        // Compressed (33 bytes)
        let compressed = EncodedPoint::from(verifying_key).compress();
        assert_eq!(compressed.len(), 33);
        assert!(verify_p256(compressed.as_bytes(), &sig_bytes, message));

        // Uncompressed (65 bytes)
        let uncompressed = verifying_key.to_encoded_point(false);
        assert_eq!(uncompressed.len(), 65);
        assert!(verify_p256(uncompressed.as_bytes(), &sig_bytes, message));
    }

    #[test]
    fn test_verify_by_key_type_dispatches_correctly() {
        // Ed25519
        let ed_sk = SecretKey::from_bytes([0x50; 32]);
        let ed_pubkey = ed_sk.verifying_key().to_bytes();
        let message = b"dispatch test";
        let ed_sig = ed_sk.sign(message);
        assert!(verify_by_key_type(KeyType::Ed25519, &ed_pubkey, &ed_sig.0, message));
        assert!(!verify_by_key_type(KeyType::P256, &ed_pubkey, &ed_sig.0, message)); // Wrong curve

        // P256
        use p256::ecdsa::{SigningKey, signature::Signer};
        let p256_sk = SigningKey::from_bytes(&[0x51; 32].into()).unwrap();
        let p256_pubkey = p256_sk.verifying_key().to_sec1_bytes();
        let p256_sig: p256::ecdsa::Signature = p256_sk.sign(message);
        assert!(verify_by_key_type(KeyType::P256, &p256_pubkey, &p256_sig.to_bytes(), message));
        assert!(!verify_by_key_type(KeyType::Ed25519, &p256_pubkey, &p256_sig.to_bytes(), message)); // Wrong curve
    }

    #[test]
    fn test_p256_authorized_key_validation() {
        use p256::ecdsa::SigningKey;

        let p256_sk = SigningKey::from_bytes(&[0x52; 32].into()).unwrap();
        let p256_pubkey = p256_sk.verifying_key().to_sec1_bytes();
        let key_id = AuthorizedKey::compute_key_id(KeyType::P256, &p256_pubkey);

        let key = AuthorizedKey {
            key_id,
            key_type: KeyType::P256,
            pubkey: p256_pubkey.to_vec(),
            label: "iPhone".to_string(),
            daily_limit: Some(100_000_000_000), // 1000 UDAG
            daily_spent: (0, 0),
        };
        assert!(key.validate().is_ok());
    }

    #[test]
    fn test_smart_transfer_verify_with_p256_key() {
        use p256::ecdsa::{SigningKey, signature::Signer};

        let p256_sk = SigningKey::from_bytes(&[0x53; 32].into()).unwrap();
        let p256_pubkey = p256_sk.verifying_key().to_sec1_bytes();
        let key_id = AuthorizedKey::compute_key_id(KeyType::P256, &p256_pubkey);

        let authorized_key = AuthorizedKey {
            key_id,
            key_type: KeyType::P256,
            pubkey: p256_pubkey.to_vec(),
            label: "test".to_string(),
            daily_limit: None,
            daily_spent: (0, 0),
        };

        let from = Address([0x01; 20]);
        let to = Address([0x02; 20]);

        let mut tx = SmartTransferTx {
            from,
            to,
            amount: 100_000_000,
            fee: 10_000,
            nonce: 0,
            signing_key_id: key_id,
            signature: vec![],
            memo: None,
            webauthn: None,
        };

        // Sign with P256
        let signable = tx.signable_bytes();
        let p256_signature: p256::ecdsa::Signature = p256_sk.sign(&signable);
        tx.signature = p256_signature.to_bytes().to_vec();

        // Verify with the authorized key
        assert!(tx.verify_with_key(&authorized_key));

        // Wrong key should fail
        let wrong_sk = SigningKey::from_bytes(&[0x54; 32].into()).unwrap();
        let wrong_pubkey = wrong_sk.verifying_key().to_sec1_bytes();
        let wrong_key = AuthorizedKey {
            key_id: AuthorizedKey::compute_key_id(KeyType::P256, &wrong_pubkey),
            key_type: KeyType::P256,
            pubkey: wrong_pubkey.to_vec(),
            label: "wrong".to_string(),
            daily_limit: None,
            daily_spent: (0, 0),
        };
        assert!(!tx.verify_with_key(&wrong_key));
    }

    #[test]
    fn test_smart_transfer_verify_with_ed25519_key() {
        let ed_sk = SecretKey::from_bytes([0x55; 32]);
        let ed_pubkey = ed_sk.verifying_key().to_bytes();
        let key_id = AuthorizedKey::compute_key_id(KeyType::Ed25519, &ed_pubkey);

        let authorized_key = AuthorizedKey {
            key_id,
            key_type: KeyType::Ed25519,
            pubkey: ed_pubkey.to_vec(),
            label: "laptop".to_string(),
            daily_limit: None,
            daily_spent: (0, 0),
        };

        let from = Address([0x01; 20]);
        let to = Address([0x02; 20]);

        let mut tx = SmartTransferTx {
            from,
            to,
            amount: 50_000_000,
            fee: 10_000,
            nonce: 0,
            signing_key_id: key_id,
            signature: vec![],
            memo: None,
            webauthn: None,
        };

        // Sign with Ed25519
        let signable = tx.signable_bytes();
        let sig = ed_sk.sign(&signable);
        tx.signature = sig.0.to_vec();

        assert!(tx.verify_with_key(&authorized_key));
    }

    #[test]
    fn test_webauthn_verification() {
        use p256::ecdsa::{SigningKey, signature::Signer};
        use sha2::{Sha256, Digest};

        let p256_sk = SigningKey::from_bytes(&[0x60; 32].into()).unwrap();
        let p256_pubkey = p256_sk.verifying_key().to_sec1_bytes();
        let key_id = AuthorizedKey::compute_key_id(KeyType::P256, &p256_pubkey);

        let authorized_key = AuthorizedKey {
            key_id,
            key_type: KeyType::P256,
            pubkey: p256_pubkey.to_vec(),
            label: "iphone".to_string(),
            daily_limit: None,
            daily_spent: (0, 0),
        };

        let from = Address([0x01; 20]);
        let to = Address([0x02; 20]);

        let mut tx = SmartTransferTx {
            from, to, amount: 100_000_000, fee: 10_000, nonce: 0,
            signing_key_id: key_id, signature: vec![], memo: None, webauthn: None,
        };

        // Simulate WebAuthn signing flow:
        // 1. Compute challenge = SHA-256(signable_bytes)
        let signable = tx.signable_bytes();
        let challenge = Sha256::digest(&signable);
        let challenge_b64 = base64url_encode(&challenge);

        // 2. Build fake authenticatorData (37 bytes: rpIdHash(32) + flags(1) + signCount(4))
        let authenticator_data = vec![0u8; 37];

        // 3. Build clientDataJSON with the challenge
        let client_data_json = format!(
            r#"{{"type":"webauthn.get","challenge":"{}","origin":"https://wallet.ultradag.com"}}"#,
            challenge_b64
        ).into_bytes();

        // 4. Compute the bytes the browser signs: authenticatorData || SHA-256(clientDataJSON).
        //    The ECDSA Signer trait applies SHA-256 internally, so we pass the raw
        //    concatenation — prehashing here would produce SHA-256(SHA-256(...)),
        //    which neither verification path in verify_webauthn expects.
        let client_data_hash = Sha256::digest(&client_data_json);
        let mut signed_data = Vec::new();
        signed_data.extend_from_slice(&authenticator_data);
        signed_data.extend_from_slice(&client_data_hash);

        // 5. Sign with P256 — raw bytes; the Signer internally SHA-256s.
        let p256_signature: p256::ecdsa::Signature = p256_sk.sign(&signed_data);

        // 6. Package into WebAuthnSignature
        tx.webauthn = Some(WebAuthnSignature {
            authenticator_data,
            client_data_json,
            signature: p256_signature.to_bytes().to_vec(),
        });

        // Verify
        assert!(tx.verify_with_key(&authorized_key));
    }

    #[test]
    fn test_webauthn_wrong_challenge_rejected() {
        use p256::ecdsa::{SigningKey, signature::Signer};
        use sha2::{Sha256, Digest};

        let p256_sk = SigningKey::from_bytes(&[0x61; 32].into()).unwrap();
        let p256_pubkey = p256_sk.verifying_key().to_sec1_bytes();
        let key_id = AuthorizedKey::compute_key_id(KeyType::P256, &p256_pubkey);

        let authorized_key = AuthorizedKey {
            key_id, key_type: KeyType::P256, pubkey: p256_pubkey.to_vec(),
            label: "test".to_string(), daily_limit: None, daily_spent: (0, 0),
        };

        let mut tx = SmartTransferTx {
            from: Address([0x01; 20]), to: Address([0x02; 20]),
            amount: 100_000_000, fee: 10_000, nonce: 0,
            signing_key_id: key_id, signature: vec![], memo: None, webauthn: None,
        };

        // Use a WRONG challenge (not derived from this tx's signable_bytes)
        let wrong_challenge = Sha256::digest(b"wrong data");
        let wrong_challenge_b64 = base64url_encode(&wrong_challenge);

        let authenticator_data = vec![0u8; 37];
        let client_data_json = format!(
            r#"{{"type":"webauthn.get","challenge":"{}","origin":"https://evil.com"}}"#,
            wrong_challenge_b64
        ).into_bytes();

        let client_data_hash = Sha256::digest(&client_data_json);
        let mut signed_data = Vec::new();
        signed_data.extend_from_slice(&authenticator_data);
        signed_data.extend_from_slice(&client_data_hash);
        let signed_message = Sha256::digest(&signed_data);
        let p256_signature: p256::ecdsa::Signature = p256_sk.sign(&signed_message);

        tx.webauthn = Some(WebAuthnSignature {
            authenticator_data,
            client_data_json,
            signature: p256_signature.to_bytes().to_vec(),
        });

        // Should fail — challenge doesn't match signable_bytes
        assert!(!tx.verify_with_key(&authorized_key));
    }

    #[test]
    fn test_base64url_encode() {
        assert_eq!(base64url_encode(&[0]), "AA");
        assert_eq!(base64url_encode(&[255, 255]), "__8");
        // Known test vector
        let data = b"Hello, World!";
        let encoded = base64url_encode(data);
        assert!(!encoded.contains('+'));
        assert!(!encoded.contains('/'));
        assert!(!encoded.contains('='));
    }

    #[test]
    fn test_extract_json_string() {
        let json = r#"{"type":"webauthn.get","challenge":"abc123","origin":"https://example.com"}"#;
        assert_eq!(extract_json_string(json, "challenge"), Some("abc123"));
        assert_eq!(extract_json_string(json, "type"), Some("webauthn.get"));
        assert_eq!(extract_json_string(json, "origin"), Some("https://example.com"));
        assert_eq!(extract_json_string(json, "missing"), None);
    }
}
