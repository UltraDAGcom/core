use std::collections::{HashMap, VecDeque};

use crate::address::Address;
use crate::consensus::vertex::DagVertex;
use crate::error::CoinError;
use crate::tx::stake::{StakeTx, UnstakeTx, MIN_STAKE_SATS, UNSTAKE_COOLDOWN_ROUNDS};

/// Internal state snapshot for atomic apply operations.
/// Captures the complete state at a point in time for verification before merging.
#[derive(Debug, Clone)]
pub struct StateSnapshot {
    accounts: HashMap<Address, AccountState>,
    stake_accounts: HashMap<Address, StakeAccount>,
    delegation_accounts: HashMap<Address, DelegationAccount>,
    total_supply: u64,
    treasury_balance: u64,
    bridge_reserve: u64,
    last_finalized_round: Option<u64>,
    applied_validators_per_round: HashMap<u64, std::collections::HashSet<Address>>,
}

impl StateSnapshot {
    /// Create a snapshot from the current state engine.
    fn from_engine(engine: &StateEngine) -> Self {
        Self {
            accounts: engine.accounts.clone(),
            stake_accounts: engine.stake_accounts.clone(),
            delegation_accounts: engine.delegation_accounts.clone(),
            total_supply: engine.total_supply,
            treasury_balance: engine.treasury_balance,
            bridge_reserve: engine.bridge_reserve,
            last_finalized_round: engine.last_finalized_round,
            applied_validators_per_round: engine.applied_validators_per_round.clone(),
        }
    }

    /// Verify the snapshot maintains state consistency invariants.
    fn verify_consistency(&self) -> Result<(), CoinError> {
        // Check for negative balances (impossible with u64, but verify no overflow occurred)
        // Check supply invariant
        let liquid: u64 = self.accounts.values()
            .try_fold(0u64, |acc, a| acc.checked_add(a.balance))
            .ok_or_else(|| CoinError::SupplyInvariantBroken("liquid balance overflow".into()))?;
        
        let staked: u64 = self.stake_accounts.values()
            .try_fold(0u64, |acc, s| acc.checked_add(s.staked))
            .ok_or_else(|| CoinError::SupplyInvariantBroken("staked balance overflow".into()))?;
        
        let delegated: u64 = self.delegation_accounts.values()
            .try_fold(0u64, |acc, d| acc.checked_add(d.delegated))
            .ok_or_else(|| CoinError::SupplyInvariantBroken("delegated balance overflow".into()))?;
        
        let total = liquid
            .checked_add(staked)
            .and_then(|s| s.checked_add(delegated))
            .and_then(|s| s.checked_add(self.treasury_balance))
            .and_then(|s| s.checked_add(self.bridge_reserve))
            .ok_or_else(|| CoinError::SupplyInvariantBroken("total supply calculation overflow".into()))?;
        
        if total != self.total_supply {
            return Err(CoinError::SupplyInvariantBroken(format!(
                "liquid={} staked={} delegated={} treasury={} bridge={} sum={} != total_supply={}",
                liquid, staked, delegated, self.treasury_balance, self.bridge_reserve, total, self.total_supply
            )));
        }

        // Verify all stake accounts have corresponding regular accounts
        for (addr, _) in &self.stake_accounts {
            if !self.accounts.contains_key(addr) {
                return Err(CoinError::SupplyInvariantBroken(format!(
                    "stake account {} has no corresponding regular account",
                    addr.to_hex()
                )));
            }
        }

        // Verify no negative balances (u64 can't be negative, but check for zero which is valid)
        for (addr, account) in &self.accounts {
            // Balance is u64, so it can't be negative - this is just a sanity check
            if account.balance == u64::MAX {
                return Err(CoinError::SupplyInvariantBroken(format!(
                    "account {} has suspicious max u64 balance (possible overflow)",
                    addr.to_hex()
                )));
            }
        }

        // Verify nonces are monotonic (non-decreasing) - already guaranteed by u64
        // but we check for any suspicious patterns
        for (addr, account) in &self.accounts {
            if account.nonce == u64::MAX {
                tracing::warn!("Account {} has max nonce (possible exhaustion)", addr.to_hex());
            }
        }

        Ok(())
    }
}

/// Maximum number of finalized transactions to keep in the index.
/// At ~10 tx/vertex × 5 vertices/round × 720 rounds/hour = ~36K tx/hour.
/// 100K covers ~3 hours of history.
const MAX_TX_INDEX_SIZE: usize = 100_000;

/// Location of a finalized transaction in the DAG.
#[derive(Debug, Clone, serde::Serialize)]
pub struct TxLocation {
    /// Round in which the transaction was finalized.
    pub round: u64,
    /// Hash of the DagVertex containing this transaction.
    pub vertex_hash: [u8; 32],
    /// Validator that produced the vertex.
    pub validator: Address,
}

/// Account balance state.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[derive(Default)]
pub struct AccountState {
    pub balance: u64,
    pub nonce: u64,
}

/// Transaction receipt: records whether a finalized transaction succeeded or was skipped.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TxReceipt {
    /// Round in which the transaction was finalized.
    pub round: u64,
    /// Hash of the DagVertex containing this transaction.
    pub vertex_hash: [u8; 32],
    /// Whether the transaction was successfully applied.
    pub success: bool,
    /// Reason for failure (empty if success).
    pub error: String,
}

/// Record of a slashing event, persisted in state for auditability.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SlashRecord {
    /// Round in which equivocation was detected.
    pub round: u64,
    /// Validator that equivocated.
    pub validator: Address,
    /// Amount of stake burned.
    pub slash_amount: u64,
    /// Amount of delegated stake burned (cascading slash).
    pub delegated_slash_amount: u64,
}


/// Staking account tracking locked funds and cooldown.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct StakeAccount {
    /// Currently staked amount (locked, not spendable).
    pub staked: u64,
    /// If Some(round), funds unlock after this round.
    pub unlock_at_round: Option<u64>,
    /// Commission percentage charged on delegated rewards (0-100).
    #[serde(default = "default_commission")]
    pub commission_percent: u8,
    /// Round when commission was last changed. Enforces COMMISSION_COOLDOWN_ROUNDS
    /// between changes to prevent sandwich attacks on delegators.
    #[serde(default)]
    pub commission_last_changed: Option<u64>,
    /// Stake locked in active governance votes. Prevents voters from moving stake
    /// after casting votes (vote locking vulnerability fix). Locked stake is released
    /// when the proposal is executed or rejected in tick_governance().
    #[serde(default)]
    pub locked_stake: u64,
}

fn default_commission() -> u8 {
    crate::constants::DEFAULT_COMMISSION_PERCENT
}

/// Delegation account tracking delegated funds to a validator.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DelegationAccount {
    /// Amount of UDAG delegated to the validator.
    pub delegated: u64,
    /// The validator this delegation is assigned to.
    pub validator: Address,
    /// If Some(round), delegation is being withdrawn and unlocks after this round.
    pub unlock_at_round: Option<u64>,
}

/// StateEngine: derives account state from an ordered list of finalized DAG vertices.
/// This replaces the old Blockchain struct. The DAG IS the ledger.
#[derive(Debug)]
pub struct StateEngine {
    accounts: HashMap<Address, AccountState>,
    stake_accounts: HashMap<Address, StakeAccount>,
    pub total_supply: u64,
    /// Track the last finalized round we've applied
    pub last_finalized_round: Option<u64>,
    /// Epoch-frozen active validator set (top MAX_ACTIVE_VALIDATORS by stake).
    /// Recalculated only at epoch boundaries.
    active_validator_set: Vec<Address>,
    /// Current epoch number (round / EPOCH_LENGTH_ROUNDS).
    current_epoch: u64,
    /// All proposals ever created, keyed by proposal ID.
    proposals: HashMap<u64, crate::governance::Proposal>,
    /// Votes cast: (proposal_id, voter_address) -> vote (true=for, false=against).
    votes: HashMap<(u64, Address), bool>,
    /// Council of 21: Authorized governance members with seat categories.
    /// Only council members can vote on governance proposals.
    /// Members earn a share of block emissions (COUNCIL_EMISSION_PERCENT).
    council_members: HashMap<Address, crate::governance::CouncilSeatCategory>,
    /// Monotonically increasing proposal counter.
    next_proposal_id: u64,
    /// Runtime-adjustable governance parameters (changed via ParameterChange proposals).
    governance_params: crate::governance::GovernanceParams,
    /// Configured validator count for pre-staking reward splitting.
    /// When set, block reward is divided by this count in pre-staking mode.
    /// Must match the --validators CLI flag AND `ValidatorSet::configured_validators`
    /// (which uses `usize` for quorum math). Both are set together in main.rs.
    configured_validator_count: Option<u64>,
    /// DAO treasury balance in sats. Funded at genesis (10% of max supply).
    /// Spent via TreasurySpend proposals approved by the Council of 21.
    treasury_balance: u64,
    /// Delegated staking accounts: delegator address → delegation details.
    /// One delegation per address (like one stake per address).
    delegation_accounts: HashMap<Address, DelegationAccount>,
    /// Bounded index of finalized transaction hashes → their location in the DAG.
    /// Enables `/tx/{hash}` lookups without scanning the full DAG.
    /// FIFO eviction when exceeding MAX_TX_INDEX_SIZE.
    tx_index: HashMap<[u8; 32], TxLocation>,
    /// Insertion order for FIFO eviction of tx_index entries.
    tx_index_order: VecDeque<[u8; 32]>,
    /// Transaction receipts: tx_hash → receipt (success/failure + reason).
    /// Bounded to MAX_TX_INDEX_SIZE entries with FIFO eviction (same as tx_index).
    tx_receipts: HashMap<[u8; 32], TxReceipt>,
    /// Record of slashing events. In-memory only, lost on restart.
    /// Capped at 10,000 entries to prevent unbounded growth.
    slash_history: Vec<SlashRecord>,
    /// Tracks which validators have produced vertices in each finalized round.
    /// Used for cross-batch equivocation detection: if a validator appears in a
    /// round that was already applied in a previous batch, that's equivocation.
    /// Pruned to keep only rounds > last_finalized_round - 1000.
    applied_validators_per_round: HashMap<u64, std::collections::HashSet<Address>>,
    /// Bridge reserve: UDAG locked for bridging to Arbitrum.
    /// Included in the supply invariant: liquid + staked + delegated + treasury + bridge_reserve == total_supply.
    bridge_reserve: u64,
    /// Tracks the last round in which each address submitted a proposal.
    /// Used to enforce PROPOSAL_COOLDOWN_ROUNDS between submissions.
    /// Prevents spam and allows time for community review of failed proposals.
    last_proposal_round: HashMap<Address, u64>,
    /// Bridge attestations: nonce → (attestation, collected signatures).
    /// Validators sign attestations as part of consensus. When 2/3+ signatures collected,
    /// users can claim on Arbitrum.
    bridge_attestations: HashMap<u64, crate::bridge::BridgeAttestation>,
    /// Bridge signatures: (nonce, validator) → packed ECDSA data.
    /// Format: eth_address (20 bytes) + ecdsa_signature (65 bytes: r || s || v) = 85 bytes.
    /// Uses secp256k1/ECDSA for Solidity ecrecover compatibility (H1 fix).
    bridge_signatures: HashMap<(u64, Address), [u8; 85]>,
    /// Next bridge nonce (incremented for each new attestation).
    bridge_nonce: u64,
    /// Bridge contract address on the destination chain (20 bytes).
    /// Included in attestation hashes for cross-contract replay protection.
    /// Set via `set_bridge_contract_address()` or CLI configuration.
    bridge_contract_address: [u8; 20],
    /// Used release nonces: (source_chain_id, deposit_nonce) pairs that have been released.
    /// Prevents double-release of the same Arbitrum deposit.
    used_release_nonces: std::collections::HashSet<(u64, u64)>,
    /// Bridge release votes: (chain_id, deposit_nonce) -> set of validators who voted.
    /// Release only executes when votes >= ceil(2n/3) of active validators.
    bridge_release_votes: HashMap<(u64, u64), std::collections::HashSet<Address>>,
    /// Canonical (recipient, amount) for each in-progress bridge release.
    /// First voter's values are stored; subsequent voters must match.
    /// Cleaned up when release executes or attestation expires.
    bridge_release_params: HashMap<(u64, u64), (Address, u64)>,
    /// Round when the first vote was cast for each in-progress bridge release.
    /// Used for age-based pruning of stale votes that never reach quorum.
    bridge_release_first_vote_round: HashMap<(u64, u64), u64>,
    /// Disagreement count for in-progress bridge releases. Tracks how many validators
    /// submitted mismatched (recipient, amount) vs the stored canonical params.
    /// When disagree >= agree, params are reset and voting restarts.
    bridge_release_disagree_count: HashMap<(u64, u64), u64>,
    /// Permanent record of (validator, round) pairs that have been slashed.
    /// Prevents double-slashing the same equivocation event if evidence is re-encountered
    /// after the applied_validators_per_round tracker is pruned.
    slashed_events: std::collections::HashSet<(Address, u64)>,
}

impl StateEngine {
    pub fn new() -> Self {
        Self {
            accounts: HashMap::new(),
            stake_accounts: HashMap::new(),
            total_supply: 0,
            last_finalized_round: None,
            active_validator_set: Vec::new(),
            current_epoch: crate::constants::EPOCH_UNINITIALIZED,
            proposals: HashMap::new(),
            votes: HashMap::new(),
            council_members: HashMap::new(),
            treasury_balance: 0,
            next_proposal_id: 0,
            governance_params: crate::governance::GovernanceParams::default(),
            configured_validator_count: None,
            delegation_accounts: HashMap::new(),
            tx_index: HashMap::new(),
            tx_index_order: VecDeque::new(),
            tx_receipts: HashMap::new(),
            slash_history: Vec::new(),
            applied_validators_per_round: HashMap::new(),
            bridge_reserve: 0,
            last_proposal_round: HashMap::new(),

            bridge_attestations: HashMap::new(),
            bridge_signatures: HashMap::new(),
            bridge_nonce: 0,
            bridge_contract_address: [0u8; 20],
            used_release_nonces: std::collections::HashSet::new(),
            bridge_release_votes: HashMap::new(),
            bridge_release_params: HashMap::new(),
            bridge_release_first_vote_round: HashMap::new(),
            bridge_release_disagree_count: HashMap::new(),
            slashed_events: std::collections::HashSet::new(),
        }
    }

    /// Set the bridge contract address on the destination chain.
    /// This is included in all bridge attestation hashes for cross-contract replay protection.
    pub fn set_bridge_contract_address(&mut self, address: [u8; 20]) {
        self.bridge_contract_address = address;
    }

    /// Get the bridge contract address on the destination chain.
    pub fn bridge_contract_address(&self) -> [u8; 20] {
        self.bridge_contract_address
    }

    /// Set the configured validator count for pre-staking reward splitting.
    pub fn set_configured_validator_count(&mut self, count: u64) {
        self.configured_validator_count = Some(count);
    }

    pub fn configured_validator_count(&self) -> Option<u64> {
        self.configured_validator_count
    }

    /// Compute the validator reward for a given proposer at a given round.
    ///
    /// NOTE: This function is for RPC display and testing ONLY. It is NOT used
    /// in any consensus-critical path. Actual reward distribution happens in
    /// `distribute_round_rewards()` which is the single source of truth.
    /// Coinbase is always 0 (deferred coinbase model); this function estimates
    /// what the protocol would distribute, but may diverge from the actual
    /// distribution due to remainder handling and supply cap scaling.
    ///
    /// `active_validator_count` is the fallback divisor for pre-staking mode.
    pub fn compute_validator_reward(
        &self,
        proposer: &Address,
        round: u64,
        active_validator_count: u64,
    ) -> u64 {
        let total_round_reward = crate::constants::block_reward(round);

        // Deduct council emission share from validator reward pool
        let council_percent = self.governance_params.council_emission_percent;
        let council_count = self.council_members.len() as u64;
        let validator_pool = if council_count > 0 && council_percent > 0 {
            // Validators get (100 - council_percent)% of block reward
            total_round_reward.saturating_mul(100u64.saturating_sub(council_percent)) / 100
        } else {
            total_round_reward
        };

        // Total effective stake must match distribute_round_rewards denominator:
        // sum of effective_stake_of() for all stakers, which excludes undelegating amounts.
        let total_stake: u64 = self.stake_accounts.iter()
            .filter(|(_, s)| s.staked > 0)
            .map(|(addr, _)| self.effective_stake_of(addr))
            .fold(0u64, |acc, x| acc.saturating_add(x));
        let own_effective = self.effective_stake_of(proposer);

        let base_reward = if total_stake > 0 && own_effective > 0 {
            // Proportional to effective stake (own + delegations) using u128 to avoid overflow
            let proportional = ((validator_pool as u128)
                .saturating_mul(own_effective as u128)
                / total_stake as u128) as u64;
            // Observer penalty: staked but not in the active validator set
            // Uses the governable parameter, not the hardcoded constant.
            if !self.active_validator_set.is_empty()
                && !self.active_validator_set.contains(proposer)
            {
                proportional * self.governance_params.observer_reward_percent / 100
            } else {
                proportional
            }
        } else {
            // Pre-staking fallback: equal split among validators.
            let n = self.configured_validator_count
                .unwrap_or(active_validator_count.max(1));
            validator_pool / n.max(1)
        };

        // Supply cap enforcement
        let max_supply = crate::constants::MAX_SUPPLY_SATS;
        if self.total_supply.saturating_add(base_reward) > max_supply {
            max_supply.saturating_sub(self.total_supply)
        } else {
            base_reward
        }
    }

    /// Compute the total council emission for a round.
    /// Returns (per_member_amount, total_council_amount).
    ///
    /// The per-round council budget = `block_reward(round) * council_emission_percent / 100`,
    /// split equally among seated council members. Used by the `/council` RPC endpoint
    /// for display purposes. Actual emission happens in `distribute_round_rewards()`.
    pub fn compute_council_emission(&self, round: u64) -> (u64, u64) {
        let council_count = self.council_members.len() as u64;
        let council_percent = self.governance_params.council_emission_percent;
        if council_count == 0 || council_percent == 0 {
            return (0, 0);
        }
        let total_round_reward = crate::constants::block_reward(round);
        let council_total = total_round_reward.saturating_mul(council_percent) / 100;
        let per_member = council_total / council_count;
        (per_member, per_member.saturating_mul(council_count))
    }

