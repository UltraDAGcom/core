use serde::{Deserialize, Serialize};

use crate::address::{Address, Signature};

/// A streaming payment: sender deposits UDAG that accrues to recipient at a fixed rate per round.
/// The recipient can withdraw accrued funds at any time. The sender can cancel, which
/// immediately credits remaining accrued funds to the recipient and refunds the rest to the sender.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Stream {
    /// Unique stream identifier: blake3(sender.0 || recipient.0 || nonce_le).
    pub id: [u8; 32],
    /// Address that created and funded the stream.
    pub sender: Address,
    /// Address that receives the streaming payment.
    pub recipient: Address,
    /// Payment rate in sats per round.
    pub rate_sats_per_round: u64,
    /// Round at which streaming started.
    pub start_round: u64,
    /// Total UDAG deposited (locked from sender's balance at creation).
    pub deposited: u64,
    /// Total UDAG already withdrawn by the recipient.
    pub withdrawn: u64,
    /// Number of rounds after start_round before any funds accrue (vesting cliff).
    /// During the cliff period, accrued_at() returns 0. After the cliff, full accrual
    /// from start_round is unlocked at once (standard vesting behavior).
    #[serde(default)]
    pub cliff_rounds: u64,
    /// If cancelled, the round at which cancellation occurred.
    pub cancelled_at_round: Option<u64>,
    /// Whether the recipient has been credited their accrued amount on cancellation.
    pub cancel_recipient_credited: bool,
}

impl Stream {
    /// Compute the total amount accrued to the recipient at a given round.
    /// During the cliff period (round < start_round + cliff_rounds), returns 0.
    /// After the cliff, full accrual from start_round is unlocked (standard vesting cliff).
    /// Returns min(rate * elapsed_rounds, deposited).
    pub fn accrued_at(&self, round: u64) -> u64 {
        let effective_end = self.cancelled_at_round.unwrap_or(round);
        let cliff_end = self.start_round.saturating_add(self.cliff_rounds);
        if effective_end < cliff_end {
            return 0; // Still in cliff period
        }
        // After cliff: full accrual from start_round (not from cliff_end)
        let elapsed = effective_end.saturating_sub(self.start_round);
        let total_accrued = self.rate_sats_per_round.saturating_mul(elapsed);
        std::cmp::min(total_accrued, self.deposited)
    }

    /// Compute the amount the recipient can withdraw at a given round.
    pub fn withdrawable_at(&self, round: u64) -> u64 {
        self.accrued_at(round).saturating_sub(self.withdrawn)
    }

    /// Check if the stream is fully depleted (all deposited funds have accrued).
    pub fn is_depleted_at(&self, round: u64) -> bool {
        self.accrued_at(round) >= self.deposited
    }

    /// Compute the round at which the stream will be fully depleted.
    /// Returns start_round + deposited / rate_sats_per_round.
    pub fn end_round(&self) -> u64 {
        if self.rate_sats_per_round == 0 {
            return u64::MAX;
        }
        self.start_round.saturating_add(self.deposited / self.rate_sats_per_round)
    }

    /// Check if the stream is still active (not cancelled and not depleted).
    pub fn is_active_at(&self, round: u64) -> bool {
        self.cancelled_at_round.is_none() && !self.is_depleted_at(round)
    }
}

/// Create a new streaming payment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateStreamTx {
    pub from: Address,
    pub recipient: Address,
    pub rate_sats_per_round: u64,
    pub deposit: u64,
    /// Number of rounds after stream start before any funds accrue (vesting cliff).
    #[serde(default)]
    pub cliff_rounds: u64,
    pub fee: u64,
    pub nonce: u64,
    pub pub_key: [u8; 32],
    pub signature: Signature,
}

impl CreateStreamTx {
    pub fn signable_bytes(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(128);
        buf.extend_from_slice(crate::constants::NETWORK_ID);
        buf.extend_from_slice(b"create_stream");
        buf.extend_from_slice(&self.from.0);
        buf.extend_from_slice(&self.recipient.0);
        buf.extend_from_slice(&self.rate_sats_per_round.to_le_bytes());
        buf.extend_from_slice(&self.deposit.to_le_bytes());
        buf.extend_from_slice(&self.cliff_rounds.to_le_bytes());
        buf.extend_from_slice(&self.fee.to_le_bytes());
        buf.extend_from_slice(&self.nonce.to_le_bytes());
        buf
    }

