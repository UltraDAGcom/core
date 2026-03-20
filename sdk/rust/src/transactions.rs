//! Client-side transaction building and signing.
//!
//! This module provides builder functions that construct and sign each UltraDAG
//! transaction type locally using the canonical types from `ultradag-coin`.
//! The resulting [`Transaction`] can be submitted to a node via
//! [`UltraDagClient::submit_transaction`](crate::client::UltraDagClient::submit_transaction).
//!
//! All signing happens client-side — private keys never leave the caller's process.
//!
//! # Example
//!
//! ```no_run
//! use ultradag_sdk::transactions;
//! use ultradag_sdk::UltraDagClient;
//! use ultradag_coin::SecretKey;
//! use ultradag_coin::Address;
//!
//! let sk = SecretKey::from_bytes([0x42; 32]);
//! let recipient = Address([0xAB; 20]);
//! let tx = transactions::build_transfer(&sk, recipient, 1_000_000, 10_000, 0, None);
//!
//! let client = UltraDagClient::default_local();
//! // client.submit_transaction(&tx).unwrap();
//! ```

use ultradag_coin::address::{Address, SecretKey, Signature};
use ultradag_coin::tx::transaction::{Transaction, TransferTx};
use ultradag_coin::tx::stake::{StakeTx, UnstakeTx};
use ultradag_coin::tx::delegate::{DelegateTx, UndelegateTx, SetCommissionTx};
use ultradag_coin::governance::{CreateProposalTx, VoteTx, ProposalType};

/// Build and sign a transfer transaction.
///
/// # Arguments
/// * `secret_key` - Sender's Ed25519 secret key
/// * `to` - Recipient address
/// * `amount` - Amount in sats (1 UDAG = 100,000,000 sats)
/// * `fee` - Transaction fee in sats
/// * `nonce` - Sender's current nonce (fetch via `/balance/{address}`)
/// * `memo` - Optional data payload (max 256 bytes)
pub fn build_transfer(
    secret_key: &SecretKey,
    to: Address,
    amount: u64,
    fee: u64,
    nonce: u64,
    memo: Option<Vec<u8>>,
) -> Transaction {
    let mut tx = TransferTx {
        from: secret_key.address(),
        to,
        amount,
        fee,
        nonce,
        pub_key: secret_key.verifying_key().to_bytes(),
        signature: Signature([0u8; 64]),
        memo,
    };
    tx.signature = secret_key.sign(&tx.signable_bytes());
    Transaction::Transfer(tx)
}

/// Build and sign a stake transaction.
///
/// Locks UDAG as validator stake. Minimum stake: 10,000 UDAG.
/// Fee is zero (fee-exempt).
///
/// # Arguments
/// * `secret_key` - Staker's Ed25519 secret key
/// * `amount` - Amount to stake in sats
/// * `nonce` - Sender's current nonce
pub fn build_stake(
    secret_key: &SecretKey,
    amount: u64,
    nonce: u64,
) -> Transaction {
    let mut tx = StakeTx {
        from: secret_key.address(),
        amount,
        nonce,
        pub_key: secret_key.verifying_key().to_bytes(),
        signature: Signature([0u8; 64]),
    };
    tx.signature = secret_key.sign(&tx.signable_bytes());
    Transaction::Stake(tx)
}

/// Build and sign an unstake transaction.
///
/// Begins the unstake cooldown period (2,016 rounds). Fee is zero (fee-exempt).
///
/// # Arguments
/// * `secret_key` - Staker's Ed25519 secret key
/// * `nonce` - Sender's current nonce
pub fn build_unstake(
    secret_key: &SecretKey,
    nonce: u64,
) -> Transaction {
    let mut tx = UnstakeTx {
        from: secret_key.address(),
        nonce,
        pub_key: secret_key.verifying_key().to_bytes(),
        signature: Signature([0u8; 64]),
    };
    tx.signature = secret_key.sign(&tx.signable_bytes());
    Transaction::Unstake(tx)
}

