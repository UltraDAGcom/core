use ed25519_dalek::Signer;
use serde::{Deserialize, Serialize};

/// A 32-byte address derived from the Ed25519 public key: blake3(pubkey).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Address(pub [u8; 32]);

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
    pub const ZERO: Self = Self([0u8; 32]);

    pub fn to_hex(&self) -> String {
        self.0.iter().map(|b| format!("{b:02x}")).collect()
    }

    pub fn short(&self) -> String {
        self.to_hex()[..8].to_string()
    }

    pub fn from_hex(hex: &str) -> Option<Self> {
        if hex.len() != 64 {
            return None;
        }
        let mut bytes = [0u8; 32];
        for (i, chunk) in hex.as_bytes().chunks(2).enumerate() {
            let s = std::str::from_utf8(chunk).ok()?;
            bytes[i] = u8::from_str_radix(s, 16).ok()?;
        }
        Some(Self(bytes))
    }
}

impl std::fmt::Display for Address {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.short())
    }
}

impl SecretKey {
    /// Generate a new random Ed25519 signing key.
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

    /// Derive the public address: blake3(ed25519_pubkey).
    pub fn address(&self) -> Address {
        let pubkey = self.inner.verifying_key();
        Address(*blake3::hash(pubkey.as_bytes()).as_bytes())
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
}

impl Copy for Signature {}

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
        let bad = "zz".repeat(32);
        assert!(Address::from_hex(&bad).is_none());
    }

    #[test]
    fn address_zero_is_all_zeros() {
        assert_eq!(Address::ZERO.0, [0u8; 32]);
        assert_eq!(Address::ZERO.to_hex(), "0".repeat(64));
    }

    #[test]
    fn address_short_returns_8_chars() {
        let addr = SecretKey::generate().address();
        assert_eq!(addr.short().len(), 8);
    }

    #[test]
    fn address_display_uses_short() {
        let addr = SecretKey::generate().address();
        assert_eq!(format!("{addr}"), addr.short());
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
}