    pub fn hash(&self) -> [u8; 32] {
        let mut hasher = blake3::Hasher::new();
        hasher.update(b"create_stream");
        hasher.update(&self.from.0);
        hasher.update(&self.recipient.0);
        hasher.update(&self.rate_sats_per_round.to_le_bytes());
        hasher.update(&self.deposit.to_le_bytes());
        hasher.update(&self.cliff_rounds.to_le_bytes());
        hasher.update(&self.fee.to_le_bytes());
        hasher.update(&self.nonce.to_le_bytes());
        *hasher.finalize().as_bytes()
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

    pub fn total_cost(&self) -> u64 {
        self.deposit.saturating_add(self.fee)
    }
}

/// Withdraw accrued funds from a stream (called by the recipient).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WithdrawStreamTx {
    pub from: Address,
    pub stream_id: [u8; 32],
    pub fee: u64,
    pub nonce: u64,
    pub pub_key: [u8; 32],
    pub signature: Signature,
}

impl WithdrawStreamTx {
    pub fn signable_bytes(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(128);
        buf.extend_from_slice(crate::constants::NETWORK_ID);
        buf.extend_from_slice(b"withdraw_stream");
        buf.extend_from_slice(&self.from.0);
        buf.extend_from_slice(&self.stream_id);
        buf.extend_from_slice(&self.fee.to_le_bytes());
        buf.extend_from_slice(&self.nonce.to_le_bytes());
        buf
    }

    pub fn hash(&self) -> [u8; 32] {
        let mut hasher = blake3::Hasher::new();
        hasher.update(b"withdraw_stream");
        hasher.update(&self.from.0);
        hasher.update(&self.stream_id);
        hasher.update(&self.fee.to_le_bytes());
        hasher.update(&self.nonce.to_le_bytes());
        *hasher.finalize().as_bytes()
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

    pub fn total_cost(&self) -> u64 {
        self.fee
    }
}

/// Cancel a stream (called by the sender). Credits remaining accrued funds to recipient,
/// refunds unaccrued deposit to sender.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CancelStreamTx {
    pub from: Address,
    pub stream_id: [u8; 32],
    pub fee: u64,
    pub nonce: u64,
    pub pub_key: [u8; 32],
    pub signature: Signature,
}

impl CancelStreamTx {
    pub fn signable_bytes(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(128);
        buf.extend_from_slice(crate::constants::NETWORK_ID);
        buf.extend_from_slice(b"cancel_stream");
        buf.extend_from_slice(&self.from.0);
        buf.extend_from_slice(&self.stream_id);
        buf.extend_from_slice(&self.fee.to_le_bytes());
        buf.extend_from_slice(&self.nonce.to_le_bytes());
        buf
    }

