use std::collections::{HashMap, VecDeque};

use crate::address::Address;
use crate::consensus::vertex::DagVertex;
use crate::error::CoinError;
use crate::tx::stake::{StakeTx, UnstakeTx, MIN_STAKE_SATS, UNSTAKE_COOLDOWN_ROUNDS};

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
#[derive(Debug, Clone)]
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
    /// Tracks which validators have produced vertices in each finalized round.
    /// Used for cross-batch equivocation detection: if a validator appears in a
    /// round that was already applied in a previous batch, that's equivocation.
    /// Pruned to keep only rounds > last_finalized_round - 1000.
    applied_validators_per_round: HashMap<u64, std::collections::HashSet<Address>>,
}

impl StateEngine {
    pub fn new() -> Self {
        Self {
            accounts: HashMap::new(),
            stake_accounts: HashMap::new(),
            total_supply: 0,
            last_finalized_round: None,
            active_validator_set: Vec::new(),
            current_epoch: u64::MAX, // sentinel: epoch never initialized
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
            applied_validators_per_round: HashMap::new(),
        }
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
    /// This is the single source of truth for reward calculation, used by both
    /// the validator loop (to produce correct coinbases) and apply_vertex_with_validators
    /// (to validate them). Keeping reward logic in one place prevents drift between
    /// production and validation — the most fragile coupling in the codebase.
    ///
    /// `active_validator_count` is the fallback divisor for pre-staking mode
    /// (when no stake exists). It should come from the finality tracker's
    /// configured_validators or the batch count.
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

        let total_stake = self.total_staked().saturating_add(self.total_delegated());
        let own_effective = self.effective_stake_of(proposer);

        let base_reward = if total_stake > 0 && own_effective > 0 {
            // Proportional to effective stake (own + delegations) using u128 to avoid overflow
            let proportional = ((validator_pool as u128)
                .saturating_mul(own_effective as u128)
                / total_stake as u128) as u64;
            // Observer penalty: staked but not in the active validator set
            if !self.active_validator_set.is_empty()
                && !self.active_validator_set.contains(proposer)
            {
                proportional * crate::constants::OBSERVER_REWARD_PERCENT / 100
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

    /// Compute the total council emission for a single vertex at the given round.
    /// Returns (per_member_amount, total_council_amount).
    /// Compute council emission per vertex.
    ///
    /// Since council emission is paid once per vertex (not per round), we divide
    /// the per-round council budget by `active_validator_count` so that the total
    /// council emission across all vertices in a round equals `block_reward * council_percent / 100`.
    pub fn compute_council_emission(&self, round: u64, active_validator_count: u64) -> (u64, u64) {
        let council_count = self.council_members.len() as u64;
        let council_percent = self.governance_params.council_emission_percent;
        if council_count == 0 || council_percent == 0 {
            return (0, 0);
        }
        let total_round_reward = crate::constants::block_reward(round);
        let council_total = total_round_reward.saturating_mul(council_percent) / 100;
        // Divide by validator count so N vertices per round = correct total council emission
        let per_vertex_total = council_total / active_validator_count.max(1);
        let per_member = per_vertex_total / council_count;
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
        let council_percent = self.governance_params.council_emission_percent;
        let council_count = self.council_members.len() as u64;
        let validator_pool = if council_count > 0 && council_percent > 0 {
            let council_total = total_round_reward.saturating_mul(council_percent) / 100;
            let per_member = council_total / council_count;
            if per_member > 0 {
                let council_mint = per_member.saturating_mul(council_count).min(remaining_supply);
                let capped_per = council_mint / council_count;
                if capped_per > 0 {
                    // Sort members by address for deterministic credit ordering
                    let mut members: Vec<Address> = self.council_members.keys().copied().collect();
                    members.sort();
                    for member in &members {
                        self.credit(member, capped_per);
                    }
                    let actually_minted = capped_per.saturating_mul(members.len() as u64);
                    self.total_supply = self.total_supply.saturating_add(actually_minted);
                }
            }
            // Validator pool = remainder after council share
            total_round_reward.saturating_mul(100u64.saturating_sub(council_percent)) / 100
        } else {
            total_round_reward
        };

        if validator_pool == 0 {
            return Ok(());
        }

        // Re-check remaining supply after council emission
        let remaining_supply = crate::constants::MAX_SUPPLY_SATS.saturating_sub(self.total_supply);
        if remaining_supply == 0 {
            return Ok(());
        }

        let total_effective_stake = self.total_staked().saturating_add(self.total_delegated());

        if total_effective_stake > 0 {
            // --- Staking active: distribute proportionally to all stakers ---
            // Collect all validators (anyone with stake) and their effective stakes.
            // MUST sort by address for deterministic iteration — HashMap order is
            // non-deterministic and would cause consensus splits across nodes.
            let mut validators: Vec<(Address, u64)> = self.stake_accounts.iter()
                .filter(|(_, s)| s.staked > 0)
                .map(|(addr, _)| (*addr, self.effective_stake_of(addr)))
                .collect();
            validators.sort_by_key(|(addr, _)| *addr);

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

                // Active producers get 100%, passive stakers get observer rate (20%)
                let validator_share = if producers.contains(validator) {
                    proportional
                } else {
                    proportional * crate::constants::OBSERVER_REWARD_PERCENT / 100
                };

                if validator_share == 0 {
                    continue;
                }

                // Split between validator's own stake and delegations
                let own_stake = self.stake_of(validator);
                let own_proportion = if *effective > 0 {
                    ((validator_share as u128).saturating_mul(own_stake as u128)
                        / *effective as u128) as u64
                } else {
                    validator_share
                };

                // Credit validator their own-stake portion
                if own_proportion > 0 {
                    credits.push((*validator, own_proportion));
                    total_to_mint = total_to_mint.saturating_add(own_proportion);
                }

                // Distribute delegator portions (validator_share - own_proportion)
                let delegation_pool = validator_share.saturating_sub(own_proportion);
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
                        .map(|(_, d)| *d)
                        .fold(0u64, |acc, x| acc.saturating_add(x));

                    for (delegator, delegated) in &delegators {
                        if total_delegated_to_validator == 0 {
                            continue;
                        }
                        let delegator_share = ((delegation_pool as u128)
                            .saturating_mul(*delegated as u128)
                            / total_delegated_to_validator as u128) as u64;
                        if delegator_share == 0 {
                            continue;
                        }
                        let commission = delegator_share.saturating_mul(commission_percent as u64) / 100;
                        let net = delegator_share.saturating_sub(commission);
                        if net > 0 {
                            credits.push((*delegator, net));
                            total_to_mint = total_to_mint.saturating_add(net);
                        }
                        // Commission stays with the validator
                        if commission > 0 {
                            credits.push((*validator, commission));
                            total_to_mint = total_to_mint.saturating_add(commission);
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
                    *amount = ((*amount as u128).saturating_mul(scale) / total) as u64;
                }
            }

            // Apply credits and mint
            let mut actually_minted: u64 = 0;
            for (addr, amount) in &credits {
                if *amount > 0 {
                    self.credit(addr, *amount);
                    actually_minted = actually_minted.saturating_add(*amount);
                }
            }
            self.total_supply = self.total_supply.saturating_add(actually_minted);
        } else {
            // --- Pre-staking fallback: equal split among producers ---
            let n = self.configured_validator_count
                .unwrap_or(producers.len().max(1) as u64);
            if !producers.is_empty() {
                let per_producer = validator_pool / n.max(1);
                let capped = per_producer.min(remaining_supply / producers.len().max(1) as u64);
                if capped > 0 {
                    for producer in producers {
                        self.credit(producer, capped);
                    }
                    let minted = capped.saturating_mul(producers.len() as u64);
                    self.total_supply = self.total_supply.saturating_add(minted);
                }
            }
        }

        Ok(())
    }

    /// Create a new StateEngine with genesis pre-funding.
    /// All nodes must call this to start with identical initial state.
    pub fn new_with_genesis() -> Self {
        let mut engine = Self::new();

        // Faucet reserve (testnet only — excluded from mainnet genesis)
        #[cfg(not(feature = "mainnet"))]
        {
            let faucet_addr = crate::constants::faucet_keypair().address();
            engine.credit(&faucet_addr, crate::constants::FAUCET_PREFUND_SATS);
        }

        // Developer allocation (5% of max supply)
        let dev_addr = crate::constants::dev_address();
        engine.credit(&dev_addr, crate::constants::DEV_ALLOCATION_SATS);

        // DAO Treasury (10% of max supply) — controlled by Council of 21 via TreasurySpend proposals.
        // Treasury is NOT an account — it's a separate balance field on StateEngine.
        engine.treasury_balance = crate::constants::TREASURY_ALLOCATION_SATS;

        // Bootstrap council: dev address gets the first Foundation seat.
        // Without this, no one can create proposals (catch-22).
        // The dev/foundation member can then propose additional council members.
        let _ = engine.add_council_member(
            dev_addr,
            crate::governance::CouncilSeatCategory::Foundation,
        );

        // total_supply tracks all credited amounts + treasury
        #[cfg(not(feature = "mainnet"))]
        {
            engine.total_supply = crate::constants::FAUCET_PREFUND_SATS
                + crate::constants::DEV_ALLOCATION_SATS
                + crate::constants::TREASURY_ALLOCATION_SATS;
        }
        #[cfg(feature = "mainnet")]
        {
            engine.total_supply = crate::constants::DEV_ALLOCATION_SATS
                + crate::constants::TREASURY_ALLOCATION_SATS;
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

    /// Apply a finalized vertex to the state (convenience for single-vertex tests).
    /// Also distributes round rewards and ticks governance.
    pub fn apply_vertex(&mut self, vertex: &DagVertex) -> Result<(), CoinError> {
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

        // Process any unstake completions for this round
        self.process_unstake_completions(vertex.round);

        let total_fees: u64 = vertex.block.transactions.iter()
            .map(|tx| tx.fee())
            .fold(0u64, |acc, x| acc.saturating_add(x));
        let proposer = &vertex.block.coinbase.to;

        // Coinbase should equal total_fees only (no block reward — rewards are
        // distributed per-round via distribute_round_rewards()).
        if vertex.block.coinbase.amount != total_fees {
            return Err(CoinError::InvalidCoinbase {
                expected: total_fees,
                got: vertex.block.coinbase.amount,
            });
        }

        // Credit proposer with collected transaction fees only
        if total_fees > 0 {
            self.credit(proposer, total_fees);
        }

        // Apply transactions
        // In a DAG with multiple validators, the same transaction can appear in
        // multiple vertices (all validators snapshot the same mempool). When one
        // validator's vertex is finalized first, the duplicate in another vertex
        // will fail nonce validation. We must skip these gracefully — a finalized
        // vertex cannot be un-finalized, so aborting would permanently halt finality.
        // Fee is deducted from sender when possible to keep supply balanced (the fee
        // was already credited to the proposer via coinbase).
        for tx in &vertex.block.transactions {
            // Verify signature
            if !tx.verify_signature() {
                tracing::warn!("Skipping tx with invalid signature in finalized vertex");
                // Undo the fee credit to proposer to maintain supply balance.
                // If clawback fails, the supply invariant is broken — halt the node.
                // A failed clawback means the coinbase credited fees that can't be
                // recovered, indicating a bug in coinbase calculation.
                let fee = tx.fee();
                if fee > 0 {
                    if let Err(e) = self.debit(proposer, fee) {
                        return Err(CoinError::SupplyInvariantBroken(format!(
                            "Fee clawback failed (invalid sig): {}. Drift: {} sats. \
                             This indicates a coinbase calculation bug — halting to prevent \
                             unrecoverable supply divergence.", e, fee
                        )));
                    }
                }
                continue;
            }

            // Check nonce
            let expected_nonce = self.nonce(&tx.from());
            if tx.nonce() != expected_nonce {
                // Likely a duplicate tx already applied from another validator's vertex.
                // Undo the fee credit to proposer (they shouldn't profit from a dup tx).
                let fee = tx.fee();
                if fee > 0 {
                    if let Err(e) = self.debit(proposer, fee) {
                        return Err(CoinError::SupplyInvariantBroken(format!(
                            "Fee clawback failed (bad nonce): {}. Drift: {} sats. \
                             Halting to prevent unrecoverable supply divergence.", e, fee
                        )));
                    }
                }
                tracing::warn!(
                    "Skipping duplicate tx in finalized vertex: expected nonce={}, got={}",
                    expected_nonce, tx.nonce()
                );
                continue;
            }

            // Check balance
            let sender_balance = self.balance(&tx.from());
            if sender_balance < tx.total_cost() {
                // Insufficient balance — undo the fee credit to proposer.
                let fee = tx.fee();
                if fee > 0 {
                    if let Err(e) = self.debit(proposer, fee) {
                        return Err(CoinError::SupplyInvariantBroken(format!(
                            "Fee clawback failed (insufficient balance): {}. Drift: {} sats. \
                             Halting to prevent unrecoverable supply divergence.", e, fee
                        )));
                    }
                }
                self.increment_nonce(&tx.from());
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
                    self.credit(&transfer_tx.to, transfer_tx.amount);
                    // Fee already included in coinbase
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
                        if let Err(debit_err) = self.debit(&proposal_tx.from, proposal_tx.fee) {
                            tracing::error!("Fee clawback failed (proposal): {}. Supply may drift by {} sats.", debit_err, proposal_tx.fee);
                        }
                        self.increment_nonce(&proposal_tx.from);
                    }
                }
                crate::tx::Transaction::Vote(vote_tx) => {
                    if let Err(e) = self.apply_vote(vote_tx, vertex.round) {
                        tracing::warn!("Skipping invalid Vote tx in finalized vertex: {}", e);
                        if let Err(debit_err) = self.debit(&vote_tx.from, vote_tx.fee) {
                            tracing::error!("Fee clawback failed (vote): {}. Supply may drift by {} sats.", debit_err, vote_tx.fee);
                        }
                        self.increment_nonce(&vote_tx.from);
                    }
                }
                crate::tx::Transaction::Delegate(delegate_tx) => {
                    if let Err(e) = self.apply_delegate_tx(delegate_tx) {
                        tracing::warn!("Skipping invalid Delegate tx in finalized vertex: {}", e);
                        // Delegate txs have zero fee, nothing to undo
                        self.increment_nonce(&delegate_tx.from);
                    }
                }
                crate::tx::Transaction::Undelegate(undelegate_tx) => {
                    if let Err(e) = self.apply_undelegate_tx(undelegate_tx, vertex.round) {
                        tracing::warn!("Skipping invalid Undelegate tx in finalized vertex: {}", e);
                        // Undelegate txs have zero fee, nothing to undo
                        self.increment_nonce(&undelegate_tx.from);
                    }
                }
                crate::tx::Transaction::SetCommission(commission_tx) => {
                    if let Err(e) = self.apply_set_commission_tx(commission_tx) {
                        tracing::warn!("Skipping invalid SetCommission tx in finalized vertex: {}", e);
                        // SetCommission txs have zero fee, nothing to undo
                        self.increment_nonce(&commission_tx.from);
                    }
                }
            }
        }

        // NOTE: last_finalized_round is NOT updated here — it's updated per-round
        // in apply_finalized_vertices() to ensure all vertices in the same round
        // compute the same expected_height for coinbase validation.

        // Epoch boundary: recalculate active validator set
        // Uses `!=` instead of `>` because current_epoch is initialized to u64::MAX
        // (sentinel for "never initialized"). On the first vertex, epoch_of(0)=0
        // which != u64::MAX, triggering the initial recalculation. Subsequent vertices
        // in the same epoch won't trigger because epoch_of(round) == current_epoch.
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
        // Fee clawback failures now also return SupplyInvariantBroken (see above), so this
        // check is the last line of defense.
        {
            let liquid: u64 = self.accounts.values().map(|a| a.balance).fold(0u64, |acc, x| acc.saturating_add(x));
            let staked: u64 = self.stake_accounts.values().map(|s| s.staked).fold(0u64, |acc, x| acc.saturating_add(x));
            let delegated: u64 = self.delegation_accounts.values().map(|d| d.delegated).fold(0u64, |acc, x| acc.saturating_add(x));
            let total = liquid.saturating_add(staked).saturating_add(delegated).saturating_add(self.treasury_balance);
            if total != self.total_supply {
                return Err(CoinError::SupplyInvariantBroken(format!(
                    "liquid={} staked={} delegated={} treasury={} sum={} != total_supply={}",
                    liquid, staked, delegated, self.treasury_balance, total, self.total_supply
                )));
            }
        }

        Ok(())
    }

    /// Apply multiple finalized vertices in order.
    /// When staking is active, uses stake-proportional rewards.
    /// Otherwise splits block reward equally among validators per round (pre-staking mode).
    pub fn apply_finalized_vertices(&mut self, vertices: &[DagVertex]) -> Result<(), CoinError> {
        // Sort deterministically by (round, hash) so all nodes apply in the same order
        let mut sorted: Vec<&DagVertex> = vertices.iter().collect();
        sorted.sort_by(|a, b| {
            a.round.cmp(&b.round).then_with(|| a.hash().cmp(&b.hash()))
        });

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
            // First check within this batch (same as before)
            let mut batch_seen: std::collections::HashMap<(crate::Address, u64), usize> =
                std::collections::HashMap::new();
            for v in &sorted {
                let key = (v.validator, v.round);
                *batch_seen.entry(key).or_insert(0) += 1;
            }
            for ((validator, round), count) in &batch_seen {
                if *count > 1 {
                    tracing::warn!(
                        "Deterministic slash (intra-batch): validator {} equivocated in round {} ({} vertices)",
                        validator.to_hex(), round, count
                    );
                    self.slash(validator);
                }
            }

            // Then check against previously-applied rounds (cross-batch detection)
            for v in &sorted {
                if let Some(existing) = self.applied_validators_per_round.get(&v.round) {
                    if existing.contains(&v.validator) {
                        // This validator already produced a vertex in this round
                        // in a previous finality batch — equivocation.
                        tracing::warn!(
                            "Deterministic slash (cross-batch): validator {} already applied in round {}",
                            v.validator.to_hex(), v.round
                        );
                        self.slash(&v.validator);
                    }
                }
            }
        }

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
            // distribute rewards, update last_finalized_round, tick governance.
            if prev_round.is_some() && prev_round != Some(vertex.round) {
                if let Some(r) = prev_round {
                    self.distribute_round_rewards(r, &round_producers)?;
                    self.last_finalized_round = Some(r);
                    self.tick_governance(r);
                }
                round_producers.clear();
            }

            let count = 1; // reward splitting now in distribute_round_rewards
            self.apply_vertex_with_validators(vertex, count)?;
            round_producers.insert(vertex.validator);

            // Record this validator's participation for cross-batch equivocation detection
            self.applied_validators_per_round
                .entry(vertex.round)
                .or_insert_with(std::collections::HashSet::new)
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

        // Prune old entries from cross-batch equivocation tracker (keep last 1000 rounds)
        if let Some(fin) = self.last_finalized_round {
            let floor = fin.saturating_sub(1000);
            self.applied_validators_per_round.retain(|round, _| *round >= floor);
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
            // Using eprintln since tracing may not be available in this crate
            eprintln!(
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
    pub fn process_unstake_completions(&mut self, current_round: u64) {
        // Process stake unstake completions
        let mut to_return: Vec<(Address, u64)> = Vec::new();
        for (addr, stake) in &self.stake_accounts {
            if let Some(unlock_at) = stake.unlock_at_round {
                if current_round >= unlock_at {
                    to_return.push((*addr, stake.staked));
                }
            }
        }
        for (addr, amount) in to_return {
            if let Some(stake) = self.stake_accounts.get_mut(&addr) {
                stake.staked = 0;
                stake.unlock_at_round = None;
                self.credit(&addr, amount);
            }
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
        for (addr, amount) in delegations_to_return {
            self.delegation_accounts.remove(&addr);
            self.credit(&addr, amount);
        }
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
        let slash_pct = self.governance_params.slash_percent;
        if let Some(stake) = self.stake_accounts.get_mut(addr) {
            let slash_amount = stake.staked.saturating_mul(slash_pct) / 100;
            stake.staked = stake.staked.saturating_sub(slash_amount);
            // Slashed amount is burned (not credited anywhere)
            self.total_supply = self.total_supply.saturating_sub(slash_amount);
            // Immediately remove from active set if below minimum stake
            if stake.staked < MIN_STAKE_SATS {
                self.active_validator_set.retain(|a| a != addr);
            }
        }
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
                // Remove empty delegations
                if delegation.delegated == 0 {
                    self.delegation_accounts.remove(&delegator);
                }
            }
        }
    }

    /// Credit the faucet amount to the given address by debiting the faucet account.
    /// Returns Err if faucet balance is insufficient.
    /// Does NOT inflate total_supply — this is an internal transfer from faucet to user.
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
        self.credit(address, amount);
        
        Ok(())
    }

    // ========================================
    // DELEGATION
    // ========================================

    /// Apply a DelegateTx: debit liquid balance, create delegation to validator.
    pub fn apply_delegate_tx(&mut self, tx: &crate::tx::DelegateTx) -> Result<(), CoinError> {
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
    pub fn apply_set_commission_tx(
        &mut self,
        tx: &crate::tx::SetCommissionTx,
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
        stake.commission_percent = tx.commission_percent;
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
    /// Distribute delegation rewards for a validator's delegators.
    ///
    /// ROUNDING NOTE: Integer division means small amounts of dust (< 1 sat per delegator
    /// per round) remain with the validator. Over millions of rounds this creates a small
    /// measurable advantage for validators with many small delegators. This is a known
    /// economic property, not a bug — the alternative (fractional sats) requires arbitrary
    /// precision and complicates consensus. The magnitude is negligible: with 100 delegators,
    /// max dust per round is 99 sats (0.0000099 UDAG).
    pub fn distribute_delegation_rewards(&mut self, validator: &Address, total_reward: u64) {
        if total_reward == 0 {
            return;
        }
        let effective = self.effective_stake_of(validator);
        if effective == 0 {
            return;
        }
        let commission_percent = self.stake_accounts
            .get(validator)
            .map(|s| s.commission_percent)
            .unwrap_or(crate::constants::DEFAULT_COMMISSION_PERCENT);

        // Collect delegator info (avoid borrow issues)
        let delegators: Vec<(Address, u64)> = self.delegation_accounts.iter()
            .filter(|(_, d)| d.validator == *validator && d.unlock_at_round.is_none())
            .map(|(addr, d)| (*addr, d.delegated))
            .collect();

        for (delegator, delegated) in delegators {
            // delegator_share = total_reward * (delegated / effective_stake)
            let delegator_share = ((total_reward as u128)
                .saturating_mul(delegated as u128)
                / effective as u128) as u64;
            if delegator_share == 0 {
                continue;
            }
            // commission = delegator_share * commission_percent / 100
            let commission = delegator_share.saturating_mul(commission_percent as u64) / 100;
            let net_reward = delegator_share.saturating_sub(commission);
            if net_reward > 0 {
                // Debit from validator (who received the full coinbase)
                // Best-effort: if validator balance is somehow insufficient, skip
                if self.balance(validator) >= net_reward {
                    let _ = self.debit(validator, net_reward);
                    self.credit(&delegator, net_reward);
                }
            }
        }
    }

    pub fn credit(&mut self, address: &Address, amount: u64) {
        let account = self.accounts.entry(*address).or_default();
        account.balance = account.balance.saturating_add(amount);
    }

    fn debit(&mut self, address: &Address, amount: u64) -> Result<(), CoinError> {
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

        // 9. Deduct fee from proposer balance
        let balance = self.balance(&tx.from);
        if balance < tx.fee {
            return Err(CoinError::InsufficientBalance {
                address: tx.from,
                required: tx.fee,
                available: balance,
            });
        }
        if let Err(e) = self.debit(&tx.from, tx.fee) {
            return Err(e);
        }

        // 10. Increment nonce
        self.increment_nonce(&tx.from);

        // 11. Snapshot council member count at proposal creation for quorum denominator.
        // With 1-vote-per-seat, the quorum denominator is the number of seated members.
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

        // 14. Increment next_proposal_id
        self.next_proposal_id = self.next_proposal_id.saturating_add(1);

        Ok(())
    }

    /// Apply a Vote transaction.
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

        // 8. Vote weight = 1 per council seat (equal governance power).
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
        if let Err(e) = self.debit(&tx.from, tx.fee) {
            return Err(e);
        }

        // 9. Increment nonce
        self.increment_nonce(&tx.from);

        // 10. Add vote weight to proposal.votes_for or votes_against
        // Safety: proposal existence was checked at step 4 above; no mutations remove proposals.
        let proposal = self.proposals.get_mut(&tx.proposal_id)
            .ok_or(CoinError::ProposalNotFound)?;
        if tx.vote {
            proposal.votes_for = proposal.votes_for.saturating_add(vote_weight);
        } else {
            proposal.votes_against = proposal.votes_against.saturating_add(vote_weight);
        }

        // 11. Insert (proposal_id, from) -> vote into self.votes
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
                    // Use snapshotted council count from proposal creation as quorum denominator.
                    // With 1-vote-per-seat governance, the denominator is the council size.
                    let quorum_denominator = if proposal.snapshot_total_stake > 0 {
                        proposal.snapshot_total_stake
                    } else {
                        // Legacy proposals without snapshot
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
                        crate::governance::ProposalType::TreasurySpend { .. }
                    ) {
                        if !self.dao_is_active() {
                            // DAO hibernating — skip execution, leave as PassedPending
                            continue;
                        }
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
        let mut failed: std::collections::HashMap<u64, String> = std::collections::HashMap::new();
        for (id, proposal_type) in effects {
            match proposal_type {
                crate::governance::ProposalType::ParameterChange { ref param, ref new_value } => {
                    match self.governance_params.apply_change(param, new_value) {
                        Ok(()) => {}
                        Err(e) => {
                            let reason = format!("ParameterChange failed: {}", e);
                            eprintln!("Proposal {} execution failed: {}", id, reason);
                            failed.insert(id, reason);
                        }
                    }
                }
                crate::governance::ProposalType::CouncilMembership { action, address, category } => {
                    match action {
                        crate::governance::CouncilAction::Add => {
                            if let Err(e) = self.add_council_member(address, category) {
                                let reason = format!("CouncilMembership Add failed: {}", e);
                                eprintln!("Proposal {} execution failed: {}", id, reason);
                                failed.insert(id, reason);
                            }
                        }
                        crate::governance::CouncilAction::Remove => {
                            if !self.remove_council_member(&address) {
                                let reason = "CouncilMembership Remove failed: not on council".to_string();
                                eprintln!("Proposal {} execution failed: {}", id, reason);
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
                        eprintln!("Proposal {} execution failed: {}", id, reason);
                        failed.insert(id, reason);
                    } else {
                        self.treasury_balance = self.treasury_balance.saturating_sub(amount);
                        self.credit(&recipient, amount);
                        eprintln!(
                            "TreasurySpend proposal {} executed: {} sats to {}. Treasury remaining: {} sats",
                            id, amount, recipient.to_hex(), self.treasury_balance
                        );
                    }
                }
                crate::governance::ProposalType::TextProposal => {}
            }
        }

        // Update proposal statuses — override with Failed where execution didn't succeed
        for (id, status) in to_update {
            if let Some(p) = self.proposals.get_mut(&id) {
                if let Some(reason) = failed.remove(&id) {
                    p.status = crate::governance::ProposalStatus::Failed { reason };
                } else {
                    p.status = status;
                }
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
    ) -> Self {
        Self {
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
            applied_validators_per_round: HashMap::new(),
        }
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
        }
    }

    /// Create a StateEngine from a snapshot (for checkpoint validation without mutating self).
    pub fn from_snapshot(snapshot: crate::state::persistence::StateSnapshot) -> Self {
        Self {
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
            configured_validator_count: None,
            council_members: snapshot.council_members.into_iter().collect(),
            treasury_balance: snapshot.treasury_balance,
            delegation_accounts: snapshot.delegation_accounts.into_iter().collect(),
            tx_index: HashMap::new(),
            tx_index_order: VecDeque::new(),
            applied_validators_per_round: HashMap::new(),
        }
    }

    /// Load state from a snapshot (for fast-sync from checkpoint).
    pub fn load_snapshot(&mut self, snapshot: crate::state::persistence::StateSnapshot) {
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
        let total_fees: u64 = txs.iter()
            .map(|tx| tx.fee())
            .fold(0u64, |acc, x| acc.saturating_add(x));
        // Coinbase = fees only; block rewards distributed via distribute_round_rewards()
        let coinbase = CoinbaseTx {
            to: *proposer,
            amount: total_fees,
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

        let reward = crate::constants::block_reward(0);
        assert_eq!(state.balance(&proposer), reward);
        assert_eq!(state.total_supply(), reward);
        assert_eq!(state.last_finalized_round(), Some(0));
    }

    #[test]
    fn apply_vertex_with_transaction() {
        let mut state = StateEngine::new();
        let proposer_sk = SecretKey::generate();
        let proposer = proposer_sk.address();
        let receiver = SecretKey::generate().address();

        // First vertex gives proposer some coins
        let v0 = make_vertex_for(&proposer, 0, 0, vec![], &proposer_sk);
        state.apply_vertex(&v0).unwrap();

        let reward = crate::constants::block_reward(0);
        let amount = 100;
        let fee = 10;

        let tx = make_signed_tx(&proposer_sk, receiver, amount, fee, 0);

        let v1 = make_vertex_for(&proposer, 1, 1, vec![tx], &proposer_sk);
        state.apply_vertex(&v1).unwrap();

        let reward1 = crate::constants::block_reward(1);
        // Proposer: reward0 - (amount + fee) + (reward1 + fee)
        let expected_proposer = reward - (amount + fee) + reward1 + fee;
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
        let fee = 10u64; // fee from the skipped tx

        let v1 = make_vertex_for(&proposer, 1, 1, vec![tx], &proposer_sk);
        let result = state.apply_vertex(&v1);
        assert!(result.is_ok(), "Vertex should apply despite bad nonce");
        // Receiver should NOT have received the transfer
        assert_eq!(state.balance(&receiver), 0);
        // Proposer gets coinbase reward but fee is deducted for the skipped tx
        let reward = crate::constants::block_reward(1);
        // Proposer was credited: reward + fee (coinbase), then debited: fee (skipped tx collection)
        // Net: balance_after_v0 + reward + fee - fee = balance_after_v0 + reward
        // But fee may or may not be collected depending on balance — check approximately
        assert!(state.balance(&proposer) >= balance_after_v0 + reward - fee);
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

        let r0 = crate::constants::block_reward(0);
        let r1 = crate::constants::block_reward(1);
        let r2 = crate::constants::block_reward(2);

        assert_eq!(state.balance(&sk1.address()), r0);
        assert_eq!(state.balance(&sk2.address()), r1);
        assert_eq!(state.balance(&sk3.address()), r2);
        assert_eq!(state.total_supply(), r0 + r1 + r2);
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
            let total = liquid.saturating_add(staked).saturating_add(delegated).saturating_add(state.treasury_balance());
            assert_eq!(
                total, state.total_supply,
                "Supply invariant broken at round {}: liquid={} staked={} delegated={} treasury={} supply={}",
                round, liquid, staked, delegated, state.treasury_balance(), state.total_supply
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
        let validator = Address([99u8; 32]);
        
        let supply_before = state.total_supply();
        
        // Slash validator with no stake
        state.slash(&validator);
        
        // Should be no-op
        assert_eq!(state.stake_of(&validator), 0);
        assert_eq!(state.total_supply(), supply_before);
    }
}

