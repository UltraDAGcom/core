use thiserror::Error;

use crate::address::Address;

#[derive(Debug, Error)]
pub enum CoinError {
    #[error("insufficient balance: {address} needs {required}, has {available}")]
    InsufficientBalance {
        address: Address,
        required: u64,
        available: u64,
    },

    #[error("invalid nonce: expected {expected}, got {got}")]
    InvalidNonce { expected: u64, got: u64 },

    #[error("invalid block height: expected {expected}, got {got}")]
    InvalidHeight { expected: u64, got: u64 },

    #[error("invalid previous block hash")]
    InvalidPrevHash,

    #[error("invalid merkle root")]
    InvalidMerkleRoot,

    #[error("invalid block reward")]
    InvalidReward,

    #[error("block timestamp too far in the future")]
    TimestampTooFar,

    #[error("invalid transaction signature")]
    InvalidSignature,

    #[error("duplicate transaction")]
    DuplicateTransaction,
}
