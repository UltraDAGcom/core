//! TransferTx signing — byte-exact replica of the ultradag-coin Rust
//! implementation at `crates/ultradag-coin/src/tx/transaction.rs`.
//!
//! We do NOT depend on `ultradag-coin` directly because that would pull in
//! `redb`, `chrono`, `bech32`, `k256`, `p256`, `sha3`, `dashmap`, and a few
//! hundred KB of other weight we don't need on a 4 MB ESP32. Instead, this
//! module re-implements exactly the two functions that matter for
//! transferring UDAG:
//!
//!   - `address_from_pubkey`: `blake3(ed25519_pubkey)[..20]`
//!   - `transfer_signable_bytes`: the exact byte layout the node verifies
//!
//! Anything that drifts from the node's layout will silently produce
//! invalid signatures — keep this module in lock-step with the Rust source.
//! The total length of `transfer_signable_bytes` with no memo is 91 bytes,
//! which the node asserts in `signable_bytes_is_consistent`.

use ed25519_dalek::{Signature, Signer, SigningKey};

/// The 19-byte network identifier used as a domain-separation prefix in
/// every signable_bytes computation. MUST match `ultradag-coin`'s
/// `constants::NETWORK_ID` for the target network. If you rebuild the node
/// with `--features mainnet` (or testnet→mainnet swap), change this.
pub const NETWORK_ID: &[u8] = b"ultradag-testnet-v1";

/// Type tag for TransferTx in signable_bytes. Domain-separates transfers
/// from other transaction kinds so a bit-aligned collision across types
/// can't reuse a signature.
const TRANSFER_TAG: &[u8] = b"transfer";

/// Derive the 20-byte on-chain address from a 32-byte Ed25519 public key.
///
/// Matches `Address::from_pubkey` in `ultradag-coin/src/address/keys.rs`:
/// ```ignore
/// pub fn from_pubkey(pubkey: &[u8; 32]) -> Self {
///     let full_hash = blake3::hash(pubkey);
///     let mut addr = [0u8; 20];
///     addr.copy_from_slice(&full_hash.as_bytes()[..20]);
///     Self(addr)
/// }
/// ```
pub fn address_from_pubkey(pubkey: &[u8; 32]) -> [u8; 20] {
    let full = blake3::hash(pubkey);
    let mut addr = [0u8; 20];
    addr.copy_from_slice(&full.as_bytes()[..20]);
    addr
}

/// Hex-encode a byte slice (lowercase, no `0x` prefix).
pub fn hex_lower(bytes: &[u8]) -> String {
    hex::encode(bytes)
}

/// Build the signable bytes for a TransferTx with no memo.
///
/// Layout (91 bytes):
/// ```text
/// NETWORK_ID (19)  ||  "transfer" (8)  ||  from (20)  ||  to (20)
///   ||  amount LE (8)  ||  fee LE (8)  ||  nonce LE (8)
/// ```
///
/// Memos are NOT supported here — the ESP32 flow doesn't need them, and
/// skipping the memo-length-prefix branch keeps this easy to audit. If you
/// ever add one, mirror the `memo_len as u32 LE || memo_bytes` branch from
/// `TransferTx::signable_bytes` exactly.
pub fn transfer_signable_bytes(
    from: &[u8; 20],
    to: &[u8; 20],
    amount_sats: u64,
    fee_sats: u64,
    nonce: u64,
) -> Vec<u8> {
    let mut buf = Vec::with_capacity(91);
    buf.extend_from_slice(NETWORK_ID);
    buf.extend_from_slice(TRANSFER_TAG);
    buf.extend_from_slice(from);
    buf.extend_from_slice(to);
    buf.extend_from_slice(&amount_sats.to_le_bytes());
    buf.extend_from_slice(&fee_sats.to_le_bytes());
    buf.extend_from_slice(&nonce.to_le_bytes());
    debug_assert_eq!(buf.len(), 91, "signable_bytes length drifted from Rust node");
    buf
}

