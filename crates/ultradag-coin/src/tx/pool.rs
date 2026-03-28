use std::collections::HashMap;
use std::time::Instant;

use crate::address::Address;
use crate::tx::transaction::Transaction;

/// Maximum transactions in mempool to prevent DoS
pub const MAX_MEMPOOL_SIZE: usize = 10_000;

/// Maximum transactions per sender address in the mempool.
/// Prevents a single address from monopolizing mempool capacity.
const MAX_TXS_PER_SENDER: usize = 100;

/// Mempool entry: transaction + insertion timestamp for TTL eviction.
#[derive(Clone)]
struct MempoolEntry {
    tx: Transaction,
    inserted_at: Instant,
}

/// In-memory transaction pool (mempool).
#[derive(Clone)]
pub struct Mempool {
    txs: HashMap<[u8; 32], MempoolEntry>,
    /// Secondary index: sender address → transaction hashes for O(1) sender lookups.
    by_sender: HashMap<Address, Vec<[u8; 32]>>,
}

impl Mempool {
    pub fn new() -> Self {
        Self {
            txs: HashMap::new(),
            by_sender: HashMap::new(),
        }
    }

    /// Add a transaction with a specific rejection reason on failure.
    /// If mempool is full, evicts lowest-fee transaction if new tx has higher fee.
    pub fn insert_with_reason(&mut self, tx: Transaction) -> Result<(), &'static str> {
        let hash = tx.hash();
        if self.txs.contains_key(&hash) {
            return Err("duplicate transaction");
        }

        // Reject transactions with fee below minimum (spam prevention).
        // Fee-exempt transactions (fee=0 by design or allowed for free names).
        let fee_exempt = matches!(
            tx,
            Transaction::Stake(_)
            | Transaction::Unstake(_)
            | Transaction::Delegate(_)
            | Transaction::Undelegate(_)
            | Transaction::SetCommission(_)
            | Transaction::BridgeRelease(_)
            | Transaction::AddKey(_) // Account setup — fee-exempt
            | Transaction::RemoveKey(_)
            | Transaction::RecoverAccount(_)
            | Transaction::CancelRecovery(_)
            | Transaction::ExecuteVault(_)
            | Transaction::CancelVault(_)
            | Transaction::RegisterName(_) // Standard names (6+ chars) are free
            | Transaction::SmartOp(_) // Fee-exempt ops checked at apply time
        );
        if !fee_exempt && tx.fee() < crate::constants::MIN_FEE_SATS {
            return Err("fee below minimum");
        }

        // Per-sender limit: prevent one address from filling the entire mempool
        let sender = tx.from();
        let sender_count = self.by_sender.get(&sender).map_or(0, |v| v.len());
        if sender_count >= MAX_TXS_PER_SENDER {
            return Err("per-sender limit reached");
        }

        // If mempool is full, try to evict lowest-fee transaction
        if self.txs.len() >= MAX_MEMPOOL_SIZE {
            // Find lowest-fee transaction (stake/unstake have 0 fee)
            if let Some((lowest_hash, lowest_fee)) = self.txs.iter()
                .map(|(h, entry)| (*h, entry.tx.fee()))
                .min_by_key(|(_, fee)| *fee)
            {
                let new_fee = tx.fee();
                let can_evict = new_fee > lowest_fee || (fee_exempt && lowest_fee == 0);
                if can_evict {
                    if let Some(evicted) = self.txs.remove(&lowest_hash) {
                        let evicted_sender = evicted.tx.from();
                        if let Some(hashes) = self.by_sender.get_mut(&evicted_sender) {
                            hashes.retain(|h| h != &lowest_hash);
                            if hashes.is_empty() {
                                self.by_sender.remove(&evicted_sender);
                            }
                        }
                    }
                } else {
                    return Err("mempool full");
                }
            }
        }

