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

    #[error("invalid coinbase: expected {expected}, got {got}")]
    InvalidCoinbase { expected: u64, got: u64 },

    #[error("block timestamp too far in the future")]
    TimestampTooFar,

    #[error("invalid transaction signature")]
    InvalidSignature,

    #[error("duplicate transaction")]
    DuplicateTransaction,

    #[error("address is not staking")]
    NotStaking,

    #[error("already unstaking — wait for cooldown to complete")]
    AlreadyUnstaking,

    #[error("stake below minimum: need {minimum}, got {got}")]
    BelowMinStake { minimum: u64, got: u64 },

    #[error("no stake to unstake")]
    NoStakeToUnstake,

    #[error("fee too low")]
    FeeTooLow,

    #[error("insufficient stake to propose: need {}", crate::constants::MIN_STAKE_TO_PROPOSE)]
    InsufficientStakeToPropose,

    #[error("proposal title too long")]
    ProposalTitleTooLong,

    #[error("proposal description too long")]
    ProposalDescriptionTooLong,

    #[error("invalid proposal ID")]
    InvalidProposalId,

    #[error("too many active proposals")]
    TooManyActiveProposals,

    #[error("proposal not found")]
    ProposalNotFound,

    #[error("voting is closed")]
    VotingClosed,

    #[error("already voted on this proposal")]
    AlreadyVoted,

    #[error("already has an active delegation")]
    AlreadyDelegating,

    #[error("no active delegation")]
    NotDelegating,

    #[error("already undelegating — wait for cooldown to complete")]
    AlreadyUndelegating,

    #[error("validation error: {0}")]
    ValidationError(String),

    #[error("FATAL: supply invariant broken — node must halt: {0}")]
    SupplyInvariantBroken(String),
}

impl CoinError {
    /// Returns true if this error represents an unrecoverable state corruption
    /// that requires immediate node shutdown. Used by server.rs to decide
    /// whether to call process::exit(101).
    pub fn is_fatal(&self) -> bool {
        matches!(self, CoinError::SupplyInvariantBroken(_))
    }
}
