use std::collections::HashMap;

use crate::tx::transaction::Transaction;

/// Maximum transactions in mempool to prevent DoS
const MAX_MEMPOOL_SIZE: usize = 10_000;

/// In-memory transaction pool (mempool).
#[derive(Clone)]
pub struct Mempool {
    txs: HashMap<[u8; 32], Transaction>,
}

impl Mempool {
    pub fn new() -> Self {
        Self {
            txs: HashMap::new(),
        }
    }

    /// Add a transaction. Returns true if it was new.
    /// If mempool is full, evicts lowest-fee transaction if new tx has higher fee.
    pub fn insert(&mut self, tx: Transaction) -> bool {
        let hash = tx.hash();
        if self.txs.contains_key(&hash) {
            return false;
        }

        // Reject transactions with fee below minimum (spam prevention)
        if tx.fee() < crate::constants::MIN_FEE_SATS {
            return false;
        }

        // If mempool is full, try to evict lowest-fee transaction
        if self.txs.len() >= MAX_MEMPOOL_SIZE {
            // Find lowest-fee transaction (stake/unstake have 0 fee)
            if let Some((lowest_hash, lowest_fee)) = self.txs.iter()
                .map(|(h, t)| {
                    let fee = match t {
                        Transaction::Transfer(tx) => tx.fee,
                        Transaction::CreateProposal(tx) => tx.fee,
                        Transaction::Vote(tx) => tx.fee,
                        Transaction::Stake(_) | Transaction::Unstake(_) => 0,
                    };
                    (*h, fee)
                })
                .min_by_key(|(_, fee)| *fee)
            {
                let new_fee = match &tx {
                    Transaction::Transfer(tx) => tx.fee,
                    Transaction::CreateProposal(tx) => tx.fee,
                    Transaction::Vote(tx) => tx.fee,
                    Transaction::Stake(_) | Transaction::Unstake(_) => 0,
                };
                // Only evict if new transaction has higher fee
                if new_fee > lowest_fee {
                    self.txs.remove(&lowest_hash);
                } else {
                    // Mempool full and new tx has lower/equal fee - reject
                    return false;
                }
            }
        }

        self.txs.insert(hash, tx);
        true
    }

    /// Remove a transaction by hash (after it's been included in a block).
    pub fn remove(&mut self, hash: &[u8; 32]) -> Option<Transaction> {
        self.txs.remove(hash)
    }

    /// Get the best transactions for a block (sorted by fee descending).
    /// Transfers sorted by fee, stake/unstake transactions have priority 0.
    pub fn best(&self, max: usize) -> Vec<Transaction> {
        let mut txs: Vec<&Transaction> = self.txs.values().collect();
        txs.sort_by(|a, b| {
            let fee_a = match a {
                Transaction::Transfer(tx) => tx.fee,
                Transaction::CreateProposal(tx) => tx.fee,
                Transaction::Vote(tx) => tx.fee,
                Transaction::Stake(_) | Transaction::Unstake(_) => 0,
            };
            let fee_b = match b {
                Transaction::Transfer(tx) => tx.fee,
                Transaction::CreateProposal(tx) => tx.fee,
                Transaction::Vote(tx) => tx.fee,
                Transaction::Stake(_) | Transaction::Unstake(_) => 0,
            };
            fee_b.cmp(&fee_a)
        });
        txs.into_iter().take(max).cloned().collect()
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
    }

    /// Check if a transaction is in the pool.
    pub fn contains(&self, hash: &[u8; 32]) -> bool {
        self.txs.contains_key(hash)
    }

    /// Count pending transactions from a specific sender address.
    pub fn pending_count(&self, from: &crate::Address) -> u64 {
        self.txs.values().filter(|tx| &tx.from() == from).count() as u64
    }

    /// Get the highest nonce for a sender in the mempool, if any.
    pub fn pending_nonce(&self, from: &crate::Address) -> Option<u64> {
        self.txs.values()
            .filter(|tx| &tx.from() == from)
            .map(|tx| tx.nonce())
            .max()
    }

    /// Save mempool to disk
    pub fn save(&self, path: &std::path::Path) -> Result<(), crate::persistence::PersistenceError> {
        let snapshot = crate::tx::persistence::MempoolSnapshot {
            transactions: self.txs.values().cloned().collect(),
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
}
