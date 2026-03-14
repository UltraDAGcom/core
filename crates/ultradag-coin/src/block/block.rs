use serde::{Deserialize, Serialize};

use crate::block::header::BlockHeader;
use crate::tx::{CoinbaseTx, Transaction};

/// A complete block: header + coinbase + transactions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Block {
    pub header: BlockHeader,
    pub coinbase: CoinbaseTx,
    pub transactions: Vec<Transaction>,
}

impl Block {
    pub fn hash(&self) -> [u8; 32] {
        self.header.hash()
    }

    pub fn height(&self) -> u64 {
        self.header.height
    }

    /// Compute the Merkle root of all transactions in this block.
    pub fn compute_merkle_root(&self) -> [u8; 32] {
        let mut leaves: Vec<[u8; 32]> = Vec::new();
        leaves.push(self.coinbase.hash());
        for tx in &self.transactions {
            leaves.push(tx.hash());
        }
        merkle_root(&leaves)
    }

    /// Total fees collected in this block.
    pub fn total_fees(&self) -> u64 {
        self.transactions.iter().map(|tx| {
            match tx {
                Transaction::Transfer(t) => t.fee,
                Transaction::CreateProposal(t) => t.fee,
                Transaction::Vote(t) => t.fee,
                Transaction::Stake(_) | Transaction::Unstake(_) => 0,
            }
        }).fold(0u64, |acc, f| acc.saturating_add(f))
    }
}

pub fn merkle_root(leaves: &[[u8; 32]]) -> [u8; 32] {
    if leaves.is_empty() {
        return [0u8; 32];
    }
    if leaves.len() == 1 {
        return leaves[0];
    }

    let mut level = leaves.to_vec();
    while level.len() > 1 {
        if !level.len().is_multiple_of(2) {
            // SAFETY: level.len() > 1 guarantees last() exists
            let last = level.last().expect("level has at least 1 element");
            level.push(*last);
        }
        level = level
            .chunks(2)
            .map(|pair| {
                let mut hasher = blake3::Hasher::new();
                hasher.update(&pair[0]);
                hasher.update(&pair[1]);
                *hasher.finalize().as_bytes()
            })
            .collect();
    }
    level[0]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::address::{Address, Signature};
    use crate::tx::{CoinbaseTx, Transaction};

    fn make_block(txs: Vec<Transaction>) -> Block {
        let coinbase = CoinbaseTx {
            to: Address::ZERO,
            amount: 5_000_000_000,
            height: 0,
        };
        let mut block = Block {
            header: BlockHeader {
                version: 1,
                height: 0,
                timestamp: 1_000_000,
                prev_hash: [0u8; 32],
                merkle_root: [0u8; 32],
            },
            coinbase,
            transactions: txs,
        };
        block.header.merkle_root = block.compute_merkle_root();
        block
    }

    fn make_tx(amount: u64, fee: u64) -> Transaction {
        let sk = crate::address::SecretKey::generate();
        let mut transfer = crate::tx::TransferTx {
            from: sk.address(),
            to: Address::ZERO,
            amount,
            fee,
            nonce: 0,
            pub_key: sk.verifying_key().to_bytes(),
            signature: Signature([0u8; 64]),
            memo: None,
        };
        transfer.signature = sk.sign(&transfer.signable_bytes());
        Transaction::Transfer(transfer)
    }

    #[test]
    fn compute_merkle_root_is_deterministic() {
        let block = make_block(vec![]);
        let root1 = block.compute_merkle_root();
        let root2 = block.compute_merkle_root();
        assert_eq!(root1, root2);
    }

    #[test]
    fn compute_merkle_root_changes_with_transactions() {
        let block_empty = make_block(vec![]);
        let block_with_tx = make_block(vec![make_tx(100, 10)]);
        assert_ne!(
            block_empty.compute_merkle_root(),
            block_with_tx.compute_merkle_root()
        );
    }

    #[test]
    fn total_fees_empty_block() {
        let block = make_block(vec![]);
        assert_eq!(block.total_fees(), 0);
    }

    #[test]
    fn total_fees_sums_correctly() {
        let block = make_block(vec![make_tx(100, 5), make_tx(200, 15), make_tx(50, 3)]);
        assert_eq!(block.total_fees(), 5 + 15 + 3);
    }

    #[test]
    fn block_hash_delegates_to_header() {
        let block = make_block(vec![]);
        assert_eq!(block.hash(), block.header.hash());
    }

    #[test]
    fn block_height_delegates_to_header() {
        let block = make_block(vec![]);
        assert_eq!(block.height(), block.header.height);
    }
}