        self.by_sender.entry(sender).or_default().push(hash);
        self.txs.insert(hash, MempoolEntry { tx, inserted_at: Instant::now() });
        Ok(())
    }

    /// Add a transaction. Returns true if it was new.
    /// If mempool is full, evicts lowest-fee transaction if new tx has higher fee.
    /// For specific rejection reasons, use `insert_with_reason()` instead.
    pub fn insert(&mut self, tx: Transaction) -> bool {
        self.insert_with_reason(tx).is_ok()
    }

    /// Remove a transaction by hash (after it's been included in a block).
    pub fn remove(&mut self, hash: &[u8; 32]) -> Option<Transaction> {
        if let Some(entry) = self.txs.remove(hash) {
            let sender = entry.tx.from();
            if let Some(hashes) = self.by_sender.get_mut(&sender) {
                hashes.retain(|h| h != hash);
                if hashes.is_empty() {
                    self.by_sender.remove(&sender);
                }
            }
            Some(entry.tx)
        } else {
            None
        }
    }

    /// Evict transactions older than the configured TTL.
    /// Returns the number of transactions evicted.
    pub fn evict_expired(&mut self) -> usize {
        let ttl = std::time::Duration::from_secs(crate::constants::MEMPOOL_TX_TTL_SECS);
        let now = Instant::now();
        let expired: Vec<[u8; 32]> = self.txs.iter()
            .filter(|(_, entry)| now.duration_since(entry.inserted_at) >= ttl)
            .map(|(h, _)| *h)
            .collect();
        let count = expired.len();
        for hash in expired {
            self.remove(&hash);
        }
        count
    }

    /// Get the best transactions for a block (sorted by fee descending).
    /// Transfers sorted by fee, stake/unstake transactions have priority 0.
    pub fn best(&self, max: usize) -> Vec<Transaction> {
        let mut txs: Vec<&Transaction> = self.txs.values().map(|e| &e.tx).collect();
        txs.sort_by_key(|tx| std::cmp::Reverse(tx.fee()));
        txs.into_iter().take(max).cloned().collect()
    }

    /// Calculate total cost of pending transactions from a specific sender.
    /// Uses the by_sender index for O(K) where K = sender's pending tx count,
    /// instead of O(N) scanning the entire mempool.
    pub fn pending_cost_for(&self, sender: &Address) -> u64 {
        match self.by_sender.get(sender) {
            Some(hashes) => hashes.iter()
                .filter_map(|h| self.txs.get(h))
                .map(|e| e.tx.total_cost())
                .fold(0u64, |acc, x| acc.saturating_add(x)),
            None => 0,
        }
    }

    /// Get the next nonce for a sender (max pending nonce + 1, or 0 if no pending txs).
    pub fn pending_nonce_for(&self, sender: &Address) -> Option<u64> {
        match self.by_sender.get(sender) {
            Some(hashes) => hashes.iter()
                .filter_map(|h| self.txs.get(h))
                .map(|e| e.tx.nonce())
                .max()
                .map(|n| n.saturating_add(1)),
            None => None,
        }
    }

    pub fn len(&self) -> usize {
        self.txs.len()
    }

    pub fn is_empty(&self) -> bool {
        self.txs.is_empty()
    }

    /// Remove all transactions from the mempool.
    pub fn clear(&mut self) {
        self.txs.clear();
        self.by_sender.clear();
    }

    /// Check if a transaction is in the pool.
    pub fn contains(&self, hash: &[u8; 32]) -> bool {
        self.txs.contains_key(hash)
    }

    /// Get a transaction by hash (for status lookups).
    pub fn get(&self, hash: &[u8; 32]) -> Option<&Transaction> {
        self.txs.get(hash).map(|e| &e.tx)
    }

    /// Count pending transactions from a specific sender address.
    pub fn pending_count(&self, from: &crate::Address) -> u64 {
        self.by_sender.get(from).map_or(0, |v| v.len()) as u64
    }

    /// Get the highest nonce for a sender in the mempool, if any.
    pub fn pending_nonce(&self, from: &crate::Address) -> Option<u64> {
        self.by_sender.get(from).and_then(|hashes| {
            hashes.iter()
                .filter_map(|h| self.txs.get(h))
                .map(|entry| entry.tx.nonce())
                .max()
        })
    }

    /// Get the highest pending proposal ID in the mempool, if any.
    /// Used to avoid TOCTOU races when two concurrent RPC requests both read the same
    /// next_proposal_id from state.
    pub fn max_pending_proposal_id(&self) -> Option<u64> {
        self.txs.values()
            .filter_map(|e| {
                if let Transaction::CreateProposal(tx) = &e.tx {
                    Some(tx.proposal_id)
                } else {
                    None
                }
            })
            .max()
    }

    /// Save mempool to disk
    pub fn save(&self, path: &std::path::Path) -> Result<(), crate::persistence::PersistenceError> {
        let snapshot = crate::tx::persistence::MempoolSnapshot {
            transactions: self.txs.values().map(|e| e.tx.clone()).collect(),
        };
        snapshot.save(path)
    }

    /// Load mempool from disk
    pub fn load(path: &std::path::Path) -> Result<Self, crate::persistence::PersistenceError> {
        let snapshot = crate::tx::persistence::MempoolSnapshot::load(path)?;
        let mut mempool = Self::new();
        for tx in snapshot.transactions {
            mempool.insert(tx);
        }
        Ok(mempool)
    }

    /// Check if saved state exists
    pub fn exists(path: &std::path::Path) -> bool {
        crate::tx::persistence::MempoolSnapshot::exists(path)
    }
}

