use ed25519_dalek::Signer;
use serde::{Deserialize, Serialize};

/// A 20-byte address derived from the Ed25519 public key: blake3(pubkey)[..20].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Address(pub [u8; 20]);

/// An Ed25519 signing key (32-byte seed).
#[derive(Clone)]
pub struct SecretKey {
    inner: ed25519_dalek::SigningKey,
}

/// An Ed25519 signature (64 bytes).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Signature(pub [u8; 64]);

impl Serialize for Signature {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        if serializer.is_human_readable() {
            // JSON/RPC: hex string for readability
            let hex: String = self.0.iter().map(|b| format!("{b:02x}")).collect();
            serializer.serialize_str(&hex)
        } else {
            // Binary formats (postcard): raw bytes
            serializer.serialize_bytes(&self.0)
        }
    }
}

impl<'de> Deserialize<'de> for Signature {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        if deserializer.is_human_readable() {
            // JSON/RPC: hex string
            let hex = String::deserialize(deserializer)?;
            if hex.len() != 128 {
                return Err(serde::de::Error::custom("signature hex must be 128 chars"));
            }
            let mut bytes = [0u8; 64];
            for (i, chunk) in hex.as_bytes().chunks(2).enumerate() {
                let s = std::str::from_utf8(chunk).map_err(serde::de::Error::custom)?;
                bytes[i] = u8::from_str_radix(s, 16).map_err(serde::de::Error::custom)?;
            }
            Ok(Signature(bytes))
        } else {
            // Binary formats (postcard): raw bytes
            let bytes: Vec<u8> = Deserialize::deserialize(deserializer)?;
            if bytes.len() != 64 {
                return Err(serde::de::Error::custom("signature must be 64 bytes"));
            }
            let mut arr = [0u8; 64];
            arr.copy_from_slice(&bytes);
            Ok(Signature(arr))
        }
    }
}

impl Address {
    pub const ZERO: Self = Self([0u8; 20]);

    /// Human-readable prefix for mainnet addresses
    #[cfg(feature = "mainnet")]
    pub const HRP: &'static str = "udag";

    /// Human-readable prefix for testnet addresses
    #[cfg(not(feature = "mainnet"))]
    pub const HRP: &'static str = "tudg";

    /// Derive an address from a raw Ed25519 public key (32 bytes): blake3(pubkey)[..20].
    pub fn from_pubkey(pubkey: &[u8; 32]) -> Self {
        let full_hash = blake3::hash(pubkey);
        let mut addr = [0u8; 20];
        addr.copy_from_slice(&full_hash.as_bytes()[..20]);
        Self(addr)
    }

    pub fn to_hex(&self) -> String {
        self.0.iter().map(|b| format!("{b:02x}")).collect()
    }

    pub fn short(&self) -> String {
        let b = self.to_bech32();
        if b.len() > 20 {
            format!("{}...{}", &b[..12], &b[b.len()-6..])
        } else {
            b
        }
    }

    pub fn from_hex(hex: &str) -> Option<Self> {
        if hex.len() != 40 {
            return None;
        }
        let mut bytes = [0u8; 20];
        for (i, chunk) in hex.as_bytes().chunks(2).enumerate() {
            let s = std::str::from_utf8(chunk).ok()?;
            bytes[i] = u8::from_str_radix(s, 16).ok()?;
        }
        Some(Self(bytes))
    }

    /// Encode this address as a Bech32m string: udag1... (mainnet) or tudg1... (testnet)
    pub fn to_bech32(&self) -> String {
        let hrp = bech32::Hrp::parse(Self::HRP).expect("valid HRP");
        bech32::encode::<bech32::Bech32m>(hrp, &self.0).expect("valid bech32m encoding")
    }

    /// Decode a Bech32m address string. Accepts both mainnet (udag1) and testnet (tudg1) prefixes.
    pub fn from_bech32(s: &str) -> Option<Self> {
        let (hrp, data) = bech32::decode(s).ok()?;
        let hrp_str = hrp.as_str();
        if hrp_str != "udag" && hrp_str != "tudg" {
            return None;
        }
        if data.len() != 20 {
            return None;
        }
        let mut bytes = [0u8; 20];
        bytes.copy_from_slice(&data);
        Some(Self(bytes))
    }

