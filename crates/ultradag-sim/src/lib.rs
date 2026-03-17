pub mod network;
pub mod validator;
pub mod byzantine;
pub mod harness;
pub mod invariants;
pub mod txgen;
pub mod fuzz;
pub mod oracle;
pub mod properties;
// P2P integration test module — uses tokio, reqwest, tempfile.
// Always compiled because test crates need access to the types,
// and dev-dependencies provide the required crates.
pub mod p2p;