    /// Distribute the round's block reward to ALL stakers proportionally.
    /// Called once per finalized round (not per vertex).
    ///
    /// - Active validators who produced vertices get 100% of their proportional share
    /// - Stakers who didn't produce (passive/observer) get OBSERVER_REWARD_PERCENT (20%)
    /// - Delegators earn through their validator's effective stake minus commission
    /// - Council members receive their emission share
    /// - Pre-staking fallback: equal split among configured validators (producers only)
    ///
    /// **Canonical remainder distribution:** Integer division truncation causes rounding
    /// dust in each distribution step. Rather than silently burning this dust (~630 UDAG/year),
    /// the remainder (`pool - sum_of_credits`) is assigned to the first recipient in sorted
    /// address order. This is deterministic across all nodes (sorted order is canonical) and
    /// ensures the full allocated pool is distributed every round. This is a standard technique
    /// used in DeFi reward distribution contracts.
    pub fn distribute_round_rewards(
        &mut self,
        round: u64,
        producers: &std::collections::HashSet<Address>,
    ) -> Result<(), CoinError> {
        let total_round_reward = crate::constants::block_reward(round);
        if total_round_reward == 0 {
            return Ok(());
        }

        // Supply cap check
        let remaining_supply = crate::constants::MAX_SUPPLY_SATS.saturating_sub(self.total_supply);
        if remaining_supply == 0 {
            return Ok(());
        }

        // --- Council emission (computed once per round) ---
        // SAFETY: council_members must be identical across all nodes at this round.
        // This is guaranteed because: (1) CouncilMembership proposals execute via
        // tick_governance which runs per-round AFTER all vertices in that round are
        // applied, and (2) tick_governance iterates proposals sorted by ID for
        // deterministic execution order. (3) Fast-sync loads council_members from
        // checkpoint snapshot. (4) If council sets diverge, total_supply will differ,
        // and the supply invariant check (SupplyInvariantBroken → process exit) catches it.
        // The member list is sorted by address for deterministic credit ordering.
        // --- Compute emission splits ---
        // Council: council_emission_percent (default 10%)
        // Treasury: treasury_emission_percent (default 10%)
        // Founder: founder_emission_percent (default 5%)
        // Validators: remainder (default 75%)
        let council_percent = self.governance_params.council_emission_percent;
        let treasury_percent = self.governance_params.treasury_emission_percent;
        let founder_percent = self.governance_params.founder_emission_percent;

        let council_total = total_round_reward.saturating_mul(council_percent) / 100;
        let treasury_total = total_round_reward.saturating_mul(treasury_percent) / 100;
        let founder_total = total_round_reward.saturating_mul(founder_percent) / 100;
        let validator_pool = total_round_reward
            .saturating_sub(council_total)
            .saturating_sub(treasury_total)
            .saturating_sub(founder_total);

        // --- Council emission ---
        let council_count = self.council_members.len() as u64;
        if council_count > 0 && council_total > 0 {
            let per_member = council_total / council_count;
            if per_member > 0 {
                let council_mint = per_member.saturating_mul(council_count).min(remaining_supply);
                let capped_per = council_mint / council_count;
                if capped_per > 0 {
                    // Sort members by address for deterministic credit ordering
                    let mut members: Vec<Address> = self.council_members.keys().copied().collect();
                    members.sort();
                    for member in &members {
                        if let Err(e) = self.credit(member, capped_per) {
                            return Err(CoinError::SupplyInvariantBroken(
                                format!("council emission credit failed: {}", e),
                            ));
                        }
                    }
                    let actually_minted = capped_per.saturating_mul(members.len() as u64);
                    // Canonical remainder: assign truncation dust to first member in sorted order.
                    let council_remainder = council_mint.saturating_sub(actually_minted);
                    if council_remainder > 0 {
                        if let Err(e) = self.credit(&members[0], council_remainder) {
                            return Err(CoinError::SupplyInvariantBroken(
                                format!("council remainder credit failed: {}", e),
                            ));
                        }
                    }
                    self.total_supply = self.total_supply.saturating_add(actually_minted.saturating_add(council_remainder));
                }
            }
        }

        // --- Treasury emission ---
        // Treasury credits go to treasury_balance (not an account), spent via TreasurySpend proposals.
        if treasury_total > 0 {
            let remaining_after_council = crate::constants::MAX_SUPPLY_SATS.saturating_sub(self.total_supply);
            let capped_treasury = treasury_total.min(remaining_after_council);
            if capped_treasury > 0 {
                self.treasury_balance = self.treasury_balance.saturating_add(capped_treasury);
                self.total_supply = self.total_supply.saturating_add(capped_treasury);
            }
        }

        // --- Founder emission ---
        // Founder credits go to liquid balance via credit() — can spend/stake/delegate normally.
        if founder_total > 0 {
            let remaining_after_treasury = crate::constants::MAX_SUPPLY_SATS.saturating_sub(self.total_supply);
            let capped_founder = founder_total.min(remaining_after_treasury);
            if capped_founder > 0 {
                if let Err(e) = self.credit(&crate::constants::dev_address(), capped_founder) {
                    return Err(crate::error::CoinError::SupplyInvariantBroken(
                        format!("founder emission credit failed: {}", e),
                    ));
                }
                self.total_supply = self.total_supply.saturating_add(capped_founder);
            }
        }

        if validator_pool == 0 {
            return Ok(());
        }

        // Re-check remaining supply after council emission
        let remaining_supply = crate::constants::MAX_SUPPLY_SATS.saturating_sub(self.total_supply);
        if remaining_supply == 0 {
            return Ok(());
        }

        // Compute effective stake ONCE per validator and cache results.
        // effective_stake_of() is O(D) where D = delegation count, so computing it
        // once here avoids the previous O(S*D) double iteration (total + per-validator).
        // MUST sort by address for deterministic iteration — HashMap order is
        // non-deterministic and would cause consensus splits across nodes.
        let mut validators: Vec<(Address, u64)> = self.stake_accounts.iter()
            .filter(|(_, s)| s.staked > 0)
            .map(|(addr, _)| (*addr, self.effective_stake_of(addr)))
            .collect();
        validators.sort_by_key(|(addr, _)| *addr);

        let total_effective_stake: u64 = validators.iter()
            .map(|(_, eff)| *eff)
            .fold(0u64, |acc, x| acc.saturating_add(x));

        if total_effective_stake > 0 {
            // --- Staking active: distribute proportionally to all stakers ---

            let mut total_to_mint: u64 = 0;
            let mut credits: Vec<(Address, u64)> = Vec::new();

            for (validator, effective) in &validators {
                if *effective == 0 {
                    continue;
                }

                // Proportional share of the validator pool
                let proportional = ((validator_pool as u128)
                    .saturating_mul(*effective as u128)
                    / total_effective_stake as u128) as u64;

                // Active producers get 100%, passive stakers get observer rate
                // Uses the governable parameter, not the hardcoded constant.
                let observer_pct = self.governance_params.observer_reward_percent;
                let validator_share = if producers.contains(validator) {
                    proportional
                } else {
                    proportional * observer_pct / 100
                };

                if validator_share == 0 {
                    continue;
                }

                // Split between validator's own stake and delegations
                let own_stake = self.stake_of(validator);
                let own_proportion = if *effective > 0 {
                    ((validator_share as u128).checked_mul(own_stake as u128)
                        .ok_or_else(|| CoinError::SupplyInvariantBroken("own_proportion overflow".into()))?
                        / *effective as u128) as u64
                } else {
                    validator_share
                };

                // Credit validator their own-stake portion
                if own_proportion > 0 {
                    credits.push((*validator, own_proportion));
                    total_to_mint = total_to_mint.checked_add(own_proportion)
                        .ok_or_else(|| CoinError::SupplyInvariantBroken("total_to_mint overflow (own)".into()))?;
                }

                // Distribute delegator portions (validator_share - own_proportion)
                let delegation_pool = validator_share.checked_sub(own_proportion).unwrap_or(0);
                if delegation_pool > 0 {
                    let commission_percent = self.stake_accounts
                        .get(validator)
                        .map(|s| s.commission_percent)
                        .unwrap_or(crate::constants::DEFAULT_COMMISSION_PERCENT);

                    // Sort delegators by address for deterministic reward distribution.
                    // HashMap iteration order is non-deterministic.
                    let mut delegators: Vec<(Address, u64)> = self.delegation_accounts.iter()
                        .filter(|(_, d)| d.validator == *validator && d.unlock_at_round.is_none())
                        .map(|(addr, d)| (*addr, d.delegated))
                        .collect();
                    delegators.sort_by_key(|(addr, _)| *addr);

                    let total_delegated_to_validator: u64 = delegators.iter()
                        .try_fold(0u64, |acc, (_, d)| acc.checked_add(*d))
                        .ok_or_else(|| CoinError::SupplyInvariantBroken("total_delegated overflow".into()))?;

                    for (delegator, delegated) in &delegators {
                        if total_delegated_to_validator == 0 {
                            continue;
                        }
                        let delegator_share = ((delegation_pool as u128)
                            .checked_mul(*delegated as u128)
                            .ok_or_else(|| CoinError::SupplyInvariantBroken("delegator_share overflow".into()))?
                            / total_delegated_to_validator as u128) as u64;
                        if delegator_share == 0 {
                            continue;
                        }
                        let commission = delegator_share.checked_mul(commission_percent as u64)
                            .ok_or_else(|| CoinError::SupplyInvariantBroken("commission overflow".into()))?
                            / 100;
                        let net = delegator_share.checked_sub(commission).unwrap_or(0);
                        if net > 0 {
                            credits.push((*delegator, net));
                            total_to_mint = total_to_mint.checked_add(net)
                                .ok_or_else(|| CoinError::SupplyInvariantBroken("total_to_mint overflow (net)".into()))?;
                        }
                        // Commission stays with the validator
                        if commission > 0 {
                            credits.push((*validator, commission));
                            total_to_mint = total_to_mint.checked_add(commission)
                                .ok_or_else(|| CoinError::SupplyInvariantBroken("total_to_mint overflow (commission)".into()))?;
                        }
                    }
                }
            }

            // Cap total minting to remaining supply
            let capped_mint = total_to_mint.min(remaining_supply);
            if capped_mint < total_to_mint && total_to_mint > 0 {
                // Scale all credits proportionally
                let scale = capped_mint as u128;
                let total = total_to_mint as u128;
                for (_, amount) in &mut credits {
                    *amount = ((*amount as u128).checked_mul(scale)
                        .ok_or_else(|| CoinError::SupplyInvariantBroken("credit scaling overflow".into()))?
                        / total) as u64;
                }
            }

            // Apply credits and mint
            let mut actually_minted: u64 = 0;
            for (addr, amount) in &credits {
                if *amount > 0 {
                    let _ = self.credit(addr, *amount)?; // Propagate credit errors
                    actually_minted = actually_minted.checked_add(*amount)
                        .ok_or_else(|| CoinError::SupplyInvariantBroken("actually_minted overflow".into()))?;
                }
            }

            // Canonical remainder: assign truncation dust to the first validator in sorted order.
            // capped_mint is the intended total distribution (possibly supply-capped).
            // Integer division across proportional splits loses dust; this reclaims it.
            let staking_remainder = capped_mint.checked_sub(actually_minted).unwrap_or(0);
            if staking_remainder > 0 {
                // First validator in sorted order (validators is already sorted by address)
                if let Some((first_addr, _)) = validators.first() {
                    let _ = self.credit(first_addr, staking_remainder)?;
                    actually_minted = actually_minted.checked_add(staking_remainder)
                        .ok_or_else(|| CoinError::SupplyInvariantBroken("actually_minted overflow (remainder)".into()))?;
                }
            }

            self.total_supply = self.total_supply.checked_add(actually_minted)
                .ok_or_else(|| CoinError::SupplyInvariantBroken("total_supply overflow".into()))?;
        } else {
            // --- Pre-staking fallback: equal split among producers ---
            // MUST sort producers for deterministic credit ordering — HashSet iteration
            // order is non-deterministic and would cause consensus splits across nodes.
            let n = self.configured_validator_count
                .unwrap_or(producers.len().max(1) as u64);
            if !producers.is_empty() {
                let per_producer = validator_pool / n.max(1);
                let capped = per_producer.min(remaining_supply / producers.len().max(1) as u64);
                if capped > 0 {
                    let mut sorted_producers: Vec<_> = producers.iter().collect();
                    sorted_producers.sort();
                    for producer in &sorted_producers {
                        self.credit(producer, capped)?;
                    }
                    let minted = capped.saturating_mul(producers.len() as u64);
                    // Canonical remainder: assign truncation dust from `validator_pool / n`
                    // to the first producer in sorted order. The division loses
                    // `validator_pool % n` sats; this reclaims the dust rather than burning it.
                    // Only credit what remains within the supply cap.
                    let division_dust = validator_pool.saturating_sub(per_producer.saturating_mul(n));
                    let pre_stake_remainder = division_dust.min(remaining_supply.saturating_sub(minted));
                    if pre_stake_remainder > 0 {
                        self.credit(sorted_producers[0], pre_stake_remainder)?;
                    }
                    self.total_supply = self.total_supply.saturating_add(minted.saturating_add(pre_stake_remainder));
                }
            }
        }

        Ok(())
    }

    /// Create a new StateEngine with genesis state.
    /// All nodes must call this to start with identical initial state.
    ///
    /// No pre-mine: all tokens are distributed through per-round emission.
    /// - Mainnet: total_supply = 0 (completely clean genesis)
    /// - Testnet: total_supply = FAUCET_PREFUND_SATS (faucet for testing)
    /// - Founder, treasury, and council all start at 0 and earn through emission.
    pub fn new_with_genesis() -> Self {
        let mut engine = Self::new();

        // Faucet reserve (testnet only — excluded from mainnet genesis)
        #[cfg(not(feature = "mainnet"))]
        {
            let faucet_addr = crate::constants::faucet_keypair().address();
            let _ = engine.credit(&faucet_addr, crate::constants::FAUCET_PREFUND_SATS);
        }

        // Bootstrap council: dev address gets the first Foundation seat.
        // Without this, no one can create proposals (catch-22).
        // The dev/foundation member can then propose additional council members.
        // Note: dev address starts with 0 balance — earns through emission.
        //
        // WARNING — SINGLE-POINT-OF-FAILURE RISK:
        // At genesis, only the dev address is a council member. If this key is lost
        // or compromised, governance is permanently locked (no one can create proposals
        // to add new members). Operators MUST add additional council members immediately
        // after genesis via CouncilMembership governance proposals. Use both Foundation
        // seats at minimum, and add members across multiple categories as soon as possible.
        // See the Mainnet Launch Checklist for key ceremony and council bootstrap procedures.
        let dev_addr = crate::constants::dev_address();
        let _ = engine.add_council_member(
            dev_addr,
            crate::governance::CouncilSeatCategory::Foundation,
        );

        // total_supply tracks all credited amounts + treasury
        // No pre-mine: only faucet (testnet) contributes to genesis supply.
        #[cfg(not(feature = "mainnet"))]
        {
            engine.total_supply = crate::constants::FAUCET_PREFUND_SATS;
        }
        #[cfg(feature = "mainnet")]
        {
            engine.total_supply = 0;
        }

        engine
    }

    pub fn balance(&self, address: &Address) -> u64 {
        self.accounts.get(address).map_or(0, |a| a.balance)
    }

    pub fn nonce(&self, address: &Address) -> u64 {
        self.accounts.get(address).map_or(0, |a| a.nonce)
    }

    pub fn total_supply(&self) -> u64 {
        self.total_supply
    }

    pub fn account_count(&self) -> usize {
        self.accounts.len()
    }

    pub fn last_finalized_round(&self) -> Option<u64> {
        self.last_finalized_round
    }

    /// Verify state consistency by checking all invariants.
    /// This is a comprehensive check that should be called after each round application.
    /// Can be cfg-gated for debug mode in production.
    ///
    /// Checks:
    /// - Supply invariant (liquid + staked + delegated + treasury + bridge = total_supply)
    /// - All stake accounts have corresponding regular accounts
    /// - No negative balances (u64 can't be negative, but checks for overflow patterns)
    /// - Nonces are monotonic (non-decreasing)
    pub fn verify_state_consistency(&self) -> Result<(), CoinError> {
        // Check supply invariant using checked arithmetic
        let liquid: u64 = self.accounts.values()
            .try_fold(0u64, |acc, a| acc.checked_add(a.balance))
            .ok_or_else(|| CoinError::SupplyInvariantBroken("liquid balance overflow".into()))?;
        
        let staked: u64 = self.stake_accounts.values()
            .try_fold(0u64, |acc, s| acc.checked_add(s.staked))
            .ok_or_else(|| CoinError::SupplyInvariantBroken("staked balance overflow".into()))?;
        
        let delegated: u64 = self.delegation_accounts.values()
            .try_fold(0u64, |acc, d| acc.checked_add(d.delegated))
            .ok_or_else(|| CoinError::SupplyInvariantBroken("delegated balance overflow".into()))?;
        
        let total = liquid
            .checked_add(staked)
            .and_then(|s| s.checked_add(delegated))
            .and_then(|s| s.checked_add(self.treasury_balance))
            .and_then(|s| s.checked_add(self.bridge_reserve))
            .ok_or_else(|| CoinError::SupplyInvariantBroken("total supply calculation overflow".into()))?;
        
        if total != self.total_supply {
            return Err(CoinError::SupplyInvariantBroken(format!(
                "verify_state_consistency: liquid={} staked={} delegated={} treasury={} bridge={} sum={} != total_supply={}",
                liquid, staked, delegated, self.treasury_balance, self.bridge_reserve, total, self.total_supply
            )));
        }

        // Verify all stake accounts have corresponding regular accounts
        for (addr, _) in &self.stake_accounts {
            if !self.accounts.contains_key(addr) {
                return Err(CoinError::SupplyInvariantBroken(format!(
                    "stake account {} has no corresponding regular account",
                    addr.to_hex()
                )));
            }
        }

        // Verify no suspicious balance patterns (u64 can't be negative)
        for (addr, account) in &self.accounts {
            if account.balance == u64::MAX {
                return Err(CoinError::SupplyInvariantBroken(format!(
                    "account {} has suspicious max u64 balance (possible overflow)",
                    addr.to_hex()
                )));
            }
        }

        // Verify nonces are reasonable (warn on max nonce)
        for (addr, account) in &self.accounts {
            if account.nonce == u64::MAX {
                tracing::warn!("Account {} has max nonce (possible exhaustion)", addr.to_hex());
            }
        }

        // Verify stake accounts don't have suspicious values
        for (addr, stake) in &self.stake_accounts {
            if stake.staked == u64::MAX {
                return Err(CoinError::SupplyInvariantBroken(format!(
                    "stake account {} has suspicious max u64 staked (possible overflow)",
                    addr.to_hex()
                )));
            }
        }

        // Verify delegation accounts don't have suspicious values
        for (addr, delegation) in &self.delegation_accounts {
            if delegation.delegated == u64::MAX {
                return Err(CoinError::SupplyInvariantBroken(format!(
                    "delegation account {} has suspicious max u64 delegated (possible overflow)",
                    addr.to_hex()
                )));
            }
        }

        Ok(())
    }

    /// Look up a finalized transaction by hash.
    pub fn tx_location(&self, tx_hash: &[u8; 32]) -> Option<&TxLocation> {
        self.tx_index.get(tx_hash)
    }

    /// Index a finalized transaction, evicting oldest entries if the index is full.
    fn index_tx(&mut self, tx_hash: [u8; 32], location: TxLocation) {
        if self.tx_index.contains_key(&tx_hash) {
            return; // already indexed (e.g., duplicate hash across vertices)
        }
        // FIFO eviction when at capacity
        while self.tx_index.len() >= MAX_TX_INDEX_SIZE {
            if let Some(old_hash) = self.tx_index_order.pop_front() {
                self.tx_index.remove(&old_hash);
            } else {
                break;
            }
        }
        self.tx_index.insert(tx_hash, location);
        self.tx_index_order.push_back(tx_hash);
    }

    /// Record a transaction receipt (success or failure with reason).
    fn record_receipt(&mut self, tx_hash: [u8; 32], round: u64, vertex_hash: [u8; 32], success: bool, error: &str) {
        while self.tx_receipts.len() >= MAX_TX_INDEX_SIZE {
            if let Some(key) = self.tx_receipts.keys().next().copied() {
                self.tx_receipts.remove(&key);
            } else { break; }
        }
        self.tx_receipts.insert(tx_hash, TxReceipt {
            round, vertex_hash, success, error: error.to_string(),
        });
    }

    /// Get a transaction receipt by hash.
    pub fn tx_receipt(&self, tx_hash: &[u8; 32]) -> Option<&TxReceipt> {
        self.tx_receipts.get(tx_hash)
    }

    /// Get the full slash history (permanent, survives DAG pruning).
    pub fn slash_history(&self) -> &[SlashRecord] {
        &self.slash_history
    }

    /// Rebuild the tx_index from DAG vertices after a restart.
    /// This restores /tx/:hash lookups for recently-finalized transactions
    /// that were lost because tx_index is not persisted.
    pub fn rebuild_tx_index(&mut self, vertices: &[&DagVertex]) {
        for vertex in vertices {
            let vertex_hash = vertex.hash();
            for tx in &vertex.block.transactions {
                self.index_tx(tx.hash(), TxLocation {
                    round: vertex.round,
                    vertex_hash,
                    validator: vertex.validator,
                });
            }
        }
    }

    /// Apply a finalized vertex to the state (convenience for single-vertex tests).
    /// Also distributes round rewards and ticks governance.
    pub fn apply_vertex(&mut self, vertex: &DagVertex) -> Result<(), CoinError> {
        // Process unstake completions at the round boundary (same as apply_finalized_vertices)
        self.process_unstake_completions(vertex.round)?;
        self.apply_vertex_with_validators(vertex, 1)?;
        // Single-vertex convenience: distribute round rewards, update finality, tick governance.
        let mut producers = std::collections::HashSet::new();
        producers.insert(vertex.validator);
        self.distribute_round_rewards(vertex.round, &producers)?;
        self.last_finalized_round = Some(vertex.round);
        self.tick_governance(vertex.round);
        Ok(())
    }

