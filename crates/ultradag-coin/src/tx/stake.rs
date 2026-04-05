use serde::{Deserialize, Serialize};

use crate::address::{Address, Signature};

/// Minimum stake required to become a validator.
pub const MIN_STAKE_SATS: u64 = 2_000 * crate::constants::COIN; // 2,000 UDAG

/// Cooldown period before unstaked funds are returned (in rounds).
pub const UNSTAKE_COOLDOWN_ROUNDS: u64 = 2_016; // ~2.8 hours at 5s rounds

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StakeTx {
    pub from: Address,
    pub amount: u64,
    pub nonce: u64,
    pub pub_key: [u8; 32],
    pub signature: Signature,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct UnstakeTx {
    pub from: Address,
    pub nonce: u64,
    pub pub_key: [u8; 32],
    pub signature: Signature,
}

impl StakeTx {
    pub fn signable_bytes(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(crate::constants::NETWORK_ID);
        buf.extend_from_slice(b"stake");
        buf.extend_from_slice(&self.from.0);
        buf.extend_from_slice(&self.amount.to_le_bytes());
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

impl UnstakeTx {
    pub fn signable_bytes(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(crate::constants::NETWORK_ID);
        buf.extend_from_slice(b"unstake");
        buf.extend_from_slice(&self.from.0);
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::address::SecretKey;

    fn make_stake_tx(sk: &SecretKey, amount: u64, nonce: u64) -> StakeTx {
        let mut tx = StakeTx {
            from: sk.address(),
            amount,
            nonce,
            pub_key: sk.verifying_key().to_bytes(),
            signature: Signature([0u8; 64]),
        };
        tx.signature = sk.sign(&tx.signable_bytes());
        tx
    }

    fn make_unstake_tx(sk: &SecretKey, nonce: u64) -> UnstakeTx {
        let mut tx = UnstakeTx {
            from: sk.address(),
            nonce,
            pub_key: sk.verifying_key().to_bytes(),
            signature: Signature([0u8; 64]),
        };
        tx.signature = sk.sign(&tx.signable_bytes());
        tx
    }

    #[test]
    fn stake_tx_signature_valid() {
        let sk = SecretKey::generate();
        let tx = make_stake_tx(&sk, MIN_STAKE_SATS, 0);
        assert!(tx.verify_signature());
    }

    #[test]
    fn stake_tx_tampered_amount_rejected() {
        let sk = SecretKey::generate();
        let mut tx = make_stake_tx(&sk, MIN_STAKE_SATS, 0);
        tx.amount = MIN_STAKE_SATS + 1;
        assert!(!tx.verify_signature());
    }

    #[test]
    fn unstake_tx_signature_valid() {
        let sk = SecretKey::generate();
        let tx = make_unstake_tx(&sk, 0);
        assert!(tx.verify_signature());
    }

    #[test]
    fn unstake_tx_wrong_key_rejected() {
        let sk1 = SecretKey::generate();
        let sk2 = SecretKey::generate();
        let mut tx = make_unstake_tx(&sk1, 0);
        tx.pub_key = sk2.verifying_key().to_bytes();
        assert!(!tx.verify_signature());
    }

    #[test]
    fn stake_hash_deterministic() {
        let sk = SecretKey::generate();
        let tx = make_stake_tx(&sk, MIN_STAKE_SATS, 0);
        assert_eq!(tx.hash(), tx.hash());
    }
}
