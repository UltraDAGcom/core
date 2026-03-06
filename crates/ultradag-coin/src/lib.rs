pub mod address;
pub mod block;
pub mod consensus;
pub mod constants;
pub mod error;
pub mod block_producer;
pub mod persistence;
pub mod state;
pub mod tx;

pub use address::{Address, SecretKey, Signature};
pub use block::{Block, BlockHeader};
pub use consensus::{BlockDag, DagVertex, FinalityTracker, ValidatorSet};
pub use constants::{COIN, HALVING_INTERVAL, INITIAL_REWARD_SATS, MAX_SUPPLY_SATS};
pub use error::CoinError;
pub use block_producer::create_block;
pub use state::StateEngine;
pub use tx::{CoinbaseTx, Mempool, Transaction};
