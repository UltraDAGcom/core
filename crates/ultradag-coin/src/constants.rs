/// Total supply: 21,000,000 UDAG (stored as smallest unit = 1 sat = 0.00000001 UDAG)
pub const MAX_SUPPLY_SATS: u64 = 21_000_000 * COIN;

/// 1 UDAG = 100,000,000 satoshis
pub const COIN: u64 = 100_000_000;

/// Initial block reward: 50 UDAG
pub const INITIAL_REWARD_SATS: u64 = 50 * COIN;

/// Reward halves every 210,000 blocks
pub const HALVING_INTERVAL: u64 = 210_000;

/// Target block time: 30 seconds
pub const TARGET_BLOCK_TIME_SECS: u64 = 30;

/// Genesis block timestamp
pub const GENESIS_TIMESTAMP: i64 = 1741132800; // 2025-03-05T00:00:00Z

/// Maximum transactions per block
pub const MAX_TXS_PER_BLOCK: usize = 10_000;

/// Coinbase maturity: coinbase outputs can't be spent for N blocks
pub const COINBASE_MATURITY: u64 = 100;

/// Network identifier included in all signatures to prevent cross-network replay attacks.
/// Different for mainnet, testnet, devnet, etc.
pub const NETWORK_ID: &[u8] = b"ultradag-testnet-v1";

/// Calculate block reward for a given block height.
pub fn block_reward(height: u64) -> u64 {
    let halvings = height / HALVING_INTERVAL;
    if halvings >= 64 {
        return 0;
    }
    INITIAL_REWARD_SATS >> halvings
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn block_reward_at_height_zero() {
        assert_eq!(block_reward(0), 50 * COIN);
    }

    #[test]
    fn block_reward_first_halving() {
        assert_eq!(block_reward(HALVING_INTERVAL), 25 * COIN);
    }

    #[test]
    fn block_reward_second_halving() {
        assert_eq!(block_reward(2 * HALVING_INTERVAL), 12 * COIN + COIN / 2);
    }

    #[test]
    fn block_reward_just_before_halving() {
        assert_eq!(block_reward(HALVING_INTERVAL - 1), 50 * COIN);
    }

    #[test]
    fn block_reward_zero_after_64_halvings() {
        assert_eq!(block_reward(64 * HALVING_INTERVAL), 0);
        assert_eq!(block_reward(64 * HALVING_INTERVAL + 1), 0);
        assert_eq!(block_reward(u64::MAX), 0);
    }

    #[test]
    fn block_reward_decreases_monotonically() {
        let mut prev = block_reward(0);
        for i in 1..64 {
            let reward = block_reward(i * HALVING_INTERVAL);
            assert!(reward <= prev, "reward should decrease at halving {i}");
            prev = reward;
        }
    }

    #[test]
    fn constants_sanity() {
        assert_eq!(COIN, 100_000_000);
        assert_eq!(MAX_SUPPLY_SATS, 21_000_000 * COIN);
        assert!(MAX_TXS_PER_BLOCK > 0);
    }
}