/// Build and sign a delegate transaction.
///
/// Delegates UDAG to a validator for passive reward earning. Minimum: 100 UDAG.
/// Fee is zero (fee-exempt).
///
/// # Arguments
/// * `secret_key` - Delegator's Ed25519 secret key
/// * `validator` - Target validator address
/// * `amount` - Amount to delegate in sats
/// * `nonce` - Sender's current nonce
pub fn build_delegate(
    secret_key: &SecretKey,
    validator: Address,
    amount: u64,
    nonce: u64,
) -> Transaction {
    let mut tx = DelegateTx {
        from: secret_key.address(),
        validator,
        amount,
        nonce,
        pub_key: secret_key.verifying_key().to_bytes(),
        signature: Signature([0u8; 64]),
    };
    tx.signature = secret_key.sign(&tx.signable_bytes());
    Transaction::Delegate(tx)
}

/// Build and sign an undelegate transaction.
///
/// Begins undelegation cooldown. Fee is zero (fee-exempt).
///
/// # Arguments
/// * `secret_key` - Delegator's Ed25519 secret key
/// * `nonce` - Sender's current nonce
pub fn build_undelegate(
    secret_key: &SecretKey,
    nonce: u64,
) -> Transaction {
    let mut tx = UndelegateTx {
        from: secret_key.address(),
        nonce,
        pub_key: secret_key.verifying_key().to_bytes(),
        signature: Signature([0u8; 64]),
    };
    tx.signature = secret_key.sign(&tx.signable_bytes());
    Transaction::Undelegate(tx)
}

/// Build and sign a set-commission transaction.
///
/// Validators use this to set their commission rate on delegated rewards.
/// Fee is zero (fee-exempt).
///
/// # Arguments
/// * `secret_key` - Validator's Ed25519 secret key
/// * `commission_percent` - Commission rate (0-100%)
/// * `nonce` - Sender's current nonce
pub fn build_set_commission(
    secret_key: &SecretKey,
    commission_percent: u8,
    nonce: u64,
) -> Transaction {
    let mut tx = SetCommissionTx {
        from: secret_key.address(),
        commission_percent,
        nonce,
        pub_key: secret_key.verifying_key().to_bytes(),
        signature: Signature([0u8; 64]),
    };
    tx.signature = secret_key.sign(&tx.signable_bytes());
    Transaction::SetCommission(tx)
}

/// Build and sign a create-proposal transaction.
///
/// # Arguments
/// * `secret_key` - Proposer's Ed25519 secret key
/// * `proposal_id` - Proposal ID (fetch next available from node)
/// * `title` - Proposal title (max 128 bytes)
/// * `description` - Proposal description (max 4096 bytes)
/// * `proposal_type` - Type of proposal (text, parameter change, council membership, treasury spend)
/// * `fee` - Transaction fee in sats
/// * `nonce` - Sender's current nonce
pub fn build_create_proposal(
    secret_key: &SecretKey,
    proposal_id: u64,
    title: String,
    description: String,
    proposal_type: ProposalType,
    fee: u64,
    nonce: u64,
) -> Transaction {
    let mut tx = CreateProposalTx {
        from: secret_key.address(),
        proposal_id,
        title,
        description,
        proposal_type,
        fee,
        nonce,
        pub_key: secret_key.verifying_key().to_bytes(),
        signature: Signature([0u8; 64]),
    };
    tx.signature = secret_key.sign(&tx.signable_bytes());
    Transaction::CreateProposal(tx)
}