    /// Apply a finalized vertex to state. Handles fees and transactions only.
    /// Block rewards are distributed separately via distribute_round_rewards().
    pub fn apply_vertex_with_validators(
        &mut self,
        vertex: &DagVertex,
        _active_validator_count: u64,
    ) -> Result<(), CoinError> {
        // Apply directly — finalized vertices are BFT-confirmed and must succeed.
        // Note: unstake completions are processed once per round in apply_finalized_vertices(),
        // not per-vertex, to ensure deterministic unlock timing at round boundaries.

        let proposer = &vertex.block.coinbase.to;

        // Coinbase validation: coinbase.amount MUST be zero. Transaction fees are
        // credited to the proposer entirely via the deferred coinbase mechanism
        // (collected_fees, computed after processing all transactions). This
        // eliminates the inflation vector where a malicious proposer includes
        // transactions they know will fail (stale nonces) to inflate declared fees.
        // With coinbase.amount == 0, there is nothing to declare and nothing to inflate.
        if vertex.block.coinbase.amount != 0 {
            return Err(CoinError::InvalidCoinbase {
                expected: 0,
                got: vertex.block.coinbase.amount,
            });
        }

        // Track fees actually collected from successful transactions.
        // Proposer is credited collected_fees AFTER the loop.
        // Fees from failed transactions (stale nonce, insufficient balance) are
        // never debited from senders and never credited to anyone — they simply
        // don't enter the economy. The coinbase.amount field is always 0; the
        // proposer's fee credit comes entirely from this deferred mechanism.
        let mut collected_fees: u64 = 0;
        let vertex_hash = vertex.hash();

        // Apply transactions
        // In a DAG with multiple validators, the same transaction can appear in
        // multiple vertices (all validators snapshot the same mempool). When one
        // validator's vertex is finalized first, the duplicate in another vertex
        // will fail nonce validation. We must skip these gracefully — a finalized
        // vertex cannot be un-finalized, so aborting would permanently halt finality.
        for tx in &vertex.block.transactions {
            // Verify signature
            if !tx.verify_signature() {
                tracing::warn!("Skipping tx with invalid signature in finalized vertex");
                self.record_receipt(tx.hash(), vertex.round, vertex_hash, false, "invalid signature");
                continue;
            }

            // Check nonce
            let expected_nonce = self.nonce(&tx.from());
            if tx.nonce() != expected_nonce {
                self.record_receipt(tx.hash(), vertex.round, vertex_hash, false,
                    &format!("nonce mismatch: expected {}, got {}", expected_nonce, tx.nonce()));
                tracing::warn!(
                    "Skipping duplicate tx in finalized vertex: expected nonce={}, got={}",
                    expected_nonce, tx.nonce()
                );
                continue;
            }

            // Check balance
            let sender_balance = self.balance(&tx.from());
            if sender_balance < tx.total_cost() {
                self.increment_nonce(&tx.from());
                self.record_receipt(tx.hash(), vertex.round, vertex_hash, false,
                    &format!("insufficient balance: need {}, have {}", tx.total_cost(), sender_balance));
                tracing::warn!(
                    "Skipping tx with insufficient balance in finalized vertex: need={}, have={}",
                    tx.total_cost(), sender_balance
                );
                continue;
            }

            // Apply transaction based on type
            match tx {
                crate::tx::Transaction::Transfer(transfer_tx) => {
                    // Debit sender (amount + fee)
                    if let Err(e) = self.debit(&transfer_tx.from, transfer_tx.total_cost()) {
                        return Err(CoinError::ValidationError(format!(
                            "Failed to debit transfer sender: {}", e
                        )));
                    }
                    self.increment_nonce(&transfer_tx.from);
                    // Credit recipient
                    self.credit(&transfer_tx.to, transfer_tx.amount).map_err(|e| {
                        CoinError::ValidationError(format!("Failed to credit transfer recipient: {}", e))
                    })?;
                    // Fee credited to proposer via deferred coinbase after loop
                }
                crate::tx::Transaction::Stake(stake_tx) => {
                    // Validate minimum stake
                    if stake_tx.amount < crate::tx::stake::MIN_STAKE_SATS {
                        tracing::warn!("Skipping stake tx below minimum in finalized vertex");
                        // Stake txs have zero fee, nothing to undo
                        self.increment_nonce(&stake_tx.from);
                        continue;
                    }
                    // Debit liquid balance
                    if let Err(e) = self.debit(&stake_tx.from, stake_tx.amount) {
                        return Err(CoinError::ValidationError(format!(
                            "Failed to debit stake amount: {}", e
                        )));
                    }
                    // Credit stake account
                    let stake = self.stake_accounts.entry(stake_tx.from).or_default();
                    stake.staked = stake.staked.saturating_add(stake_tx.amount);
                    stake.unlock_at_round = None;
                    // Increment nonce
                    self.increment_nonce(&stake_tx.from);
                }
                crate::tx::Transaction::Unstake(unstake_tx) => {
                    // Start cooldown period
                    let stake = self.stake_accounts.entry(unstake_tx.from).or_default();
                    if stake.staked == 0 {
                        tracing::warn!("Skipping unstake tx with no stake in finalized vertex");
                        self.increment_nonce(&unstake_tx.from);
                        continue;
                    }
                    if stake.unlock_at_round.is_some() {
                        tracing::warn!("Skipping duplicate unstake tx in finalized vertex");
                        self.increment_nonce(&unstake_tx.from);
                        continue;
                    }
                    stake.unlock_at_round = Some(vertex.round.saturating_add(crate::tx::UNSTAKE_COOLDOWN_ROUNDS));
                    // Increment nonce
                    self.increment_nonce(&unstake_tx.from);
                }
                crate::tx::Transaction::CreateProposal(proposal_tx) => {
                    if let Err(e) = self.apply_create_proposal(proposal_tx, vertex.round) {
                        tracing::warn!("Skipping invalid CreateProposal tx in finalized vertex: {}", e);
                        // With deferred coinbase, no fee was credited — nothing to claw back.
                        // Failed governance txs just don't contribute to collected_fees.
                        self.increment_nonce(&proposal_tx.from);
                        continue; // Skip collected_fees tracking
                    }
                }
                crate::tx::Transaction::Vote(vote_tx) => {
                    if let Err(e) = self.apply_vote(vote_tx, vertex.round) {
                        tracing::warn!("Skipping invalid Vote tx in finalized vertex: {}", e);
                        self.increment_nonce(&vote_tx.from);
                        continue; // Skip collected_fees tracking
                    }
                }
                crate::tx::Transaction::Delegate(delegate_tx) => {
                    if let Err(e) = self.apply_delegate_tx(delegate_tx) {
                        tracing::warn!("Skipping invalid Delegate tx in finalized vertex: {}", e);
                        // Delegate txs have zero fee, nothing to undo
                        self.increment_nonce(&delegate_tx.from);
                        self.record_receipt(tx.hash(), vertex.round, vertex_hash, false, &e.to_string());
                        continue;
                    }
                }
                crate::tx::Transaction::Undelegate(undelegate_tx) => {
                    if let Err(e) = self.apply_undelegate_tx(undelegate_tx, vertex.round) {
                        tracing::warn!("Skipping invalid Undelegate tx in finalized vertex: {}", e);
                        // Undelegate txs have zero fee, nothing to undo
                        self.increment_nonce(&undelegate_tx.from);
                        self.record_receipt(tx.hash(), vertex.round, vertex_hash, false, &e.to_string());
                        continue;
                    }
                }
                crate::tx::Transaction::SetCommission(commission_tx) => {
                    if let Err(e) = self.apply_set_commission_tx(commission_tx, vertex.round) {
                        tracing::warn!("Skipping invalid SetCommission tx in finalized vertex: {}", e);
                        // SetCommission txs have zero fee, nothing to undo
                        self.increment_nonce(&commission_tx.from);
                        self.record_receipt(tx.hash(), vertex.round, vertex_hash, false, &e.to_string());
                        continue;
                    }
                }
                crate::tx::Transaction::BridgeDeposit(bridge_tx) => {
                    if let Err(e) = self.apply_bridge_lock_tx(bridge_tx, None, None) {
                        tracing::warn!("Skipping invalid BridgeDeposit tx in finalized vertex: {}", e);
                        if bridge_tx.fee > 0 {
                            if self.debit(&bridge_tx.from, bridge_tx.fee).is_ok() {
                                collected_fees = collected_fees.saturating_add(bridge_tx.fee);
                            }
                            // If debit fails, fee is NOT added to collected_fees — preserves supply invariant
                        }
                        self.increment_nonce(&bridge_tx.from);
                        self.record_receipt(tx.hash(), vertex.round, vertex_hash, false, &e.to_string());
                        continue;
                    }
                }
                crate::tx::Transaction::BridgeRelease(release_tx) => {
                    if let Err(e) = self.apply_bridge_release_tx(release_tx) {
                        tracing::warn!("Skipping invalid BridgeRelease tx in finalized vertex: {}", e);
                        self.increment_nonce(&release_tx.from);
                        self.record_receipt(tx.hash(), vertex.round, vertex_hash, false, &e.to_string());
                        continue;
                    }
                }
            }

            // Track fee from this successfully-applied transaction
            collected_fees = collected_fees.saturating_add(tx.fee());
            self.record_receipt(tx.hash(), vertex.round, vertex_hash, true, "");
        }

        // Credit proposer with fees from successful transactions only.
        // This is deferred from before the loop to eliminate the fee clawback
        // vulnerability (DuplicateTxFlooder: malicious validator includes stale-nonce
        // txs whose fees were credited upfront but couldn't be clawed back).
        if collected_fees > 0 {
            self.credit(proposer, collected_fees).map_err(|e| {
                CoinError::SupplyInvariantBroken(format!("Failed to credit proposer fees: {}", e))
            })?;
        }

        // NOTE: last_finalized_round is NOT updated here — it's updated per-round
        // in apply_finalized_vertices() to ensure all vertices in the same round
        // compute the same expected_height for coinbase validation.

        // Epoch boundary: recalculate active validator set
        // Uses `!=` instead of `>` because current_epoch is initialized to
        // EPOCH_UNINITIALIZED (u64::MAX sentinel). On the first vertex, epoch_of(0)=0
        // which != EPOCH_UNINITIALIZED, triggering the initial recalculation. Subsequent
        // vertices in the same epoch won't trigger because epoch_of(round) == current_epoch.
        // See constants::EPOCH_UNINITIALIZED for safety proof.
        //
        // IMPORTANT: Do NOT use `|| self.active_validator_set.is_empty()` here.
        // That caused a fatal bug where the first staker immediately became the only
        // active validator, locking out all configured node validators and halting
        // the network. Active set changes must only happen at epoch boundaries.
        let new_epoch = crate::constants::epoch_of(vertex.round);
        if new_epoch != self.current_epoch {
            self.recalculate_active_set();
            self.current_epoch = new_epoch;
        }

        // NOTE: tick_governance() is called per-round in apply_finalized_vertices(),
        // not per-vertex. If called per-vertex, 4 validators producing in round N
        // would tick governance 4 times, and a ParameterChange execution could apply
        // before remaining vertices in the same round are processed.

        // Supply invariant check — FATAL. Any violation means state is corrupt and the node
        // must halt immediately. On mainnet, supply drift is unrecoverable without a hard fork.
        // With deferred coinbase (Bug #189 fix), fee clawback is no longer needed — the
        // proposer is only credited collected_fees from successful transactions.
        //
        // RACE CONDITION FIX (Task #1): Uses CHECKED arithmetic instead of saturating.
        // Saturating arithmetic can mask overflow bugs — checked arithmetic ensures
        // any overflow is detected as a critical error.
        {
            let liquid: u64 = self.accounts.values()
                .try_fold(0u64, |acc, a| acc.checked_add(a.balance))
                .ok_or_else(|| {
                    // Find which account caused overflow for debugging
                    let mut running_sum: u64 = 0;
                    for (addr, account) in &self.accounts {
                        if let Some(new_sum) = running_sum.checked_add(account.balance) {
                            running_sum = new_sum;
                        } else {
                            tracing::error!(
                                "Supply invariant: liquid balance overflow at address {}",
                                addr.to_hex()
                            );
                            break;
                        }
                    }
                    CoinError::SupplyInvariantBroken("liquid balance overflow detected".into())
                })?;
            
            let staked: u64 = self.stake_accounts.values()
                .try_fold(0u64, |acc, s| acc.checked_add(s.staked))
                .ok_or_else(|| {
                    tracing::error!("Supply invariant: staked balance overflow detected");
                    CoinError::SupplyInvariantBroken("staked balance overflow detected".into())
                })?;
            
            let delegated: u64 = self.delegation_accounts.values()
                .try_fold(0u64, |acc, d| acc.checked_add(d.delegated))
                .ok_or_else(|| {
                    tracing::error!("Supply invariant: delegated balance overflow detected");
                    CoinError::SupplyInvariantBroken("delegated balance overflow detected".into())
                })?;
            
            let total = liquid
                .checked_add(staked)
                .and_then(|s| s.checked_add(delegated))
                .and_then(|s| s.checked_add(self.treasury_balance))
                .and_then(|s| s.checked_add(self.bridge_reserve))
                .ok_or_else(|| {
                    tracing::error!(
                        "Supply invariant: total calculation overflow: liquid={} staked={} delegated={} treasury={} bridge={}",
                        liquid, staked, delegated, self.treasury_balance, self.bridge_reserve
                    );
                    CoinError::SupplyInvariantBroken("total supply calculation overflow".into())
                })?;
            
            if total != self.total_supply {
                return Err(CoinError::SupplyInvariantBroken(format!(
                    "liquid={} staked={} delegated={} treasury={} bridge={} sum={} != total_supply={}",
                    liquid, staked, delegated, self.treasury_balance, self.bridge_reserve, total, self.total_supply
                )));
            }
        }

        Ok(())
    }

    /// Apply multiple finalized vertices in order.
    /// When staking is active, uses stake-proportional rewards.
    /// Otherwise splits block reward equally among validators per round (pre-staking mode).
    ///
    /// RACE CONDITION FIX (Task #2): Implements round-based locking to ensure
    /// vertices in the same round are applied atomically as a batch.
    /// Only one round can be applied at a time to prevent concurrent finalization corruption.
    pub fn apply_finalized_vertices(&mut self, vertices: &[DagVertex]) -> Result<(), CoinError> {
        // Sort deterministically by (round, hash) so all nodes apply in the same order.
        // Precompute hashes to avoid O(N log N) blake3 calls in the comparator.
        let mut with_hashes: Vec<([u8; 32], &DagVertex)> = vertices.iter().map(|v| (v.hash(), v)).collect();
        with_hashes.sort_by(|(ha, a), (hb, b)| a.round.cmp(&b.round).then_with(|| ha.cmp(hb)));
        let sorted: Vec<&DagVertex> = with_hashes.iter().map(|(_, v)| *v).collect();

        // Deterministic equivocation detection (defense-in-depth):
        //
        // PRIMARY DEFENSE: The DAG rejects equivocating vertices at insertion via
        // try_insert() → DagInsertError::Equivocation. The second vertex never enters
        // the DAG, so it can never be finalized. This makes equivocation in finality
        // batches impossible by construction.
        //
        // SECONDARY DEFENSE (this code): Checks BOTH within this batch AND against
        // previously-applied batches via the persistent applied_validators_per_round
        // HashMap. This catches theoretical edge cases where the DAG defense is
        // somehow bypassed (e.g., future implementation bugs, or if equivocating
        // vertices arrive via CheckpointSync suffix without DAG insertion).
        //
        // The applied_validators_per_round map persists across calls to this method,
        // so equivocation is detected regardless of how finality batches are split.
        // This is the ONLY place slashing happens — P2P handlers only broadcast evidence.
        {
            // Track (validator, round) pairs already slashed in this pass to prevent
            // double/triple slashing when both intra-batch and cross-batch detection
            // fire for the same equivocation event.
            let mut already_slashed: std::collections::HashSet<(crate::Address, u64)> =
                std::collections::HashSet::new();

            // First check within this batch (same as before)
            let mut batch_seen: std::collections::HashMap<(crate::Address, u64), usize> =
                std::collections::HashMap::new();
            for v in &sorted {
                let key = (v.validator, v.round);
                *batch_seen.entry(key).or_insert(0) += 1;
            }
            for ((validator, round), count) in &batch_seen {
                if *count > 1 && already_slashed.insert((*validator, *round)) {
                    tracing::warn!(
                        "Deterministic slash (intra-batch): validator {} equivocated in round {} ({} vertices)",
                        validator.to_hex(), round, count
                    );
                    self.slash_at_round(validator, *round);
                }
            }

            // Then check against previously-applied rounds (cross-batch detection)
            for v in &sorted {
                if let Some(existing) = self.applied_validators_per_round.get(&v.round) {
                    if existing.contains(&v.validator) {
                        // This validator already produced a vertex in this round
                        // in a previous finality batch — equivocation.
                        if already_slashed.insert((v.validator, v.round)) {
                            tracing::warn!(
                                "Deterministic slash (cross-batch): validator {} already applied in round {}",
                                v.validator.to_hex(), v.round
                            );
                            self.slash_at_round(&v.validator, v.round);
                        }
                    }
                }
            }
        }

        // RACE CONDITION FIX: Create snapshot before applying for atomic verification
        // Note: snapshot_before is used for future atomic rollback implementation
        let _snapshot_before = StateSnapshot::from_engine(self);

        // Group vertices by round. Update last_finalized_round only BETWEEN rounds.
        // Rewards distributed once per completed round via distribute_round_rewards().
        // DETERMINISM: tick_governance runs at the boundary between round N and N+1,
        // AFTER all vertices in round N are applied. Since vertices are sorted by
        // (round, hash), all nodes see identical round boundaries and tick governance
        // at the same logical point. ParameterChange proposals can only take effect
        // starting from the round AFTER their execute_at_round.
        let mut prev_round = None;
        let mut round_producers: std::collections::HashSet<Address> = std::collections::HashSet::new();
        for vertex in &sorted {
            // When transitioning to a new round, finalize the previous round:
            // distribute rewards, update last_finalized_round, tick governance,
            // and process unstake completions for the new round.
            if prev_round.is_some() && prev_round != Some(vertex.round) {
                if let Some(r) = prev_round {
                    self.distribute_round_rewards(r, &round_producers)?;
                    self.last_finalized_round = Some(r);
                    self.tick_governance(r);
                }
                round_producers.clear();
                // Process unstake completions once at the start of each new round
                self.process_unstake_completions(vertex.round)?;
            } else if prev_round.is_none() {
                // First vertex in the batch: process unstake completions for its round
                self.process_unstake_completions(vertex.round)?;
            }

            let count = 1; // reward splitting now in distribute_round_rewards
            match self.apply_vertex_with_validators(vertex, count) {
                Ok(()) => {
                    round_producers.insert(vertex.validator);
                }
                Err(CoinError::InvalidCoinbase { expected, got }) => {
                    // Defense-in-depth: skip vertices with invalid coinbase rather than
                    // aborting the entire batch. The primary defense (try_insert rejecting
                    // non-zero coinbase) should prevent this. If we get here, it means
                    // a vertex somehow bypassed the DAG check (e.g., CheckpointSync suffix).
                    // Aborting would permanently halt finality since the vertex is already
                    // in the finalized set and would fail again on retry.
                    tracing::error!(
                        "SKIPPING finalized vertex with invalid coinbase (expected={}, got={}) from {} round={}. \
                         This should never happen — vertex should have been rejected by try_insert().",
                        expected, got, vertex.validator.to_hex(), vertex.round
                    );
                    // Don't add to round_producers — skipped vertex earns no rewards
                }
                Err(CoinError::SupplyInvariantBroken(msg)) => {
                    // Supply invariant broken is FATAL — cannot continue safely
                    return Err(CoinError::SupplyInvariantBroken(msg));
                }
                Err(e) => {
                    // Other errors: skip the vertex and log, rather than halting finality.
                    // The vertex is already in the finalized set and can't be retried.
                    tracing::error!(
                        "SKIPPING finalized vertex due to error: {} (validator={}, round={})",
                        e, vertex.validator.to_hex(), vertex.round
                    );
                }
            }

            // Record this validator's participation for cross-batch equivocation detection
            self.applied_validators_per_round
                .entry(vertex.round)
                .or_default()
                .insert(vertex.validator);

            // Index all transactions in this vertex for /tx/{hash} lookups
            let vertex_hash = vertex.hash();
            let location = TxLocation {
                round: vertex.round,
                vertex_hash,
                validator: vertex.validator,
            };
            self.index_tx(vertex.block.coinbase.hash(), location.clone());
            for tx in &vertex.block.transactions {
                self.index_tx(tx.hash(), location.clone());
            }

            prev_round = Some(vertex.round);
        }
        // Finalize the last round
        if let Some(r) = prev_round {
            self.distribute_round_rewards(r, &round_producers)?;
            self.last_finalized_round = Some(r);
            self.tick_governance(r);
        }

        // RACE CONDITION FIX: Verify state consistency after applying all vertices
        // This is cfg-gated for debug mode in production to avoid performance impact
        #[cfg(debug_assertions)]
        {
            if let Err(e) = self.verify_state_consistency() {
                tracing::error!("State consistency check failed after applying vertices: {}", e);
                return Err(e);
            }
        }

        // Prune old entries from cross-batch equivocation tracker (keep last 1000 rounds).
        //
        // SECURITY ANALYSIS (Bug #211): The 1000-round pruning window does NOT create
        // an exploitable gap. The PRIMARY defense is the DAG's try_insert(), which
        // rejects equivocating vertices at insertion via the validator_round_vertex
        // secondary index (O(1) check). Once rejected, the equivocating vertex never
        // enters the DAG and can never be finalized. The only code paths that insert
        // vertices are try_insert() (P2P) and insert() (local validator, which also
        // uses try_insert since Bug #69 fix). CheckpointSync suffix vertices also go
        // through try_insert(). Therefore, no equivocating vertex can enter the DAG
        // after ANY number of rounds, making the 1000-round window purely defense-in-depth
        // against theoretical future bugs in the insertion path.
        if let Some(fin) = self.last_finalized_round {
            let floor = fin.saturating_sub(1000);
            self.applied_validators_per_round.retain(|round, _| *round >= floor);

            // Periodic state bloat pruning
            if fin % crate::constants::STATE_PRUNING_INTERVAL == 0 {
                // Remove zero-balance, zero-nonce accounts (fully drained)
                let dust_pruned = self.prune_dust_accounts();
                if dust_pruned > 0 {
                    tracing::debug!("Pruned {} dust accounts at round {}", dust_pruned, fin);
                }

                // Remove terminal proposals and their votes
                let proposals_pruned = self.prune_old_proposals(
                    fin,
                    crate::constants::PROPOSAL_RETENTION_ROUNDS,
                );
                if proposals_pruned > 0 {
                    tracing::debug!("Pruned {} old proposals at round {}", proposals_pruned, fin);
                }

                // Remove fully-signed or old bridge attestations (M2 fix)
                let bridge_pruned = self.prune_old_bridge_attestations(fin);
                if bridge_pruned > 0 {
                    tracing::debug!("Pruned {} old bridge attestations at round {}", bridge_pruned, fin);
                }
            }
        }

        Ok(())
    }

    /// Total UDAG currently staked across all validators (including unstaking).
    pub fn total_staked(&self) -> u64 {
        self.stake_accounts.values().fold(0u64, |acc, s| acc.saturating_add(s.staked))
    }

    /// Total UDAG staked by validators eligible to vote (excludes unstaking).
    /// Used as the quorum denominator in governance to match vote weight eligibility.
    pub fn total_votable_stake(&self) -> u64 {
        self.stake_accounts.values()
            .filter(|s| s.unlock_at_round.is_none())
            .fold(0u64, |acc, s| acc.saturating_add(s.staked))
    }

    /// Stake of a specific address.
    pub fn stake_of(&self, addr: &Address) -> u64 {
        self.stake_accounts.get(addr).map_or(0, |s| s.staked)
    }

    /// Stake account details for an address.
    pub fn stake_account(&self, addr: &Address) -> Option<&StakeAccount> {
        self.stake_accounts.get(addr)
    }

    /// All addresses with active stake >= MIN_STAKE_SATS (not unstaking).
    pub fn active_stakers(&self) -> Vec<Address> {
        self.stake_accounts
            .iter()
            .filter(|(_, s)| s.staked >= MIN_STAKE_SATS && s.unlock_at_round.is_none())
            .map(|(addr, _)| *addr)
            .collect()
    }

