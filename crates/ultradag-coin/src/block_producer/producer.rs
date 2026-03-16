use crate::address::Address;
use crate::block::{Block, BlockHeader, merkle_root};
use crate::constants;
use crate::tx::{CoinbaseTx, Mempool, Transaction};

/// Create a new block for inclusion in a DAG vertex.
/// In DAG-BFT, blocks are produced directly by validators — no PoW needed.
/// Coinbase contains transaction fees only. Block rewards are distributed
/// by the protocol via `distribute_round_rewards()` at round boundaries.
pub fn create_block(
    prev_hash: [u8; 32],
    height: u64,
    validator_address: &Address,
    mempool: &Mempool,
) -> Block {
    let mut txs: Vec<Transaction> = mempool.best(constants::MAX_TXS_PER_BLOCK);
    // Sort by (sender, nonce) to ensure valid execution order for sequential nonces
    txs.sort_by(|a, b| a.from().0.cmp(&b.from().0).then(a.nonce().cmp(&b.nonce())));
    let total_fees: u64 = txs.iter()
        .map(|tx| tx.fee())
        .fold(0u64, |acc, f| acc.saturating_add(f));

    let coinbase = CoinbaseTx {
        to: *validator_address,
        amount: total_fees,
        height,
    };

    // Build merkle root (uses shared implementation from block module)
    let mut leaves: Vec<[u8; 32]> = vec![coinbase.hash()];
    for tx in &txs {
        leaves.push(tx.hash());
    }
    let root = merkle_root(&leaves);

    let timestamp = chrono::Utc::now().timestamp();

    let header = BlockHeader {
        version: 1,
        height,
        timestamp,
        prev_hash,
        merkle_root: root,
    };

    Block {
        header,
        coinbase,
        transactions: txs,
    }
}
