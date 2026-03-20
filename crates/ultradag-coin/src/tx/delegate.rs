use serde::{Deserialize, Serialize};

use crate::address::{Address, Signature};

/// Delegate UDAG to a validator. Delegators earn passive rewards minus commission.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DelegateTx {
    pub from: Address,
    pub validator: Address,
    pub amount: u64,
    pub nonce: u64,
    pub pub_key: [u8; 32],
    pub signature: Signature,
}

/// Undelegate (begin cooldown to withdraw delegated UDAG).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct UndelegateTx {
    pub from: Address,
    pub nonce: u64,
    pub pub_key: [u8; 32],
    pub signature: Signature,
}

/// Set validator commission rate on delegated rewards.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SetCommissionTx {
    pub from: Address,
    pub commission_percent: u8,
    pub nonce: u64,
    pub pub_key: [u8; 32],
    pub signature: Signature,
}

impl DelegateTx {
    pub fn signable_bytes(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(crate::constants::NETWORK_ID);
        buf.extend_from_slice(b"delegate");
        buf.extend_from_slice(&self.from.0);
        buf.extend_from_slice(&self.validator.0);
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

impl UndelegateTx {
    pub fn signable_bytes(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(crate::constants::NETWORK_ID);
        buf.extend_from_slice(b"undelegate");
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

impl SetCommissionTx {
    pub fn signable_bytes(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(crate::constants::NETWORK_ID);
        buf.extend_from_slice(b"set_commission");
        buf.extend_from_slice(&self.from.0);
        buf.extend_from_slice(&[self.commission_percent]);
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

    fn make_delegate_tx(sk: &SecretKey, validator: Address, amount: u64, nonce: u64) -> DelegateTx {
        let mut tx = DelegateTx {
            from: sk.address(),
            validator,
            amount,
            nonce,
            pub_key: sk.verifying_key().to_bytes(),
            signature: Signature([0u8; 64]),
        };
        tx.signature = sk.sign(&tx.signable_bytes());
        tx
    }

    fn make_undelegate_tx(sk: &SecretKey, nonce: u64) -> UndelegateTx {
        let mut tx = UndelegateTx {
            from: sk.address(),
            nonce,
            pub_key: sk.verifying_key().to_bytes(),
            signature: Signature([0u8; 64]),
        };
        tx.signature = sk.sign(&tx.signable_bytes());
        tx
    }

    fn make_set_commission_tx(sk: &SecretKey, commission: u8, nonce: u64) -> SetCommissionTx {
        let mut tx = SetCommissionTx {
            from: sk.address(),
            commission_percent: commission,
            nonce,
            pub_key: sk.verifying_key().to_bytes(),
            signature: Signature([0u8; 64]),
        };
        tx.signature = sk.sign(&tx.signable_bytes());
        tx
    }

    #[test]
    fn delegate_tx_signature_valid() {
        let sk = SecretKey::generate();
        let validator = SecretKey::generate().address();
        let tx = make_delegate_tx(&sk, validator, 100 * crate::constants::COIN, 0);
        assert!(tx.verify_signature());
    }

    #[test]
    fn delegate_tx_tampered_amount_rejected() {
        let sk = SecretKey::generate();
        let validator = SecretKey::generate().address();
        let mut tx = make_delegate_tx(&sk, validator, 100 * crate::constants::COIN, 0);
        tx.amount = 200 * crate::constants::COIN;
        assert!(!tx.verify_signature());
    }

    #[test]
    fn undelegate_tx_signature_valid() {
        let sk = SecretKey::generate();
        let tx = make_undelegate_tx(&sk, 0);
        assert!(tx.verify_signature());
    }

    #[test]
    fn undelegate_tx_wrong_key_rejected() {
        let sk1 = SecretKey::generate();
        let sk2 = SecretKey::generate();
        let mut tx = make_undelegate_tx(&sk1, 0);
        tx.pub_key = sk2.verifying_key().to_bytes();
        assert!(!tx.verify_signature());
    }

    #[test]
    fn set_commission_tx_signature_valid() {
        let sk = SecretKey::generate();
        let tx = make_set_commission_tx(&sk, 15, 0);
        assert!(tx.verify_signature());
    }

    #[test]
    fn set_commission_tx_tampered_rejected() {
        let sk = SecretKey::generate();
        let mut tx = make_set_commission_tx(&sk, 15, 0);
        tx.commission_percent = 20;
        assert!(!tx.verify_signature());
    }

    #[test]
    fn delegate_hash_deterministic() {
        let sk = SecretKey::generate();
        let validator = SecretKey::generate().address();
        let tx = make_delegate_tx(&sk, validator, 100 * crate::constants::COIN, 0);
        assert_eq!(tx.hash(), tx.hash());
    }
}
