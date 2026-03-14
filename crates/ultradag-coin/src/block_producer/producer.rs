use crate::address::Address;
use crate::block::{Block, BlockHeader};
use crate::constants;
use crate::tx::{CoinbaseTx, Mempool, Transaction};

/// Create a new block on top of the given chain tip.
/// In DAG-BFT, blocks are produced directly by validators — no PoW needed.
/// `validator_reward` is the pre-computed per-validator reward for this round
/// (total_round_reward / n in equal-split mode, or proportional to stake).
pub fn create_block(
    prev_hash: [u8; 32],
    height: u64,
    validator_address: &Address,
    mempool: &Mempool,
    validator_reward: u64,
) -> Block {
    let mut txs: Vec<Transaction> = mempool.best(constants::MAX_TXS_PER_BLOCK);
    // Sort by (sender, nonce) to ensure valid execution order for sequential nonces
    txs.sort_by(|a, b| a.from().0.cmp(&b.from().0).then(a.nonce().cmp(&b.nonce())));
    let total_fees: u64 = txs.iter().map(|tx| {
        match tx {
            Transaction::Transfer(t) => t.fee,
            Transaction::CreateProposal(t) => t.fee,
            Transaction::Vote(t) => t.fee,
            Transaction::Stake(_) | Transaction::Unstake(_) => 0,
        }
    }).fold(0u64, |acc, f| acc.saturating_add(f));

    let coinbase = CoinbaseTx {
        to: *validator_address,
        amount: validator_reward.saturating_add(total_fees),
        height,
    };

    // Build merkle root
    let mut leaves: Vec<[u8; 32]> = vec![coinbase.hash()];
    for tx in &txs {
        leaves.push(tx.hash());
    }
    let merkle_root = compute_merkle(&leaves);

    let timestamp = chrono::Utc::now().timestamp();

    let header = BlockHeader {
        version: 1,
        height,
        timestamp,
        prev_hash,
        merkle_root,
    };

    Block {
        header,
        coinbase,
        transactions: txs,
    }
}

fn compute_merkle(leaves: &[[u8; 32]]) -> [u8; 32] {
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
                let mut h = blake3::Hasher::new();
                h.update(&pair[0]);
                h.update(&pair[1]);
                *h.finalize().as_bytes()
            })
            .collect();
    }
    level[0]
}
