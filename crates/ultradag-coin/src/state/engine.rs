use std::collections::HashMap;

use crate::address::Address;
use crate::consensus::vertex::DagVertex;
use crate::error::CoinError;

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

/// StateEngine: derives account state from an ordered list of finalized DAG vertices.
/// This replaces the old Blockchain struct. The DAG IS the ledger.
#[derive(Debug, Clone)]
pub struct StateEngine {
    accounts: HashMap<Address, AccountState>,
    total_supply: u64,
    /// Track the last finalized round we've applied
    last_finalized_round: Option<u64>,
}

impl StateEngine {
    pub fn new() -> Self {
        Self {
            accounts: HashMap::new(),
            total_supply: 0,
            last_finalized_round: None,
        }
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
    /// The vertex contains a batch of transactions that are now committed.
    /// Returns error if invalid. On failure, no state changes are committed (atomic).
    pub fn apply_vertex(&mut self, vertex: &DagVertex) -> Result<(), CoinError> {
        // Apply to a snapshot first to ensure atomicity
        let mut snapshot = self.clone();

        // Apply coinbase reward (full amount including fees)
        let proposer = &vertex.block.coinbase.to;
        let coinbase_amount = vertex.block.coinbase.amount;
        snapshot.credit(proposer, coinbase_amount);

        // Only the block reward is new supply; fees are transfers, not new money
        let mut block_reward = crate::constants::block_reward(vertex.block.coinbase.height);

        // Supply cap enforcement: cap reward if it would exceed MAX_SUPPLY_SATS
        let max_supply = crate::constants::MAX_SUPPLY_SATS;
        if snapshot.total_supply + block_reward > max_supply {
            block_reward = max_supply.saturating_sub(snapshot.total_supply);
        }
        snapshot.total_supply += block_reward;

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

        // All transactions valid — commit snapshot
        *self = snapshot;
        Ok(())
    }

    /// Apply multiple finalized vertices in order.
    /// This is the primary way to update state from DAG finality output.
    pub fn apply_finalized_vertices(&mut self, vertices: &[DagVertex]) -> Result<(), CoinError> {
        for vertex in vertices {
            self.apply_vertex(vertex)?;
        }
        Ok(())
    }

    /// Testnet faucet: directly credit an address with coins (no transaction needed).
    /// This bypasses normal consensus and is for testing purposes only.
    pub fn faucet_credit(&mut self, address: &Address, amount: u64) {
        self.credit(address, amount);
        self.total_supply += amount;
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
            total_supply: self.total_supply,
            last_finalized_round: self.last_finalized_round,
        };
        snapshot.save(path)
    }

    /// Load state from disk
    pub fn load(path: &std::path::Path) -> Result<Self, crate::persistence::PersistenceError> {
        let snapshot = crate::state::persistence::StateSnapshot::load(path)?;
        Ok(Self {
            accounts: snapshot.accounts.into_iter().collect(),
            total_supply: snapshot.total_supply,
            last_finalized_round: snapshot.last_finalized_round,
        })
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

        // Manually set total_supply close to max
        let max = crate::constants::MAX_SUPPLY_SATS;
        state.total_supply = max - 100; // Only 100 sats remaining

        // Apply a vertex — reward should be capped to remaining supply
        let vertex = make_vertex_for(&proposer, 0, 0, vec![], &sk);
        state.apply_vertex(&vertex).unwrap();

        assert_eq!(state.total_supply(), max);
        // Proposer gets full coinbase amount (reward + fees credited to account),
        // but only 100 sats count as new supply
        assert!(state.balance(&proposer) > 0);
    }

    #[test]
    fn supply_cap_zero_reward_at_max() {
        let mut state = StateEngine::new();
        let sk = SecretKey::generate();
        let proposer = sk.address();

        // Set total_supply to exactly max
        let max = crate::constants::MAX_SUPPLY_SATS;
        state.total_supply = max;

        let vertex = make_vertex_for(&proposer, 0, 0, vec![], &sk);
        state.apply_vertex(&vertex).unwrap();

        // Supply should not exceed max
        assert_eq!(state.total_supply(), max);
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
}
