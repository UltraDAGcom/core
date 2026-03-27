use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::address::Address;
use crate::persistence::{self, PersistenceError};
use crate::state::engine::{AccountState, DelegationAccount, StakeAccount};

/// Serializable snapshot of StateEngine
/// Uses Vec of tuples instead of HashMap for deterministic serialization order
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateSnapshot {
    pub accounts: Vec<(Address, AccountState)>,
    #[serde(default)]
    pub stake_accounts: Vec<(Address, StakeAccount)>,
    #[serde(default)]
    pub active_validator_set: Vec<Address>,
    #[serde(default)]
    pub current_epoch: u64,
    pub total_supply: u64,
    pub last_finalized_round: Option<u64>,
    #[serde(default)]
    pub proposals: Vec<(u64, crate::governance::Proposal)>,
    #[serde(default)]
    pub votes: Vec<((u64, Address), bool)>,
    #[serde(default)]
    pub next_proposal_id: u64,
    #[serde(default)]
    pub governance_params: crate::governance::GovernanceParams,
    /// Council of 21 members with seat categories.
    #[serde(default)]
    pub council_members: Vec<(crate::address::Address, crate::governance::CouncilSeatCategory)>,
    /// DAO treasury balance in sats. Controlled by Council of 21 via TreasurySpend proposals.
    #[serde(default)]
    pub treasury_balance: u64,
    /// Delegated staking accounts: delegator address → delegation details.
    #[serde(default)]
    pub delegation_accounts: Vec<(crate::address::Address, DelegationAccount)>,
    /// Configured validator count from --validators N CLI flag.
    /// Affects pre-staking reward distribution (divisor). Must be in state root
    /// to prevent divergence between nodes with different --validators N values.
    #[serde(default)]
    pub configured_validator_count: Option<u64>,
    /// Bridge reserve: UDAG locked for bridging to Arbitrum.
    #[serde(default)]
    pub bridge_reserve: u64,
    /// Bridge attestations: nonce → attestation.
    #[serde(default)]
    pub bridge_attestations: Vec<(u64, crate::bridge::BridgeAttestation)>,
    /// Bridge signatures: (nonce, validator) → signature (as Vec<u8> for serde).
    #[serde(default)]
    pub bridge_signatures: Vec<((u64, Address), Vec<u8>)>,
    /// Next bridge nonce.
    #[serde(default)]
    pub bridge_nonce: u64,
    /// Bridge contract address on the destination chain (20 bytes).
    /// Included in attestation hashes for cross-contract replay protection.
    #[serde(default)]
    pub bridge_contract_address: [u8; 20],
    /// Used bridge release nonces: (source_chain_id, deposit_nonce) pairs already released.
    /// Prevents double-release of Arbitrum deposits.
    #[serde(default)]
    pub used_release_nonces: Vec<(u64, u64)>,
    /// Bridge release votes: (chain_id, deposit_nonce) -> list of validators who voted.
    /// Release executes when votes >= ceil(2n/3) of active validators.
    #[serde(default)]
    pub bridge_release_votes: Vec<((u64, u64), Vec<crate::address::Address>)>,
    /// Canonical (recipient, amount) for each in-progress bridge release.
    #[serde(default)]
    pub bridge_release_params: Option<Vec<((u64, u64), (crate::address::Address, u64))>>,
    /// Last round each address created a proposal (spam prevention).
    #[serde(default)]
    pub last_proposal_round: Vec<(crate::address::Address, u64)>,
    /// Bridge release first vote round: (chain_id, deposit_nonce) -> round of first vote.
    /// Used for stale vote pruning — releases that haven't completed after N rounds are cleaned up.
    #[serde(default)]
    pub bridge_release_first_vote_round: Option<Vec<((u64, u64), u64)>>,
    /// Bridge release disagree count: (chain_id, deposit_nonce) -> number of disagreeing votes.
    /// Tracks how many validators voted with different (recipient, amount) than the canonical params.
    #[serde(default)]
    pub bridge_release_disagree_count: Option<Vec<((u64, u64), u64)>>,
    /// Slashed events: (address, round) pairs of already-applied slashes.
    /// Idempotency guard to prevent double-slashing the same (validator, round) pair.
    #[serde(default)]
    pub slashed_events: Vec<(crate::address::Address, u64)>,
    /// SmartAccount configurations: (address, config) pairs.
    #[serde(default)]
    pub smart_accounts: Vec<(crate::address::Address, crate::tx::smart_account::SmartAccountConfig)>,
    /// Name registry: name → address.
    #[serde(default)]
    pub name_to_address: Vec<(String, crate::address::Address)>,
    /// Name expiry: name → expiry round.
    #[serde(default)]
    pub name_expiry: Vec<(String, u64)>,
    /// Name creation: name → creation round.
    #[serde(default)]
    pub name_created_at: Vec<(String, u64)>,
    /// Name profiles: name → profile data.
    #[serde(default)]
    pub name_profiles: Vec<(String, crate::tx::name_registry::NameProfile)>,
}

impl StateSnapshot {
    pub fn save(&self, path: &Path) -> Result<(), PersistenceError> {
        persistence::save(self, path)
    }

    pub fn load(path: &Path) -> Result<Self, PersistenceError> {
        persistence::load(path)
    }

    pub fn exists(path: &Path) -> bool {
        persistence::exists(path)
    }
}