/// Build and sign a vote transaction.
///
/// # Arguments
/// * `secret_key` - Voter's Ed25519 secret key
/// * `proposal_id` - ID of the proposal to vote on
/// * `approve` - `true` for yes, `false` for no
/// * `fee` - Transaction fee in sats
/// * `nonce` - Sender's current nonce
pub fn build_vote(
    secret_key: &SecretKey,
    proposal_id: u64,
    approve: bool,
    fee: u64,
    nonce: u64,
) -> Transaction {
    let mut tx = VoteTx {
        from: secret_key.address(),
        proposal_id,
        vote: approve,
        fee,
        nonce,
        pub_key: secret_key.verifying_key().to_bytes(),
        signature: Signature([0u8; 64]),
    };
    tx.signature = secret_key.sign(&tx.signable_bytes());
    Transaction::Vote(tx)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ultradag_coin::address::SecretKey;

    fn test_key() -> SecretKey {
        SecretKey::from_bytes([0x42; 32])
    }

    fn other_address() -> Address {
        SecretKey::from_bytes([0xBB; 32]).address()
    }

    #[test]
    fn transfer_signs_and_verifies() {
        let sk = test_key();
        let tx = build_transfer(&sk, other_address(), 1_000_000, 10_000, 0, None);
        assert!(tx.verify_signature());
        assert_eq!(tx.from(), sk.address());
        assert_eq!(tx.fee(), 10_000);
        assert_eq!(tx.amount(), 1_000_000);
        assert_eq!(tx.nonce(), 0);
    }

    #[test]
    fn transfer_with_memo_signs_and_verifies() {
        let sk = test_key();
        let memo = b"sensor:temp=22.5C".to_vec();
        let tx = build_transfer(&sk, other_address(), 100_000, 10_000, 1, Some(memo));
        assert!(tx.verify_signature());
    }

    #[test]
    fn stake_signs_and_verifies() {
        let sk = test_key();
        let tx = build_stake(&sk, 10_000 * ultradag_coin::COIN, 0);
        assert!(tx.verify_signature());
        assert_eq!(tx.from(), sk.address());
        assert_eq!(tx.fee(), 0);
        assert_eq!(tx.nonce(), 0);
    }

    #[test]
    fn unstake_signs_and_verifies() {
        let sk = test_key();
        let tx = build_unstake(&sk, 5);
        assert!(tx.verify_signature());
        assert_eq!(tx.from(), sk.address());
        assert_eq!(tx.fee(), 0);
        assert_eq!(tx.nonce(), 5);
    }

    #[test]
    fn delegate_signs_and_verifies() {
        let sk = test_key();
        let validator = other_address();
        let tx = build_delegate(&sk, validator, 100 * ultradag_coin::COIN, 0);
        assert!(tx.verify_signature());
        assert_eq!(tx.from(), sk.address());
        assert_eq!(tx.to(), Some(validator));
        assert_eq!(tx.fee(), 0);
    }

    #[test]
    fn undelegate_signs_and_verifies() {
        let sk = test_key();
        let tx = build_undelegate(&sk, 3);
        assert!(tx.verify_signature());
        assert_eq!(tx.from(), sk.address());
        assert_eq!(tx.nonce(), 3);
    }

    #[test]
    fn set_commission_signs_and_verifies() {
        let sk = test_key();
        let tx = build_set_commission(&sk, 15, 0);
        assert!(tx.verify_signature());
        assert_eq!(tx.from(), sk.address());
    }

    #[test]
    fn create_proposal_signs_and_verifies() {
        let sk = test_key();
        let tx = build_create_proposal(
            &sk,
            1,
            "Test Proposal".to_string(),
            "A test description".to_string(),
            ProposalType::TextProposal,
            10_000,
            0,
        );
        assert!(tx.verify_signature());
        assert_eq!(tx.from(), sk.address());
        assert_eq!(tx.fee(), 10_000);
    }

    #[test]
    fn create_parameter_change_proposal_signs_and_verifies() {
        let sk = test_key();
        let tx = build_create_proposal(
            &sk,
            2,
            "Change min fee".to_string(),
            "Lower the minimum fee".to_string(),
            ProposalType::ParameterChange {
                param: "min_fee_sats".to_string(),
                new_value: "5000".to_string(),
            },
            10_000,
            1,
        );
        assert!(tx.verify_signature());
    }

    #[test]
    fn vote_signs_and_verifies() {
        let sk = test_key();
        let tx = build_vote(&sk, 1, true, 10_000, 0);
        assert!(tx.verify_signature());
        assert_eq!(tx.from(), sk.address());
        assert_eq!(tx.fee(), 10_000);
    }

    #[test]
    fn vote_no_signs_and_verifies() {
        let sk = test_key();
        let tx = build_vote(&sk, 1, false, 10_000, 2);
        assert!(tx.verify_signature());
        assert_eq!(tx.nonce(), 2);
    }

    #[test]
    fn different_nonces_produce_different_hashes() {
        let sk = test_key();
        let tx1 = build_transfer(&sk, other_address(), 100, 10, 0, None);
        let tx2 = build_transfer(&sk, other_address(), 100, 10, 1, None);
        assert_ne!(tx1.hash(), tx2.hash());
    }

    #[test]
    fn serialization_roundtrip() {
        let sk = test_key();
        let tx = build_transfer(&sk, other_address(), 500_000, 10_000, 0, None);
        let json = serde_json::to_string(&tx).expect("serialize");
        let deserialized: Transaction = serde_json::from_str(&json).expect("deserialize");
        assert!(deserialized.verify_signature());
        assert_eq!(tx.hash(), deserialized.hash());
    }
}
