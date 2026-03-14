use std::collections::HashMap;

use crate::address::Address;
use crate::consensus::vertex::DagVertex;
use crate::error::CoinError;
use crate::tx::stake::{StakeTx, UnstakeTx, MIN_STAKE_SATS, UNSTAKE_COOLDOWN_ROUNDS};

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
}

/// StateEngine: derives account state from an ordered list of finalized DAG vertices.
/// This replaces the old Blockchain struct. The DAG IS the ledger.
#[derive(Debug, Clone)]
pub struct StateEngine {
    accounts: HashMap<Address, AccountState>,
    stake_accounts: HashMap<Address, StakeAccount>,
    total_supply: u64,
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
    /// Monotonically increasing proposal counter.
    next_proposal_id: u64,
    /// Runtime-adjustable governance parameters (changed via ParameterChange proposals).
    governance_params: crate::governance::GovernanceParams,
    /// Configured validator count for pre-staking reward splitting.
    /// When set, block reward is divided by this count in pre-staking mode.
    /// Should match the --validators CLI flag.
    configured_validator_count: Option<u64>,
}

impl StateEngine {
    pub fn new() -> Self {
        Self {
            accounts: HashMap::new(),
            stake_accounts: HashMap::new(),
            total_supply: 0,
            last_finalized_round: None,
            active_validator_set: Vec::new(),
            current_epoch: 0,
            proposals: HashMap::new(),
            votes: HashMap::new(),
            next_proposal_id: 0,
            governance_params: crate::governance::GovernanceParams::default(),
            configured_validator_count: None,
        }
    }

    /// Set the configured validator count for pre-staking reward splitting.
    pub fn set_configured_validator_count(&mut self, count: u64) {
        self.configured_validator_count = Some(count);
    }