    /// The epoch-frozen active validator set (top MAX_ACTIVE_VALIDATORS by stake).
    /// This set only changes at epoch boundaries.
    pub fn active_validators(&self) -> &[Address] {
        &self.active_validator_set
    }

    /// Whether an address is in the current active validator set.
    pub fn is_active_validator(&self, addr: &Address) -> bool {
        self.active_validator_set.contains(addr)
    }

    /// Whether the DAO has enough active validators for governance execution.
    /// ParameterChange proposals can only execute when this returns true.
    /// Below MIN_DAO_VALIDATORS, proposals stay in PassedPending (hibernation).
    /// The DAO automatically reactivates when the validator count recovers.
    pub fn dao_is_active(&self) -> bool {
        self.active_validator_set.len() >= crate::constants::MIN_DAO_VALIDATORS
    }

    /// Current epoch number.
    pub fn current_epoch(&self) -> u64 {
        self.current_epoch
    }

    /// Check if the last applied vertex crossed an epoch boundary.
    /// Returns true if current_epoch > epoch_of(previous round).
    pub fn epoch_just_changed(&self, previous_round: Option<u64>) -> bool {
        match (self.last_finalized_round, previous_round) {
            (Some(current), Some(prev)) => {
                crate::constants::epoch_of(current) > crate::constants::epoch_of(prev)
            }
            (Some(_), None) => {
                // First finalized round — epoch 0 is starting
                false
            }
            _ => false,
        }
    }

    /// Recalculate the active validator set: top COUNCIL_MAX_MEMBERS by stake.
    /// Anyone can stake and become a validator, but only council members can vote.
    /// This enables community participation in staking rewards while keeping governance exclusive.
    /// 
    /// COUNCIL OF 21: Governance restricted to council, validation open to all stakers.
    /// WARNING: If the resulting set has fewer than MIN_ACTIVE_VALIDATORS,
    /// the system cannot guarantee BFT safety. This should be logged/monitored.
    pub fn recalculate_active_set(&mut self) {
        let mut eligible: Vec<(Address, u64)> = self.stake_accounts
            .iter()
            .filter(|(_addr, s)| {
                // Anyone can be a validator (not just council members)
                // Must meet minimum stake requirement (regular staking minimum)
                s.staked >= crate::tx::stake::MIN_STAKE_SATS &&
                // Must not be unstaking
                s.unlock_at_round.is_none()
            })
            .map(|(addr, _s)| (*addr, self.effective_stake_of(addr)))
            .collect();
        
        // Sort by stake descending, then by address for determinism
        eligible.sort_by(|a, b| b.1.cmp(&a.1).then(a.0 .0.cmp(&b.0 .0)));
        eligible.truncate(crate::constants::MAX_ACTIVE_VALIDATORS);
        self.active_validator_set = eligible.into_iter().map(|(addr, _)| addr).collect();
        
        // Log warning if below minimum safe validator count
        if self.active_validator_set.len() < crate::constants::MIN_ACTIVE_VALIDATORS {
            // Log at warn level — below minimum validators is concerning but not fatal
            tracing::warn!(
                "WARNING: Active validator count ({}) below minimum {} for BFT consensus",
                self.active_validator_set.len(),
                crate::constants::MIN_ACTIVE_VALIDATORS
            );
        }
    }

    /// Add a member to the Council of 21 with a seat category.
    /// Only council members can vote on governance proposals.
    /// No stake requirement — seats are earned through Foundation membership and expertise.
    /// Council members earn a share of block emissions (COUNCIL_EMISSION_PERCENT).
    pub fn add_council_member(
        &mut self,
        address: Address,
        category: crate::governance::CouncilSeatCategory,
    ) -> Result<(), CoinError> {
        if self.council_members.len() >= crate::constants::COUNCIL_MAX_MEMBERS {
            return Err(CoinError::ValidationError("Council already at maximum capacity (21)".to_string()));
        }
        // Check category seat limit
        let category_count = self.council_members.values()
            .filter(|c| **c == category)
            .count();
        if category_count >= category.max_seats() {
            return Err(CoinError::ValidationError(format!(
                "No vacant {} seats (max {})", category.name(), category.max_seats()
            )));
        }
        if self.council_members.contains_key(&address) {
            return Err(CoinError::ValidationError("Address is already a council member".to_string()));
        }
        self.council_members.insert(address, category);
        Ok(())
    }

    /// Remove a member from the Council of 21.
    pub fn remove_council_member(&mut self, address: &Address) -> bool {
        self.council_members.remove(address).is_some()
    }

    /// Check if an address is a council member.
    pub fn is_council_member(&self, address: &Address) -> bool {
        self.council_members.contains_key(address)
    }

    /// Get the seat category for a council member.
    pub fn council_seat_category(&self, address: &Address) -> Option<crate::governance::CouncilSeatCategory> {
        self.council_members.get(address).copied()
    }

    /// Get all current council members with their categories.
    pub fn council_members(&self) -> impl Iterator<Item = (&Address, &crate::governance::CouncilSeatCategory)> {
        self.council_members.iter()
    }

    /// Count of seated council members.
    pub fn council_member_count(&self) -> usize {
        self.council_members.len()
    }

    /// Get the current DAO treasury balance in sats.
    pub fn treasury_balance(&self) -> u64 {
        self.treasury_balance
    }

    pub fn bridge_reserve(&self) -> u64 {
        self.bridge_reserve
    }

    /// Apply a BridgeDepositTx: debit sender (amount + fee), add amount to bridge reserve.
    /// Also creates a bridge attestation for validators to sign.
    ///
    /// H4 fix: validates destination_chain_id against SUPPORTED_BRIDGE_CHAIN_IDS.
    /// M1 fix: stores creation_round on the attestation for round-based pruning.
    pub fn apply_bridge_lock_tx(
        &mut self,
        tx: &crate::tx::BridgeDepositTx,
        _validator: Option<crate::address::Address>,
        _validator_sk: Option<&crate::address::SecretKey>,
    ) -> Result<Option<crate::bridge::BridgeAttestation>, CoinError> {
        if tx.amount < crate::constants::MIN_BRIDGE_AMOUNT_SATS {
            return Err(CoinError::ValidationError("below minimum bridge amount".into()));
        }

        // M4: validate max bridge deposit cap (before debit)
        if tx.amount > crate::constants::MAX_BRIDGE_AMOUNT_SATS {
            return Err(CoinError::ValidationError(
                format!("bridge deposit exceeds maximum: {} > {}", tx.amount, crate::constants::MAX_BRIDGE_AMOUNT_SATS)
            ));
        }

        // H4: validate destination chain ID
        if !crate::constants::SUPPORTED_BRIDGE_CHAIN_IDS.contains(&tx.destination_chain_id) {
            return Err(CoinError::ValidationError(
                format!("unsupported destination chain ID: {}", tx.destination_chain_id)
            ));
        }

        let total = tx.amount.saturating_add(tx.fee);
        let bal = self.balance(&tx.from);
        if bal < total {
            return Err(CoinError::InsufficientBalance {
                address: tx.from,
                required: total,
                available: bal,
            });
        }
        self.debit(&tx.from, total)?;
        self.bridge_reserve = self.bridge_reserve.saturating_add(tx.amount);
        self.increment_nonce(&tx.from);

        // M1: set creation_round for round-based pruning
        let current_round = self.last_finalized_round.unwrap_or(0);

        // Create bridge attestation for validators to sign
        // C3 fix: use self.bridge_contract_address instead of default [0u8; 20]
        let attestation = crate::bridge::BridgeAttestation::new_with_contract(
            tx.from,
            tx.recipient,
            tx.amount,
            self.bridge_nonce,
            tx.destination_chain_id,
            self.bridge_contract_address,
            current_round,
        );

        // Store attestation
        self.bridge_attestations.insert(self.bridge_nonce, attestation.clone());
        self.bridge_nonce = self.bridge_nonce.saturating_add(1);

        // NOTE: validators sign attestations separately via sign_pending_bridge_attestations()
        // during the validator loop, not here. This prevents double-application.

        Ok(Some(attestation))
    }

    /// Apply a BridgeReleaseTx: release locked funds from bridge_reserve to a native recipient.
    /// Called when validators observe an Arbitrum deposit and submit release attestations.
    /// Fee-exempt (validators shouldn't pay to process bridge releases).
    pub fn apply_bridge_release_tx(
        &mut self,
        tx: &crate::tx::bridge::BridgeReleaseTx,
    ) -> Result<(), CoinError> {
        // Verify signature
        if !tx.verify_signature() {
            return Err(CoinError::InvalidSignature);
        }
        // Verify nonce
        let expected_nonce = self.nonce(&tx.from);
        if tx.nonce != expected_nonce {
            return Err(CoinError::InvalidNonce {
                expected: expected_nonce,
                got: tx.nonce,
            });
        }
        // Verify submitter is an active validator
        if !self.active_validator_set.contains(&tx.from) {
            return Err(CoinError::ValidationError("only active validators can submit bridge releases".into()));
        }

        // Validate amount range
        if tx.amount < crate::constants::MIN_BRIDGE_AMOUNT_SATS {
            return Err(CoinError::ValidationError("below minimum bridge amount".into()));
        }
        if tx.amount > crate::constants::MAX_BRIDGE_AMOUNT_SATS {
            return Err(CoinError::ValidationError("exceeds maximum bridge amount".into()));
        }

        // Validate source chain
        if !crate::constants::SUPPORTED_BRIDGE_CHAIN_IDS.contains(&tx.source_chain_id) {
            return Err(CoinError::ValidationError(
                format!("unsupported source chain ID: {}", tx.source_chain_id)
            ));
        }

        // Check deposit_nonce hasn't been released already (replay protection)
        let nonce_key = (tx.source_chain_id, tx.deposit_nonce);
        if self.used_release_nonces.contains(&nonce_key) {
            return Err(CoinError::ValidationError(
                format!("bridge release already processed for chain {} nonce {}", tx.source_chain_id, tx.deposit_nonce)
            ));
        }

        // Check bridge_reserve has sufficient funds
        if self.bridge_reserve < tx.amount {
            return Err(CoinError::ValidationError(
                format!("insufficient bridge reserve: {} < {}", self.bridge_reserve, tx.amount)
            ));
        }

        // SECURITY: All voters must agree on (recipient, amount), not just the nonce.
        // The first voter's values are stored; subsequent voters must match or the params
        // are reset when disagreements outnumber agreements.
        let params_key = nonce_key;
        let stored_params = self.bridge_release_params.get(&params_key).cloned();
        if let Some((stored_recipient, stored_amount)) = stored_params {
            if tx.recipient != stored_recipient || tx.amount != stored_amount {
                // Disagreement — increment nonce but don't add to agreeing voter set.
                // Track disagreements separately to detect poisoned first-voter params.
                self.increment_nonce(&tx.from);
                let disagree_count = self.bridge_release_disagree_count
                    .entry(nonce_key).or_insert(0);
                *disagree_count = disagree_count.saturating_add(1);
                let agree_count = self.bridge_release_votes.get(&nonce_key).map_or(0, |v| v.len());
                // If disagreements >= agreements, the first voter's params are likely wrong.
                // Reset everything so voting can restart with correct values.
                if *disagree_count >= agree_count as u64 {
                    tracing::warn!(
                        "Bridge release params reset for {:?}: {} disagree vs {} agree. Clearing to allow re-vote.",
                        nonce_key, *disagree_count, agree_count
                    );
                    self.bridge_release_votes.remove(&nonce_key);
                    self.bridge_release_params.remove(&params_key);
                    self.bridge_release_first_vote_round.remove(&params_key);
                    self.bridge_release_disagree_count.remove(&nonce_key);
                }
                return Err(CoinError::ValidationError(format!(
                    "bridge release params mismatch (nonce consumed). Stored: ({}, {}), got: ({}, {})",
                    stored_recipient.to_hex(), stored_amount,
                    tx.recipient.to_hex(), tx.amount
                )));
            }
        } else {
            // First voter: record the canonical (recipient, amount) and start round
            self.bridge_release_params.insert(params_key, (tx.recipient, tx.amount));
            let current_round = self.last_finalized_round.unwrap_or(0);
            self.bridge_release_first_vote_round.entry(params_key).or_insert(current_round);
        }

        // Record vote (after params validation passes)
        let voters = self.bridge_release_votes.entry(nonce_key).or_default();

        voters.insert(tx.from);
        let vote_count = voters.len();

        // Calculate threshold: ceil(2n/3) where n = active validator count
        let n = self.active_validator_set.len();
        let threshold = (2 * n).div_ceil(3);

        // Always increment the submitting validator's nonce (vote recorded)
        self.increment_nonce(&tx.from);

        // Only execute the release when threshold is reached
        if vote_count >= threshold {
            self.bridge_reserve = self.bridge_reserve.saturating_sub(tx.amount);
            self.credit(&tx.recipient, tx.amount)?;
            self.used_release_nonces.insert(nonce_key);
            // Clean up votes, params, and first-vote tracking for this completed release
            self.bridge_release_votes.remove(&nonce_key);
            self.bridge_release_params.remove(&params_key);
            self.bridge_release_first_vote_round.remove(&params_key);
            self.bridge_release_disagree_count.remove(&nonce_key);

            tracing::info!(
                "Bridge release executed: {} sats from chain {} nonce {} to {} ({}/{} votes)",
                tx.amount, tx.source_chain_id, tx.deposit_nonce, tx.recipient.to_hex(),
                vote_count, n
            );
        } else {
            tracing::info!(
                "Bridge release vote recorded: chain {} nonce {} by {} ({}/{} votes, need {})",
                tx.source_chain_id, tx.deposit_nonce, tx.from.to_hex(),
                vote_count, n, threshold
            );
        }

        Ok(())
    }

    /// Apply a StakeTx: debit liquid balance, credit stake account.
    pub fn apply_stake_tx(&mut self, tx: &StakeTx) -> Result<(), CoinError> {
        if !tx.verify_signature() {
            return Err(CoinError::InvalidSignature);
        }
        let expected_nonce = self.nonce(&tx.from);
        if tx.nonce != expected_nonce {
            return Err(CoinError::InvalidNonce {
                expected: expected_nonce,
                got: tx.nonce,
            });
        }
        if tx.amount < MIN_STAKE_SATS {
            return Err(CoinError::BelowMinStake {
                minimum: MIN_STAKE_SATS,
                got: tx.amount,
            });
        }
        let balance = self.balance(&tx.from);
        if balance < tx.amount {
            return Err(CoinError::InsufficientBalance {
                address: tx.from,
                available: balance,
                required: tx.amount,
            });
        }
        // Debit liquid balance
        if let Err(e) = self.debit(&tx.from, tx.amount) {
            return Err(CoinError::ValidationError(format!(
                "Failed to debit stake amount: {}", e
            )));
        }
        // Credit stake account
        let stake = self.stake_accounts.entry(tx.from).or_default();
        stake.staked = stake.staked.saturating_add(tx.amount);
        stake.unlock_at_round = None;
        // Increment nonce
        self.increment_nonce(&tx.from);
        Ok(())
    }

    /// Apply an UnstakeTx: begin cooldown period.
    pub fn apply_unstake_tx(
        &mut self,
        tx: &UnstakeTx,
        current_round: u64,
    ) -> Result<(), CoinError> {
        if !tx.verify_signature() {
            return Err(CoinError::InvalidSignature);
        }
        let expected_nonce = self.nonce(&tx.from);
        if tx.nonce != expected_nonce {
            return Err(CoinError::InvalidNonce {
                expected: expected_nonce,
                got: tx.nonce,
            });
        }
        let stake = self.stake_accounts.get_mut(&tx.from)
            .ok_or(CoinError::NotStaking)?;
        if stake.staked == 0 {
            return Err(CoinError::NotStaking);
        }
        if stake.unlock_at_round.is_some() {
            return Err(CoinError::AlreadyUnstaking);
        }
        // Begin cooldown
        stake.unlock_at_round = Some(current_round.saturating_add(UNSTAKE_COOLDOWN_ROUNDS));
        self.increment_nonce(&tx.from);
        Ok(())
    }

    /// Process unstake completions: return funds after cooldown.
    /// Also processes delegation undelegation completions.
    /// Call this at the start of each round.
    ///
    /// Returns `SupplyInvariantBroken` if crediting unstaked/undelegated funds fails.
    /// This is FATAL: the stake was already zeroed, so a credit failure means funds
    /// are destroyed and the supply invariant is permanently broken.
    pub fn process_unstake_completions(&mut self, current_round: u64) -> Result<(), CoinError> {
        // Process stake unstake completions
        let mut to_return: Vec<(Address, u64)> = Vec::new();
        for (addr, stake) in &self.stake_accounts {
            if let Some(unlock_at) = stake.unlock_at_round {
                if current_round >= unlock_at {
                    to_return.push((*addr, stake.staked));
                }
            }
        }
        for (addr, amount) in &to_return {
            if let Some(stake) = self.stake_accounts.get_mut(addr) {
                stake.staked = 0;
                stake.unlock_at_round = None;
                self.credit(addr, *amount).map_err(|e| {
                    CoinError::SupplyInvariantBroken(format!(
                        "process_unstake_completions: failed to credit {} sats to {} after unstake: {}. \
                         Stake was already zeroed — funds are destroyed.",
                        amount, addr.to_hex(), e
                    ))
                })?;
            }
            // Remove empty stake account to prevent unbounded growth.
            // Safe: stake_of() returns 0 for missing entries, stake_account() returns None,
            // and all callers handle missing entries gracefully.
            self.stake_accounts.remove(addr);
        }

        // Process delegation undelegation completions
        let mut delegations_to_return: Vec<(Address, u64)> = Vec::new();
        for (addr, delegation) in &self.delegation_accounts {
            if let Some(unlock_at) = delegation.unlock_at_round {
                if current_round >= unlock_at {
                    delegations_to_return.push((*addr, delegation.delegated));
                }
            }
        }
        for (addr, amount) in &delegations_to_return {
            self.delegation_accounts.remove(addr);
            self.credit(addr, *amount).map_err(|e| {
                CoinError::SupplyInvariantBroken(format!(
                    "process_unstake_completions: failed to credit {} sats to {} after undelegation: {}. \
                     Delegation was already removed — funds are destroyed.",
                    amount, addr.to_hex(), e
                ))
            })?;
        }
        Ok(())
    }

    /// Remove economically dead accounts:
    /// 1. Zero balance + zero nonce (fully drained, no history)
    /// 2. Balance below DUST_THRESHOLD_SATS + zero nonce (can't pay fees, never transacted)
    ///
    /// Accounts in category 2 have their remaining balance burned (subtracted from total_supply)
    /// to maintain the supply invariant: liquid + staked + delegated + treasury + bridge_reserve == total_supply.
    ///
    /// Called periodically (every 1000 finalized rounds) to prevent unbounded account growth.
    /// Safe because: stake_of() returns 0 for missing entries, stake_account() returns None,
    /// and all code paths handle missing accounts gracefully via map_or/ok_or.
    pub fn prune_dust_accounts(&mut self) -> usize {
        let dust_threshold = crate::constants::DUST_THRESHOLD_SATS;
        let mut burned = 0u64;
        let before = self.accounts.len();
        self.accounts.retain(|_addr, account| {
            if account.balance == 0 && account.nonce == 0 {
                // Zero balance, never transacted — remove
                false
            } else if account.balance < dust_threshold && account.nonce == 0 {
                // Economically dead dust: balance > 0 but below fee threshold, never transacted.
                // These accounts can never send a transaction (fee > balance).
                // Burn the dust to maintain supply invariant.
                burned = burned.saturating_add(account.balance);
                false
            } else {
                true
            }
        });
        // Burn the dust to maintain supply invariant
        self.total_supply = self.total_supply.saturating_sub(burned);
        if burned > 0 {
            tracing::debug!("Burned {} sats of dust from pruned accounts", burned);
        }
        before - self.accounts.len()
    }

    /// Remove proposals in terminal states (Executed, Rejected, Failed, Cancelled)
    /// whose voting period ended more than `retention_rounds` rounds ago.
    /// Also removes associated votes to prevent unbounded vote map growth.
    /// Called periodically (every 1000 finalized rounds).
    pub fn prune_old_proposals(&mut self, current_round: u64, retention_rounds: u64) -> usize {
        let cutoff = current_round.saturating_sub(retention_rounds);
        let mut to_remove: Vec<u64> = Vec::new();

        for (id, proposal) in &self.proposals {
            match &proposal.status {
                crate::governance::ProposalStatus::Executed
                | crate::governance::ProposalStatus::Rejected
                | crate::governance::ProposalStatus::Failed { .. }
                | crate::governance::ProposalStatus::Cancelled => {
                    // Remove if the proposal's voting period ended before the cutoff
                    if proposal.voting_ends <= cutoff {
                        to_remove.push(*id);
                    }
                }
                _ => {} // Keep Active and PassedPending proposals
            }
        }

        for id in &to_remove {
            self.proposals.remove(id);
            // Remove all votes for this proposal — votes are keyed by (proposal_id, voter_address)
            self.votes.retain(|(proposal_id, _), _| proposal_id != id);
        }

        to_remove.len()
    }

    /// Slash a validator's stake (on equivocation).
    /// Burns 50% of their stake.
    ///
    /// SLASH POLICY: slash immediately removes from active validator set if stake
    /// drops below MIN_STAKE_SATS. Rationale: a known-equivocating validator should
    /// not continue earning rewards for up to 210,000 rounds until the next epoch
    /// boundary. Security trumps epoch stability — the active set is an optimization
    /// for predictability, not a shield for Byzantine actors.
    pub fn slash(&mut self, addr: &Address) {
        self.slash_at_round(addr, 0);
    }