    /// Parse an address from either hex (40 chars) or bech32m (udag1.../tudg1...) format.
    pub fn parse(s: &str) -> Option<Self> {
        // Try hex first (fast path)
        if s.len() == 40 && s.chars().all(|c| c.is_ascii_hexdigit()) {
            return Self::from_hex(s);
        }
        // Try bech32m
        Self::from_bech32(s)
    }
}

impl std::fmt::Display for Address {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.to_bech32())
    }
}

impl SecretKey {
    /// Generate a new random Ed25519 signing key using OS CSPRNG.
    ///
    /// Uses `rand::thread_rng()` which delegates to the OS CSPRNG (getrandom).
    /// **TESTNET/TEST ONLY** — disabled in mainnet builds to prevent accidental
    /// use of non-auditable key generation in production. Mainnet keys must be
    /// generated offline with explicit `OsRng` sourcing and hardware wallet storage.
    #[cfg(not(feature = "mainnet"))]
    pub fn generate() -> Self {
        let mut rng = rand::thread_rng();
        Self {
            inner: ed25519_dalek::SigningKey::generate(&mut rng),
        }
    }

    /// Create from raw 32-byte seed.
    pub fn from_bytes(bytes: [u8; 32]) -> Self {
        Self {
            inner: ed25519_dalek::SigningKey::from_bytes(&bytes),
        }
    }

    /// Return the raw 32-byte seed.
    pub fn to_bytes(&self) -> [u8; 32] {
        self.inner.to_bytes()
    }

    /// Derive the public address: blake3(ed25519_pubkey)[..20].
    pub fn address(&self) -> Address {
        let pubkey = self.inner.verifying_key();
        let full_hash = blake3::hash(pubkey.as_bytes());
        let mut addr = [0u8; 20];
        addr.copy_from_slice(&full_hash.as_bytes()[..20]);
        Address(addr)
    }

    /// Return the Ed25519 verifying (public) key.
    pub fn verifying_key(&self) -> ed25519_dalek::VerifyingKey {
        self.inner.verifying_key()
    }

    /// Sign data with Ed25519.
    pub fn sign(&self, data: &[u8]) -> Signature {
        let sig = self.inner.sign(data);
        Signature(sig.to_bytes())
    }
}

impl Signature {
    /// Verify this signature against an Ed25519 verifying key and data.
    pub fn verify(&self, verifying_key: &ed25519_dalek::VerifyingKey, data: &[u8]) -> bool {
        let sig = ed25519_dalek::Signature::from_bytes(&self.0);
        verifying_key.verify_strict(data, &sig).is_ok()
    }

    /// Verify this signature against a raw 32-byte Ed25519 public key and data.
    /// Returns false if the public key bytes are invalid or signature doesn't match.
    pub fn verify_with_pubkey_bytes(&self, pubkey_bytes: &[u8; 32], data: &[u8]) -> bool {
        let vk = match ed25519_dalek::VerifyingKey::from_bytes(pubkey_bytes) {
            Ok(vk) => vk,
            Err(_) => return false,
        };
        self.verify(&vk, data)
    }
}

