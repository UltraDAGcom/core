use crate::address::Address;
use crate::block::{Block, BlockHeader, merkle_root};
use crate::constants::{self, MAX_VERTEX_BYTES};
use crate::tx::{CoinbaseTx, Mempool, Transaction};

/// Overhead budget reserved for the DagVertex wrapper around the block.
/// Accounts for: parent_hashes (up to 64 × 32 bytes), round (u64), validator
/// address (32 bytes), pub_key (32 bytes), Ed25519 signature (64 bytes),
/// block header fields, coinbase, and postcard encoding overhead.
/// 4 KB is conservative — typical overhead is ~2-3 KB even with 64 parents.
const VERTEX_OVERHEAD_BUDGET: usize = 4096;

/// Maximum serialized block size that leaves room for the DagVertex wrapper.
const MAX_BLOCK_BYTES: usize = MAX_VERTEX_BYTES - VERTEX_OVERHEAD_BUDGET;

/// Create a new block for inclusion in a DAG vertex.
/// In DAG-BFT, blocks are produced directly by validators — no PoW needed.
/// Coinbase contains transaction fees only. Block rewards are distributed
/// by the protocol via `distribute_round_rewards()` at round boundaries.
///
/// Coinbase amount is always 0. Transaction fees are credited to the proposer
/// via the deferred coinbase mechanism in `apply_vertex_with_validators()`,
/// which credits only fees from successfully-applied transactions. This
/// eliminates the inflation vector where a malicious proposer includes
/// transactions they know will fail to inflate declared fees.
///
/// **Byte budget enforcement:** After selecting and sorting transactions, the
/// block is serialized to check its size. If it exceeds `MAX_VERTEX_BYTES -
/// VERTEX_OVERHEAD_BUDGET` (4 KB reserved for the DagVertex wrapper: parents,
/// round, validator, pubkey, signature), transactions are trimmed from the end
/// (lowest priority after sort) until the block fits. This prevents the
/// validator loop from producing vertices that exceed `MAX_VERTEX_BYTES` and
/// get rejected by `try_insert()`.
pub fn create_block(
    prev_hash: [u8; 32],
    height: u64,
    validator_address: &Address,
    mempool: &Mempool,
) -> Block {
    let mut txs: Vec<Transaction> = mempool.best(constants::MAX_TXS_PER_BLOCK);
    // Sort by (sender, nonce) to ensure valid execution order for sequential nonces
    txs.sort_by(|a, b| a.from().0.cmp(&b.from().0).then(a.nonce().cmp(&b.nonce())));

    let timestamp = chrono::Utc::now().timestamp();

    // Build a candidate block and enforce byte budget. If the serialized block
    // exceeds MAX_BLOCK_BYTES, trim transactions from the end (lowest priority
    // after the sender/nonce sort) until it fits.
    loop {
        // Coinbase amount is always 0. Fees are credited via deferred mechanism.
        let coinbase = CoinbaseTx {
            to: *validator_address,
            amount: 0,
            height,
        };

        let mut leaves: Vec<[u8; 32]> = vec![coinbase.hash()];
        for tx in &txs {
            leaves.push(tx.hash());
        }
        let root = merkle_root(&leaves);

        let header = BlockHeader {
            version: 1,
            height,
            timestamp,
            prev_hash,
            merkle_root: root,
        };

        let block = Block {
            header,
            coinbase,
            transactions: txs.clone(),
        };

        // Check serialized size against byte budget
        match postcard::to_allocvec(&block) {
            Ok(bytes) if bytes.len() <= MAX_BLOCK_BYTES => return block,
            Ok(_) if txs.is_empty() => return block, // Can't trim further
            Ok(bytes) => {
                // Estimate how many transactions to remove. Each trim iteration
                // is expensive (re-serialize), so remove proportionally rather
                // than one-at-a-time. Remove at least 1 to guarantee progress.
                let excess = bytes.len() - MAX_BLOCK_BYTES;
                let avg_tx_size = bytes.len() / (txs.len() + 1); // +1 for header/coinbase
                let trim_count = (excess / avg_tx_size.max(1)).max(1);
                let new_len = txs.len().saturating_sub(trim_count);
                txs.truncate(new_len);
            }
            // Serialization failure — return block as-is (should never happen
            // with valid types, and try_insert will catch oversized vertices).
            Err(_) => return block,
        }
    }
}
