use crate::address::Address;
use crate::block::block::Block;
use crate::block::header::BlockHeader;
use crate::constants::{self, GENESIS_TIMESTAMP};
use crate::tx::CoinbaseTx;

/// Create the genesis block.
pub fn genesis_block() -> Block {
    let coinbase = CoinbaseTx {
        to: Address::ZERO,
        amount: constants::block_reward(0),
        height: 0,
    };

    let merkle_root = coinbase.hash();

    let header = BlockHeader {
        version: 1,
        height: 0,
        timestamp: GENESIS_TIMESTAMP,
        prev_hash: [0u8; 32],
        merkle_root,
    };

    Block {
        header,
        coinbase,
        transactions: vec![],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn genesis_height_is_zero() {
        let gen = genesis_block();
        assert_eq!(gen.height(), 0);
    }

    #[test]
    fn genesis_prev_hash_is_all_zeros() {
        let gen = genesis_block();
        assert_eq!(gen.header.prev_hash, [0u8; 32]);
    }

    #[test]
    fn genesis_coinbase_amount_equals_block_reward_zero() {
        let gen = genesis_block();
        assert_eq!(gen.coinbase.amount, constants::block_reward(0));
    }

    #[test]
    fn genesis_has_no_transactions() {
        let gen = genesis_block();
        assert!(gen.transactions.is_empty());
    }

    #[test]
    fn genesis_coinbase_goes_to_address_zero() {
        let gen = genesis_block();
        assert_eq!(gen.coinbase.to, Address::ZERO);
    }

    #[test]
    fn genesis_merkle_root_matches() {
        let gen = genesis_block();
        assert_eq!(gen.header.merkle_root, gen.compute_merkle_root());
    }

    #[test]
    fn genesis_timestamp_is_expected() {
        let gen = genesis_block();
        assert_eq!(gen.header.timestamp, GENESIS_TIMESTAMP);
    }

    #[test]
    fn genesis_is_deterministic() {
        let g1 = genesis_block();
        let g2 = genesis_block();
        assert_eq!(g1.hash(), g2.hash());
    }
}