// NOTE: Signature intentionally does NOT implement Copy.
// At 64 bytes, implicit stack copies are wasteful. Use .clone() or references instead.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generate_produces_unique_keys() {
        let sk1 = SecretKey::generate();
        let sk2 = SecretKey::generate();
        assert_ne!(sk1.to_bytes(), sk2.to_bytes(), "two generated keys should differ");
    }

    #[test]
    fn address_derivation_is_deterministic() {
        let sk = SecretKey::from_bytes([42u8; 32]);
        let addr1 = sk.address();
        let addr2 = sk.address();
        assert_eq!(addr1, addr2);
    }

    #[test]
    fn address_hex_roundtrip() {
        let sk = SecretKey::generate();
        let addr = sk.address();
        let hex = addr.to_hex();
        let recovered = Address::from_hex(&hex).expect("valid hex should parse");
        assert_eq!(addr, recovered);
    }

    #[test]
    fn address_from_hex_rejects_short_string() {
        assert!(Address::from_hex("abcd").is_none());
    }

    #[test]
    fn address_from_hex_rejects_invalid_chars() {
        let bad = "zz".repeat(20);
        assert!(Address::from_hex(&bad).is_none());
    }

    #[test]
    fn address_zero_is_all_zeros() {
        assert_eq!(Address::ZERO.0, [0u8; 20]);
        assert_eq!(Address::ZERO.to_hex(), "0".repeat(40));
    }

    #[test]
    fn address_short_returns_truncated_bech32m() {
        let addr = SecretKey::generate().address();
        let short = addr.short();
        assert!(short.starts_with("tudg1"));
        assert!(short.contains("..."));
    }

    #[test]
    fn address_display_uses_bech32m() {
        let addr = SecretKey::generate().address();
        let display = format!("{addr}");
        assert!(display.starts_with("tudg1"));
        assert_eq!(display, addr.to_bech32());
    }

    #[test]
    fn sign_produces_deterministic_signature() {
        let sk = SecretKey::from_bytes([7u8; 32]);
        let sig1 = sk.sign(b"hello");
        let sig2 = sk.sign(b"hello");
        assert_eq!(sig1, sig2);
    }

    #[test]
    fn sign_different_data_gives_different_signature() {
        let sk = SecretKey::from_bytes([7u8; 32]);
        let sig1 = sk.sign(b"hello");
        let sig2 = sk.sign(b"world");
        assert_ne!(sig1, sig2);
    }

    #[test]
    fn sign_and_verify_roundtrip() {
        let sk = SecretKey::generate();
        let data = b"test message";
        let sig = sk.sign(data);
        assert!(sig.verify(&sk.verifying_key(), data));
    }

    #[test]
    fn verify_rejects_wrong_data() {
        let sk = SecretKey::generate();
        let sig = sk.sign(b"correct");
        assert!(!sig.verify(&sk.verifying_key(), b"wrong"));
    }

    #[test]
    fn verify_rejects_wrong_key() {
        let sk1 = SecretKey::generate();
        let sk2 = SecretKey::generate();
        let sig = sk1.sign(b"test");
        assert!(!sig.verify(&sk2.verifying_key(), b"test"));
    }

    #[test]
    fn from_bytes_roundtrip() {
        let sk = SecretKey::generate();
        let bytes = sk.to_bytes();
        let sk2 = SecretKey::from_bytes(bytes);
        assert_eq!(sk.address(), sk2.address());
        // Same signing behavior
        let sig1 = sk.sign(b"data");
        let sig2 = sk2.sign(b"data");
        assert_eq!(sig1, sig2);
    }

    #[test]
    fn bech32m_roundtrip() {
        let sk = SecretKey::generate();
        let addr = sk.address();
        let bech = addr.to_bech32();
        assert!(bech.starts_with("tudg1")); // testnet
        let recovered = Address::from_bech32(&bech).expect("valid bech32m should parse");
        assert_eq!(addr, recovered);
    }

    #[test]
    fn bech32m_rejects_invalid() {
        assert!(Address::from_bech32("udag1invalid").is_none());
        assert!(Address::from_bech32("btc1qw508d6q").is_none());
        assert!(Address::from_bech32("not_bech32").is_none());
    }

    #[test]
    fn parse_accepts_both_formats() {
        let sk = SecretKey::from_bytes([42u8; 32]);
        let addr = sk.address();

        // Parse from hex
        let from_hex = Address::parse(&addr.to_hex()).unwrap();
        assert_eq!(addr, from_hex);

        // Parse from bech32m
        let from_bech = Address::parse(&addr.to_bech32()).unwrap();
        assert_eq!(addr, from_bech);
    }

    #[test]
    fn display_uses_bech32m() {
        let addr = SecretKey::from_bytes([42u8; 32]).address();
        let display = format!("{addr}");
        assert!(display.starts_with("tudg1"));
    }
}