impl Default for Mempool {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::address::{Address, SecretKey, Signature};
    use crate::tx::TransferTx;

    fn make_tx(fee: u64, nonce: u64) -> Transaction {
        let sk = SecretKey::generate();
        let mut transfer = TransferTx {
            from: sk.address(),
            to: Address::ZERO,
            amount: 100,
            fee: crate::constants::MIN_FEE_SATS + fee, // Add to MIN_FEE_SATS to ensure valid fees
            nonce,
            pub_key: sk.verifying_key().to_bytes(),
            signature: Signature([0u8; 64]),
            memo: None,
        };
        transfer.signature = sk.sign(&transfer.signable_bytes());
        Transaction::Transfer(transfer)
    }

    #[test]
    fn new_pool_is_empty() {
        let pool = Mempool::new();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn insert_new_returns_true() {
        let mut pool = Mempool::new();
        let tx = make_tx(10, 0);
        assert!(pool.insert(tx));
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn insert_duplicate_returns_false() {
        let mut pool = Mempool::new();
        let tx = make_tx(10, 0);
        let tx_clone = tx.clone();
        assert!(pool.insert(tx));
        assert!(!pool.insert(tx_clone));
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn remove_existing_returns_some() {
        let mut pool = Mempool::new();
        let tx = make_tx(10, 0);
        let hash = tx.hash();
        pool.insert(tx);
        assert!(pool.remove(&hash).is_some());
        assert!(pool.is_empty());
    }

    #[test]
    fn remove_nonexistent_returns_none() {
        let mut pool = Mempool::new();
        let bogus = [0u8; 32];
        assert!(pool.remove(&bogus).is_none());
    }

    #[test]
    fn contains_works() {
        let mut pool = Mempool::new();
        let tx = make_tx(10, 0);
        let hash = tx.hash();
        assert!(!pool.contains(&hash));
        pool.insert(tx);
        assert!(pool.contains(&hash));
    }

    #[test]
    fn best_returns_sorted_by_fee_descending() {
        use crate::constants::MIN_FEE_SATS;
        let mut pool = Mempool::new();
        pool.insert(make_tx(5, 0));
        pool.insert(make_tx(20, 1));
        pool.insert(make_tx(10, 2));

        let best = pool.best(10);
        assert_eq!(best.len(), 3);
        // Extract fees from Transaction enum
        let fee0 = if let Transaction::Transfer(ref t) = best[0] { t.fee } else { 0 };
        let fee1 = if let Transaction::Transfer(ref t) = best[1] { t.fee } else { 0 };
        let fee2 = if let Transaction::Transfer(ref t) = best[2] { t.fee } else { 0 };
        assert_eq!(fee0, MIN_FEE_SATS + 20);
        assert_eq!(fee1, MIN_FEE_SATS + 10);
        assert_eq!(fee2, MIN_FEE_SATS + 5);
    }

    #[test]
    fn best_respects_max_limit() {
        let mut pool = Mempool::new();
        for i in 0..10 {
            pool.insert(make_tx(i, i));
        }
        let best = pool.best(3);
        assert_eq!(best.len(), 3);
    }

    #[test]
    fn default_is_empty() {
        let pool = Mempool::default();
        assert!(pool.is_empty());
    }

    #[test]
    fn mempool_size_limit_enforced() {
        let mut pool = Mempool::new();
        
        // Fill mempool to capacity with fee=10 transactions
        for i in 0..MAX_MEMPOOL_SIZE {
            let tx = make_tx(10, i as u64);
            assert!(pool.insert(tx), "Should accept tx {} when below limit", i);
        }
        assert_eq!(pool.len(), MAX_MEMPOOL_SIZE);

        // Try to insert transaction with lower fee - should be rejected
        let low_fee_tx = make_tx(5, MAX_MEMPOOL_SIZE as u64);
        assert!(!pool.insert(low_fee_tx), "Should reject low-fee tx when mempool is full");
        assert_eq!(pool.len(), MAX_MEMPOOL_SIZE, "Size should remain at limit");

        // Insert transaction with higher fee - should evict lowest-fee tx
        let high_fee_tx = make_tx(20, (MAX_MEMPOOL_SIZE + 1) as u64);
        assert!(pool.insert(high_fee_tx.clone()), "Should accept high-fee tx and evict lowest");
        assert_eq!(pool.len(), MAX_MEMPOOL_SIZE, "Size should remain at limit");
        assert!(pool.contains(&high_fee_tx.hash()), "High-fee tx should be in pool");
    }

    #[test]
    fn fee_exempt_tx_can_enter_full_mempool() {
        use crate::tx::StakeTx;

        let mut pool = Mempool::new();

        // Fill mempool with zero-fee stake transactions
        for i in 0..MAX_MEMPOOL_SIZE {
            let sk = SecretKey::from_bytes([(i % 250 + 1) as u8; 32]);
            let mut stake_tx = StakeTx {
                from: sk.address(),
                amount: 100_000_000,
                nonce: (i / 250) as u64,
                pub_key: sk.verifying_key().to_bytes(),
                signature: Signature([0u8; 64]),
            };
            stake_tx.signature = sk.sign(&stake_tx.signable_bytes());
            pool.insert(Transaction::Stake(stake_tx));
        }
        assert_eq!(pool.len(), MAX_MEMPOOL_SIZE);

        // A new fee-exempt tx should be able to evict one of the zero-fee txs
        let new_sk = SecretKey::from_bytes([0xFE; 32]);
        let mut new_stake = StakeTx {
            from: new_sk.address(),
            amount: 100_000_000,
            nonce: 0,
            pub_key: new_sk.verifying_key().to_bytes(),
            signature: Signature([0u8; 64]),
        };
        new_stake.signature = new_sk.sign(&new_stake.signable_bytes());
        assert!(
            pool.insert(Transaction::Stake(new_stake)),
            "Fee-exempt tx must be able to enter a full mempool by evicting another zero-fee tx"
        );
        assert_eq!(pool.len(), MAX_MEMPOOL_SIZE);
    }
}