/// A fully-signed TransferTx ready to be JSON-encoded into the body of
/// `POST /tx/submit`. Fields match the serde layout of
/// `ultradag-coin::tx::TransferTx` verbatim:
///
///   - `from`, `to`, `pub_key` are byte arrays (serde's default for
///     `Address([u8; 20])` and `pub_key: [u8; 32]` is a JSON array of
///     numbers, 0..=255, one per byte).
///   - `signature` is a 128-char lowercase hex string (not an array) —
///     `Signature` has a custom `Serialize` impl that emits hex for any
///     human-readable serializer like `serde_json`.
///   - `memo` is `null` when absent.
///
/// Wrap this value in `{"Transfer": <SignedTransfer>}` when building the
/// POST body, because the `Transaction` enum on the node is
/// externally-tagged (serde default).
pub struct SignedTransfer {
    pub from: [u8; 20],
    pub to: [u8; 20],
    pub amount: u64,
    pub fee: u64,
    pub nonce: u64,
    pub pub_key: [u8; 32],
    pub signature: [u8; 64],
}

impl SignedTransfer {
    /// Sign a new transfer using the given Ed25519 signing key and
    /// parameters. Computes signable_bytes, signs with the key, and
    /// verifies locally before returning — an invalid signature here is
    /// almost always a bug in signable_bytes drift, so catching it
    /// immediately saves a round trip to the RPC.
    pub fn build(sk: &SigningKey, to: [u8; 20], amount_sats: u64, fee_sats: u64, nonce: u64) -> Self {
        let pubkey: [u8; 32] = sk.verifying_key().to_bytes();
        let from = address_from_pubkey(&pubkey);

        let signable = transfer_signable_bytes(&from, &to, amount_sats, fee_sats, nonce);
        let sig: Signature = sk.sign(&signable);

        // Belt-and-braces: verify locally before returning. If this ever
        // trips, either ed25519-dalek's pubkey derivation is inconsistent
        // with the signing path (should never happen) or signable_bytes has
        // drifted from the node. Either way we want to know NOW, not after
        // a 200 OK from a node that silently rejects the tx in mempool.
        sk.verifying_key()
            .verify_strict(&signable, &sig)
            .expect("local Ed25519 self-verification failed — signable_bytes drifted?");

        Self {
            from,
            to,
            amount: amount_sats,
            fee: fee_sats,
            nonce,
            pub_key: pubkey,
            signature: sig.to_bytes(),
        }
    }

    /// Serialize to the JSON shape the node expects under `/tx/submit`.
    /// Returns a `serde_json::Value` so the caller can wrap it in the
    /// `{"Transfer": ...}` enum envelope and POST it.
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "from":      self.from,
            "to":        self.to,
            "amount":    self.amount,
            "fee":       self.fee,
            "nonce":     self.nonce,
            "pub_key":   self.pub_key,
            "signature": hex_lower(&self.signature),
            "memo":      serde_json::Value::Null,
        })
    }
}

// ────────────────────────────────────────────────────────────────────
// AddKeyTx — authorize a new key on a SmartAccount
// ────────────────────────────────────────────────────────────────────

/// `key_id = blake3([key_type_byte] || pubkey)[..8]` — matches
/// `AuthorizedKey::compute_key_id` in `ultradag-coin`. Used for both
/// AddKeyTx's `new_key.key_id` field and SmartTransferTx's
/// `signing_key_id` field.
pub fn compute_ed25519_key_id(pubkey: &[u8; 32]) -> [u8; 8] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(&[0u8]); // KeyType::Ed25519 = 0
    hasher.update(pubkey);
    let h = hasher.finalize();
    let mut id = [0u8; 8];
    id.copy_from_slice(&h.as_bytes()[..8]);
    id
}

