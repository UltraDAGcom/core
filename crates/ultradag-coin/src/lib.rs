//! UltraDAG Core Protocol
//!
//! A high-performance DAG-BFT consensus engine for permissioned networks and IoT applications.
//!
//! # Architecture
//!
//! UltraDAG uses a directed acyclic graph (DAG) structure instead of a linear blockchain,
//! enabling parallel block production by multiple validators. The protocol achieves
//! Byzantine fault tolerance with deterministic finality—once a vertex is finalized,
//! it cannot be reverted.
//!
//! ## Key Components
//!
//! - [`consensus::BlockDag`] - The core DAG data structure for vertex storage and finality tracking
//! - [`state::StateEngine`] - Account-based state machine with staking and governance
//! - [`tx::Mempool`] - Transaction pool with fee-based ordering
//! - [`governance::Proposal`] - On-chain governance with Council of 21
//!
//! # Security Model
//!
//! - **BFT Consensus**: Requires 2/3+1 validators for finality (tolerates f Byzantine faults where n ≥ 3f+1)
//! - **Equivocation Slashing**: 50% stake burn for validators producing conflicting vertices
//! - **Supply Invariant**: Hard cap of 21M UDAG enforced with checked arithmetic
//! - **Key Management**: Mainnet requires offline key generation (no hardcoded secrets)
//!
//! # Tokenomics
//!
//! - **Max Supply**: 21,000,000 UDAG (2.1 quadrillion sats)
//! - **Initial Reward**: 1 UDAG per round (halving every 10.5M rounds)
//! - **Validator Rewards**: Proportional to stake when staking is active
//! - **Council Emission**: 10% of block rewards distributed to Council of 21
//!
//! # Example
//!
//! ```rust,no_run
//! use ultradag_coin::{BlockDag, FinalityTracker, StateEngine, Mempool};
//!
//! // Initialize core components
//! let mut dag = BlockDag::new();
//! let mut finality = FinalityTracker::new(4); // Minimum 4 validators for BFT
//! let mut state = StateEngine::new_with_genesis();
//! let mempool = Mempool::new();
//! ```
//!
//! # Safety Guarantees
//!
//! - **Deterministic Finality**: Vertices finalize when 2/3+ validators have descendants
//! - **Causal Ordering**: Vertices sorted by (round, topological depth, hash)
//! - **Supply Invariant**: `liquid + staked + delegated + treasury + bridge == total_supply`
//! - **No Double Spend**: Equivocation detected in O(1) via validator_round_vertex index

pub mod address;
pub mod block;
pub mod bridge;
pub mod consensus;
pub mod constants;
pub mod error;
pub mod block_producer;
pub mod governance;
pub mod persistence;
pub mod safety;
pub mod state;
pub mod tx;

pub use address::{Address, SecretKey, Signature};
pub use block::{Block, BlockHeader};
pub use consensus::{BlockDag, DagVertex, FinalityTracker, ValidatorSet, sync_epoch_validators, Checkpoint, EquivocationEvidence, K_PARENTS, MAX_PARENTS};
pub use constants::{COIN, SATS_PER_UDAG, sats_to_udag, DEV_ADDRESS_SEED, DEV_ALLOCATION_SATS, EPOCH_LENGTH_ROUNDS, HALVING_INTERVAL, INITIAL_REWARD_SATS, MAX_ACTIVE_VALIDATORS, MAX_SUPPLY_SATS, OBSERVER_REWARD_PERCENT, CHECKPOINT_INTERVAL, MIN_DELEGATION_SATS, MIN_BRIDGE_AMOUNT_SATS, DEFAULT_COMMISSION_PERCENT, MAX_COMMISSION_PERCENT, SUPPORTED_BRIDGE_CHAIN_IDS, block_reward, dev_address, epoch_of, is_epoch_boundary};
#[cfg(not(feature = "mainnet"))]
pub use constants::{FAUCET_PREFUND_SATS, FAUCET_SEED, faucet_keypair};
pub use error::CoinError;
pub use block_producer::create_block;
pub use state::{StateEngine, TxLocation};
pub use tx::{CoinbaseTx, Mempool, Transaction, TransferTx, StakeTx, UnstakeTx, DelegateTx, UndelegateTx, SetCommissionTx, BridgeDepositTx, MIN_STAKE_SATS, UNSTAKE_COOLDOWN_ROUNDS};
