use thiserror::Error;

use crate::address::Address;

/// Core error type for all UltraDAG protocol operations.
///
/// This enum covers all error conditions that can occur during:
/// - Transaction validation and execution
/// - Block/vertex processing
/// - State transitions
/// - Governance operations
/// - Staking and delegation
///
/// # Error Categories
///
/// - **Validation Errors**: Invalid input (nonce, signature, amount)
/// - **Balance Errors**: Insufficient funds for operation
/// - **Protocol Errors**: Invariant violations requiring node halt
/// - **Governance Errors**: Proposal/voting rule violations
#[derive(Debug, Error)]
pub enum CoinError {
    /// Account has insufficient balance for the requested operation.
    ///
    /// # Fields
    /// - `address`: The account that lacks sufficient funds
    /// - `required`: Amount needed (in sats)
    /// - `available`: Current balance (in sats)
    ///
    /// # Resolution
    /// Fund the account or reduce the transaction amount.
    #[error("insufficient balance: {address} needs {required} sats, has {available} sats")]
    InsufficientBalance {
        address: Address,
        required: u64,
        available: u64,
    },

    /// Transaction nonce does not match the expected value.
    ///
    /// # Fields
    /// - `expected`: The nonce value the account is at
    /// - `got`: The nonce value provided in the transaction
    ///
    /// # Resolution
    /// Resubmit the transaction with the correct nonce. Nonces must be sequential.
    #[error("invalid nonce: expected {expected}, got {got}")]
    InvalidNonce { expected: u64, got: u64 },

    /// Block height does not match the expected chain height.
    ///
    /// # Fields
    /// - `expected`: The height the block should have
    /// - `got`: The height the block claims
    #[error("invalid block height: expected {expected}, got {got}")]
    InvalidHeight { expected: u64, got: u64 },

    /// Block references an invalid or unknown previous block hash.
    #[error("invalid previous block hash")]
    InvalidPrevHash,

    /// Block merkle root does not match the computed root from transactions.
    #[error("invalid merkle root")]
    InvalidMerkleRoot,

    /// Block reward amount does not match the expected emission schedule.
    #[error("invalid block reward")]
    InvalidReward,

    /// Coinbase transaction amount is incorrect.
    ///
    /// # Fields
    /// - `expected`: The correct coinbase amount (in sats)
    /// - `got`: The amount the coinbase claims
    #[error("invalid coinbase: expected {expected} sats, got {got} sats")]
    InvalidCoinbase { expected: u64, got: u64 },

    /// Block timestamp is too far in the future (>5 minutes ahead of local time).
    ///
    /// # Resolution
    /// Ensure node clock is synchronized. Reject blocks from future timestamps.
    #[error("block timestamp too far in the future (>5 minutes)")]
    TimestampTooFar,

    /// Cryptographic signature verification failed.
    ///
    /// # Causes
    /// - Tampered transaction data
    /// - Wrong signing key
    /// - Corrupted signature bytes
    #[error("invalid transaction signature")]
    InvalidSignature,

    /// Transaction hash already exists in the mempool or chain.
    ///
    /// # Resolution
    /// This is a duplicate submission. The original transaction is already being processed.
    #[error("duplicate transaction")]
    DuplicateTransaction,

    /// Account has no active stake.
    ///
    /// # Resolution
    /// Submit a StakeTx before attempting to unstake or vote.
    #[error("address is not staking")]
    NotStaking,

    /// Account is already in unstaking cooldown period.
    ///
    /// # Resolution
    /// Wait for UNSTAKE_COOLDOWN_ROUNDS (2,016 rounds ≈ 2.8 hours) to complete.
    #[error("already unstaking — wait for cooldown to complete")]
    AlreadyUnstaking,

    /// Stake amount is below the minimum threshold.
    ///
    /// # Fields
    /// - `minimum`: Required minimum stake (in sats)
    /// - `got`: Amount provided
    #[error("stake below minimum: need {minimum} sats, got {got} sats")]
    BelowMinStake { minimum: u64, got: u64 },

    /// No stake available to unstake.
    ///
    /// # Resolution
    /// Submit a StakeTx before attempting to unstake.
    #[error("no stake to unstake")]
    NoStakeToUnstake,

    /// Transaction fee is below the minimum required.
    ///
    /// # Resolution
    /// Increase fee to at least MIN_FEE_SATS (10,000 sats = 0.0001 UDAG).
    #[error("fee too low (minimum: 10,000 sats)")]
    FeeTooLow,

    /// Insufficient stake to submit a governance proposal.
    ///
    /// # Resolution
    /// Stake at least MIN_STAKE_TO_PROPOSE (10,000 UDAG) before proposing.
    #[error("insufficient stake to propose: need {} sats", crate::constants::MIN_STAKE_TO_PROPOSE)]
    InsufficientStakeToPropose,

    /// Proposal title exceeds the maximum allowed length.
    ///
    /// # Limit
    /// Maximum: PROPOSAL_TITLE_MAX_BYTES (128 bytes)
    #[error("proposal title too long (max 128 bytes)")]
    ProposalTitleTooLong,