/// `AddKeyTx::signable_bytes` — byte-exact replica of
/// `ultradag-coin/src/tx/smart_account.rs::AddKeyTx::signable_bytes`.
///
/// Layout:
/// ```text
/// NETWORK_ID (19)  ||  "smart_add_key" (13)  ||  from (20)
///   ||  new_key.key_id (8)  ||  new_key.key_type (1)
///   ||  pubkey_len LE u32 (4)  ||  pubkey (N)
///   ||  label_len LE u32 (4)  ||  label (M)
///   ||  fee LE (8)  ||  nonce LE (8)
/// ```
pub fn add_key_signable_bytes(
    from: &[u8; 20],
    new_key_id: &[u8; 8],
    new_key_type: u8,
    new_pubkey: &[u8],
    new_label: &str,
    fee_sats: u64,
    nonce: u64,
) -> Vec<u8> {
    let mut buf = Vec::with_capacity(128 + new_pubkey.len() + new_label.len());
    buf.extend_from_slice(NETWORK_ID);
    buf.extend_from_slice(b"smart_add_key");
    buf.extend_from_slice(from);
    buf.extend_from_slice(new_key_id);
    buf.push(new_key_type);
    buf.extend_from_slice(&(new_pubkey.len() as u32).to_le_bytes());
    buf.extend_from_slice(new_pubkey);
    buf.extend_from_slice(&(new_label.len() as u32).to_le_bytes());
    buf.extend_from_slice(new_label.as_bytes());
    buf.extend_from_slice(&fee_sats.to_le_bytes());
    buf.extend_from_slice(&nonce.to_le_bytes());
    buf
}

/// A fully-signed AddKeyTx ready to POST to `/tx/submit` under the
/// `{"AddKey": ...}` envelope. Matches serde layout of
/// `ultradag-coin::tx::smart_account::AddKeyTx`.
pub struct SignedAddKey {
    pub from: [u8; 20],
    pub new_key_id: [u8; 8],
    pub new_pubkey: [u8; 32],
    pub new_label: String,
    pub fee: u64,
    pub nonce: u64,
    pub pub_key: [u8; 32],
    pub signature: [u8; 64],
}

impl SignedAddKey {
    /// Build and sign an AddKeyTx that registers `new_pubkey` as a new
    /// authorized key on the sender's SmartAccount. The sender is
    /// implicitly `address_from_pubkey(sk.pubkey)` — the same legacy
    /// Ed25519 address the chip already transacts from.
    ///
    /// If the account doesn't exist yet, the node will create it and
    /// auto-register `sk.pubkey` as the "default" key **before** the
    /// `new_key` check runs. So after this tx lands, the SmartAccount
    /// has at least two keys:
    ///   - "default" — the signer's Ed25519 pubkey (auto-registered)
    ///   - `new_label` — whatever was passed in (explicitly added)
    pub fn build(
        sk: &SigningKey,
        new_pubkey: [u8; 32],
        new_label: &str,
        fee_sats: u64,
        nonce: u64,
    ) -> Self {
        let signer_pubkey: [u8; 32] = sk.verifying_key().to_bytes();
        let from = address_from_pubkey(&signer_pubkey);
        let new_key_id = compute_ed25519_key_id(&new_pubkey);

        let signable = add_key_signable_bytes(
            &from,
            &new_key_id,
            /*key_type=*/ 0, // Ed25519
            &new_pubkey,
            new_label,
            fee_sats,
            nonce,
        );
        let sig: Signature = sk.sign(&signable);

        sk.verifying_key()
            .verify_strict(&signable, &sig)
            .expect("local AddKey self-verification failed — signable_bytes drifted?");

        Self {
            from,
            new_key_id,
            new_pubkey,
            new_label: new_label.to_string(),
            fee: fee_sats,
            nonce,
            pub_key: signer_pubkey,
            signature: sig.to_bytes(),
        }
    }

    /// Serialize to the JSON shape the node expects. AuthorizedKey's
    /// `daily_limit` is `Option<u64>` (None → null) and `daily_spent`
    /// is a tuple that serializes as a JSON array `[0, 0]` — both have
    /// `#[serde(default)]` on the server side, so we still send them
    /// explicitly to be conservative.
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "from": self.from,
            "new_key": {
                "key_id":      self.new_key_id,
                "key_type":    "Ed25519",
                "pubkey":      self.new_pubkey,
                "label":       self.new_label,
                "daily_limit": serde_json::Value::Null,
                "daily_spent": [0u64, 0u64],
            },
            "fee":       self.fee,
            "nonce":     self.nonce,
            "pub_key":   self.pub_key,
            "signature": hex_lower(&self.signature),
        })
    }
}