    pub fn hash(&self) -> [u8; 32] {
        let mut hasher = blake3::Hasher::new();
        hasher.update(b"cancel_stream");
        hasher.update(&self.from.0);
        hasher.update(&self.stream_id);
        hasher.update(&self.fee.to_le_bytes());
        hasher.update(&self.nonce.to_le_bytes());
        *hasher.finalize().as_bytes()
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

    pub fn total_cost(&self) -> u64 {
        self.fee
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::address::SecretKey;

    fn make_create_stream_tx(
        sk: &SecretKey,
        recipient: Address,
        rate: u64,
        deposit: u64,
        fee: u64,
        nonce: u64,
    ) -> CreateStreamTx {
        make_create_stream_tx_with_cliff(sk, recipient, rate, deposit, 0, fee, nonce)
    }

    fn make_create_stream_tx_with_cliff(
        sk: &SecretKey,
        recipient: Address,
        rate: u64,
        deposit: u64,
        cliff_rounds: u64,
        fee: u64,
        nonce: u64,
    ) -> CreateStreamTx {
        let mut tx = CreateStreamTx {
            from: sk.address(),
            recipient,
            rate_sats_per_round: rate,
            deposit,
            cliff_rounds,
            fee,
            nonce,
            pub_key: sk.verifying_key().to_bytes(),
            signature: Signature([0u8; 64]),
        };
        tx.signature = sk.sign(&tx.signable_bytes());
        tx
    }

    fn make_withdraw_stream_tx(
        sk: &SecretKey,
        stream_id: [u8; 32],
        fee: u64,
        nonce: u64,
    ) -> WithdrawStreamTx {
        let mut tx = WithdrawStreamTx {
            from: sk.address(),
            stream_id,
            fee,
            nonce,
            pub_key: sk.verifying_key().to_bytes(),
            signature: Signature([0u8; 64]),
        };
        tx.signature = sk.sign(&tx.signable_bytes());
        tx
    }

    fn make_cancel_stream_tx(
        sk: &SecretKey,
        stream_id: [u8; 32],
        fee: u64,
        nonce: u64,
    ) -> CancelStreamTx {
        let mut tx = CancelStreamTx {
            from: sk.address(),
            stream_id,
            fee,
            nonce,
            pub_key: sk.verifying_key().to_bytes(),
            signature: Signature([0u8; 64]),
        };
        tx.signature = sk.sign(&tx.signable_bytes());
        tx
    }

    #[test]
    fn stream_accrued_at_basic() {
        let stream = Stream {
            id: [0u8; 32],
            sender: Address::ZERO,
            recipient: Address::ZERO,
            rate_sats_per_round: 100,
            start_round: 10,
            deposited: 1000,
            withdrawn: 0,
            cliff_rounds: 0,
            cancelled_at_round: None,
            cancel_recipient_credited: false,
        };
        assert_eq!(stream.accrued_at(10), 0);
        assert_eq!(stream.accrued_at(15), 500);
        assert_eq!(stream.accrued_at(20), 1000);
        assert_eq!(stream.accrued_at(25), 1000); // capped at deposited
    }

    #[test]
    fn stream_accrued_at_cancelled() {
        let stream = Stream {
            id: [0u8; 32],
            sender: Address::ZERO,
            recipient: Address::ZERO,
            rate_sats_per_round: 100,
            start_round: 10,
            deposited: 1000,
            withdrawn: 0,
            cliff_rounds: 0,
            cancelled_at_round: Some(15),
            cancel_recipient_credited: false,
        };
        // Cancelled at round 15 — accrued should be frozen at 500
        assert_eq!(stream.accrued_at(15), 500);
        assert_eq!(stream.accrued_at(20), 500);
        assert_eq!(stream.accrued_at(100), 500);
    }

    #[test]
    fn stream_withdrawable_at() {
        let stream = Stream {
            id: [0u8; 32],
            sender: Address::ZERO,
            recipient: Address::ZERO,
            rate_sats_per_round: 100,
            start_round: 10,
            deposited: 1000,
            withdrawn: 300,
            cliff_rounds: 0,
            cancelled_at_round: None,
            cancel_recipient_credited: false,
        };
        assert_eq!(stream.withdrawable_at(15), 200); // 500 accrued - 300 withdrawn
        assert_eq!(stream.withdrawable_at(20), 700); // 1000 accrued - 300 withdrawn
    }

    #[test]
    fn stream_is_depleted_at() {
        let stream = Stream {
            id: [0u8; 32],
            sender: Address::ZERO,
            recipient: Address::ZERO,
            rate_sats_per_round: 100,
            start_round: 10,
            deposited: 1000,
            withdrawn: 0,
            cliff_rounds: 0,
            cancelled_at_round: None,
            cancel_recipient_credited: false,
        };
        assert!(!stream.is_depleted_at(15));
        assert!(stream.is_depleted_at(20));
        assert!(stream.is_depleted_at(25));
    }

    #[test]
    fn stream_end_round() {
        let stream = Stream {
            id: [0u8; 32],
            sender: Address::ZERO,
            recipient: Address::ZERO,
            rate_sats_per_round: 100,
            start_round: 10,
            deposited: 1000,
            withdrawn: 0,
            cliff_rounds: 0,
            cancelled_at_round: None,
            cancel_recipient_credited: false,
        };
        assert_eq!(stream.end_round(), 20);
    }

    #[test]
    fn stream_end_round_zero_rate() {
        let stream = Stream {
            id: [0u8; 32],
            sender: Address::ZERO,
            recipient: Address::ZERO,
            rate_sats_per_round: 0,
            start_round: 10,
            deposited: 1000,
            withdrawn: 0,
            cliff_rounds: 0,
            cancelled_at_round: None,
            cancel_recipient_credited: false,
        };
        assert_eq!(stream.end_round(), u64::MAX);
    }

    #[test]
    fn stream_is_active_at() {
        let active = Stream {
            id: [0u8; 32],
            sender: Address::ZERO,
            recipient: Address::ZERO,
            rate_sats_per_round: 100,
            start_round: 10,
            deposited: 1000,
            withdrawn: 0,
            cliff_rounds: 0,
            cancelled_at_round: None,
            cancel_recipient_credited: false,
        };
        assert!(active.is_active_at(15));
        assert!(!active.is_active_at(20)); // depleted

        let cancelled = Stream {
            cancelled_at_round: Some(12),
            ..active.clone()
        };
        assert!(!cancelled.is_active_at(15));
    }

    #[test]
    fn create_stream_tx_signature_valid() {
        let sk = SecretKey::generate();
        let recipient = SecretKey::generate().address();
        let tx = make_create_stream_tx(&sk, recipient, 100, 1000, 10000, 0);
        assert!(tx.verify_signature());
    }

    #[test]
    fn create_stream_tx_tampered_rejected() {
        let sk = SecretKey::generate();
        let recipient = SecretKey::generate().address();
        let mut tx = make_create_stream_tx(&sk, recipient, 100, 1000, 10000, 0);
        tx.deposit = 2000;
        assert!(!tx.verify_signature());
    }

    #[test]
    fn create_stream_tx_hash_deterministic() {
        let sk = SecretKey::generate();
        let recipient = SecretKey::generate().address();
        let tx = make_create_stream_tx(&sk, recipient, 100, 1000, 10000, 0);
        assert_eq!(tx.hash(), tx.hash());
    }

    #[test]
    fn create_stream_tx_total_cost() {
        let sk = SecretKey::generate();
        let recipient = SecretKey::generate().address();
        let tx = make_create_stream_tx(&sk, recipient, 100, 1000, 500, 0);
        assert_eq!(tx.total_cost(), 1500);
    }

    #[test]
    fn withdraw_stream_tx_signature_valid() {
        let sk = SecretKey::generate();
        let tx = make_withdraw_stream_tx(&sk, [1u8; 32], 10000, 0);
        assert!(tx.verify_signature());
    }

    #[test]
    fn withdraw_stream_tx_tampered_rejected() {
        let sk = SecretKey::generate();
        let mut tx = make_withdraw_stream_tx(&sk, [1u8; 32], 10000, 0);
        tx.stream_id = [2u8; 32];
        assert!(!tx.verify_signature());
    }

    #[test]
    fn cancel_stream_tx_signature_valid() {
        let sk = SecretKey::generate();
        let tx = make_cancel_stream_tx(&sk, [1u8; 32], 10000, 0);
        assert!(tx.verify_signature());
    }

    #[test]
    fn cancel_stream_tx_tampered_rejected() {
        let sk = SecretKey::generate();
        let mut tx = make_cancel_stream_tx(&sk, [1u8; 32], 10000, 0);
        tx.stream_id = [2u8; 32];
        assert!(!tx.verify_signature());
    }

    #[test]
    fn different_tx_types_different_hashes() {
        let sk = SecretKey::generate();
        let recipient = SecretKey::generate().address();
        let create = make_create_stream_tx(&sk, recipient, 100, 1000, 10000, 0);
        let withdraw = make_withdraw_stream_tx(&sk, [1u8; 32], 10000, 0);
        let cancel = make_cancel_stream_tx(&sk, [1u8; 32], 10000, 0);
        assert_ne!(create.hash(), withdraw.hash());
        assert_ne!(withdraw.hash(), cancel.hash());
        assert_ne!(create.hash(), cancel.hash());
    }

    #[test]
    fn stream_accrued_saturating() {
        // Test that rate * elapsed doesn't overflow — uses saturating_mul
        let stream = Stream {
            id: [0u8; 32],
            sender: Address::ZERO,
            recipient: Address::ZERO,
            rate_sats_per_round: u64::MAX,
            start_round: 0,
            deposited: 1000,
            withdrawn: 0,
            cliff_rounds: 0,
            cancelled_at_round: None,
            cancel_recipient_credited: false,
        };
        // saturating_mul(u64::MAX, 10) = u64::MAX, but capped at deposited=1000
        assert_eq!(stream.accrued_at(10), 1000);
    }

    #[test]
    fn stream_cliff_no_accrual_during_cliff() {
        let stream = Stream {
            id: [0u8; 32],
            sender: Address::ZERO,
            recipient: Address::ZERO,
            rate_sats_per_round: 100,
            start_round: 10,
            deposited: 1000,
            withdrawn: 0,
            cliff_rounds: 5,
            cancelled_at_round: None,
            cancel_recipient_credited: false,
        };
        // Cliff ends at round 15. Before that, nothing accrues.
        assert_eq!(stream.accrued_at(10), 0);
        assert_eq!(stream.accrued_at(12), 0);
        assert_eq!(stream.accrued_at(14), 0);
    }

    #[test]
    fn stream_cliff_unlocks_full_accrual_after_cliff() {
        let stream = Stream {
            id: [0u8; 32],
            sender: Address::ZERO,
            recipient: Address::ZERO,
            rate_sats_per_round: 100,
            start_round: 10,
            deposited: 1000,
            withdrawn: 0,
            cliff_rounds: 5,
            cancelled_at_round: None,
            cancel_recipient_credited: false,
        };
        // Cliff ends at round 15. At round 15: rate * (15-10) = 500
        assert_eq!(stream.accrued_at(15), 500);
        // At round 18: rate * (18-10) = 800
        assert_eq!(stream.accrued_at(18), 800);
        // At round 20: rate * (20-10) = 1000 (capped at deposited)
        assert_eq!(stream.accrued_at(20), 1000);
        assert_eq!(stream.accrued_at(25), 1000); // still capped
    }

    #[test]
    fn stream_cliff_cancelled_during_cliff() {
        let stream = Stream {
            id: [0u8; 32],
            sender: Address::ZERO,
            recipient: Address::ZERO,
            rate_sats_per_round: 100,
            start_round: 10,
            deposited: 1000,
            withdrawn: 0,
            cliff_rounds: 5,
            cancelled_at_round: Some(13), // cancelled before cliff ends
            cancel_recipient_credited: false,
        };
        // Cancelled at round 13, cliff ends at 15 — nothing accrued ever
        assert_eq!(stream.accrued_at(13), 0);
        assert_eq!(stream.accrued_at(20), 0);
        assert_eq!(stream.accrued_at(100), 0);
    }

    #[test]
    fn stream_cliff_cancelled_after_cliff() {
        let stream = Stream {
            id: [0u8; 32],
            sender: Address::ZERO,
            recipient: Address::ZERO,
            rate_sats_per_round: 100,
            start_round: 10,
            deposited: 1000,
            withdrawn: 0,
            cliff_rounds: 5,
            cancelled_at_round: Some(18), // cancelled after cliff
            cancel_recipient_credited: false,
        };
        // Cancelled at round 18, cliff ends at 15. Accrued = 100 * (18-10) = 800
        assert_eq!(stream.accrued_at(18), 800);
        assert_eq!(stream.accrued_at(25), 800); // frozen at cancellation
    }

    #[test]
    fn stream_cliff_withdrawable_after_cliff() {
        let stream = Stream {
            id: [0u8; 32],
            sender: Address::ZERO,
            recipient: Address::ZERO,
            rate_sats_per_round: 100,
            start_round: 10,
            deposited: 1000,
            withdrawn: 0,
            cliff_rounds: 5,
            cancelled_at_round: None,
            cancel_recipient_credited: false,
        };
        // During cliff: nothing withdrawable
        assert_eq!(stream.withdrawable_at(12), 0);
        // After cliff: full accrual available
        assert_eq!(stream.withdrawable_at(15), 500);
    }

    #[test]
    fn stream_cliff_zero_means_no_cliff() {
        // cliff_rounds=0 should behave identically to the old behavior
        let stream = Stream {
            id: [0u8; 32],
            sender: Address::ZERO,
            recipient: Address::ZERO,
            rate_sats_per_round: 100,
            start_round: 10,
            deposited: 1000,
            withdrawn: 0,
            cliff_rounds: 0,
            cancelled_at_round: None,
            cancel_recipient_credited: false,
        };
        assert_eq!(stream.accrued_at(10), 0);
        assert_eq!(stream.accrued_at(15), 500);
        assert_eq!(stream.accrued_at(20), 1000);
    }

    #[test]
    fn create_stream_tx_with_cliff_signature_valid() {
        let sk = SecretKey::generate();
        let recipient = SecretKey::generate().address();
        let tx = make_create_stream_tx_with_cliff(&sk, recipient, 100, 1000, 50, 10000, 0);
        assert!(tx.verify_signature());
        assert_eq!(tx.cliff_rounds, 50);
    }

    #[test]
    fn create_stream_tx_cliff_changes_hash() {
        let sk = SecretKey::generate();
        let recipient = SecretKey::generate().address();
        let tx_no_cliff = make_create_stream_tx_with_cliff(&sk, recipient, 100, 1000, 0, 10000, 0);
        let tx_with_cliff = make_create_stream_tx_with_cliff(&sk, recipient, 100, 1000, 50, 10000, 0);
        assert_ne!(tx_no_cliff.hash(), tx_with_cliff.hash());
    }
}
