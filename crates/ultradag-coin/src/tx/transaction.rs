use serde::{Deserialize, Serialize};

use crate::address::{Address, Signature};
use crate::tx::stake::{StakeTx, UnstakeTx};
use crate::tx::delegate::{DelegateTx, UndelegateTx, SetCommissionTx};
use crate::tx::bridge::{BridgeDepositTx, BridgeReleaseTx};
use crate::tx::smart_account::{AddKeyTx, RemoveKeyTx, SmartTransferTx, SetRecoveryTx, RecoverAccountTx, CancelRecoveryTx, SetPolicyTx, ExecuteVaultTx, CancelVaultTx};
use crate::tx::name_registry::{RegisterNameTx, RenewNameTx, TransferNameTx, UpdateProfileTx};
use crate::governance::{CreateProposalTx, VoteTx};

/// Unified transaction type supporting transfers, staking, unstaking, delegation,
/// governance, bridge, and smart account operations.
/// All variants go through consensus and are included in DAG vertices.
///
/// IMPORTANT: New variants MUST be appended at the end to preserve postcard
/// serialization compatibility. Inserting in the middle changes variant indices
/// and breaks deserialization of existing data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Transaction {
    Transfer(TransferTx),
    Stake(StakeTx),
    Unstake(UnstakeTx),
    CreateProposal(CreateProposalTx),
    Vote(VoteTx),
    Delegate(DelegateTx),
    Undelegate(UndelegateTx),
    SetCommission(SetCommissionTx),
    BridgeDeposit(BridgeDepositTx),
    BridgeRelease(BridgeReleaseTx),
    // SmartAccount transaction types
    AddKey(AddKeyTx),
    RemoveKey(RemoveKeyTx),
    SmartTransfer(SmartTransferTx),
    SetRecovery(SetRecoveryTx),
    RecoverAccount(RecoverAccountTx),
    CancelRecovery(CancelRecoveryTx),
    SetPolicy(SetPolicyTx),
    ExecuteVault(ExecuteVaultTx),
    CancelVault(CancelVaultTx),
    // Name Registry transaction types
    RegisterName(RegisterNameTx),
    RenewName(RenewNameTx),
    TransferName(TransferNameTx),
    UpdateProfile(UpdateProfileTx),
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
    #[serde(default)]
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
            Transaction::Delegate(tx) => tx.hash(),
            Transaction::Undelegate(tx) => tx.hash(),
            Transaction::SetCommission(tx) => tx.hash(),
            Transaction::BridgeDeposit(tx) => tx.hash(),
            Transaction::BridgeRelease(tx) => tx.hash(),
            Transaction::AddKey(tx) => tx.hash(),
            Transaction::RemoveKey(tx) => tx.hash(),
            Transaction::SmartTransfer(tx) => tx.hash(),
            Transaction::SetRecovery(tx) => tx.hash(),
            Transaction::RecoverAccount(tx) => tx.hash(),
            Transaction::CancelRecovery(tx) => tx.hash(),
            Transaction::SetPolicy(tx) => tx.hash(),
            Transaction::ExecuteVault(tx) => tx.hash(),
            Transaction::CancelVault(tx) => tx.hash(),
            Transaction::RegisterName(tx) => tx.hash(),
            Transaction::RenewName(tx) => tx.hash(),
            Transaction::TransferName(tx) => tx.hash(),
            Transaction::UpdateProfile(tx) => tx.hash(),
        }
    }

    /// Verify the transaction signature.
    /// NOTE: SmartTransfer returns false here — it requires state access for key lookup.
    /// The real verification for SmartTransfer happens in StateEngine.
    pub fn verify_signature(&self) -> bool {
        match self {
            Transaction::Transfer(tx) => tx.verify_signature(),
            Transaction::Stake(tx) => tx.verify_signature(),
            Transaction::Unstake(tx) => tx.verify_signature(),
            Transaction::CreateProposal(tx) => tx.verify_signature(),
            Transaction::Vote(tx) => tx.verify_signature(),
            Transaction::Delegate(tx) => tx.verify_signature(),
            Transaction::Undelegate(tx) => tx.verify_signature(),
            Transaction::SetCommission(tx) => tx.verify_signature(),
            Transaction::BridgeDeposit(tx) => tx.verify_signature(),
            Transaction::BridgeRelease(tx) => tx.verify_signature(),
            Transaction::AddKey(tx) => tx.verify_signature(),
            Transaction::RemoveKey(tx) => tx.verify_signature(),
            Transaction::SmartTransfer(tx) => tx.verify_signature(),
            Transaction::SetRecovery(tx) => tx.verify_signature(),
            Transaction::RecoverAccount(tx) => tx.verify_signature(),
            Transaction::CancelRecovery(tx) => tx.verify_signature(),
            Transaction::SetPolicy(tx) => tx.verify_signature(),
            Transaction::ExecuteVault(tx) => tx.verify_signature(),
            Transaction::CancelVault(tx) => tx.verify_signature(),
            Transaction::RegisterName(tx) => tx.verify_signature(),
            Transaction::RenewName(tx) => tx.verify_signature(),
            Transaction::TransferName(tx) => tx.verify_signature(),
            Transaction::UpdateProfile(tx) => tx.verify_signature(),
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
            Transaction::Delegate(tx) => tx.from,
            Transaction::Undelegate(tx) => tx.from,
            Transaction::SetCommission(tx) => tx.from,
            Transaction::BridgeDeposit(tx) => tx.from,
            Transaction::BridgeRelease(tx) => tx.from,
            Transaction::AddKey(tx) => tx.from,
            Transaction::RemoveKey(tx) => tx.from,
            Transaction::SmartTransfer(tx) => tx.from,
            Transaction::SetRecovery(tx) => tx.from,
            Transaction::RecoverAccount(tx) => tx.from,
            Transaction::CancelRecovery(tx) => tx.from,
            Transaction::SetPolicy(tx) => tx.from,
            Transaction::ExecuteVault(tx) => tx.from,
            Transaction::CancelVault(tx) => tx.from,
            Transaction::RegisterName(tx) => tx.from,
            Transaction::RenewName(tx) => tx.from,
            Transaction::TransferName(tx) => tx.from,
            Transaction::UpdateProfile(tx) => tx.from,
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
            Transaction::Delegate(tx) => tx.nonce,
            Transaction::Undelegate(tx) => tx.nonce,
            Transaction::SetCommission(tx) => tx.nonce,
            Transaction::BridgeDeposit(tx) => tx.nonce,
            Transaction::BridgeRelease(tx) => tx.nonce,
            Transaction::AddKey(tx) => tx.nonce,
            Transaction::RemoveKey(tx) => tx.nonce,
            Transaction::SmartTransfer(tx) => tx.nonce,
            Transaction::SetRecovery(tx) => tx.nonce,
            Transaction::RecoverAccount(tx) => tx.nonce,
            Transaction::CancelRecovery(tx) => tx.nonce,
            Transaction::SetPolicy(tx) => tx.nonce,
            Transaction::ExecuteVault(tx) => tx.nonce,
            Transaction::CancelVault(tx) => tx.nonce,
            Transaction::RegisterName(tx) => tx.nonce,
            Transaction::RenewName(tx) => tx.nonce,
            Transaction::TransferName(tx) => tx.nonce,
            Transaction::UpdateProfile(tx) => tx.nonce,
        }
    }

    /// Get the fee.
    pub fn fee(&self) -> u64 {
        match self {
            Transaction::Transfer(tx) => tx.fee,
            Transaction::CreateProposal(tx) => tx.fee,
            Transaction::Vote(tx) => tx.fee,
            Transaction::BridgeDeposit(tx) => tx.fee,
            Transaction::AddKey(tx) => tx.fee,
            Transaction::SmartTransfer(tx) => tx.fee,
            Transaction::SetRecovery(tx) => tx.fee,
            Transaction::SetPolicy(tx) => tx.fee,
            Transaction::RegisterName(tx) => tx.fee,
            Transaction::RenewName(tx) => tx.fee,
            Transaction::TransferName(tx) => tx.fee,
            Transaction::UpdateProfile(tx) => tx.fee,
            Transaction::Stake(_)
            | Transaction::Unstake(_)
            | Transaction::Delegate(_)
            | Transaction::Undelegate(_)
            | Transaction::SetCommission(_)
            | Transaction::BridgeRelease(_)
            | Transaction::RemoveKey(_)
            | Transaction::RecoverAccount(_)
            | Transaction::CancelRecovery(_)
            | Transaction::ExecuteVault(_)
            | Transaction::CancelVault(_) => 0,
        }
    }

    /// Get the amount.
    pub fn amount(&self) -> u64 {
        match self {
            Transaction::Transfer(tx) => tx.amount,
            Transaction::Stake(tx) => tx.amount,
            Transaction::Delegate(tx) => tx.amount,
            Transaction::BridgeDeposit(tx) => tx.amount,
            Transaction::BridgeRelease(tx) => tx.amount,
            Transaction::SmartTransfer(tx) => tx.amount,
            Transaction::Unstake(_)
            | Transaction::Undelegate(_)
            | Transaction::SetCommission(_)
            | Transaction::CreateProposal(_)
            | Transaction::Vote(_)
            | Transaction::AddKey(_)
            | Transaction::RemoveKey(_)
            | Transaction::SetRecovery(_)
            | Transaction::RecoverAccount(_)
            | Transaction::CancelRecovery(_)
            | Transaction::SetPolicy(_)
            | Transaction::ExecuteVault(_)
            | Transaction::CancelVault(_)
            | Transaction::RegisterName(_)
            | Transaction::RenewName(_)
            | Transaction::TransferName(_)
            | Transaction::UpdateProfile(_) => 0,
        }
    }

    /// Get the recipient address.
    pub fn to(&self) -> Option<Address> {
        match self {
            Transaction::Transfer(tx) => Some(tx.to),
            Transaction::SmartTransfer(tx) => Some(tx.to),
            Transaction::Delegate(tx) => Some(tx.validator),
            _ => None,
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
            Transaction::Delegate(tx) => tx.pub_key,
            Transaction::Undelegate(tx) => tx.pub_key,
            Transaction::SetCommission(tx) => tx.pub_key,
            Transaction::BridgeDeposit(tx) => tx.pub_key,
            Transaction::BridgeRelease(tx) => tx.pub_key,
            Transaction::AddKey(tx) => tx.pub_key,
            Transaction::RemoveKey(tx) => tx.pub_key,
            Transaction::SetRecovery(tx) => tx.pub_key,
            Transaction::RecoverAccount(tx) => tx.pub_key,
            Transaction::CancelRecovery(tx) => tx.pub_key,
            Transaction::SetPolicy(tx) => tx.pub_key,
            Transaction::ExecuteVault(tx) => tx.pub_key,
            Transaction::CancelVault(tx) => tx.pub_key,
            Transaction::RegisterName(tx) => tx.pub_key,
            Transaction::RenewName(tx) => tx.pub_key,
            Transaction::TransferName(tx) => tx.pub_key,
            Transaction::UpdateProfile(tx) => tx.pub_key,
            // SmartTransfer uses signing_key_id, not a raw pub_key.
            Transaction::SmartTransfer(_) => [0u8; 32],
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
            Transaction::Delegate(tx) => tx.signable_bytes(),
            Transaction::Undelegate(tx) => tx.signable_bytes(),
            Transaction::SetCommission(tx) => tx.signable_bytes(),
            Transaction::BridgeDeposit(tx) => tx.signable_bytes(),
            Transaction::BridgeRelease(tx) => tx.signable_bytes(),
            Transaction::AddKey(tx) => tx.signable_bytes(),
            Transaction::RemoveKey(tx) => tx.signable_bytes(),
            Transaction::SmartTransfer(tx) => tx.signable_bytes(),
            Transaction::SetRecovery(tx) => tx.signable_bytes(),
            Transaction::RecoverAccount(tx) => tx.signable_bytes(),
            Transaction::CancelRecovery(tx) => tx.signable_bytes(),
            Transaction::SetPolicy(tx) => tx.signable_bytes(),
            Transaction::ExecuteVault(tx) => tx.signable_bytes(),
            Transaction::CancelVault(tx) => tx.signable_bytes(),
            Transaction::RegisterName(tx) => tx.signable_bytes(),
            Transaction::RenewName(tx) => tx.signable_bytes(),
            Transaction::TransferName(tx) => tx.signable_bytes(),
            Transaction::UpdateProfile(tx) => tx.signable_bytes(),
        }
    }

    /// Get the total cost.
    pub fn total_cost(&self) -> u64 {
        match self {
            Transaction::Transfer(tx) => tx.total_cost(),
            Transaction::Stake(tx) => tx.amount,
            Transaction::Delegate(tx) => tx.amount,
            Transaction::BridgeDeposit(tx) => tx.total_cost(),
            Transaction::CreateProposal(tx) => tx.fee,
            Transaction::Vote(tx) => tx.fee,
            Transaction::AddKey(tx) => tx.total_cost(),
            Transaction::SmartTransfer(tx) => tx.total_cost(),
            Transaction::SetRecovery(tx) => tx.total_cost(),
            Transaction::SetPolicy(tx) => tx.total_cost(),
            Transaction::RegisterName(tx) => tx.total_cost(),
            Transaction::RenewName(tx) => tx.total_cost(),
            Transaction::TransferName(tx) => tx.total_cost(),
            Transaction::UpdateProfile(tx) => tx.total_cost(),
            Transaction::Unstake(_)
            | Transaction::Undelegate(_)
            | Transaction::SetCommission(_)
            | Transaction::BridgeRelease(_)
            | Transaction::RemoveKey(_)
            | Transaction::RecoverAccount(_)
            | Transaction::CancelRecovery(_)
            | Transaction::ExecuteVault(_)
            | Transaction::CancelVault(_) => 0,
        }
    }
}

impl TransferTx {
    /// Compute the transaction hash (its unique identifier).
    /// Includes a type discriminator to prevent cross-type hash collisions
    /// (e.g., a TransferTx and VoteTx with aligned field bytes producing the
    /// same hash, which would break mempool deduplication and tx_index lookups).
    pub fn hash(&self) -> [u8; 32] {
        let mut hasher = blake3::Hasher::new();
        hasher.update(b"transfer");
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
        let expected_addr = Address::from_pubkey(&self.pub_key);
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
            // Should be NETWORK_ID (19) + "transfer" (8) + from (20) + to (20) + amount (8) + fee (8) + nonce (8) = 91 bytes
            assert_eq!(transfer.signable_bytes().len(), 91);
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