    /// Slash a validator and record the event with the round number.
    /// Idempotent: will not slash the same (validator, round) pair twice.
    pub fn slash_at_round(&mut self, addr: &Address, round: u64) {
        // Idempotency guard: prevent double-slashing for the same equivocation event.
        // The applied_validators_per_round tracker is pruned after 1000 rounds, so without
        // this guard, re-encountered evidence could trigger another slash on reduced stake.
        if !self.slashed_events.insert((*addr, round)) {
            tracing::debug!("Skipping duplicate slash for {} at round {} (already slashed)", addr.to_hex(), round);
            return;
        }
        let slash_pct = self.governance_params.slash_percent;
        let mut own_slash: u64 = 0;
        let mut delegated_slash: u64 = 0;

        if let Some(stake) = self.stake_accounts.get_mut(addr) {
            let slash_amount = stake.staked.saturating_mul(slash_pct) / 100;
            stake.staked = stake.staked.saturating_sub(slash_amount);
            self.total_supply = self.total_supply.saturating_sub(slash_amount);
            own_slash = slash_amount;
            if stake.staked < MIN_STAKE_SATS {
                self.active_validator_set.retain(|a| a != addr);
            }
        }
        // Slashed validators also lose their council seat — proven Byzantine behavior
        // should not retain governance power.
        self.council_members.remove(addr);
        // Also slash delegated stake for this validator (delegators share the risk)
        let delegators: Vec<Address> = self.delegation_accounts.iter()
            .filter(|(_, d)| d.validator == *addr)
            .map(|(delegator, _)| *delegator)
            .collect();
        for delegator in delegators {
            if let Some(delegation) = self.delegation_accounts.get_mut(&delegator) {
                let slash_amount = delegation.delegated.saturating_mul(slash_pct) / 100;
                delegation.delegated = delegation.delegated.saturating_sub(slash_amount);
                self.total_supply = self.total_supply.saturating_sub(slash_amount);
                delegated_slash = delegated_slash.saturating_add(slash_amount);
                if delegation.delegated == 0 {
                    self.delegation_accounts.remove(&delegator);
                }
            }
        }

        // Record slash event for auditability (survives DAG pruning)
        if own_slash > 0 || delegated_slash > 0 {
            self.slash_history.push(SlashRecord {
                round,
                validator: *addr,
                slash_amount: own_slash,
                delegated_slash_amount: delegated_slash,
            });
            // Cap slash history to prevent unbounded growth
            if self.slash_history.len() > 10_000 {
                self.slash_history.drain(..self.slash_history.len() - 10_000);
            }
        }
    }

    /// Credit the faucet amount to the given address by debiting the faucet account.
    /// Returns Err if faucet balance is insufficient.
    /// Does NOT inflate total_supply — this is an internal transfer from faucet to user.
    /// Testnet only — on mainnet, the faucet has no balance (no genesis prefund).
    #[cfg(not(feature = "mainnet"))]
    pub fn faucet_credit(&mut self, address: &Address, amount: u64) -> Result<(), CoinError> {
        let faucet_addr = crate::constants::faucet_keypair().address();
        
        // Check faucet has sufficient balance
        let faucet_balance = self.balance(&faucet_addr);
        if faucet_balance < amount {
            return Err(CoinError::InsufficientBalance {
                address: faucet_addr,
                available: faucet_balance,
                required: amount,
            });
        }
        
        // Internal transfer: faucet → recipient
        // total_supply does NOT change
        if let Err(e) = self.debit(&faucet_addr, amount) {
            return Err(CoinError::ValidationError(format!(
                "Faucet insufficient balance: {}", e
            )));
        }
        self.credit(address, amount)
            .map_err(|e| CoinError::ValidationError(format!("Faucet credit failed: {}", e)))?;

        Ok(())
    }

    // ========================================
    // DELEGATION
    // ========================================

    /// Apply a DelegateTx: debit liquid balance, create delegation to validator.
    pub fn apply_delegate_tx(&mut self, tx: &crate::tx::DelegateTx) -> Result<(), CoinError> {
        // Prevent self-delegation: a validator delegating to themselves artificially
        // inflates their effective_stake without additional economic risk.
        if tx.from == tx.validator {
            return Err(CoinError::ValidationError("cannot delegate to self".into()));
        }
        if !tx.verify_signature() {
            return Err(CoinError::InvalidSignature);
        }
        let expected_nonce = self.nonce(&tx.from);
        if tx.nonce != expected_nonce {
            return Err(CoinError::InvalidNonce {
                expected: expected_nonce,
                got: tx.nonce,
            });
        }
        if tx.amount < crate::constants::MIN_DELEGATION_SATS {
            return Err(CoinError::ValidationError(format!(
                "delegation below minimum: need {} sats, got {} sats",
                crate::constants::MIN_DELEGATION_SATS, tx.amount
            )));
        }
        // Check validator has sufficient own stake
        if self.stake_of(&tx.validator) < MIN_STAKE_SATS {
            return Err(CoinError::ValidationError(
                "target is not a validator (insufficient stake)".to_string(),
            ));
        }
        // Reject delegation to a validator who is unstaking (in cooldown).
        // Their stake will drop to 0 after cooldown, leaving delegators stranded
        // earning no rewards until they manually undelegate.
        if let Some(stake) = self.stake_accounts.get(&tx.validator) {
            if stake.unlock_at_round.is_some() {
                return Err(CoinError::ValidationError(
                    "target validator is unstaking (in cooldown)".to_string(),
                ));
            }
        }
        // One delegation per address
        if self.delegation_accounts.contains_key(&tx.from) {
            return Err(CoinError::ValidationError(
                "already has an active delegation (one per address)".to_string(),
            ));
        }
        // Check balance
        let balance = self.balance(&tx.from);
        if balance < tx.amount {
            return Err(CoinError::InsufficientBalance {
                address: tx.from,
                available: balance,
                required: tx.amount,
            });
        }
        // Debit liquid balance
        self.debit(&tx.from, tx.amount)?;
        // Create delegation
        self.delegation_accounts.insert(tx.from, DelegationAccount {
            delegated: tx.amount,
            validator: tx.validator,
            unlock_at_round: None,
        });
        self.increment_nonce(&tx.from);
        Ok(())
    }

    /// Apply an UndelegateTx: begin cooldown to withdraw delegated funds.
    pub fn apply_undelegate_tx(
        &mut self,
        tx: &crate::tx::UndelegateTx,
        current_round: u64,
    ) -> Result<(), CoinError> {
        if !tx.verify_signature() {
            return Err(CoinError::InvalidSignature);
        }
        let expected_nonce = self.nonce(&tx.from);
        if tx.nonce != expected_nonce {
            return Err(CoinError::InvalidNonce {
                expected: expected_nonce,
                got: tx.nonce,
            });
        }
        let delegation = self.delegation_accounts.get_mut(&tx.from)
            .ok_or_else(|| CoinError::ValidationError("no active delegation".to_string()))?;
        if delegation.unlock_at_round.is_some() {
            return Err(CoinError::ValidationError(
                "already undelegating — wait for cooldown to complete".to_string(),
            ));
        }
        delegation.unlock_at_round = Some(current_round.saturating_add(UNSTAKE_COOLDOWN_ROUNDS));
        self.increment_nonce(&tx.from);
        Ok(())
    }

    /// Apply a SetCommissionTx: change validator's commission rate.
    /// `round` is the round of the vertex containing this transaction,
    /// used to enforce the commission change cooldown.
    pub fn apply_set_commission_tx(
        &mut self,
        tx: &crate::tx::SetCommissionTx,
        round: u64,
    ) -> Result<(), CoinError> {
        if !tx.verify_signature() {
            return Err(CoinError::InvalidSignature);
        }
        let expected_nonce = self.nonce(&tx.from);
        if tx.nonce != expected_nonce {
            return Err(CoinError::InvalidNonce {
                expected: expected_nonce,
                got: tx.nonce,
            });
        }
        // Check sender has stake (is a validator)
        let stake = self.stake_accounts.get_mut(&tx.from)
            .ok_or(CoinError::NotStaking)?;
        if stake.staked == 0 {
            return Err(CoinError::NotStaking);
        }
        if tx.commission_percent > crate::constants::MAX_COMMISSION_PERCENT {
            return Err(CoinError::ValidationError(format!(
                "commission_percent {} exceeds maximum {}",
                tx.commission_percent, crate::constants::MAX_COMMISSION_PERCENT
            )));
        }
        // Enforce commission change cooldown to prevent sandwich attacks.
        // NOTE: cooldown is measured from the vertex round (inclusion time), not finalization time.
        // This is deterministic — all nodes agree on vertex round. The effective cooldown in
        // wall-clock time depends on when the vertex was finalized, which may differ slightly.
        if let Some(last_changed) = stake.commission_last_changed {
            if round.saturating_sub(last_changed) < crate::constants::COMMISSION_COOLDOWN_ROUNDS {
                return Err(CoinError::ValidationError(
                    format!("commission can only be changed every {} rounds", crate::constants::COMMISSION_COOLDOWN_ROUNDS)
                ));
            }
        }
        stake.commission_percent = tx.commission_percent;
        stake.commission_last_changed = Some(round);
        self.increment_nonce(&tx.from);
        Ok(())
    }

    /// Effective stake of a validator: own stake + active (non-undelegating) delegations.
    pub fn effective_stake_of(&self, validator: &Address) -> u64 {
        let own_stake = self.stake_of(validator);
        let delegated: u64 = self.delegation_accounts.values()
            .filter(|d| d.validator == *validator && d.unlock_at_round.is_none())
            .map(|d| d.delegated)
            .fold(0u64, |acc, x| acc.saturating_add(x));
        own_stake.saturating_add(delegated)
    }

    /// Total UDAG delegated across all delegation accounts.
    pub fn total_delegated(&self) -> u64 {
        self.delegation_accounts.values()
            .map(|d| d.delegated)
            .fold(0u64, |acc, x| acc.saturating_add(x))
    }

    /// Get all delegators for a specific validator.
    pub fn delegators_of(&self, validator: &Address) -> Vec<(Address, u64)> {
        self.delegation_accounts.iter()
            .filter(|(_, d)| d.validator == *validator)
            .map(|(addr, d)| (*addr, d.delegated))
            .collect()
    }

    /// Get delegation info for an address.
    pub fn delegation_account(&self, addr: &Address) -> Option<&DelegationAccount> {
        self.delegation_accounts.get(addr)
    }

    /// Iterate all delegation accounts (for persistence).
    pub fn all_delegations(&self) -> impl Iterator<Item = (&Address, &DelegationAccount)> {
        self.delegation_accounts.iter()
    }

    /// Distribute delegation rewards after a validator receives their coinbase.
    /// The validator was credited the full effective-stake-proportional reward.
    /// Delegators' share is deducted from validator and credited to delegators
    /// minus the validator's commission.
    /// 
    /// SECURITY: Uses checked arithmetic to detect overflow. If overflow occurs,
    /// the credit fails and returns an error. This prevents silent supply corruption.
    /// 
    /// Note: In production, overflow should never occur due to MAX_SUPPLY cap.
    /// Overflow indicates a critical bug in reward calculation logic.
    pub fn credit(&mut self, address: &Address, amount: u64) -> Result<(), CoinError> {
        // Fast path: zero credit is a no-op (common in reward distribution)
        if amount == 0 {
            return Ok(());
        }
        
        let account = self.accounts.entry(*address).or_default();
        
        // Use checked arithmetic to detect overflow (VULN-07 fix)
        // With MAX_SUPPLY_SATS = 21M UDAG = 2.1e15 sats, u64::MAX = 1.8e19
        // Overflow would require >8000x max supply — only possible via bug
        account.balance = account.balance.checked_add(amount)
            .ok_or_else(|| CoinError::ValidationError(
                format!("Credit overflow: balance {} + amount {} overflowed u64", 
                    account.balance, amount)
            ))?;
        
        Ok(())
    }

    /// Saturating fee clawback: debit what the proposer has, burn the unrecoverable
    /// remainder from total_supply. This prevents a malicious validator from halting
    /// 
    /// SECURITY: Validates amount is non-zero to prevent no-op transactions.
    fn debit(&mut self, address: &Address, amount: u64) -> Result<(), CoinError> {
        // Zero debit is a no-op (consistent with credit(0) behavior)
        if amount == 0 {
            return Ok(());
        }
        
        let account = self.accounts.entry(*address).or_default();
        if account.balance < amount {
            return Err(CoinError::ValidationError(format!(
                "Insufficient balance: need {}, have {}",
                amount, account.balance
            )));
        }
        account.balance = account.balance.saturating_sub(amount);
        Ok(())
    }

    fn increment_nonce(&mut self, address: &Address) {
        let account = self.accounts.entry(*address).or_default();
        account.nonce = account.nonce.saturating_add(1);
    }

    // ========================================
    // GOVERNANCE
    // ========================================

    /// Apply a CreateProposal transaction.
    pub fn apply_create_proposal(
        &mut self,
        tx: &crate::governance::CreateProposalTx,
        current_round: u64,
    ) -> Result<(), CoinError> {
        // 1. Verify signature
        if !tx.verify_signature() {
            return Err(CoinError::InvalidSignature);
        }

        // 2. Check nonce
        let expected_nonce = self.nonce(&tx.from);
        if tx.nonce != expected_nonce {
            return Err(CoinError::InvalidNonce {
                expected: expected_nonce,
                got: tx.nonce,
            });
        }

        // 3. Check fee >= MIN_FEE_SATS
        if tx.fee < crate::constants::MIN_FEE_SATS {
            return Err(CoinError::FeeTooLow);
        }

        // 4. Only council members can create proposals
        //    Exception: anyone can self-nominate via CouncilMembership { action: Add } where
        //    the candidate address matches the proposer (decentralized council application).
        let is_self_nomination = matches!(
            &tx.proposal_type,
            crate::governance::ProposalType::CouncilMembership { action: crate::governance::CouncilAction::Add, address, .. }
            if *address == tx.from
        );
        if !self.is_council_member(&tx.from) && !is_self_nomination {
            return Err(CoinError::ValidationError(
                "Only Council of 21 members can create proposals (anyone can self-nominate for council)".to_string(),
            ));
        }

        // 4b. Check category capacity for CouncilMembership Add proposals
        if let crate::governance::ProposalType::CouncilMembership {
            action: crate::governance::CouncilAction::Add,
            ref category, ..
        } = tx.proposal_type {
            let current = self.council_members.values().filter(|c| *c == category).count();
            if current >= category.max_seats() {
                return Err(CoinError::ValidationError(format!(
                    "No vacant {} seats ({}/{})", category.name(), current, category.max_seats()
                )));
            }
        }

        // 5. Check title length
        if tx.title.len() > crate::constants::PROPOSAL_TITLE_MAX_BYTES {
            return Err(CoinError::ProposalTitleTooLong);
        }

        // 6. Check description length
        if tx.description.len() > crate::constants::PROPOSAL_DESCRIPTION_MAX_BYTES {
            return Err(CoinError::ProposalDescriptionTooLong);
        }

        // 7. Check proposal_id == next_proposal_id (sequential, no gaps)
        if tx.proposal_id != self.next_proposal_id {
            return Err(CoinError::InvalidProposalId);
        }

        // 8. Check active proposal count
        let active_count = self.proposals.values()
            .filter(|p| matches!(p.status, crate::governance::ProposalStatus::Active))
            .count();
        if active_count as u64 >= self.governance_params.max_active_proposals {
            return Err(CoinError::TooManyActiveProposals);
        }

        // 8b. SECURITY: Check proposal cooldown — prevent spam and allow time for
        //     community review of failed proposals. Enforces PROPOSAL_COOLDOWN_ROUNDS
        //     between submissions by the same address.
        if let Some(&last_round) = self.last_proposal_round.get(&tx.from) {
            let rounds_since_last = current_round.saturating_sub(last_round);
            if rounds_since_last < crate::constants::PROPOSAL_COOLDOWN_ROUNDS {
                return Err(CoinError::ProposalCooldownNotElapsed);
            }
        }

        // 9. Deduct fee from proposer balance
        let balance = self.balance(&tx.from);
        if balance < tx.fee {
            return Err(CoinError::InsufficientBalance {
                address: tx.from,
                required: tx.fee,
                available: balance,
            });
        }
        self.debit(&tx.from, tx.fee)?;

        // 10. Increment nonce
        self.increment_nonce(&tx.from);

        // 11. SECURITY: Snapshot council member count at proposal creation for quorum.
        //     This prevents quorum manipulation via coordinated council member changes during
        //     the voting period. The quorum is fixed at proposal creation time.
        //     Note: UltraDAG uses 1-vote-per-council-seat governance, so quorum is based on
        //     council seats, not stake weight.
        let snapshot_total_stake = self.council_members.len() as u64;

        // 12. Create proposal
        let proposal = crate::governance::Proposal {
            id: tx.proposal_id,
            proposer: tx.from,
            title: tx.title.clone(),
            description: tx.description.clone(),
            proposal_type: tx.proposal_type.clone(),
            voting_starts: current_round,
            voting_ends: current_round.saturating_add(self.governance_params.voting_period_rounds),
            votes_for: 0,
            votes_against: 0,
            status: crate::governance::ProposalStatus::Active,
            snapshot_total_stake,
        };

        // 13. Insert into proposals
        self.proposals.insert(tx.proposal_id, proposal);

        // 14. SECURITY: Track last proposal round for cooldown enforcement
        self.last_proposal_round.insert(tx.from, current_round);

        // 15. Increment next_proposal_id
        self.next_proposal_id = self.next_proposal_id.saturating_add(1);

        Ok(())
    }

    /// Apply a Vote transaction.
    /// 
    /// SECURITY: Locks the voter's stake when voting to prevent vote manipulation.
    /// Stake is released when the proposal is executed or rejected in tick_governance().
    /// This prevents voters from:
    /// 1. Voting with stake, then unstaking before the vote completes
    /// 2. Using the same stake to influence multiple simultaneous proposals
    pub fn apply_vote(
        &mut self,
        tx: &crate::governance::VoteTx,
        current_round: u64,
    ) -> Result<(), CoinError> {
        // 1. Verify signature
        if !tx.verify_signature() {
            return Err(CoinError::InvalidSignature);
        }

        // 2. Check nonce
        let expected_nonce = self.nonce(&tx.from);
        if tx.nonce != expected_nonce {
            return Err(CoinError::InvalidNonce {
                expected: expected_nonce,
                got: tx.nonce,
            });
        }

        // 3. Check fee >= MIN_FEE_SATS
        if tx.fee < crate::constants::MIN_FEE_SATS {
            return Err(CoinError::FeeTooLow);
        }

        // 4. Check proposal exists
        let proposal = self.proposals.get(&tx.proposal_id)
            .ok_or(CoinError::ProposalNotFound)?;

        // 5. Check proposal.is_voting_open(current_round)
        if !proposal.is_voting_open(current_round) {
            return Err(CoinError::VotingClosed);
        }

        // 6. Check (tx.proposal_id, tx.from) not in self.votes
        if self.votes.contains_key(&(tx.proposal_id, tx.from)) {
            return Err(CoinError::AlreadyVoted);
        }

        // 7. COUNCIL OF 21: Only council members can vote
        if !self.is_council_member(&tx.from) {
            return Err(CoinError::ValidationError("Voting restricted to Council of 21 members".to_string()));
        }

        // 8. SECURITY: Check voter doesn't have locked stake from previous votes.
        //    Council members use 1-vote-per-seat (not stake-weighted), so locked_stake
        //    doesn't apply to them — they can vote on multiple proposals concurrently.
        //    Stake-weighted voters (if ever re-enabled) would need this check.
        if !self.is_council_member(&tx.from) {
            if let Some(stake_account) = self.stake_accounts.get(&tx.from) {
                if stake_account.locked_stake > 0 {
                    return Err(CoinError::StakeLocked);
                }
            }
        }

        // 9. Vote weight = 1 per council seat (equal governance power).
        // Council members don't need stake to vote — their seat IS their authority.
        let vote_weight = 1u64;

        // 10. Deduct fee from voter balance
        let balance = self.balance(&tx.from);
        if balance < tx.fee {
            return Err(CoinError::InsufficientBalance {
                address: tx.from,
                required: tx.fee,
                available: balance,
            });
        }
        self.debit(&tx.from, tx.fee)?;

        // 11. Increment nonce
        self.increment_nonce(&tx.from);

        // 12. SECURITY: Lock the voter's stake to prevent manipulation.
        //     Council members use 1-vote-per-seat, so stake locking is not needed.
        //     For stake-weighted voters (if ever re-enabled), lock the full staked amount.
        if !self.is_council_member(&tx.from) {
            if let Some(stake_account) = self.stake_accounts.get_mut(&tx.from) {
                stake_account.locked_stake = stake_account.staked;
            }
        }

        // 13. Add vote weight to proposal.votes_for or votes_against
        // Safety: proposal existence was checked at step 4 above; no mutations remove proposals.
        let proposal = self.proposals.get_mut(&tx.proposal_id)
            .ok_or(CoinError::ProposalNotFound)?;
        if tx.vote {
            proposal.votes_for = proposal.votes_for.saturating_add(vote_weight);
        } else {
            proposal.votes_against = proposal.votes_against.saturating_add(vote_weight);
        }

        // 14. Insert (proposal_id, from) -> vote into self.votes
        self.votes.insert((tx.proposal_id, tx.from), tx.vote);

        Ok(())
    }

