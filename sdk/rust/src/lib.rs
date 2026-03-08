//! # ultradag-sdk
//!
//! Lightweight HTTP client SDK for interacting with UltraDAG nodes.
//!
//! This crate wraps the UltraDAG node's HTTP RPC API and provides local
//! Ed25519 key generation with Blake3 address derivation.
//!
//! ## Quick Start
//!
//! ```no_run
//! use ultradag_sdk::{UltraDagClient, crypto::Keypair};
//!
//! // Connect to a local node
//! let client = UltraDagClient::default_local();
//!
//! // Check node health
//! let health = client.health().unwrap();
//! assert_eq!(health.status, "ok");
//!
//! // Generate a keypair offline
//! let kp = Keypair::generate();
//! println!("Address: {}", kp.address_hex());
//!
//! // Check balance
//! let bal = client.balance(&kp.address_hex()).unwrap();
//! println!("Balance: {} sats", bal.balance);
//! ```

pub mod client;
pub mod crypto;
pub mod error;
pub mod types;

pub use client::UltraDagClient;
pub use crypto::Keypair;
pub use error::{Result, UltraDagError};
pub use types::*;
