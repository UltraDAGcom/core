use ed25519_dalek::{Signer, SigningKey, Verifier, VerifyingKey};
use rand::rngs::OsRng;

use crate::error::{Result, UltraDagError};

/// An Ed25519 keypair with the derived UltraDAG address.
///
/// The address is computed as `blake3(ed25519_public_key_bytes)`.
#[derive(Debug, Clone)]
pub struct Keypair {
    /// Ed25519 secret key seed (32 bytes).
    pub secret_key: [u8; 32],
    /// Ed25519 public key (32 bytes).
    pub public_key: [u8; 32],
    /// UltraDAG address: `blake3(public_key)` (32 bytes).
    pub address: [u8; 32],
}

impl Keypair {
    /// Generate a new random keypair.
    pub fn generate() -> Self {
        let mut csprng = OsRng;
        let signing_key = SigningKey::generate(&mut csprng);
        Self::from_signing_key(&signing_key)
    }

    /// Recreate a keypair from a 32-byte secret key seed.
    pub fn from_secret_bytes(bytes: [u8; 32]) -> Self {
        let signing_key = SigningKey::from_bytes(&bytes);
        Self::from_signing_key(&signing_key)
    }

    /// Recreate a keypair from a hex-encoded secret key.
    ///
    /// # Errors
    ///
    /// Returns [`UltraDagError::InvalidKey`] if the hex string is invalid
    /// or not exactly 32 bytes.
    pub fn from_hex(secret_hex: &str) -> Result<Self> {
        let bytes = hex::decode(secret_hex)
            .map_err(|e| UltraDagError::InvalidKey(format!("bad hex: {e}")))?;
        let arr: [u8; 32] = bytes
            .try_into()
            .map_err(|_| UltraDagError::InvalidKey("secret key must be 32 bytes".into()))?;
        Ok(Self::from_secret_bytes(arr))
    }

    /// Return the secret key as a hex string.
    pub fn secret_key_hex(&self) -> String {
        hex::encode(self.secret_key)
    }

    /// Return the public key as a hex string.
    pub fn public_key_hex(&self) -> String {
        hex::encode(self.public_key)
    }

    /// Return the address as a hex string.
    pub fn address_hex(&self) -> String {
        hex::encode(self.address)
    }

    /// Sign an arbitrary message, returning the 64-byte Ed25519 signature.
    pub fn sign(&self, message: &[u8]) -> [u8; 64] {
        let signing_key = SigningKey::from_bytes(&self.secret_key);
        let sig = signing_key.sign(message);
        sig.to_bytes()
    }

    /// Verify a signature against the keypair's public key.
    ///
    /// Returns `true` if the signature is valid.
    pub fn verify(&self, message: &[u8], signature: &[u8; 64]) -> bool {
        let Ok(verifying_key) = VerifyingKey::from_bytes(&self.public_key) else {
            return false;
        };
        let sig = ed25519_dalek::Signature::from_bytes(signature);
        verifying_key.verify(message, &sig).is_ok()
    }

    // -- internal --

    fn from_signing_key(signing_key: &SigningKey) -> Self {
        let verifying_key = signing_key.verifying_key();
        let public_key = verifying_key.to_bytes();
        let address = derive_address(&public_key);
        Self {
            secret_key: signing_key.to_bytes(),
            public_key,
            address,
        }
    }
}

/// Derive an UltraDAG address from an Ed25519 public key.
///
/// `address = blake3(public_key_bytes)`
pub fn derive_address(public_key: &[u8; 32]) -> [u8; 32] {
    let hash = blake3::hash(public_key);
    *hash.as_bytes()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generate_produces_valid_keypair() {
        let kp = Keypair::generate();
        assert_eq!(kp.address, derive_address(&kp.public_key));
    }

    #[test]
    fn from_secret_bytes_deterministic() {
        let seed = [0xAB; 32];
        let a = Keypair::from_secret_bytes(seed);
        let b = Keypair::from_secret_bytes(seed);
        assert_eq!(a.secret_key, b.secret_key);
        assert_eq!(a.public_key, b.public_key);
        assert_eq!(a.address, b.address);
    }

    #[test]
    fn from_hex_roundtrip() {
        let kp = Keypair::generate();
        let restored = Keypair::from_hex(&kp.secret_key_hex()).unwrap();
        assert_eq!(kp.public_key, restored.public_key);
        assert_eq!(kp.address, restored.address);
    }

    #[test]
    fn from_hex_invalid() {
        assert!(Keypair::from_hex("not_hex").is_err());
        assert!(Keypair::from_hex("aabb").is_err()); // too short
    }

    #[test]
    fn sign_and_verify() {
        let kp = Keypair::generate();
        let msg = b"hello ultradag";
        let sig = kp.sign(msg);
        assert!(kp.verify(msg, &sig));
        // wrong message
        assert!(!kp.verify(b"wrong", &sig));
    }

    #[test]
    fn address_is_blake3_of_pubkey() {
        let kp = Keypair::from_secret_bytes([0x42; 32]);
        let expected = blake3::hash(&kp.public_key);
        assert_eq!(kp.address, *expected.as_bytes());
    }

    #[test]
    fn hex_helpers() {
        let kp = Keypair::from_secret_bytes([0x01; 32]);
        assert_eq!(kp.secret_key_hex().len(), 64);
        assert_eq!(kp.public_key_hex().len(), 64);
        assert_eq!(kp.address_hex().len(), 64);
    }
}