// ────────────────────────────────────────────────────────────────────
// SmartTransferTx — SmartAccount-backed transfer
// ────────────────────────────────────────────────────────────────────

/// `SmartTransferTx::signable_bytes` — byte-exact replica of
/// `ultradag-coin/src/tx/smart_account.rs::SmartTransferTx::signable_bytes`
/// for the no-memo case.
///
/// Layout (99 bytes, no memo):
/// ```text
/// NETWORK_ID (19)  ||  "smart_transfer" (14)  ||  from (20)  ||  to (20)
///   ||  amount LE (8)  ||  fee LE (8)  ||  nonce LE (8)  ||  signing_key_id (8)
/// ```
pub fn smart_transfer_signable_bytes(
    from: &[u8; 20],
    to: &[u8; 20],
    amount_sats: u64,
    fee_sats: u64,
    nonce: u64,
    signing_key_id: &[u8; 8],
) -> Vec<u8> {
    let mut buf = Vec::with_capacity(105);
    buf.extend_from_slice(NETWORK_ID);
    buf.extend_from_slice(b"smart_transfer");
    buf.extend_from_slice(from);
    buf.extend_from_slice(to);
    buf.extend_from_slice(&amount_sats.to_le_bytes());
    buf.extend_from_slice(&fee_sats.to_le_bytes());
    buf.extend_from_slice(&nonce.to_le_bytes());
    buf.extend_from_slice(signing_key_id);
    buf
}

/// A signed SmartTransferTx ready to POST to `/tx/submit` under the
/// `{"SmartTransfer": ...}` envelope. Matches the serde layout of
/// `ultradag-coin::tx::smart_account::SmartTransferTx`.
pub struct SignedSmartTransfer {
    pub from: [u8; 20],
    pub to: [u8; 20],
    pub amount: u64,
    pub fee: u64,
    pub nonce: u64,
    pub signing_key_id: [u8; 8],
    pub signature: [u8; 64],
}

impl SignedSmartTransfer {
    /// Build and sign a SmartTransferTx. The `from` address is derived
    /// from the signing key's pubkey via the same blake3 rule. The
    /// `signing_key_id` is computed from the pubkey so the node can
    /// look up the AuthorizedKey in state.
    pub fn build(
        sk: &SigningKey,
        to: [u8; 20],
        amount_sats: u64,
        fee_sats: u64,
        nonce: u64,
    ) -> Self {
        let pubkey: [u8; 32] = sk.verifying_key().to_bytes();
        let from = address_from_pubkey(&pubkey);
        let signing_key_id = compute_ed25519_key_id(&pubkey);

        let signable = smart_transfer_signable_bytes(
            &from,
            &to,
            amount_sats,
            fee_sats,
            nonce,
            &signing_key_id,
        );
        let sig: Signature = sk.sign(&signable);

        sk.verifying_key()
            .verify_strict(&signable, &sig)
            .expect("local SmartTransfer self-verification failed — signable_bytes drifted?");

        Self {
            from,
            to,
            amount: amount_sats,
            fee: fee_sats,
            nonce,
            signing_key_id,
            signature: sig.to_bytes(),
        }
    }

    /// Serialize to the JSON shape the node expects. Note: `signature`
    /// is a `Vec<u8>` in the Rust struct (to support both Ed25519 64 B
    /// and P256 64-72 B), so in JSON it's a plain array of numbers,
    /// NOT a hex string like `TransferTx::signature`.
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "from":            self.from,
            "to":              self.to,
            "amount":          self.amount,
            "fee":             self.fee,
            "nonce":           self.nonce,
            "signing_key_id":  self.signing_key_id,
            "signature":       self.signature.to_vec(),
            "memo":            serde_json::Value::Null,
            "webauthn":        serde_json::Value::Null,
        })
    }
}