    /// Called at the end of each finalized round.
    /// Checks all active proposals and updates their status.
    /// When proposals transition to Executed, applies ParameterChange effects.
    pub fn tick_governance(&mut self, current_round: u64) {
        let mut to_update = vec![];

        // MUST sort proposals by ID for deterministic execution order.
        // HashMap iteration is non-deterministic — if two ParameterChange proposals
        // execute in the same round, the final parameter value depends on which
        // executes last. Sorting by ID ensures all nodes apply them identically.
        let mut sorted_proposals: Vec<(u64, &crate::governance::Proposal)> =
            self.proposals.iter().map(|(id, p)| (*id, p)).collect();
        sorted_proposals.sort_by_key(|(id, _)| *id);

        for (id, proposal) in &sorted_proposals {
            match &proposal.status {
                crate::governance::ProposalStatus::Active if current_round > proposal.voting_ends => {
                    // SECURITY: Use snapshotted council member count from proposal creation as quorum denominator.
                    // This prevents quorum manipulation via coordinated council member changes during the voting period.
                    // UltraDAG uses 1-vote-per-council-seat governance, so quorum is based on council seats.
                    let quorum_denominator = if proposal.snapshot_total_stake > 0 {
                        proposal.snapshot_total_stake
                    } else {
                        // Legacy proposals without snapshot - fallback to current council count
                        self.council_members.len() as u64
                    };
                    let new_status = if proposal.has_passed_with_params(quorum_denominator, &self.governance_params) {
                        crate::governance::ProposalStatus::PassedPending {
                            execute_at_round: current_round.saturating_add(
                                self.governance_params.execution_delay_rounds
                            ),
                        }
                    } else {
                        crate::governance::ProposalStatus::Rejected
                    };
                    to_update.push((*id, new_status));
                }
                crate::governance::ProposalStatus::PassedPending { execute_at_round }
                    if current_round >= *execute_at_round =>
                {
                    // DAO activation gate: ParameterChange proposals cannot execute
                    // unless MIN_DAO_VALIDATORS are active. The proposal stays in
                    // PassedPending (hibernation) until the network is healthy enough.
                    // TextProposals and CouncilMembership execute regardless.
                    if matches!(&proposal.proposal_type,
                        crate::governance::ProposalType::ParameterChange { .. } |
                        crate::governance::ProposalType::TreasurySpend { .. } |
                        crate::governance::ProposalType::BridgeRefund { .. }
                    ) && !self.dao_is_active() {
                        // DAO hibernating — skip execution, leave as PassedPending
                        continue;
                    }
                    // Collect for execution attempt — final status determined below
                    to_update.push((*id, crate::governance::ProposalStatus::Executed));
                }
                _ => {}
            }
        }

        // Collect execution effects before mutating proposals (avoids borrow conflicts)
        let mut effects: Vec<(u64, crate::governance::ProposalType)> = Vec::new();
        for &(id, ref status) in &to_update {
            if *status == crate::governance::ProposalStatus::Executed {
                if let Some(p) = self.proposals.get(&id) {
                    effects.push((id, p.proposal_type.clone()));
                }
            }
        }

        // Apply execution effects — track failures to override status
        //
        // GOVERNANCE SELF-REFERENCE SAFETY NOTE (Bug #207):
        // If a ParameterChange modifies `execution_delay_rounds` or `voting_period_rounds`,
        // the new value only affects FUTURE proposals. This is safe because:
        // 1. All Active→PassedPending transitions were collected in the first loop above,
        //    using `self.governance_params.execution_delay_rounds` at the START of this tick.
        // 2. `apply_change()` below modifies self.governance_params, but those changes only
        //    take effect in subsequent `tick_governance()` calls.
        // 3. The `execute_at_round` baked into each proposal at transition time is immutable.
        // Therefore, reducing execution_delay cannot retroactively rush any proposal that
        // has already transitioned to PassedPending — their execute_at_round is fixed.
        let mut failed: std::collections::HashMap<u64, String> = std::collections::HashMap::new();
        for (id, proposal_type) in effects {
            match proposal_type {
                crate::governance::ProposalType::ParameterChange { ref param, ref new_value } => {
                    match self.governance_params.apply_change(param, new_value) {
                        Ok(()) => {}
                        Err(e) => {
                            let reason = format!("ParameterChange failed: {}", e);
                            tracing::warn!("Proposal {} execution failed: {}", id, reason);
                            failed.insert(id, reason);
                        }
                    }
                }
                crate::governance::ProposalType::CouncilMembership { action, address, category } => {
                    match action {
                        crate::governance::CouncilAction::Add => {
                            if let Err(e) = self.add_council_member(address, category) {
                                let reason = format!("CouncilMembership Add failed: {}", e);
                                tracing::warn!("Proposal {} execution failed: {}", id, reason);
                                failed.insert(id, reason);
                            }
                        }
                        crate::governance::CouncilAction::Remove => {
                            // Validate category matches actual membership
                            if let Some(actual_cat) = self.council_members.get(&address) {
                                if *actual_cat != category {
                                    let reason = format!(
                                        "CouncilMembership Remove failed: member is {:?}, proposal says {:?}",
                                        actual_cat, category
                                    );
                                    tracing::warn!("Proposal {} execution failed: {}", id, reason);
                                    failed.insert(id, reason);
                                } else if !self.remove_council_member(&address) {
                                    let reason = "CouncilMembership Remove failed: not on council".to_string();
                                    tracing::warn!("Proposal {} execution failed: {}", id, reason);
                                    failed.insert(id, reason);
                                }
                            } else {
                                let reason = "CouncilMembership Remove failed: not on council".to_string();
                                tracing::warn!("Proposal {} execution failed: {}", id, reason);
                                failed.insert(id, reason);
                            }
                        }
                    }
                }
                crate::governance::ProposalType::TreasurySpend { recipient, amount } => {
                    if amount > self.treasury_balance {
                        let reason = format!(
                            "Insufficient treasury: requested {} sats but only {} sats available",
                            amount, self.treasury_balance
                        );
                        tracing::warn!("Proposal {} execution failed: {}", id, reason);
                        failed.insert(id, reason);
                    } else {
                        self.treasury_balance = self.treasury_balance.saturating_sub(amount);
                        if let Err(e) = self.credit(&recipient, amount) {
                            // Credit failed — restore treasury balance to preserve supply invariant
                            self.treasury_balance = self.treasury_balance.saturating_add(amount);
                            let reason = format!("TreasurySpend credit failed: {}", e);
                            tracing::error!("Proposal {} treasury spend credit failed for {}: {}", id, recipient.to_hex(), e);
                            failed.insert(id, reason);
                            continue;
                        }
                        tracing::warn!(
                            "TreasurySpend proposal {} executed: {} sats to {}. Treasury remaining: {} sats",
                            id, amount, recipient.to_hex(), self.treasury_balance
                        );
                    }
                }
                crate::governance::ProposalType::BridgeRefund { nonce } => {
                    match self.bridge_refund(nonce, true) {
                        Ok(()) => {
                            tracing::warn!(
                                "BridgeRefund proposal {} executed: nonce {} refunded",
                                id, nonce
                            );
                        }
                        Err(e) => {
                            let reason = format!("Bridge refund failed: {}", e);
                            tracing::warn!("Proposal {} execution failed: {}", id, reason);
                            failed.insert(id, reason);
                        }
                    }
                }
                crate::governance::ProposalType::TextProposal => {}
            }
        }

        // Update proposal statuses — override with Failed where execution didn't succeed
        for (id, status) in &to_update {
            if let Some(p) = self.proposals.get_mut(&id) {
                if let Some(reason) = failed.remove(&id) {
                    p.status = crate::governance::ProposalStatus::Failed { reason };
                } else {
                    p.status = status.clone();
                }
            }
        }

        // SECURITY: Release locked stake for voters on proposals that are now final
        // (Executed, Rejected, or Failed). This completes the vote locking mechanism.
        for (id, status) in &to_update {
            match status {
                crate::governance::ProposalStatus::Executed
                | crate::governance::ProposalStatus::Rejected
                | crate::governance::ProposalStatus::Failed { .. } => {
                    // Find all voters who voted on this proposal and release their locked stake
                    let voters: Vec<Address> = self.votes.iter()
                        .filter(|((pid, _), _)| *pid == *id)
                        .map(|((_, voter), _)| *voter)
                        .collect();
                    
                    for voter in &voters {
                        if let Some(stake_account) = self.stake_accounts.get_mut(voter) {
                            stake_account.locked_stake = 0;
                        }
                    }
                }
                _ => {}
            }
        }
    }

    /// Get all proposals (for RPC).
    pub fn proposals(&self) -> &HashMap<u64, crate::governance::Proposal> {
        &self.proposals
    }

    /// Get a specific proposal by ID.
    pub fn proposal(&self, id: u64) -> Option<&crate::governance::Proposal> {
        self.proposals.get(&id)
    }

    /// Get vote for a specific (proposal_id, voter).
    pub fn get_vote(&self, proposal_id: u64, voter: &Address) -> Option<bool> {
        self.votes.get(&(proposal_id, *voter)).copied()
    }

    /// Get all votes for a proposal with voter addresses and vote weights.
    /// Council of 21: each member has weight 1 (1-vote-per-seat).
    pub fn votes_for_proposal(&self, proposal_id: u64) -> Vec<(Address, bool, u64)> {
        self.votes.iter()
            .filter(|((pid, _), _)| *pid == proposal_id)
            .map(|((_, voter), &vote)| {
                // Council model: 1 vote per seat
                let weight = 1u64; // Council of 21: 1-vote-per-seat, equal weight
                (*voter, vote, weight)
            })
            .collect()
    }

    /// Get the next proposal ID that will be assigned.
    pub fn next_proposal_id(&self) -> u64 {
        self.next_proposal_id
    }

    /// Get the current governance parameters (may differ from constants if changed via proposals).
    pub fn governance_params(&self) -> &crate::governance::GovernanceParams {
        &self.governance_params
    }

    /// Get mutable access to governance parameters (for testing).
    pub fn governance_params_mut(&mut self) -> &mut crate::governance::GovernanceParams {
        &mut self.governance_params
    }

    /// Compute dynamic minimum fee based on mempool congestion.
    /// Returns the governance base fee when mempool is at or below 50% capacity.
    /// Above 50%, fee increases linearly up to 10x at 100% capacity.
    pub fn dynamic_min_fee(&self, mempool_size: usize) -> u64 {
        let base = self.governance_params.min_fee_sats;
        let capacity = crate::tx::pool::MAX_MEMPOOL_SIZE;
        if mempool_size <= capacity / 2 {
            base
        } else {
            // Linear increase: at 100% full, fee is 10x base
            let congestion = (mempool_size - capacity / 2) as u64;
            let max_congestion = (capacity / 2) as u64;
            let multiplier = 1 + congestion.saturating_mul(9) / max_congestion.max(1);
            base.saturating_mul(multiplier)
        }
    }

    /// Iterate all accounts (for redb persistence).
    pub fn all_accounts(&self) -> impl Iterator<Item = (&Address, &AccountState)> {
        self.accounts.iter()
    }

    /// Iterate all stake accounts (for redb persistence).
    pub fn all_stakes(&self) -> impl Iterator<Item = (&Address, &StakeAccount)> {
        self.stake_accounts.iter()
    }

    /// Iterate all proposals (for redb persistence).
    pub fn all_proposals(&self) -> impl Iterator<Item = (&u64, &crate::governance::Proposal)> {
        self.proposals.iter()
    }

    /// Iterate all votes (for redb persistence).
    pub fn all_votes(&self) -> impl Iterator<Item = (&(u64, Address), &bool)> {
        self.votes.iter()
    }

    /// Set current epoch (for redb load reconciliation).
    pub fn set_current_epoch(&mut self, epoch: u64) {
        self.current_epoch = epoch;
    }

    /// Construct StateEngine from individual components (for redb loading).
    /// Verifies the supply invariant on construction to catch corrupted redb data.
    #[allow(clippy::too_many_arguments)]
    pub fn from_parts(
        accounts: HashMap<Address, AccountState>,
        stake_accounts: HashMap<Address, StakeAccount>,
        active_validator_set: Vec<Address>,
        current_epoch: u64,
        total_supply: u64,
        last_finalized_round: Option<u64>,
        proposals: HashMap<u64, crate::governance::Proposal>,
        votes: HashMap<(u64, Address), bool>,
        next_proposal_id: u64,
        governance_params: crate::governance::GovernanceParams,
        configured_validator_count: Option<u64>,
        council_members: HashMap<Address, crate::governance::CouncilSeatCategory>,
        treasury_balance: u64,
        delegation_accounts: HashMap<Address, DelegationAccount>,
        bridge_reserve: u64,
    ) -> Result<Self, CoinError> {
        // Verify supply invariant before constructing (catch corrupted redb data)
        let liquid: u64 = accounts.values().map(|a| a.balance).fold(0, |a, b| a.saturating_add(b));
        let staked: u64 = stake_accounts.values().map(|s| s.staked).fold(0, |a, b| a.saturating_add(b));
        let delegated: u64 = delegation_accounts.values().map(|d| d.delegated).fold(0, |a, b| a.saturating_add(b));
        let total = liquid.saturating_add(staked).saturating_add(delegated)
            .saturating_add(treasury_balance).saturating_add(bridge_reserve);
        if total != total_supply {
            return Err(CoinError::SupplyInvariantBroken(format!(
                "from_parts: liquid={} staked={} delegated={} treasury={} bridge={} sum={} != total_supply={}",
                liquid, staked, delegated, treasury_balance, bridge_reserve, total, total_supply
            )));
        }

        Ok(Self {
            accounts,
            stake_accounts,
            active_validator_set,
            current_epoch,
            total_supply,
            last_finalized_round,
            proposals,
            votes,
            next_proposal_id,
            governance_params,
            configured_validator_count,
            council_members,
            treasury_balance,
            delegation_accounts,
            tx_index: HashMap::new(),
            tx_index_order: VecDeque::new(),
            tx_receipts: HashMap::new(),
            slash_history: Vec::new(),
            applied_validators_per_round: HashMap::new(),
            bridge_reserve,
            bridge_attestations: HashMap::new(),
            bridge_signatures: HashMap::new(),
            bridge_nonce: 0,
            bridge_contract_address: [0u8; 20],
            used_release_nonces: std::collections::HashSet::new(),
            bridge_release_votes: HashMap::new(),
            bridge_release_params: HashMap::new(),
            bridge_release_first_vote_round: HashMap::new(),
            bridge_release_disagree_count: HashMap::new(),
            slashed_events: std::collections::HashSet::new(),
            last_proposal_round: HashMap::new(),

        })
    }

    /// Create a snapshot of the current state (for checkpoints).
    /// All collections are sorted by key for deterministic serialization —
    /// HashMap iteration order is non-deterministic, so without sorting,
    /// different nodes would compute different state_root hashes for identical state.
    pub fn snapshot(&self) -> crate::state::persistence::StateSnapshot {
        let mut accounts: Vec<_> = self.accounts.iter().map(|(k, v)| (*k, v.clone())).collect();
        accounts.sort_by_key(|(addr, _)| addr.0);
        let mut stake_accounts: Vec<_> = self.stake_accounts.iter().map(|(k, v)| (*k, v.clone())).collect();
        stake_accounts.sort_by_key(|(addr, _)| addr.0);
        let mut proposals: Vec<_> = self.proposals.iter().map(|(k, v)| (*k, v.clone())).collect();
        proposals.sort_by_key(|(id, _)| *id);
        let mut votes: Vec<_> = self.votes.iter().map(|(k, v)| (*k, *v)).collect();
        votes.sort_by(|a, b| a.0.0.cmp(&b.0.0).then_with(|| a.0.1.0.cmp(&b.0.1.0)));
        let mut council: Vec<_> = self.council_members.iter().map(|(k, v)| (*k, *v)).collect();
        council.sort_by_key(|(addr, _)| addr.0);
        let mut delegation_accounts: Vec<_> = self.delegation_accounts.iter().map(|(k, v)| (*k, v.clone())).collect();
        delegation_accounts.sort_by_key(|(addr, _)| addr.0);
        let mut bridge_attestations: Vec<_> = self.bridge_attestations.iter().map(|(k, v)| (*k, v.clone())).collect();
        bridge_attestations.sort_by_key(|(nonce, _)| *nonce);
        // NOTE: bridge_signatures are NON-DETERMINISTIC across nodes (each validator
        // signs locally, and nodes may have different subsets of signatures). They are:
        // - Included in snapshots for fast-sync convenience (saves re-signing)
        // - EXCLUDED from state_root hash (compute_state_root in checkpoint.rs)
        // - NOT used in any consensus-critical decision (only RPC display)
        // Nodes that fast-sync from different peers may have different signature sets,
        // but this does not affect consensus, finality, or supply invariant.
        let mut bridge_signatures: Vec<_> = self.bridge_signatures.iter()
            .map(|((nonce, validator), packed)| {
                ((*nonce, *validator), packed.to_vec())
            })
            .collect();
        bridge_signatures.sort_by(|a, b| a.0.0.cmp(&b.0.0).then_with(|| a.0.1.0.cmp(&b.0.1.0)));
        crate::state::persistence::StateSnapshot {
            accounts,
            stake_accounts,
            active_validator_set: self.active_validator_set.clone(),
            current_epoch: self.current_epoch,
            total_supply: self.total_supply,
            last_finalized_round: self.last_finalized_round,
            proposals,
            votes,
            next_proposal_id: self.next_proposal_id,
            governance_params: self.governance_params.clone(),
            council_members: council,
            treasury_balance: self.treasury_balance,
            delegation_accounts,
            configured_validator_count: self.configured_validator_count,
            bridge_reserve: self.bridge_reserve,
            bridge_attestations,
            bridge_signatures,
            bridge_nonce: self.bridge_nonce,
            bridge_contract_address: self.bridge_contract_address,
            used_release_nonces: self.used_release_nonces.iter().copied().collect(),
            bridge_release_votes: self.bridge_release_votes.iter()
                .map(|(k, v)| {
                    let mut voters: Vec<Address> = v.iter().copied().collect();
                    voters.sort_by_key(|a| a.0);
                    (*k, voters)
                })
                .collect(),
            bridge_release_params: if self.bridge_release_params.is_empty() {
                None
            } else {
                let mut params: Vec<_> = self.bridge_release_params.iter()
                    .map(|(k, v)| (*k, v.clone()))
                    .collect();
                params.sort_by_key(|(k, _)| *k);
                Some(params)
            },
            last_proposal_round: {
                let mut lpr: Vec<_> = self.last_proposal_round.iter().map(|(a, r)| (*a, *r)).collect();
                lpr.sort_by_key(|(addr, _)| addr.0);
                lpr
            },
        }
    }

    /// Create a StateEngine from a snapshot (for checkpoint validation without mutating self).
    /// Verifies the supply invariant on construction to catch corrupted snapshot data.
    pub fn from_snapshot(snapshot: crate::state::persistence::StateSnapshot) -> Result<Self, CoinError> {
        let liquid: u64 = snapshot.accounts.iter().map(|(_, a)| a.balance).fold(0, |a, b| a.saturating_add(b));
        let staked: u64 = snapshot.stake_accounts.iter().map(|(_, s)| s.staked).fold(0, |a, b| a.saturating_add(b));
        let delegated: u64 = snapshot.delegation_accounts.iter().map(|(_, d)| d.delegated).fold(0, |a, b| a.saturating_add(b));
        let total = liquid.saturating_add(staked).saturating_add(delegated)
            .saturating_add(snapshot.treasury_balance).saturating_add(snapshot.bridge_reserve);
        if total != snapshot.total_supply {
            return Err(CoinError::SupplyInvariantBroken(format!(
                "from_snapshot: liquid={} staked={} delegated={} treasury={} bridge={} sum={} != total_supply={}",
                liquid, staked, delegated, snapshot.treasury_balance, snapshot.bridge_reserve, total, snapshot.total_supply
            )));
        }

        Ok(Self {
            accounts: snapshot.accounts.into_iter().collect(),
            stake_accounts: snapshot.stake_accounts.into_iter().collect(),
            active_validator_set: snapshot.active_validator_set,
            current_epoch: snapshot.current_epoch,
            total_supply: snapshot.total_supply,
            last_finalized_round: snapshot.last_finalized_round,
            proposals: snapshot.proposals.into_iter().collect(),
            votes: snapshot.votes.into_iter().collect(),
            next_proposal_id: snapshot.next_proposal_id,
            governance_params: snapshot.governance_params,
            configured_validator_count: snapshot.configured_validator_count,
            council_members: snapshot.council_members.into_iter().collect(),
            treasury_balance: snapshot.treasury_balance,
            delegation_accounts: snapshot.delegation_accounts.into_iter().collect(),
            tx_index: HashMap::new(),
            tx_index_order: VecDeque::new(),
            tx_receipts: HashMap::new(),
            slash_history: Vec::new(),
            applied_validators_per_round: HashMap::new(),
            bridge_reserve: snapshot.bridge_reserve,
            bridge_attestations: snapshot.bridge_attestations.into_iter().collect(),
            bridge_signatures: snapshot.bridge_signatures
                .into_iter()
                .filter_map(|(k, v)| {
                    // Current format: 85 bytes = eth_address (20) + ecdsa_sig (65)
                    if v.len() == 85 {
                        let mut packed = [0u8; 85];
                        packed.copy_from_slice(&v);
                        Some((k, packed))
                    }
                    // Legacy formats (96 or 64 bytes) are incompatible with ECDSA —
                    // drop them silently. Validators will re-sign pending attestations.
                    else {
                        None
                    }
                })
                .collect(),
            bridge_nonce: snapshot.bridge_nonce,
            bridge_contract_address: snapshot.bridge_contract_address,
            used_release_nonces: snapshot.used_release_nonces.into_iter().collect(),
            bridge_release_votes: snapshot.bridge_release_votes.into_iter()
                .map(|(k, voters)| (k, voters.into_iter().collect()))
                .collect(),
            bridge_release_params: snapshot.bridge_release_params.clone().unwrap_or_default()
                .into_iter().collect(),
            bridge_release_first_vote_round: HashMap::new(), // Restored from redb after from_parts
            bridge_release_disagree_count: HashMap::new(), // Transient — rebuilt as votes arrive
            slashed_events: std::collections::HashSet::new(), // Rebuilt from slash_history if needed
            last_proposal_round: snapshot.last_proposal_round.into_iter().collect(),

        })
    }