    /// Proposal description exceeds the maximum allowed length.
    ///
    /// # Limit
    /// Maximum: PROPOSAL_DESCRIPTION_MAX_BYTES (4096 bytes)
    #[error("proposal description too long (max 4096 bytes)")]
    ProposalDescriptionTooLong,

    /// Proposal ID is invalid or out of range.
    #[error("invalid proposal ID")]
    InvalidProposalId,

    /// Too many active proposals already exist.
    ///
    /// # Limit
    /// Maximum: MAX_ACTIVE_PROPOSALS (20 simultaneous proposals)
    #[error("too many active proposals (max 20)")]
    TooManyActiveProposals,

    /// Governance proposal not found.
    ///
    /// # Resolution
    /// Check the proposal ID. Proposals may have been pruned after execution.
    #[error("proposal not found")]
    ProposalNotFound,

    /// Voting period has ended or not yet started.
    ///
    /// # Resolution
    /// Check proposal voting_starts and voting_ends rounds.
    #[error("voting is closed")]
    VotingClosed,

    /// Address has already voted on this proposal.
    ///
    /// # Resolution
    /// Each address can only vote once per proposal. Votes are binding.
    #[error("already voted on this proposal")]
    AlreadyVoted,

    /// Address already has an active delegation.
    ///
    /// # Resolution
    /// One delegation per address. Undelegate first to change validators.
    #[error("already has an active delegation")]
    AlreadyDelegating,

    /// Address has no active delegation to undelegate.
    ///
    /// # Resolution
    /// Submit a DelegateTx before attempting to undelegate.
    #[error("no active delegation")]
    NotDelegating,

    /// Delegation is already in cooldown (undelegation in progress).
    ///
    /// # Resolution
    /// Wait for cooldown to complete before redelegating.
    #[error("already undelegating — wait for cooldown to complete")]
    AlreadyUndelegating,

    /// General validation error with human-readable message.
    ///
    /// # Usage
    /// Used for custom validation logic not covered by specific variants.
    #[error("validation error: {0}")]
    ValidationError(String),

    /// **FATAL**: Supply invariant violation detected.
    ///
    /// # Severity
    /// This is a critical error indicating state corruption or a critical bug.
    /// The node MUST halt immediately to prevent further corruption.
    ///
    /// # Invariant
    /// `liquid + staked + delegated + treasury + bridge_reserve == total_supply`
    ///
    /// # Resolution
    /// 1. Do NOT restart the node
    /// 2. Contact other validators to compare state
    /// 3. Investigate root cause (logs, disk corruption, software bug)
    /// 4. Coordinate network-wide recovery if needed
    #[error("FATAL: supply invariant broken — node must halt: {0}")]
    SupplyInvariantBroken(String),

    /// Voter has stake locked in active governance votes.
    ///
    /// # Resolution
    /// Wait for active proposals to complete execution before voting again.
    /// This prevents vote manipulation via stake movement.
    #[error("stake is locked in active votes")]
    StakeLocked,

    /// Proposal cooldown period has not elapsed since last proposal.
    ///
    /// # Resolution
    /// Wait PROPOSAL_COOLDOWN_ROUNDS (1,008 rounds ≈ 1.4 hours) before submitting another proposal.
    #[error("proposal cooldown not elapsed — must wait {} rounds", crate::constants::PROPOSAL_COOLDOWN_ROUNDS)]
    ProposalCooldownNotElapsed,

    /// Parameter change would violate BFT safety minimums.
    ///
    /// # Safety Bounds
    /// Governance cannot change parameters below BFT safety thresholds:
    /// - min_stake_to_propose >= 1,000 sats
    /// - quorum_numerator: 10-50%
    /// - execution_delay_rounds >= 2,016
    ///
    /// # Resolution
    /// The proposed parameter value is unsafe. Choose a value within BFT bounds.
    #[error("parameter change violates BFT safety constraints: {0}")]
    BFTSafetyViolation(String),
}

impl CoinError {
    /// Returns true if this error represents an unrecoverable state corruption
    /// that requires immediate node shutdown.
    ///
    /// # Fatal Errors
    /// - `SupplyInvariantBroken`: Indicates potential state corruption or critical bug
    ///
    /// # Non-Fatal Errors
    /// All other errors are validation failures and can be handled gracefully.
    ///
    /// # Usage
    /// ```rust,ignore
    /// match result {
    ///     Err(e) if e.is_fatal() => {
    ///         // Save state and halt immediately
    ///         process::exit(101);
    ///     }
    ///     Err(e) => {
    ///         // Log and continue (validation failure)
    ///         tracing::warn!("Validation failed: {}", e);
    ///     }
    ///     Ok(_) => {}
    /// }
    /// ```
    pub fn is_fatal(&self) -> bool {
        matches!(self, CoinError::SupplyInvariantBroken(_))
    }
}
