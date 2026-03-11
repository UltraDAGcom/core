use serde::{Deserialize, Serialize};

use crate::address::{Address, Signature};
use crate::tx::{StakeTx, UnstakeTx};
use crate::governance::{CreateProposalTx, VoteTx};

/// Unified transaction type supporting transfers, staking, unstaking, and governance.
/// All variants go through consensus and are included in DAG vertices.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Transaction {
    Transfer(TransferTx),
    Stake(StakeTx),
    Unstake(UnstakeTx),
    CreateProposal(CreateProposalTx),
    Vote(VoteTx),
}

/// A transaction transferring UDAG from one address to another.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransferTx {
    pub from: Address,
    pub to: Address,
    pub amount: u64,
    pub fee: u64,
    pub nonce: u64,
    /// Ed25519 public key of the sender (32 bytes). Used to verify the signature
    /// and must hash to the `from` address: `blake3(pub_key) == from`.
    pub pub_key: [u8; 32],
    pub signature: Signature,
    /// Optional data payload (max 256 bytes). Used for IoT sensor data, receipts, etc.
    /// Stored on-chain permanently. Keep small to prevent DAG bloat.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub memo: Option<Vec<u8>>,
}

impl Transaction {
    /// Compute the transaction hash (its unique identifier).
    pub fn hash(&self) -> [u8; 32] {
        match self {
            Transaction::Transfer(tx) => tx.hash(),
            Transaction::Stake(tx) => tx.hash(),
            Transaction::Unstake(tx) => tx.hash(),
            Transaction::CreateProposal(tx) => tx.hash(),
            Transaction::Vote(tx) => tx.hash(),
        }
    }

    /// Verify the transaction signature.
    pub fn verify_signature(&self) -> bool {
        match self {
            Transaction::Transfer(tx) => tx.verify_signature(),
            Transaction::Stake(tx) => tx.verify_signature(),
            Transaction::Unstake(tx) => tx.verify_signature(),
            Transaction::CreateProposal(tx) => tx.verify_signature(),
            Transaction::Vote(tx) => tx.verify_signature(),
        }
    }

    /// Get the sender address.
    pub fn from(&self) -> Address {
        match self {
            Transaction::Transfer(tx) => tx.from,
            Transaction::Stake(tx) => tx.from,
            Transaction::Unstake(tx) => tx.from,
            Transaction::CreateProposal(tx) => tx.from,
            Transaction::Vote(tx) => tx.from,
        }
    }

    /// Get the transaction nonce.
    pub fn nonce(&self) -> u64 {
        match self {
            Transaction::Transfer(tx) => tx.nonce,
            Transaction::Stake(tx) => tx.nonce,
            Transaction::Unstake(tx) => tx.nonce,
            Transaction::CreateProposal(tx) => tx.nonce,
            Transaction::Vote(tx) => tx.nonce,
        }
    }

    /// Get the fee (0 for stake/unstake).
    pub fn fee(&self) -> u64 {
        match self {
            Transaction::Transfer(tx) => tx.fee,
            Transaction::CreateProposal(tx) => tx.fee,
            Transaction::Vote(tx) => tx.fee,
            Transaction::Stake(_) | Transaction::Unstake(_) => 0,
        }
    }

    /// Get the amount (0 for unstake).
    pub fn amount(&self) -> u64 {
        match self {
            Transaction::Transfer(tx) => tx.amount,
            Transaction::Stake(tx) => tx.amount,
            Transaction::Unstake(_)
            | Transaction::CreateProposal(_)
            | Transaction::Vote(_) => 0,
        }
    }

    /// Get the recipient address (None for stake/unstake).
    pub fn to(&self) -> Option<Address> {
        match self {
            Transaction::Transfer(tx) => Some(tx.to),
            Transaction::Stake(_)
            | Transaction::Unstake(_)
            | Transaction::CreateProposal(_)
            | Transaction::Vote(_) => None,
        }
    }

    /// Get the sender's public key.
    pub fn pub_key(&self) -> [u8; 32] {
        match self {
            Transaction::Transfer(tx) => tx.pub_key,
            Transaction::Stake(tx) => tx.pub_key,
            Transaction::Unstake(tx) => tx.pub_key,
            Transaction::CreateProposal(tx) => tx.pub_key,
            Transaction::Vote(tx) => tx.pub_key,
        }
    }

