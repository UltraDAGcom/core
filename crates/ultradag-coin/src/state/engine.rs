use std::collections::HashMap;

use crate::address::Address;
use crate::consensus::vertex::DagVertex;
use crate::error::CoinError;
use crate::tx::stake::{StakeTx, UnstakeTx, MIN_STAKE_SATS, UNSTAKE_COOLDOWN_ROUNDS};

/// Account balance state.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AccountState {
    pub balance: u64,
    pub nonce: u64,
}

impl Default for AccountState {
    fn default() -> Self {
        Self {
            balance: 0,
            nonce: 0,
        }
    }
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
    last_finalized_round: Option<u64>,
    /// Epoch-frozen active validator set (top MAX_ACTIVE_VALIDATORS by stake).
    /// Recalculated only at epoch boundaries.
    active_validator_set: Vec<Address>,
    /// Current epoch number (round / EPOCH_LENGTH_ROUNDS).
    current_epoch: u64,
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
        }
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
        self.apply_vertex_with_validators(vertex, 1)
    }

    /// Apply a finalized vertex with known validator count for reward splitting.
    pub fn apply_vertex_with_validators(
        &mut self,
        vertex: &DagVertex,
        active_validator_count: u64,
    ) -> Result<(), CoinError> {
        // Apply to a snapshot first to ensure atomicity
        let mut snapshot = self.clone();

        let total_fees: u64 = vertex.block.transactions.iter().map(|tx| tx.fee).sum();
        let total_round_reward = crate::constants::block_reward(vertex.block.coinbase.height);
        let proposer = &vertex.block.coinbase.to;
        let total_stake = snapshot.total_staked();
        let own_stake = snapshot.stake_of(proposer);

        let validator_reward = if total_stake > 0 && own_stake > 0 {
            // Proportional to stake using u128 to avoid overflow
            let base = ((total_round_reward as u128)
                .saturating_mul(own_stake as u128)
                / total_stake as u128) as u64;
            // Observer penalty: staked but not in the active validator set
            if !snapshot.active_validator_set.is_empty()
                && !snapshot.active_validator_set.contains(proposer)
            {
                base * crate::constants::OBSERVER_REWARD_PERCENT / 100
            } else {
                base
            }
        } else {
            // Pre-staking fallback: equal split among active validators in this round.
            // This handles the transition period before staking is active.
            let n = active_validator_count.max(1);
            total_round_reward / n
        };

        // Validate coinbase claims correct amount
        let expected_coinbase = validator_reward.saturating_add(total_fees);

        if vertex.block.coinbase.amount != expected_coinbase {
            return Err(CoinError::InvalidCoinbase {
                expected: expected_coinbase,
                got: vertex.block.coinbase.amount,
            });
        }

        // Supply cap enforcement
        let max_supply = crate::constants::MAX_SUPPLY_SATS;
        let mut capped_reward = validator_reward;
        if snapshot.total_supply + capped_reward > max_supply {
            capped_reward = max_supply.saturating_sub(snapshot.total_supply);
        }
        snapshot.total_supply += capped_reward;

        // Credit proposer: capped block reward + fees
        snapshot.credit(proposer, capped_reward + total_fees);

        // Apply transactions
        for tx in &vertex.block.transactions {
            // Verify signature
            if !tx.verify_signature() {
                return Err(CoinError::InvalidSignature);
            }

            // Check balance
            let sender_balance = snapshot.balance(&tx.from);
            if sender_balance < tx.total_cost() {
                return Err(CoinError::InsufficientBalance {
                    address: tx.from,
                    required: tx.total_cost(),
                    available: sender_balance,
                });
            }

            // Check nonce
            let expected_nonce = snapshot.nonce(&tx.from);
            if tx.nonce != expected_nonce {
                return Err(CoinError::InvalidNonce {
                    expected: expected_nonce,
                    got: tx.nonce,
                });
            }

            // Debit sender
            snapshot.debit(&tx.from, tx.total_cost());
            snapshot.increment_nonce(&tx.from);

            // Credit recipient
            snapshot.credit(&tx.to, tx.amount);

            // Fee already included in coinbase
        }

        // Update last finalized round
        snapshot.last_finalized_round = Some(vertex.round);

        // Epoch boundary: recalculate active validator set
        let new_epoch = crate::constants::epoch_of(vertex.round);
        if new_epoch > snapshot.current_epoch || snapshot.active_validator_set.is_empty() {
            snapshot.recalculate_active_set();
            snapshot.current_epoch = new_epoch;
        }

        // Supply invariant check (debug builds only)
        // sum(liquid balances) + sum(staked) == total_supply
        #[cfg(debug_assertions)]
        {
            let liquid: u64 = snapshot.accounts.values().map(|a| a.balance).sum();
            let staked: u64 = snapshot.stake_accounts.values().map(|s| s.staked).sum();
            assert_eq!(
                liquid + staked, snapshot.total_supply,
                "Supply invariant broken: liquid={} staked={} total_supply={}",
                liquid, staked, snapshot.total_supply
            );
        }

        // All transactions valid — commit snapshot
        *self = snapshot;
        Ok(())
    }

    /// Apply multiple finalized vertices in order.
    /// When staking is active, uses stake-proportional rewards.
    /// Otherwise each vertex gets the full block reward (pre-staking mode).
    pub fn apply_finalized_vertices(&mut self, vertices: &[DagVertex]) -> Result<(), CoinError> {
        if self.total_staked() > 0 {
            // Stake-proportional mode: validator count per round for equal-split fallback
            let mut round_counts: std::collections::HashMap<u64, u64> = std::collections::HashMap::new();
            for v in vertices {
                *round_counts.entry(v.round).or_insert(0) += 1;
            }
            for vertex in vertices {
                let count = round_counts.get(&vertex.round).copied().unwrap_or(1);
                self.apply_vertex_with_validators(vertex, count)?;
            }
        } else {
            // Pre-staking mode: each vertex gets full block_reward (backward compatible)
            for vertex in vertices {
                self.apply_vertex_with_validators(vertex, 1)?;
            }
        }
        Ok(())
    }

    /// Total UDAG currently staked across all validators.
    pub fn total_staked(&self) -> u64 {
        self.stake_accounts.values().map(|s| s.staked).sum()
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
        self.debit(&tx.from, tx.amount);
        // Credit stake account
        let stake = self.stake_accounts.entry(tx.from).or_default();
        stake.staked += tx.amount;
        stake.unlock_at_round = None;
        // Increment nonce
        self.accounts.entry(tx.from).or_default().nonce += 1;
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
        stake.unlock_at_round = Some(current_round + UNSTAKE_COOLDOWN_ROUNDS);
        self.accounts.entry(tx.from).or_default().nonce += 1;
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
            let stake = self.stake_accounts.get_mut(&addr).unwrap();
            stake.staked = 0;
            stake.unlock_at_round = None;
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
        const SLASH_PERCENTAGE: u64 = 50;
        if let Some(stake) = self.stake_accounts.get_mut(addr) {
            let slash_amount = stake.staked * SLASH_PERCENTAGE / 100;
            stake.staked -= slash_amount;
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
        self.debit(&faucet_addr, amount);
        self.credit(address, amount);
        
        Ok(())
    }

    fn credit(&mut self, address: &Address, amount: u64) {
        let account = self.accounts.entry(*address).or_default();
        account.balance += amount;
    }

    fn debit(&mut self, address: &Address, amount: u64) {
        let account = self.accounts.entry(*address).or_default();
        account.balance = account.balance.saturating_sub(amount);
    }

    fn increment_nonce(&mut self, address: &Address) {
        let account = self.accounts.entry(*address).or_default();
        account.nonce += 1;
    }

    /// Save state to disk
    pub fn save(&self, path: &std::path::Path) -> Result<(), crate::persistence::PersistenceError> {
        let snapshot = crate::state::persistence::StateSnapshot {
            accounts: self.accounts.iter().map(|(k, v)| (*k, v.clone())).collect(),
            stake_accounts: self.stake_accounts.iter().map(|(k, v)| (*k, v.clone())).collect(),
            active_validator_set: self.active_validator_set.clone(),
            current_epoch: self.current_epoch,
            total_supply: self.total_supply,
            last_finalized_round: self.last_finalized_round,
        };
        snapshot.save(path)
    }

    /// Load state from disk.
    /// If the persisted epoch is stale (node was down for multiple epochs),
    /// recalculates the active validator set to match the actual epoch.
    pub fn load(path: &std::path::Path) -> Result<Self, crate::persistence::PersistenceError> {
        let snapshot = crate::state::persistence::StateSnapshot::load(path)?;
        let mut engine = Self {
            accounts: snapshot.accounts.into_iter().collect(),
            stake_accounts: snapshot.stake_accounts.into_iter().collect(),
            active_validator_set: snapshot.active_validator_set,
            current_epoch: snapshot.current_epoch,
            total_supply: snapshot.total_supply,
            last_finalized_round: snapshot.last_finalized_round,
        };
        // Reconcile epoch after loading stale snapshot
        if let Some(round) = engine.last_finalized_round {
            let expected_epoch = crate::constants::epoch_of(round);
            if expected_epoch != engine.current_epoch {
                engine.recalculate_active_set();
                engine.current_epoch = expected_epoch;
            }
        }
        Ok(engine)
    }

    /// Check if saved state exists
    pub fn exists(path: &std::path::Path) -> bool {
        crate::state::persistence::StateSnapshot::exists(path)
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
        let mut tx = Transaction {
            from: sk.address(),
            to,
            amount,
            fee,
            nonce,
            pub_key: sk.verifying_key().to_bytes(),
            signature: Signature([0u8; 64]),
        };
        tx.signature = sk.sign(&tx.signable_bytes());
        tx
    }

    fn make_vertex_for(
        proposer: &Address,
        round: u64,
        height: u64,
        txs: Vec<Transaction>,
        sk: &SecretKey,
    ) -> DagVertex {
        let total_fees: u64 = txs.iter().map(|tx| tx.fee).sum();
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
    fn insufficient_balance_rejected() {
        let mut state = StateEngine::new();
        let proposer_sk = SecretKey::generate();
        let proposer = proposer_sk.address();
        let sender_sk = SecretKey::generate();
        let receiver = SecretKey::generate().address();

        // Give proposer coins, not sender
        let v0 = make_vertex_for(&proposer, 0, 0, vec![], &proposer_sk);
        state.apply_vertex(&v0).unwrap();

        // sender has 0 balance, tries to send 100
        let tx = make_signed_tx(&sender_sk, receiver, 100, 10, 0);

        let v1 = make_vertex_for(&proposer, 1, 1, vec![tx], &proposer_sk);
        let result = state.apply_vertex(&v1);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            CoinError::InsufficientBalance { .. }
        ));
    }

    #[test]
    fn invalid_nonce_rejected() {
        let mut state = StateEngine::new();
        let proposer_sk = SecretKey::generate();
        let proposer = proposer_sk.address();
        let receiver = SecretKey::generate().address();

        let v0 = make_vertex_for(&proposer, 0, 0, vec![], &proposer_sk);
        state.apply_vertex(&v0).unwrap();

        // nonce should be 0, but we pass 5
        let tx = make_signed_tx(&proposer_sk, receiver, 100, 10, 5);

        let v1 = make_vertex_for(&proposer, 1, 1, vec![tx], &proposer_sk);
        let result = state.apply_vertex(&v1);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            CoinError::InvalidNonce { expected: 0, got: 5 }
        ));
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
        let vertex = make_vertex_for(&proposer, 0, 0, vec![], &sk);
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

        let vertex = make_vertex_for(&proposer, 0, 0, vec![], &sk);
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
}