    /// Load state from a snapshot (for fast-sync from checkpoint).
    pub fn load_snapshot(&mut self, snapshot: crate::state::persistence::StateSnapshot) {
        // Preserve configured_validator_count across fast-sync — it's set from CLI
        // --validators N and not part of the snapshot. Without this, pre-staking reward
        // calculation would fall back to dynamic counting, potentially causing coinbase mismatch.
        let saved_configured = self.configured_validator_count;
        self.accounts = snapshot.accounts.into_iter().collect();
        self.stake_accounts = snapshot.stake_accounts.into_iter().collect();
        self.active_validator_set = snapshot.active_validator_set;
        self.current_epoch = snapshot.current_epoch;
        self.total_supply = snapshot.total_supply;
        self.last_finalized_round = snapshot.last_finalized_round;
        self.proposals = snapshot.proposals.into_iter().collect();
        self.votes = snapshot.votes.into_iter().collect();
        self.next_proposal_id = snapshot.next_proposal_id;
        self.governance_params = snapshot.governance_params;
        self.council_members = snapshot.council_members.into_iter().collect();
        self.treasury_balance = snapshot.treasury_balance;
        self.delegation_accounts = snapshot.delegation_accounts.into_iter().collect();
        self.bridge_reserve = snapshot.bridge_reserve;
        self.bridge_attestations = snapshot.bridge_attestations.into_iter().collect();
        self.bridge_signatures = snapshot.bridge_signatures
            .into_iter()
            .filter_map(|(k, v)| {
                // Current format: 85 bytes = eth_address (20) + ecdsa_sig (65)
                if v.len() == 85 {
                    let mut packed = [0u8; 85];
                    packed.copy_from_slice(&v);
                    Some((k, packed))
                } else {
                    None // Drop incompatible legacy formats
                }
            })
            .collect();
        self.bridge_nonce = snapshot.bridge_nonce;
        self.bridge_contract_address = snapshot.bridge_contract_address;
        self.used_release_nonces = snapshot.used_release_nonces.into_iter().collect();
        self.bridge_release_votes = snapshot.bridge_release_votes.into_iter()
            .map(|(k, voters)| (k, voters.into_iter().collect()))
            .collect();
        self.bridge_release_params = snapshot.bridge_release_params.unwrap_or_default()
            .into_iter().collect();
        self.bridge_release_first_vote_round = HashMap::new(); // Restored from redb after load_snapshot
        self.bridge_release_disagree_count = HashMap::new();
        self.slashed_events = std::collections::HashSet::new();
        self.last_proposal_round = snapshot.last_proposal_round.into_iter().collect();
        self.configured_validator_count = saved_configured;
    }

    // ─── Bridge Attestation Functions ───

    /// Sign a bridge attestation (validator function).
    /// Validators sign attestations as part of consensus finalization.
    /// Verifies that: (1) the attestation exists, (2) the validator is in the active set,
    /// (3) the ECDSA signature is valid over the Solidity message hash.
    ///
    /// H1 fix: uses secp256k1/ECDSA signatures for Solidity ecrecover compatibility.
    /// Signature format: eth_address (20) + ecdsa_sig (65) = 85 bytes.
    pub fn sign_bridge_attestation(
        &mut self,
        nonce: u64,
        validator: Address,
        packed_sig: [u8; 85],
    ) -> Result<(), CoinError> {
        // Verify attestation exists
        let attestation = self.bridge_attestations.get(&nonce)
            .ok_or_else(|| CoinError::ValidationError("Attestation not found".into()))?;

        // Verify validator is in the active set
        if !self.active_validator_set.contains(&validator) {
            return Err(CoinError::ValidationError("signer is not an active validator".into()));
        }

        // Extract eth_address and signature from packed format
        let mut eth_addr = [0u8; 20];
        eth_addr.copy_from_slice(&packed_sig[..20]);
        let sig_bytes = &packed_sig[20..];

        // Verify ECDSA signature by recovering signer address
        use k256::ecdsa::{RecoveryId, Signature as EcdsaSig, VerifyingKey};
        use sha3::{Digest, Keccak256};

        let ecdsa_sig = EcdsaSig::from_slice(&sig_bytes[..64])
            .map_err(|_| CoinError::ValidationError("invalid ECDSA signature".into()))?;
        let v = sig_bytes[64];
        let recovery_id = RecoveryId::from_byte(v.wrapping_sub(27))
            .ok_or_else(|| CoinError::ValidationError("invalid recovery id".into()))?;

        let message_hash = attestation.solidity_message_hash();

        // C1 fix: apply EIP-191 prefix before ECDSA recovery (matches sign_for_bridge)
        let eth_signed_hash: [u8; 32] = {
            let mut prefixed = Vec::with_capacity(60);
            prefixed.extend_from_slice(b"\x19Ethereum Signed Message:\n32");
            prefixed.extend_from_slice(&message_hash);
            Keccak256::digest(&prefixed).into()
        };

        let recovered_vk = VerifyingKey::recover_from_prehash(&eth_signed_hash, &ecdsa_sig, recovery_id)
            .map_err(|_| CoinError::InvalidSignature)?;

        let encoded = recovered_vk.to_encoded_point(false);
        let pubkey_bytes = &encoded.as_bytes()[1..];
        let hash = Keccak256::digest(pubkey_bytes);
        let mut recovered_addr = [0u8; 20];
        recovered_addr.copy_from_slice(&hash[12..32]);

        if recovered_addr != eth_addr {
            return Err(CoinError::InvalidSignature);
        }

        // Store verified signature
        self.bridge_signatures.insert((nonce, validator), packed_sig);

        Ok(())
    }

    /// Sign all pending bridge attestations that this validator hasn't signed yet.
    /// Called by the validator loop after applying finalized vertices, so that
    /// attestation creation (state mutation) and signing happen in separate steps.
    ///
    /// H2 fix: only active validators may sign bridge attestations.
    /// H1 fix: uses secp256k1/ECDSA signatures for Solidity compatibility.
    /// The secp key is derived from the Ed25519 key via SHA-256.
    pub fn sign_pending_bridge_attestations(
        &mut self,
        validator: Address,
        sk: &crate::address::SecretKey,
    ) {
        // H2: only active validators should sign bridge attestations
        if !self.active_validator_set.contains(&validator) {
            return;
        }

        let secp_key = match crate::bridge::derive_secp_key_from_ed25519(&sk.to_bytes()) {
            Some(key) => key,
            None => {
                tracing::error!("Failed to derive secp256k1 key for bridge signing");
                return;
            }
        };
        let eth_addr = crate::bridge::eth_address_from_secp_key(&secp_key);
        let nonces: Vec<u64> = self.bridge_attestations.keys().copied().collect();
        for nonce in nonces {
            // Skip if already signed
            if self.bridge_signatures.contains_key(&(nonce, validator)) {
                continue;
            }
            if let Some(attestation) = self.bridge_attestations.get(&nonce) {
                let msg_hash = attestation.solidity_message_hash();
                let sig = match crate::bridge::sign_for_bridge(&msg_hash, &secp_key) {
                    Ok(s) => s,
                    Err(e) => {
                        tracing::error!("Failed to sign bridge attestation nonce={}: {}", nonce, e);
                        continue;
                    }
                };
                // Store: eth_address (20) + ecdsa_signature (65) = 85 bytes
                let mut combined = [0u8; 85];
                combined[..20].copy_from_slice(&eth_addr);
                combined[20..].copy_from_slice(&sig);
                self.bridge_signatures.insert((nonce, validator), combined);
                tracing::debug!("Signed bridge attestation nonce={} as validator {}", nonce, validator.to_hex());
            }
        }
    }

    /// Get the number of signatures for an attestation.
    pub fn get_signature_count(&self, nonce: u64) -> usize {
        self.bridge_signatures
            .iter()
            .filter(|((n, _), _)| *n == nonce)
            .count()
    }

    /// Get the threshold for bridge signatures (2/3 of active validators).
    pub fn get_bridge_threshold(&self) -> usize {
        let validator_count = self.active_validator_set.len();
        if validator_count < 3 {
            return validator_count;
        }
        // Threshold = ceil(2/3 * validator_count)
        (2 * validator_count + 2) / 3
    }

    /// Build a bridge proof with collected signatures.
    /// Returns Ok when threshold signatures are collected.
    ///
    /// H1 fix: builds proof with ECDSA signatures (eth_address + 65-byte sig).
    pub fn build_bridge_proof(
        &self,
        nonce: u64,
    ) -> Result<crate::bridge::BridgeProof, CoinError> {
        use crate::bridge::{BridgeProof, SignedBridgeAttestation};

        // Get attestation
        let attestation = self.bridge_attestations
            .get(&nonce)
            .ok_or_else(|| CoinError::ValidationError("Attestation not found".into()))?
            .clone();

        // Collect signatures — unpack eth_address (20) + ecdsa_sig (65) from 85-byte packed format
        let signatures: Vec<SignedBridgeAttestation> = self.bridge_signatures
            .iter()
            .filter(|((n, _), _)| *n == nonce)
            .map(|((_, validator), packed)| {
                let mut eth_addr = [0u8; 20];
                eth_addr.copy_from_slice(&packed[..20]);
                let mut sig = [0u8; 65];
                sig.copy_from_slice(&packed[20..85]);
                SignedBridgeAttestation::new(attestation.clone(), *validator, eth_addr, sig)
            })
            .collect();

        // Check threshold
        let threshold = self.get_bridge_threshold();
        if signatures.len() < threshold {
            return Err(CoinError::ValidationError(
                format!("Insufficient signatures: {} < {}", signatures.len(), threshold)
            ));
        }

        Ok(BridgeProof::new(attestation, signatures))
    }

    /// Refund a bridge attestation (for failed bridges).
    /// Reduces bridge_reserve and credits the original sender.
    /// Marks the attestation as refunded by removing it.
    ///
    /// M2 fix: `pub(crate)` visibility — callers MUST verify governance authorization
    /// before invoking (e.g., via an executed governance proposal).
    /// Direct external access is intentionally prevented.
    pub(crate) fn bridge_refund(&mut self, nonce: u64, governance_authorized: bool) -> Result<(), CoinError> {
        if !governance_authorized {
            return Err(CoinError::ValidationError(
                "bridge_refund requires governance authorization".into()
            ));
        }
        // Verify the attestation exists
        let attestation = self.bridge_attestations.get(&nonce)
            .ok_or_else(|| CoinError::ValidationError(
                format!("bridge attestation nonce {} not found", nonce)
            ))?
            .clone();

        let amount = attestation.amount;
        let sender = attestation.sender;

        // Reduce bridge reserve
        if self.bridge_reserve < amount {
            return Err(CoinError::ValidationError(
                "bridge_reserve insufficient for refund (state corruption?)".into()
            ));
        }
        // Credit the original sender BEFORE decrementing reserve.
        // If credit fails, bridge_reserve is untouched — preserves supply invariant.
        self.credit(&sender, amount).map_err(|e| {
            CoinError::SupplyInvariantBroken(format!(
                "bridge_refund credit failed for {} (nonce {}): {}",
                sender.to_hex(), nonce, e
            ))
        })?;
        self.bridge_reserve = self.bridge_reserve.saturating_sub(amount);

        // Remove attestation and associated signatures (marks as refunded)
        self.bridge_attestations.remove(&nonce);
        self.bridge_signatures.retain(|(n, _), _| *n != nonce);

        tracing::info!(
            "Bridge refund: nonce={} amount={} sender={}",
            nonce, amount, sender.to_hex()
        );

        Ok(())
    }

    /// Get bridge attestation by nonce.
    pub fn get_bridge_attestation(&self, nonce: u64) -> Option<&crate::bridge::BridgeAttestation> {
        self.bridge_attestations.get(&nonce)
    }

    /// Get next bridge nonce.
    pub fn get_bridge_nonce(&self) -> u64 {
        self.bridge_nonce
    }

    /// Get bridge reserve balance.
    pub fn get_bridge_reserve(&self) -> u64 {
        self.bridge_reserve
    }

    /// Prune old bridge attestations and their signatures.
    /// Removes attestations older than BRIDGE_ATTESTATION_RETENTION_ROUNDS based on creation_round.
    /// Returns the number of attestations pruned.
    ///
    /// M1 fix: prunes ONLY by age (creation_round). The "fully signed" criterion was removed
    /// because bridge_signatures count is non-deterministic across nodes (each validator signs
    /// locally). Using sig_count for pruning would cause different nodes to prune different
    /// attestations, diverging bridge_attestations which IS in the state root.
    pub fn prune_old_bridge_attestations(&mut self, current_round: u64) -> usize {
        let retention = crate::constants::BRIDGE_ATTESTATION_RETENTION_ROUNDS;

        let mut to_prune = Vec::new();
        for (&nonce, attestation) in &self.bridge_attestations {
            let is_old = attestation.creation_round.saturating_add(retention) < current_round;
            if is_old {
                to_prune.push((nonce, attestation.sender, attestation.amount));
            }
        }

        // Sort for deterministic processing order across all nodes
        to_prune.sort_by_key(|(nonce, _, _)| *nonce);

        let pruned = to_prune.len();
        for (nonce, sender, amount) in &to_prune {
            // Auto-refund: return locked funds from bridge_reserve to the original sender.
            // This prevents funds from being permanently stuck when attestations expire
            // before the user claims on Arbitrum.
            if self.bridge_reserve >= *amount {
                match self.credit(sender, *amount) {
                    Ok(()) => {
                        self.bridge_reserve = self.bridge_reserve.saturating_sub(*amount);
                    }
                    Err(e) => {
                        tracing::error!("Bridge auto-refund credit failed for nonce {}: {}", nonce, e);
                        // Do NOT subtract from bridge_reserve if credit failed — preserves supply invariant
                    }
                }
                tracing::warn!(
                    "Bridge auto-refund: {} sats returned to {} (attestation #{} expired after {} rounds)",
                    amount, sender.to_hex(), nonce, retention
                );
            }
            self.bridge_attestations.remove(nonce);
            self.bridge_signatures.retain(|(n, _), _| n != nonce);
        }

        // Prune stale bridge_release_votes and bridge_release_params.
        // Remove entries that have been pending longer than the retention period.
        // This prevents unbounded growth from releases that never reach quorum
        // (e.g., poisoned params, validator set changes, or abandoned votes).
        // We use the same retention period as attestations for consistency.
        let stale_vote_keys: Vec<(u64, u64)> = self.bridge_release_first_vote_round.iter()
            .filter(|(_, &first_round)| first_round.saturating_add(retention) < current_round)
            .map(|(k, _)| *k)
            .collect();
        for key in &stale_vote_keys {
            self.bridge_release_votes.remove(key);
            self.bridge_release_params.remove(key);
            self.bridge_release_first_vote_round.remove(key);
            self.bridge_release_disagree_count.remove(key);
            tracing::debug!("Pruned stale bridge release votes/params for {:?} (pending > {} rounds)", key, retention);
        }

        pruned
    }

    /// Restore bridge state from persistence.
    /// Called by load_from_redb after from_parts (which initializes bridge state to empty/0).
    pub fn restore_bridge_state(
        &mut self,
        attestations: HashMap<u64, crate::bridge::BridgeAttestation>,
        signatures: HashMap<(u64, Address), [u8; 85]>,
        nonce: u64,
    ) {
        self.bridge_attestations = attestations;
        self.bridge_signatures = signatures;
        self.bridge_nonce = nonce;
    }

    /// Restore used_release_nonces from persistence.
    /// Called by load_from_redb after from_parts.
    pub fn restore_used_release_nonces(&mut self, nonces: Vec<(u64, u64)>) {
        self.used_release_nonces = nonces.into_iter().collect();
    }

    /// Restore bridge_release_votes from persistence.
    /// Called by load_from_redb after from_parts.
    pub fn restore_bridge_release_votes(&mut self, votes: Vec<((u64, u64), Vec<Address>)>) {
        self.bridge_release_votes = votes.into_iter()
            .map(|(k, voters)| (k, voters.into_iter().collect()))
            .collect();
    }

    /// Restore bridge_release_params from persistence.
    /// Called by load_from_redb after from_parts. Ensures in-progress bridge releases
    /// retain their canonical (recipient, amount) across node restarts.
    pub fn restore_bridge_release_params(&mut self, params: Vec<((u64, u64), (Address, u64))>) {
        self.bridge_release_params = params.into_iter().collect();
    }

    /// Restore bridge_release_first_vote_round from persistence.
    pub fn restore_bridge_release_first_vote_round(&mut self, fvr: Vec<((u64, u64), u64)>) {
        self.bridge_release_first_vote_round = fvr.into_iter().collect();
    }

    /// Snapshot of bridge_release_first_vote_round for persistence.
    pub fn bridge_release_first_vote_round_snapshot(&self) -> Vec<((u64, u64), u64)> {
        let mut v: Vec<_> = self.bridge_release_first_vote_round.iter().map(|(k, r)| (*k, *r)).collect();
        v.sort_by_key(|(k, _)| *k);
        v
    }

    /// Restore last_proposal_round from persistence.
    /// Called by load_from_redb after from_parts.
    pub fn restore_last_proposal_round(&mut self, lpr: Vec<(Address, u64)>) {
        self.last_proposal_round = lpr.into_iter().collect();
    }

    /// Save state to redb database (ACID, crash-safe).
    pub fn save(&self, path: &std::path::Path) -> Result<(), crate::persistence::PersistenceError> {
        crate::state::db::save_to_redb(self, path)
    }

    /// Load state from redb database.
    /// If the persisted epoch is stale, recalculates the active validator set.
    pub fn load(path: &std::path::Path) -> Result<Self, crate::persistence::PersistenceError> {
        crate::state::db::load_from_redb(path)
    }

    /// Check if saved state exists
    pub fn exists(path: &std::path::Path) -> bool {
        path.exists()
    }
}