    /// Get the signable bytes for signature verification.
    pub fn signable_bytes(&self) -> Vec<u8> {
        match self {
            Transaction::Transfer(tx) => tx.signable_bytes(),
            Transaction::Stake(tx) => tx.signable_bytes(),
            Transaction::Unstake(tx) => tx.signable_bytes(),
            Transaction::CreateProposal(tx) => tx.signable_bytes(),
            Transaction::Vote(tx) => tx.signable_bytes(),
        }
    }

    /// Get the total cost (amount + fee for transfers, amount for stake, 0 for unstake).
    pub fn total_cost(&self) -> u64 {
        match self {
            Transaction::Transfer(tx) => tx.total_cost(),
            Transaction::Stake(tx) => tx.amount,
            Transaction::CreateProposal(tx) => tx.fee,
            Transaction::Vote(tx) => tx.fee,
            Transaction::Unstake(_) => 0,
        }
    }
}

impl TransferTx {
    /// Compute the transaction hash (its unique identifier).
    pub fn hash(&self) -> [u8; 32] {
        let mut hasher = blake3::Hasher::new();
        hasher.update(&self.from.0);
        hasher.update(&self.to.0);
        hasher.update(&self.amount.to_le_bytes());
        hasher.update(&self.fee.to_le_bytes());
        hasher.update(&self.nonce.to_le_bytes());
        if let Some(ref memo) = self.memo {
            hasher.update(&(memo.len() as u32).to_le_bytes());
            hasher.update(memo);
        }
        *hasher.finalize().as_bytes()
    }

    /// The data that gets signed (everything except the signature).
    /// Includes network identifier to prevent cross-network replay attacks.
    pub fn signable_bytes(&self) -> Vec<u8> {
        let memo_len = self.memo.as_ref().map(|m| m.len()).unwrap_or(0);
        let mut buf = Vec::with_capacity(108 + memo_len + 4);
        buf.extend_from_slice(crate::constants::NETWORK_ID);
        buf.extend_from_slice(b"transfer");
        buf.extend_from_slice(&self.from.0);
        buf.extend_from_slice(&self.to.0);
        buf.extend_from_slice(&self.amount.to_le_bytes());
        buf.extend_from_slice(&self.fee.to_le_bytes());
        buf.extend_from_slice(&self.nonce.to_le_bytes());
        if let Some(ref memo) = self.memo {
            buf.extend_from_slice(&(memo.len() as u32).to_le_bytes());
            buf.extend_from_slice(memo);
        }
        buf
    }

    pub fn total_cost(&self) -> u64 {
        self.amount.saturating_add(self.fee)
    }

    /// Verify that the Ed25519 signature is valid and the pub_key hashes to `from`.
    pub fn verify_signature(&self) -> bool {
        // 1. Validate memo size
        if let Some(ref memo) = self.memo {
            if memo.len() > crate::constants::MAX_MEMO_BYTES {
                return false;
            }
        }

        // 2. Verify pub_key hashes to the from address
        let expected_addr = Address(*blake3::hash(&self.pub_key).as_bytes());
        if expected_addr != self.from {
            return false;
        }

        // 3. Parse the verifying key
        let Ok(vk) = ed25519_dalek::VerifyingKey::from_bytes(&self.pub_key) else {
            return false;
        };

        // 4. Verify the signature over signable_bytes
        self.signature.verify(&vk, &self.signable_bytes())
    }
}

/// Coinbase transaction — block reward to validator.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoinbaseTx {
    pub to: Address,
    pub amount: u64,
    pub height: u64,
}