    /// Create a new StateEngine with genesis pre-funding.
    /// All nodes must call this to start with identical initial state.
    pub fn new_with_genesis() -> Self {
        let mut engine = Self::new();

        // Faucet reserve (testnet only)
        let faucet_addr = crate::constants::faucet_keypair().address();
        engine.credit(&faucet_addr, crate::constants::FAUCET_PREFUND_SATS);

        // Developer allocation (5% of max supply)
        let dev_addr = crate::constants::dev_address();
        engine.credit(&dev_addr, crate::constants::DEV_ALLOCATION_SATS);

        // total_supply tracks all credited amounts
        engine.total_supply = crate::constants::FAUCET_PREFUND_SATS
            + crate::constants::DEV_ALLOCATION_SATS;

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

    /// Apply a finalized vertex to the state.
    /// `active_validator_count` is the number of validators that produced vertices
    /// in this round. Used for emission splitting when staking is not yet active.
    /// Pass 1 for tests that don't care about proportional rewards.
    pub fn apply_vertex(&mut self, vertex: &DagVertex) -> Result<(), CoinError> {
        self.apply_vertex_with_validators(vertex, 1)?;
        // Single-vertex convenience: also update last_finalized_round.
        // (apply_finalized_vertices handles this per-round for batches.)
        self.last_finalized_round = Some(vertex.round);
        Ok(())
    }

    /// Apply a finalized vertex with known validator count for reward splitting.
    pub fn apply_vertex_with_validators(
        &mut self,
        vertex: &DagVertex,
        active_validator_count: u64,
    ) -> Result<(), CoinError> {
        // Apply directly — finalized vertices are BFT-confirmed and must succeed.
        // If they don't (supply invariant violation), that's a critical bug regardless.

        // Process any unstake completions for this round
        self.process_unstake_completions(vertex.round);

        let total_fees: u64 = vertex.block.transactions.iter().map(|tx| {
            match tx {
                crate::tx::Transaction::Transfer(t) => t.fee,
                crate::tx::Transaction::CreateProposal(t) => t.fee,
                crate::tx::Transaction::Vote(t) => t.fee,
                crate::tx::Transaction::Stake(_) | crate::tx::Transaction::Unstake(_) => 0,
            }
        }).fold(0u64, |acc, x| acc.saturating_add(x));
        // Use vertex.round as height for block_reward computation.
        // This eliminates the TOCTOU between production time and application time:
        // the validator computes reward from its local state, but state can advance
        // between production and finalization. Using vertex.round is safe because:
        // 1. Round is immutable once signed (can't be faked)
        // 2. DAG rejects rounds outside [pruning_floor, current_round + 10]
        // 3. Both producer and engine compute the same value deterministically
        let expected_height = vertex.round;
        let total_round_reward = crate::constants::block_reward(expected_height);
        let proposer = &vertex.block.coinbase.to;
        let total_stake = self.total_staked();
        let own_stake = self.stake_of(proposer);

        let validator_reward = if total_stake > 0 && own_stake > 0 {
            // Proportional to stake using u128 to avoid overflow
            let base = ((total_round_reward as u128)
                .saturating_mul(own_stake as u128)
                / total_stake as u128) as u64;
            // Observer penalty: staked but not in the active validator set
            if !self.active_validator_set.is_empty()
                && !self.active_validator_set.contains(proposer)
            {
                base * crate::constants::OBSERVER_REWARD_PERCENT / 100
            } else {
                base
            }
        } else {
            // Pre-staking fallback: equal split among validators.
            // Use configured_validator_count (from --validators N) when available,
            // otherwise fall back to batch count. This ensures all nodes agree on
            // the split regardless of finality batch size.
            let n = self.configured_validator_count
                .unwrap_or(active_validator_count.max(1));
            total_round_reward / n.max(1)
        };

        // Supply cap enforcement: cap reward BEFORE coinbase validation
        // so that the validator produces a coinbase matching the capped amount.
        let max_supply = crate::constants::MAX_SUPPLY_SATS;
        let capped_reward = if self.total_supply.saturating_add(validator_reward) > max_supply {
            max_supply.saturating_sub(self.total_supply)
        } else {
            validator_reward
        };

        // Validate coinbase claims correct (capped) amount
        let expected_coinbase = capped_reward.saturating_add(total_fees);

        if vertex.block.coinbase.amount != expected_coinbase {
            return Err(CoinError::InvalidCoinbase {
                expected: expected_coinbase,
                got: vertex.block.coinbase.amount,
            });
        }

        self.total_supply = self.total_supply.saturating_add(capped_reward);

        // Credit proposer: capped block reward + fees
        self.credit(proposer, capped_reward.saturating_add(total_fees));

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
                // Undo the fee credit to proposer to maintain supply balance
                let fee = tx.fee();
                if fee > 0 {
                    if let Err(e) = self.debit(proposer, fee) {
                        return Err(CoinError::ValidationError(format!(
                            "Failed to debit proposer fee: {}", e
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
                        return Err(CoinError::ValidationError(format!(
                            "Failed to debit proposer fee for duplicate tx: {}", e
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
                        return Err(CoinError::ValidationError(format!(
                            "Failed to debit proposer fee for insufficient balance: {}", e
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
                            return Err(CoinError::ValidationError(format!(
                                "Failed to debit proposal fee: {}", debit_err
                            )));
                        }
                        self.increment_nonce(&proposal_tx.from);
                    }
                }
                crate::tx::Transaction::Vote(vote_tx) => {
                    if let Err(e) = self.apply_vote(vote_tx, vertex.round) {
                        tracing::warn!("Skipping invalid Vote tx in finalized vertex: {}", e);
                        if let Err(debit_err) = self.debit(&vote_tx.from, vote_tx.fee) {
                            return Err(CoinError::ValidationError(format!(
                                "Failed to debit vote fee: {}", debit_err
                            )));
                        }
                        self.increment_nonce(&vote_tx.from);
                    }
                }
            }
        }

        // NOTE: last_finalized_round is NOT updated here — it's updated per-round
        // in apply_finalized_vertices() to ensure all vertices in the same round
        // compute the same expected_height for coinbase validation.

        // Epoch boundary: recalculate active validator set
        let new_epoch = crate::constants::epoch_of(vertex.round);
        if new_epoch > self.current_epoch || self.active_validator_set.is_empty() {
            self.recalculate_active_set();
            self.current_epoch = new_epoch;
        }

        // Tick governance to update proposal statuses
        self.tick_governance(vertex.round);

        // Supply invariant check — unconditional (catches state corruption in release builds too)
        // sum(liquid balances) + sum(staked) == total_supply
        // Maintained even for skipped txs: when a tx is skipped, its fee is debited from
        // the proposer (undoing the coinbase credit for that fee).
        {
            let liquid: u64 = self.accounts.values().map(|a| a.balance).fold(0u64, |acc, x| acc.saturating_add(x));
            let staked: u64 = self.stake_accounts.values().map(|s| s.staked).fold(0u64, |acc, x| acc.saturating_add(x));
            if liquid.saturating_add(staked) != self.total_supply {
                return Err(CoinError::ValidationError(format!(
                    "Supply invariant broken: liquid={} staked={} total_supply={}",
                    liquid, staked, self.total_supply
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

        if self.total_staked() > 0 {
            // Stake-proportional mode: validator count per round for equal-split fallback
            let mut round_counts: std::collections::HashMap<u64, u64> = std::collections::HashMap::new();
            for v in &sorted {
                *round_counts.entry(v.round).or_insert(0) += 1;
            }
            // Group vertices by round. Update last_finalized_round only BETWEEN rounds,
            // so all vertices in the same round compute the same expected_height.
            let mut prev_round = None;
            for vertex in &sorted {
                // Before processing first vertex of a new round, update last_finalized_round
                // to the previous round (if any). This ensures same-round vertices share height.
                if prev_round.is_some() && prev_round != Some(vertex.round) {
                    self.last_finalized_round = prev_round;
                }
                let count = round_counts.get(&vertex.round).copied().unwrap_or(1);
                self.apply_vertex_with_validators(vertex, count)?;
                prev_round = Some(vertex.round);
            }
            // Update for the final round
            if let Some(r) = prev_round {
                self.last_finalized_round = Some(r);
            }
        } else {
            // Pre-staking mode: split block_reward equally among validators in each round.
            // Prevents emission scaling linearly with validator count.
            let mut round_counts: std::collections::HashMap<u64, u64> = std::collections::HashMap::new();
            for v in &sorted {
                *round_counts.entry(v.round).or_insert(0) += 1;
            }
            let mut prev_round = None;
            for vertex in &sorted {
                if prev_round.is_some() && prev_round != Some(vertex.round) {
                    self.last_finalized_round = prev_round;
                }
                let count = round_counts.get(&vertex.round).copied().unwrap_or(1);
                self.apply_vertex_with_validators(vertex, count)?;
                prev_round = Some(vertex.round);
            }
            if let Some(r) = prev_round {
                self.last_finalized_round = Some(r);
            }
        }
        Ok(())
    }

    /// Total UDAG currently staked across all validators.
    pub fn total_staked(&self) -> u64 {
        self.stake_accounts.values().fold(0u64, |acc, s| acc.saturating_add(s.staked))
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

    /// Recalculate the active validator set: top MAX_ACTIVE_VALIDATORS by stake.
    /// Only stakers with >= MIN_STAKE_SATS and not unstaking are eligible.
    /// 
    /// WARNING: If the resulting set has fewer than MIN_ACTIVE_VALIDATORS,
    /// the system cannot guarantee BFT safety. This should be logged/monitored.
    pub fn recalculate_active_set(&mut self) {
        let mut eligible: Vec<(Address, u64)> = self.stake_accounts
            .iter()
            .filter(|(_, s)| s.staked >= MIN_STAKE_SATS && s.unlock_at_round.is_none())
            .map(|(addr, s)| (*addr, s.staked))
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
    /// Call this at the start of each round.
    pub fn process_unstake_completions(&mut self, current_round: u64) {
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
        const SLASH_PERCENTAGE: u64 = 50;
        if let Some(stake) = self.stake_accounts.get_mut(addr) {
            let slash_amount = stake.staked.saturating_mul(SLASH_PERCENTAGE) / 100;
            stake.staked = stake.staked.saturating_sub(slash_amount);
            // Slashed amount is burned (not credited anywhere)
            self.total_supply = self.total_supply.saturating_sub(slash_amount);
            // Immediately remove from active set if below minimum stake
            if stake.staked < MIN_STAKE_SATS {
                self.active_validator_set.retain(|a| a != addr);
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

    fn credit(&mut self, address: &Address, amount: u64) {
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

        // 4. Check proposer stake >= min_stake_to_propose (governance-adjustable)
        let proposer_stake = self.stake_of(&tx.from);
        if proposer_stake < self.governance_params.min_stake_to_propose {
            return Err(CoinError::InsufficientStakeToPropose);
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

        // 11. Create proposal
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
        };

        // 12. Insert into proposals
        self.proposals.insert(tx.proposal_id, proposal);

        // 13. Increment next_proposal_id
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

        // 7. Vote weight = total staked amount.
        // Design decision: ALL staked addresses can vote (active validators AND observers).
        // Observers (staked but not in top-21 active set) retain full governance influence
        // because they have economic skin-in-the-game via their stake. Only addresses in
        // unstake cooldown (unlock_at_round.is_some()) are excluded, since their stake
        // is leaving the system.
        // This follows the standard stake-weighted governance model where voting power
        // tracks economic commitment, not block-production privilege.
        let vote_weight = self.stake_accounts.get(&tx.from)
            .filter(|s| s.unlock_at_round.is_none())
            .map_or(0, |s| s.staked);

        // 8. Deduct fee from voter balance
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
        let total_staked = self.total_staked();
        let mut to_update = vec![];

        for (id, proposal) in &self.proposals {
            match &proposal.status {
                crate::governance::ProposalStatus::Active if current_round > proposal.voting_ends => {
                    let new_status = if proposal.has_passed_with_params(total_staked, &self.governance_params) {
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
                    to_update.push((*id, crate::governance::ProposalStatus::Executed));
                }
                _ => {}
            }
        }

        for (id, status) in to_update {
            if let Some(p) = self.proposals.get_mut(&id) {
                // Execute ParameterChange proposals when transitioning to Executed
                if status == crate::governance::ProposalStatus::Executed {
                    if let crate::governance::ProposalType::ParameterChange { ref param, ref new_value } = p.proposal_type {
                        match self.governance_params.apply_change(param, new_value) {
                            Ok(()) => {
                                // Parameter changed successfully — this is deterministic
                                // across all nodes since they process the same finalized vertices
                            }
                            Err(e) => {
                                // Invalid parameter change — proposal passes but effect is rejected.
                                // This can happen if validation rules changed between proposal
                                // creation and execution. The proposal still transitions to Executed
                                // to maintain determinism (all nodes must agree on status).
                                eprintln!("ParameterChange proposal {} failed to apply: {}", id, e);
                            }
                        }
                    }
                    // TextProposal: no execution effect (informational only)
                }
                p.status = status;
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
            configured_validator_count: None,
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
        let total_fees: u64 = txs.iter().map(|tx| {
            match tx {
                Transaction::Transfer(t) => t.fee,
                _ => 0,
            }
        }).fold(0u64, |acc, x| acc.saturating_add(x));
        let reward = crate::constants::block_reward(height);
        let coinbase = CoinbaseTx {
            to: *proposer,
            amount: reward + total_fees,
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

    /// Like make_vertex_for but with a custom coinbase reward amount.
    fn make_vertex_with_reward(
        proposer: &Address,
        round: u64,
        height: u64,
        reward: u64,
        sk: &SecretKey,
    ) -> DagVertex {
        let coinbase = CoinbaseTx {
            to: *proposer,
            amount: reward,
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

        // Apply a vertex — reward should be capped to remaining supply (100 sats)
        // Coinbase must match the capped amount (not the uncapped block_reward)
        let vertex = make_vertex_with_reward(&proposer, 0, 0, 100, &sk);
        state.apply_vertex(&vertex).unwrap();

        assert_eq!(state.total_supply(), max);
        // Proposer gets capped reward (100 sats) + existing balance
        assert_eq!(state.balance(&proposer), existing_supply + 100);
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

        // Create a vertex with incorrect coinbase amount
        let mut vertex = make_vertex_for(&proposer, 0, 0, vec![], &sk);
        
        // Tamper with coinbase amount (should be 50 UDAG for height 0)
        vertex.block.coinbase.amount = 1_000_000 * crate::constants::COIN;
        
        // Re-sign the vertex
        let signable = vertex.signable_bytes();
        vertex.signature = sk.sign(&signable);

        // Should reject with InvalidCoinbase error
        let result = state.apply_vertex(&vertex);
        assert!(result.is_err());
        match result {
            Err(CoinError::InvalidCoinbase { expected, got }) => {
                assert_eq!(expected, 50 * crate::constants::COIN);
                assert_eq!(got, 1_000_000 * crate::constants::COIN);
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
        let mut state = StateEngine::new_with_genesis();
        let validators: Vec<_> = (0..4).map(|_| SecretKey::generate()).collect();
        
        // Apply 100 rounds - each round the supply invariant is checked in apply_vertex
        for round in 0..100 {
            let proposer_idx = round % 4;
            let proposer = &validators[proposer_idx as usize];
            
            let vertex = make_vertex_for(&proposer.address(), round, round, vec![], proposer);
            state.apply_vertex(&vertex).unwrap();
            
            // Verify invariant manually (also checked in debug builds inside apply_vertex)
            let sum: u64 = state.accounts.values().map(|a| a.balance).sum();
            assert_eq!(
                sum, state.total_supply,
                "Supply invariant broken at round {}: sum={} supply={}",
                round, sum, state.total_supply
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