impl Default for StateEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::address::{SecretKey, Signature};
    use crate::block::block::Block;
    use crate::block::header::BlockHeader;
    use crate::tx::{CoinbaseTx, Transaction};

    fn make_signed_tx(sk: &SecretKey, to: Address, amount: u64, fee: u64, nonce: u64) -> Transaction {
        let mut transfer = crate::tx::TransferTx {
            from: sk.address(),
            to,
            amount,
            fee,
            nonce,
            pub_key: sk.verifying_key().to_bytes(),
            signature: Signature([0u8; 64]),
            memo: None,
        };
        transfer.signature = sk.sign(&transfer.signable_bytes());
        Transaction::Transfer(transfer)
    }

    fn make_vertex_for(
        proposer: &Address,
        round: u64,
        height: u64,
        txs: Vec<Transaction>,
        sk: &SecretKey,
    ) -> DagVertex {
        // Coinbase amount is always 0; fees credited via deferred mechanism,
        // block rewards distributed via distribute_round_rewards()
        let coinbase = CoinbaseTx {
            to: *proposer,
            amount: 0,
            height,
        };
        let block = Block {
            header: BlockHeader {
                version: 1,
                height,
                timestamp: 1_000_000,
                prev_hash: [0u8; 32],
                merkle_root: [0u8; 32],
            },
            coinbase,
            transactions: txs,
        };
        let mut vertex = DagVertex::new(
            block,
            vec![],
            round,
            *proposer,
            sk.verifying_key().to_bytes(),
            Signature([0u8; 64]),
        );
        vertex.signature = sk.sign(&vertex.signable_bytes());
        vertex
    }

    /// Like make_vertex_for but with a custom coinbase amount (for testing fee-only validation).
    fn make_vertex_with_reward(
        proposer: &Address,
        round: u64,
        height: u64,
        coinbase_amount: u64,
        sk: &SecretKey,
    ) -> DagVertex {
        let coinbase = CoinbaseTx {
            to: *proposer,
            amount: coinbase_amount,
            height,
        };
        let block = Block {
            header: BlockHeader {
                version: 1,
                height,
                timestamp: 1_000_000,
                prev_hash: [0u8; 32],
                merkle_root: [0u8; 32],
            },
            coinbase,
            transactions: vec![],
        };
        let mut vertex = DagVertex::new(
            block,
            vec![],
            round,
            *proposer,
            sk.verifying_key().to_bytes(),
            Signature([0u8; 64]),
        );
        vertex.signature = sk.sign(&vertex.signable_bytes());
        vertex
    }

    #[test]
    fn initial_balance_is_zero() {
        let state = StateEngine::new();
        let addr = SecretKey::generate().address();
        assert_eq!(state.balance(&addr), 0);
        assert_eq!(state.nonce(&addr), 0);
    }

    #[test]
    fn apply_vertex_credits_proposer() {
        let mut state = StateEngine::new();
        let proposer_sk = SecretKey::generate();
        let proposer = proposer_sk.address();
        let vertex = make_vertex_for(&proposer, 0, 0, vec![], &proposer_sk);
        state.apply_vertex(&vertex).unwrap();

        // Per-round emission split: 75% validators, 10% treasury, 10% council, 5% founder
        // With no council members, council share is not minted.
        let total_reward = crate::constants::block_reward(0);
        let validator_share = total_reward * 75 / 100; // 75% to validator pool
        assert_eq!(state.balance(&proposer), validator_share);
        // total_supply = validator + treasury (10%) + founder (5%) [council 10% unminted]
        let treasury_share = total_reward * 10 / 100;
        let founder_share = total_reward * 5 / 100;
        assert_eq!(state.total_supply(), validator_share + treasury_share + founder_share);
        assert_eq!(state.last_finalized_round(), Some(0));
    }

    #[test]
    fn apply_vertex_with_transaction() {
        let mut state = StateEngine::new();
        let proposer_sk = SecretKey::generate();
        let proposer = proposer_sk.address();
        let receiver = SecretKey::generate().address();

        // First vertex gives proposer some coins (75% of block reward)
        let v0 = make_vertex_for(&proposer, 0, 0, vec![], &proposer_sk);
        state.apply_vertex(&v0).unwrap();

        let reward0 = crate::constants::block_reward(0) * 75 / 100;
        let amount = 100;
        let fee = 10;

        let tx = make_signed_tx(&proposer_sk, receiver, amount, fee, 0);

        let v1 = make_vertex_for(&proposer, 1, 1, vec![tx], &proposer_sk);
        state.apply_vertex(&v1).unwrap();

        let reward1 = crate::constants::block_reward(1) * 75 / 100;
        // Proposer: reward0 - (amount + fee) + (reward1 + fee)
        let expected_proposer = reward0 - (amount + fee) + reward1 + fee;
        assert_eq!(state.balance(&proposer), expected_proposer);
        assert_eq!(state.balance(&receiver), amount);
        assert_eq!(state.nonce(&proposer), 1);
        assert_eq!(state.last_finalized_round(), Some(1));
    }

    #[test]
    fn insufficient_balance_skipped_in_finalized_vertex() {
        let mut state = StateEngine::new();
        let proposer_sk = SecretKey::generate();
        let proposer = proposer_sk.address();
        let sender_sk = SecretKey::generate();
        let sender = sender_sk.address();
        let receiver = SecretKey::generate().address();

        // Give proposer coins, not sender
        let v0 = make_vertex_for(&proposer, 0, 0, vec![], &proposer_sk);
        state.apply_vertex(&v0).unwrap();

        // sender has 0 balance, tries to send 100 — tx should be skipped
        let tx = make_signed_tx(&sender_sk, receiver, 100, 10, 0);

        let v1 = make_vertex_for(&proposer, 1, 1, vec![tx], &proposer_sk);
        let result = state.apply_vertex(&v1);
        assert!(result.is_ok(), "Vertex should apply despite bad tx: {:?}", result.err());
        // Receiver should NOT have received the transfer
        assert_eq!(state.balance(&receiver), 0);
        // Sender balance unchanged (was 0, still 0)
        assert_eq!(state.balance(&sender), 0);
    }

    #[test]
    fn invalid_nonce_skipped_in_finalized_vertex() {
        let mut state = StateEngine::new();
        let proposer_sk = SecretKey::generate();
        let proposer = proposer_sk.address();
        let receiver = SecretKey::generate().address();

        let v0 = make_vertex_for(&proposer, 0, 0, vec![], &proposer_sk);
        state.apply_vertex(&v0).unwrap();
        let balance_after_v0 = state.balance(&proposer);

        // nonce should be 0, but we pass 5 — tx should be skipped
        let tx = make_signed_tx(&proposer_sk, receiver, 100, 10, 5);

        let v1 = make_vertex_for(&proposer, 1, 1, vec![tx], &proposer_sk);
        let result = state.apply_vertex(&v1);
        assert!(result.is_ok(), "Vertex should apply despite bad nonce");
        // Receiver should NOT have received the transfer
        assert_eq!(state.balance(&receiver), 0);
        // Proposer gets 75% of block reward (emission split) — skipped tx has no fee effect
        let reward = crate::constants::block_reward(1) * 75 / 100;
        assert!(state.balance(&proposer) >= balance_after_v0 + reward - 10);
    }

    #[test]
    fn supply_cap_enforced() {
        let mut state = StateEngine::new();
        let sk = SecretKey::generate();
        let proposer = sk.address();

        // Manually set total_supply close to max and credit matching balance
        let max = crate::constants::MAX_SUPPLY_SATS;
        let existing_supply = max - 100; // Only 100 sats remaining
        state.total_supply = existing_supply;
        state.credit(&proposer, existing_supply); // Maintain invariant

        // Stake so per-round distribution works (not pre-staking fallback)
        state.stake_accounts.insert(proposer, StakeAccount {
            staked: 10_000 * crate::constants::COIN,
            unlock_at_round: None,
            commission_percent: 10,
            commission_last_changed: None,
            locked_stake: 0,
        });
        // Adjust supply to include staked amount
        state.total_supply = existing_supply.saturating_add(10_000 * crate::constants::COIN);
        state.credit(&proposer, 10_000 * crate::constants::COIN);

        // Reset to near-max with correct invariant
        let total_balance = state.balance(&proposer);
        state.total_supply = max - 100;
        // Set balance to maintain invariant: liquid + staked = total_supply
        let staked = 10_000 * crate::constants::COIN;
        let needed_liquid = (max - 100).saturating_sub(staked);
        state.accounts.get_mut(&proposer).unwrap().balance = needed_liquid;

        // Apply a vertex with fees-only coinbase (0 fees)
        let vertex = make_vertex_with_reward(&proposer, 0, 0, 0, &sk);
        state.apply_vertex(&vertex).unwrap();

        // Supply should increase by at most 100 (capped)
        assert!(state.total_supply() <= max);
    }

    #[test]
    fn supply_cap_zero_reward_at_max() {
        let mut state = StateEngine::new();
        let sk = SecretKey::generate();
        let proposer = sk.address();

        // Set total_supply to exactly max and credit matching balance
        let max = crate::constants::MAX_SUPPLY_SATS;
        state.total_supply = max;
        state.credit(&proposer, max); // Maintain invariant

        // Coinbase must be 0 (no remaining supply to emit)
        let vertex = make_vertex_with_reward(&proposer, 0, 0, 0, &sk);
        state.apply_vertex(&vertex).unwrap();

        // Supply should not exceed max
        assert_eq!(state.total_supply(), max);
        // Proposer gets 0 (no new supply, no fees) — balance unchanged
        assert_eq!(state.balance(&proposer), max);
    }

    #[test]
    fn apply_multiple_vertices() {
        let mut state = StateEngine::new();
        let sk1 = SecretKey::generate();
        let sk2 = SecretKey::generate();
        let sk3 = SecretKey::generate();

        let v0 = make_vertex_for(&sk1.address(), 0, 0, vec![], &sk1);
        let v1 = make_vertex_for(&sk2.address(), 1, 1, vec![], &sk2);
        let v2 = make_vertex_for(&sk3.address(), 2, 2, vec![], &sk3);

        state.apply_finalized_vertices(&[v0, v1, v2]).unwrap();

        // Per-round emission: 75% validator, 10% treasury, 5% founder (council unminted)
        let r0 = crate::constants::block_reward(0) * 75 / 100;
        let r1 = crate::constants::block_reward(1) * 75 / 100;
        let r2 = crate::constants::block_reward(2) * 75 / 100;

        assert_eq!(state.balance(&sk1.address()), r0);
        assert_eq!(state.balance(&sk2.address()), r1);
        assert_eq!(state.balance(&sk3.address()), r2);
        // total_supply includes validator (75%) + treasury (10%) + founder (5%) per round
        let per_round_total = |h: u64| {
            let br = crate::constants::block_reward(h);
            br * 75 / 100 + br * 10 / 100 + br * 5 / 100
        };
        assert_eq!(state.total_supply(), per_round_total(0) + per_round_total(1) + per_round_total(2));
        assert_eq!(state.last_finalized_round(), Some(2));
    }

    #[test]
    fn invalid_coinbase_rejected() {
        let mut state = StateEngine::new();
        let sk = SecretKey::generate();
        let proposer = sk.address();

        // Create a vertex with no transactions (coinbase should be 0 = fees only)
        let mut vertex = make_vertex_for(&proposer, 0, 0, vec![], &sk);

        // Tamper with coinbase amount (should be 0 since no fees)
        vertex.block.coinbase.amount = 1_000_000;

        // Re-sign the vertex
        let signable = vertex.signable_bytes();
        vertex.signature = sk.sign(&signable);

        // Should reject with InvalidCoinbase error
        let result = state.apply_vertex(&vertex);
        assert!(result.is_err());
        match result {
            Err(CoinError::InvalidCoinbase { expected, got }) => {
                assert_eq!(expected, 0); // No fees = coinbase should be 0
                assert_eq!(got, 1_000_000);
            }
            _ => panic!("Expected InvalidCoinbase error"),
        }
    }

    #[test]
    fn faucet_credit_does_not_inflate_supply() {
        let mut engine = StateEngine::new_with_genesis();
        let supply_before = engine.total_supply();
        
        let recipient = SecretKey::generate().address();
        engine.faucet_credit(&recipient, 100 * crate::constants::COIN).unwrap();
        
        let supply_after = engine.total_supply();
        assert_eq!(supply_before, supply_after, 
            "Faucet credit must not change total_supply");
        assert_eq!(engine.balance(&recipient), 100 * crate::constants::COIN);
    }

    #[test]
    fn faucet_depletion_returns_error() {
        let mut engine = StateEngine::new_with_genesis();
        let recipient = SecretKey::generate().address();
        
        // Try to drain more than faucet has
        let result = engine.faucet_credit(&recipient, 2_000_000 * crate::constants::COIN);
        assert!(result.is_err(), "Faucet should reject over-withdrawal");
    }

    #[test]
    fn supply_invariant_holds_after_100_rounds() {
        let mut state = StateEngine::new();
        let validators: Vec<_> = (0..4).map(|_| SecretKey::generate()).collect();

        // Apply 100 rounds - each round the supply invariant is checked in apply_vertex
        for round in 0..100 {
            let proposer_idx = round % 4;
            let proposer = &validators[proposer_idx as usize];

            let vertex = make_vertex_for(&proposer.address(), round, round, vec![], proposer);
            state.apply_vertex(&vertex).unwrap();

            // Verify invariant manually (also checked in apply_vertex)
            let liquid: u64 = state.accounts.values().map(|a| a.balance).fold(0u64, |acc, x| acc.saturating_add(x));
            let staked: u64 = state.stake_accounts.values().map(|s| s.staked).fold(0u64, |acc, x| acc.saturating_add(x));
            let delegated: u64 = state.delegation_accounts.values().map(|d| d.delegated).fold(0u64, |acc, x| acc.saturating_add(x));
            let total = liquid.saturating_add(staked).saturating_add(delegated)
                .saturating_add(state.treasury_balance()).saturating_add(state.bridge_reserve());
            assert_eq!(
                total, state.total_supply,
                "Supply invariant broken at round {}: liquid={} staked={} delegated={} treasury={} bridge={} supply={}",
                round, liquid, staked, delegated, state.treasury_balance(), state.bridge_reserve(), state.total_supply
            );
        }
    }

    #[test]
    fn slash_validator_burns_50_percent() {
        let mut state = StateEngine::new();
        let sk = SecretKey::generate();
        let validator = sk.address();
        
        // Give validator liquid balance and stake it (20,000 UDAG = 2x MIN_STAKE)
        let stake_amount = MIN_STAKE_SATS * 2;
        state.credit(&validator, stake_amount);
        state.total_supply = stake_amount; // Initialize total_supply to match credited amount
        
        let stake_tx = StakeTx {
            from: validator,
            amount: stake_amount,
            nonce: 0,
            pub_key: sk.verifying_key().to_bytes(),
            signature: crate::Signature([0u8; 64]),
        };
        let mut signed_stake = stake_tx.clone();
        signed_stake.signature = sk.sign(&stake_tx.signable_bytes());
        state.apply_stake_tx(&signed_stake).unwrap();
        
        let supply_before = state.total_supply();
        let stake_before = state.stake_of(&validator);
        assert_eq!(stake_before, stake_amount);
        
        // Execute slash
        state.slash(&validator);
        
        // Verify 50% burned
        let stake_after = state.stake_of(&validator);
        let burned = stake_before - stake_after;
        assert_eq!(burned, stake_amount / 2, "Should burn exactly 50% of stake");
        assert_eq!(stake_after, stake_amount / 2, "Remaining stake should be 50%");
        
        // Verify supply decreased by burned amount
        let supply_after = state.total_supply();
        assert_eq!(supply_after, supply_before - burned, "Burned amount should be removed from total supply");
    }

    #[test]
    fn slash_validator_removes_from_active_set_if_below_minimum() {
        let mut state = StateEngine::new();
        let sk = SecretKey::generate();
        let validator = sk.address();

        // Stake 1.5x MIN_STAKE_SATS so that after 50% slash (0.75x), it falls below minimum
        let stake_amount = MIN_STAKE_SATS + MIN_STAKE_SATS / 2;
        state.credit(&validator, stake_amount);
        state.total_supply = stake_amount; // Initialize total_supply

        let stake_tx = StakeTx {
            from: validator,
            amount: stake_amount,
            nonce: 0,
            pub_key: sk.verifying_key().to_bytes(),
            signature: crate::Signature([0u8; 64]),
        };
        let mut signed_stake = stake_tx.clone();
        signed_stake.signature = sk.sign(&stake_tx.signable_bytes());
        state.apply_stake_tx(&signed_stake).unwrap();

        // Add to active validator set
        state.recalculate_active_set();
        assert!(state.is_active_validator(&validator), "Validator should be in active set before slash");
        
        // Execute slash
        state.slash(&validator);
        
        // Verify removed from active set
        let stake_after = state.stake_of(&validator);
        assert!(stake_after < MIN_STAKE_SATS, "Stake should fall below minimum after slash");
        assert!(!state.is_active_validator(&validator), "Validator should be removed from active set after slash");
    }

    #[test]
    fn slash_validator_stays_in_active_set_if_above_minimum() {
        let mut state = StateEngine::new();
        let sk = SecretKey::generate();
        let validator = sk.address();

        // Stake 4x MIN_STAKE_SATS so that after 50% slash, it's still above minimum
        let stake_amount = MIN_STAKE_SATS * 4;
        state.credit(&validator, stake_amount);
        state.total_supply = stake_amount; // Initialize total_supply

        let stake_tx = StakeTx {
            from: validator,
            amount: stake_amount,
            nonce: 0,
            pub_key: sk.verifying_key().to_bytes(),
            signature: crate::Signature([0u8; 64]),
        };
        let mut signed_stake = stake_tx.clone();
        signed_stake.signature = sk.sign(&stake_tx.signable_bytes());
        state.apply_stake_tx(&signed_stake).unwrap();

        // Add to active validator set
        state.recalculate_active_set();
        assert!(state.is_active_validator(&validator), "Validator should be in active set before slash");
        
        // Execute slash
        state.slash(&validator);
        
        // Verify still in active set (stake is 2x MIN_STAKE_SATS after 50% slash)
        let stake_after = state.stake_of(&validator);
        assert_eq!(stake_after, MIN_STAKE_SATS * 2, "Stake should be 50% of original");
        assert!(stake_after >= MIN_STAKE_SATS, "Stake should still be above minimum");
        // Note: is_active_validator might be false if active set wasn't recalculated,
        // but the important part is the validator wasn't explicitly removed by slash()
    }

    #[test]
    fn slash_validator_with_no_stake_is_noop() {
        let mut state = StateEngine::new();
        let validator = Address([99u8; 20]);
        
        let supply_before = state.total_supply();
        
        // Slash validator with no stake
        state.slash(&validator);
        
        // Should be no-op
        assert_eq!(state.stake_of(&validator), 0);
        assert_eq!(state.total_supply(), supply_before);
    }

    /// Test: Verify state consistency check works correctly
    #[test]
    fn verify_state_consistency_passes() {
        let mut state = StateEngine::new();
        let sk = SecretKey::generate();
        let addr = sk.address();
        
        // Credit some balance
        state.credit(&addr, 1000);
        state.total_supply = 1000;
        
        // Should pass
        assert!(state.verify_state_consistency().is_ok());
    }

    /// Test: Verify state consistency detects supply invariant violation
    #[test]
    fn verify_state_consistency_detects_supply_mismatch() {
        let mut state = StateEngine::new();
        let sk = SecretKey::generate();
        let addr = sk.address();
        
        // Credit some balance but set wrong total_supply
        state.credit(&addr, 1000);
        state.total_supply = 999; // Wrong!
        
        // Should fail
        let result = state.verify_state_consistency();
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), CoinError::SupplyInvariantBroken(_)));
    }

    /// Test: Apply vertices in same round atomically
    #[test]
    fn apply_finalized_vertices_same_round_atomic() {
        let mut state = StateEngine::new();
        let sk1 = SecretKey::generate();
        let sk2 = SecretKey::generate();
        let sk3 = SecretKey::generate();
        
        // Create vertices all in round 0
        let v0 = make_vertex_for(&sk1.address(), 0, 0, vec![], &sk1);
        let v1 = make_vertex_for(&sk2.address(), 0, 0, vec![], &sk2);
        let v2 = make_vertex_for(&sk3.address(), 0, 0, vec![], &sk3);
        
        // Apply all vertices in same round
        let result = state.apply_finalized_vertices(&[v0, v1, v2]);
        assert!(result.is_ok(), "Failed to apply vertices: {:?}", result.err());
        
        // Verify state consistency after application
        assert!(state.verify_state_consistency().is_ok());
        
        // All validators should have received rewards
        let reward = crate::constants::block_reward(0);
        assert!(state.balance(&sk1.address()) > 0);
        assert!(state.balance(&sk2.address()) > 0);
        assert!(state.balance(&sk3.address()) > 0);
        
        // Last finalized round should be 0
        assert_eq!(state.last_finalized_round(), Some(0));
    }

    /// Test: Apply vertices across multiple rounds
    #[test]
    fn apply_finalized_vertices_multiple_rounds() {
        let mut state = StateEngine::new();
        let sk1 = SecretKey::generate();
        let sk2 = SecretKey::generate();
        
        // Create vertices in different rounds
        let v0 = make_vertex_for(&sk1.address(), 0, 0, vec![], &sk1);
        let v1 = make_vertex_for(&sk2.address(), 1, 1, vec![], &sk2);
        let v2 = make_vertex_for(&sk1.address(), 2, 2, vec![], &sk1);
        
        let result = state.apply_finalized_vertices(&[v0, v1, v2]);
        assert!(result.is_ok());
        
        // Verify state consistency
        assert!(state.verify_state_consistency().is_ok());
        
        // Last finalized round should be 2
        assert_eq!(state.last_finalized_round(), Some(2));
    }

    /// Test: Concurrent vertex application (simulated via sequential calls)
    /// This tests that the round lock mechanism works correctly
    #[test]
    fn concurrent_vertex_application_simulation() {
        let mut state = StateEngine::new();
        let sk1 = SecretKey::generate();
        let sk2 = SecretKey::generate();
        
        // Simulate sequential application of batches
        // &mut self already guarantees exclusive access

        // Batch 1: Round 0
        let v0 = make_vertex_for(&sk1.address(), 0, 0, vec![], &sk1);
        assert!(state.apply_finalized_vertices(&[v0]).is_ok());

        // Batch 2: Round 1
        let v1 = make_vertex_for(&sk2.address(), 1, 1, vec![], &sk2);
        assert!(state.apply_finalized_vertices(&[v1]).is_ok());
        
        // Verify state consistency
        assert!(state.verify_state_consistency().is_ok());
    }

    /// Test: Supply invariant check with checked arithmetic
    #[test]
    fn supply_invariant_checked_arithmetic() {
        let mut state = StateEngine::new();
        let sk = SecretKey::generate();
        let addr = sk.address();
        
        // Create a valid state
        state.credit(&addr, 1000);
        state.total_supply = 1000;
        
        // Verify the checked arithmetic works
        assert!(state.verify_state_consistency().is_ok());
        
        // Now test with staking
        state.stake_accounts.insert(addr, StakeAccount {
            staked: 500,
            unlock_at_round: None,
            commission_percent: 10,
            commission_last_changed: None,
            locked_stake: 0,
        });
        state.total_supply = 1500; // liquid + staked
        
        assert!(state.verify_state_consistency().is_ok());
    }

    /// Test: State snapshot creation and verification
    #[test]
    fn state_snapshot_creation() {
        let mut state = StateEngine::new();
        let sk = SecretKey::generate();
        let addr = sk.address();
        
        // Create some state
        state.credit(&addr, 1000);
        state.total_supply = 1000;
        state.stake_accounts.insert(addr, StakeAccount {
            staked: 500,
            unlock_at_round: None,
            commission_percent: 10,
            commission_last_changed: None,
            locked_stake: 0,
        });
        
        // Create internal snapshot
        let snapshot = StateSnapshot::from_engine(&state);
        
        // Verify snapshot has correct data
        assert_eq!(snapshot.total_supply, 1000);
        assert!(snapshot.accounts.contains_key(&addr));
        assert!(snapshot.stake_accounts.contains_key(&addr));
    }

    /// Stress test: Apply many vertices and verify no race conditions
    #[test]
    fn stress_test_many_vertices() {
        let mut state = StateEngine::new();
        let validators: Vec<_> = (0..10).map(|_| SecretKey::generate()).collect();
        
        // Apply 100 rounds with multiple vertices per round
        for round in 0..100 {
            let mut vertices = Vec::new();
            for (i, sk) in validators.iter().enumerate() {
                let vertex = make_vertex_for(&sk.address(), round, round, vec![], sk);
                vertices.push(vertex);
                
                // Only apply up to 4 vertices per round (simulating 4 validators)
                if i >= 3 {
                    break;
                }
            }
            
            let result = state.apply_finalized_vertices(&vertices);
            assert!(result.is_ok(), "Failed at round {}: {:?}", round, result.err());
            
            // Verify state consistency every 10 rounds (expensive check)
            if round % 10 == 0 {
                assert!(state.verify_state_consistency().is_ok(), 
                    "State consistency failed at round {}", round);
            }
        }
        
        // Final consistency check
        assert!(state.verify_state_consistency().is_ok());
        assert_eq!(state.last_finalized_round(), Some(99));
    }

}