impl CoinbaseTx {
    pub fn hash(&self) -> [u8; 32] {
        let mut hasher = blake3::Hasher::new();
        hasher.update(&self.to.0);
        hasher.update(&self.amount.to_le_bytes());
        hasher.update(&self.height.to_le_bytes());
        *hasher.finalize().as_bytes()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_signed_tx(sk: &crate::address::SecretKey, amount: u64, fee: u64, nonce: u64) -> Transaction {
        let mut transfer = TransferTx {
            from: sk.address(),
            to: Address::ZERO,
            amount,
            fee,
            nonce,
            pub_key: sk.verifying_key().to_bytes(),
            signature: Signature([0u8; 64]),
            memo: None,
        };
        transfer.signature = sk.sign(&transfer.signable_bytes());
        Transaction::Transfer(transfer)
    }

    fn make_tx(amount: u64, fee: u64, nonce: u64) -> Transaction {
        let sk = crate::address::SecretKey::generate();
        make_signed_tx(&sk, amount, fee, nonce)
    }

    #[test]
    fn hash_is_deterministic() {
        let sk = crate::address::SecretKey::from_bytes([1u8; 32]);
        let tx = make_signed_tx(&sk, 100, 10, 0);
        assert_eq!(tx.hash(), tx.hash());
    }

    #[test]
    fn different_transactions_have_different_hashes() {
        let tx1 = make_tx(100, 10, 0);
        let tx2 = make_tx(200, 10, 0);
        assert_ne!(tx1.hash(), tx2.hash());
    }

    #[test]
    fn different_nonce_different_hash() {
        let sk = crate::address::SecretKey::from_bytes([1u8; 32]);
        let tx1 = make_signed_tx(&sk, 100, 10, 0);
        let tx2 = make_signed_tx(&sk, 100, 10, 1);
        assert_ne!(tx1.hash(), tx2.hash());
    }

    #[test]
    fn signable_bytes_is_consistent() {
        let sk = crate::address::SecretKey::from_bytes([5u8; 32]);
        let tx = make_signed_tx(&sk, 50, 5, 3);
        if let Transaction::Transfer(ref transfer) = tx {
            assert_eq!(transfer.signable_bytes(), transfer.signable_bytes());
            // Should be NETWORK_ID (19) + "transfer" (8) + from (32) + to (32) + amount (8) + fee (8) + nonce (8) = 115 bytes
            assert_eq!(transfer.signable_bytes().len(), 115);
        } else {
            panic!("Expected Transfer variant");
        }
    }

    #[test]
    fn signable_bytes_excludes_signature() {
        let sk = crate::address::SecretKey::from_bytes([5u8; 32]);
        let tx1 = make_signed_tx(&sk, 50, 5, 0);
        if let Transaction::Transfer(mut transfer) = tx1 {
            let tx2_signable = transfer.signable_bytes();
            transfer.signature = Signature([0xff; 64]);
            assert_eq!(tx2_signable, transfer.signable_bytes());
        } else {
            panic!("Expected Transfer variant");
        }
    }

    #[test]
    fn total_cost_equals_amount_plus_fee() {
        let tx = make_tx(100, 10, 0);
        assert_eq!(tx.total_cost(), 110);
    }

    #[test]
    fn total_cost_saturates_on_overflow() {
        let sk = crate::address::SecretKey::from_bytes([1u8; 32]);
        let tx = make_signed_tx(&sk, u64::MAX, 1, 0);
        assert_eq!(tx.total_cost(), u64::MAX);
    }

    #[test]
    fn verify_signature_valid() {
        let sk = crate::address::SecretKey::generate();
        let tx = make_signed_tx(&sk, 100, 10, 0);
        assert!(tx.verify_signature());
    }

    #[test]
    fn verify_signature_wrong_key() {
        let sk1 = crate::address::SecretKey::generate();
        let sk2 = crate::address::SecretKey::generate();
        let tx = make_signed_tx(&sk1, 100, 10, 0);
        // Replace pub_key with wrong key
        let tampered_tx = if let Transaction::Transfer(mut transfer) = tx {
            transfer.pub_key = sk2.verifying_key().to_bytes();
            Transaction::Transfer(transfer)
        } else {
            panic!("Expected Transfer variant");
        };
        assert!(!tampered_tx.verify_signature());
    }

    #[test]
    fn verify_signature_tampered_amount() {
        let sk = crate::address::SecretKey::generate();
        let tx = make_signed_tx(&sk, 100, 10, 0);
        let tampered_tx = if let Transaction::Transfer(mut transfer) = tx {
            transfer.amount = 999;
            Transaction::Transfer(transfer)
        } else {
            panic!("Expected Transfer variant");
        };
        assert!(!tampered_tx.verify_signature());
    }

    #[test]
    fn coinbase_hash_is_deterministic() {
        let cb = CoinbaseTx {
            to: Address::ZERO,
            amount: 5_000_000_000,
            height: 0,
        };
        assert_eq!(cb.hash(), cb.hash());
    }

    #[test]
    fn transaction_with_memo_hashes_correctly() {
        let sk = crate::address::SecretKey::from_bytes([1u8; 32]);
        let memo_data = b"temp:22.4C hum:61% pres:1013hPa".to_vec();
        
        let mut transfer = TransferTx {
            from: sk.address(),
            to: Address::ZERO,
            amount: 100,
            fee: 10,
            nonce: 0,
            pub_key: sk.verifying_key().to_bytes(),
            signature: Signature([0u8; 64]),
            memo: Some(memo_data.clone()),
        };
        transfer.signature = sk.sign(&transfer.signable_bytes());
        let tx = Transaction::Transfer(transfer);
        
        // Hash should be deterministic
        assert_eq!(tx.hash(), tx.hash());
        
        // Signature should verify
        assert!(tx.verify_signature());
        
        // Hash should differ from same tx without memo
        let mut transfer_no_memo = TransferTx {
            from: sk.address(),
            to: Address::ZERO,
            amount: 100,
            fee: 10,
            nonce: 0,
            pub_key: sk.verifying_key().to_bytes(),
            signature: Signature([0u8; 64]),
            memo: None,
        };
        transfer_no_memo.signature = sk.sign(&transfer_no_memo.signable_bytes());
        let tx_no_memo = Transaction::Transfer(transfer_no_memo);
        
        assert_ne!(tx.hash(), tx_no_memo.hash());
    }

    #[test]
    fn transaction_with_oversized_memo_rejected() {
        let sk = crate::address::SecretKey::from_bytes([1u8; 32]);
        // Create memo larger than MAX_MEMO_BYTES (256)
        let oversized_memo = vec![0x42; crate::constants::MAX_MEMO_BYTES + 1];
        
        let mut transfer = TransferTx {
            from: sk.address(),
            to: Address::ZERO,
            amount: 100,
            fee: 10,
            nonce: 0,
            pub_key: sk.verifying_key().to_bytes(),
            signature: Signature([0u8; 64]),
            memo: Some(oversized_memo),
        };
        transfer.signature = sk.sign(&transfer.signable_bytes());
        let tx = Transaction::Transfer(transfer);
        
        // Signature verification should fail due to oversized memo
        assert!(!tx.verify_signature());
    }

    #[test]
    fn transaction_without_memo_still_works() {
        let sk = crate::address::SecretKey::from_bytes([1u8; 32]);
        
        let mut transfer = TransferTx {
            from: sk.address(),
            to: Address::ZERO,
            amount: 100,
            fee: 10,
            nonce: 0,
            pub_key: sk.verifying_key().to_bytes(),
            signature: Signature([0u8; 64]),
            memo: None,
        };
        transfer.signature = sk.sign(&transfer.signable_bytes());
        let tx = Transaction::Transfer(transfer);
        
        // Hash should be deterministic
        assert_eq!(tx.hash(), tx.hash());
        
        // Signature should verify
        assert!(tx.verify_signature());
    }

    #[test]
    fn memo_at_max_size_accepted() {
        let sk = crate::address::SecretKey::from_bytes([1u8; 32]);
        // Create memo exactly at MAX_MEMO_BYTES (256)
        let max_memo = vec![0x42; crate::constants::MAX_MEMO_BYTES];
        
        let mut transfer = TransferTx {
            from: sk.address(),
            to: Address::ZERO,
            amount: 100,
            fee: 10,
            nonce: 0,
            pub_key: sk.verifying_key().to_bytes(),
            signature: Signature([0u8; 64]),
            memo: Some(max_memo),
        };
        transfer.signature = sk.sign(&transfer.signable_bytes());
        let tx = Transaction::Transfer(transfer);
        
        // Signature should verify (exactly at limit is OK)
        assert!(tx.verify_signature());
    }
}
