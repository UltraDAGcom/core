use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Number of satoshi units per 1 UDAG.
pub const SATS_PER_UDAG: u64 = 100_000_000;

/// Convert a sats amount to UDAG (floating-point).
pub fn sats_to_udag(sats: u64) -> f64 {
    sats as f64 / SATS_PER_UDAG as f64
}

/// Convert a UDAG amount (floating-point) to sats.
pub fn udag_to_sats(udag: f64) -> u64 {
    (udag * SATS_PER_UDAG as f64) as u64
}

// ---------------------------------------------------------------------------
// GET /health
// ---------------------------------------------------------------------------

/// Response from `GET /health`.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct HealthResponse {
    /// Should be `"ok"` when the node is healthy.
    pub status: String,
}

// ---------------------------------------------------------------------------
// GET /status
// ---------------------------------------------------------------------------

/// Response from `GET /status`.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct StatusResponse {
    /// Last finalized DAG round, if any.
    pub last_finalized_round: Option<u64>,
    /// Number of connected P2P peers.
    pub peer_count: usize,
    /// Number of pending transactions in the mempool.
    pub mempool_size: usize,
    /// Total supply in sats.
    pub total_supply: u64,
    /// Number of known accounts.
    pub account_count: usize,
    /// Total number of DAG vertices.
    pub dag_vertices: usize,
    /// Current DAG round.
    pub dag_round: u64,
    /// Number of DAG tip vertices.
    pub dag_tips: usize,
    /// Number of finalized vertices.
    pub finalized_count: usize,
    /// Number of known validators.
    pub validator_count: usize,
    /// Total staked amount in sats.
    pub total_staked: u64,
    /// Number of addresses with active stake.
    pub active_stakers: usize,
    /// Whether the node is connected to a bootstrap peer.
    #[serde(default)]
    pub bootstrap_connected: bool,
}

// ---------------------------------------------------------------------------
// GET /balance/{address}
// ---------------------------------------------------------------------------

/// Response from `GET /balance/{address}`.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct BalanceResponse {
    /// Account address (hex).
    pub address: String,
    /// Balance in sats.
    pub balance: u64,
    /// Current account nonce.
    pub nonce: u64,
    /// Balance expressed in UDAG.
    pub balance_tdag: f64,
}

// ---------------------------------------------------------------------------
// GET /round/{round}
// ---------------------------------------------------------------------------

/// A single DAG vertex summary returned by `GET /round/{round}`.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct VertexResponse {
    /// Round number.
    pub round: u64,
    /// Vertex hash (hex).
    pub hash: String,
    /// Validator address (hex).
    pub validator: String,
    /// Coinbase reward in sats.
    pub reward: u64,
    /// Number of transactions in the vertex.
    pub tx_count: usize,
    /// Number of parent vertices.
    pub parent_count: usize,
}

// ---------------------------------------------------------------------------
// GET /keygen
// ---------------------------------------------------------------------------

/// Response from `GET /keygen`.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct KeygenResponse {
    /// Ed25519 secret key seed (hex).
    pub secret_key: String,
    /// Derived address: blake3(ed25519_pubkey) (hex).
    pub address: String,
}

// ---------------------------------------------------------------------------
// GET /peers
// ---------------------------------------------------------------------------

/// A single bootstrap node entry.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct BootstrapNode {
    /// Address string (host:port).
    #[serde(alias = "addr")]
    pub address: String,
    /// Whether the node is currently connected.
    pub connected: bool,
}

/// Response from `GET /peers`.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PeersResponse {
    /// Number of connected peers.
    pub connected: usize,
    /// List of peer addresses.
    pub peers: Vec<String>,
    /// Bootstrap node status.
    pub bootstrap_nodes: Vec<BootstrapNode>,
}

// ---------------------------------------------------------------------------
// GET /validators
// ---------------------------------------------------------------------------

/// Information about a single validator.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ValidatorInfo {
    /// Validator address (hex).
    pub address: String,
    /// Amount staked in sats.
    pub stake: u64,
}

/// Response from `GET /validators`.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ValidatorsResponse {
    /// Number of active validators.
    pub count: usize,
    /// Total staked across all validators in sats.
    pub total_staked: u64,
    /// Per-validator details.
    pub validators: Vec<ValidatorInfo>,
}

// ---------------------------------------------------------------------------
// GET /stake/{address}
// ---------------------------------------------------------------------------

/// Response from `GET /stake/{address}`.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct StakeInfoResponse {
    /// Account address (hex).
    pub address: String,
    /// Amount staked in sats.
    pub staked: u64,
    /// Staked amount expressed in UDAG.
    pub staked_udag: f64,
    /// Round at which unstake cooldown completes, if unstaking.
    pub unlock_at_round: Option<u64>,
    /// Whether this address is in the active validator set.
    pub is_active_validator: bool,
}

// ---------------------------------------------------------------------------
// GET /proposals
// ---------------------------------------------------------------------------

/// Response from `GET /proposals`.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ProposalsResponse {
    /// Total number of proposals.
    pub count: usize,
    /// Proposal objects.
    pub proposals: Vec<serde_json::Value>,
}

// ---------------------------------------------------------------------------
// POST /tx
// ---------------------------------------------------------------------------

/// Request body for `POST /tx`.
#[derive(Debug, Clone, Serialize)]
pub struct SendTxRequest {
    /// Sender secret key (hex).
    pub from_secret: String,
    /// Recipient address (hex).
    pub to: String,
    /// Amount in sats.
    pub amount: u64,
    /// Fee in sats.
    pub fee: u64,
}

/// Response from `POST /tx`.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TxResponse {
    /// Transaction hash (hex).
    pub hash: String,
    /// Sender address (hex).
    pub from: String,
    /// Recipient address (hex).
    pub to: String,
    /// Amount in sats.
    pub amount: u64,
    /// Fee in sats.
    pub fee: u64,
    /// Nonce used.
    pub nonce: u64,
}

// ---------------------------------------------------------------------------
// POST /faucet
// ---------------------------------------------------------------------------

/// Request body for `POST /faucet`.
#[derive(Debug, Clone, Serialize)]
pub struct FaucetRequest {
    /// Recipient address (hex).
    pub address: String,
    /// Amount in sats.
    pub amount: u64,
}

// ---------------------------------------------------------------------------
// POST /stake
// ---------------------------------------------------------------------------

/// Request body for `POST /stake`.
#[derive(Debug, Clone, Serialize)]
pub struct StakeRequest {
    /// Staker secret key (hex).
    pub secret_key: String,
    /// Amount to stake in sats.
    pub amount: u64,
}

// ---------------------------------------------------------------------------
// POST /unstake
// ---------------------------------------------------------------------------

/// Request body for `POST /unstake`.
#[derive(Debug, Clone, Serialize)]
pub struct UnstakeRequest {
    /// Staker secret key (hex).
    pub secret_key: String,
}

// ---------------------------------------------------------------------------
// POST /proposal
// ---------------------------------------------------------------------------

/// Request body for `POST /proposal`.
#[derive(Debug, Clone, Serialize)]
pub struct ProposalRequest {
    /// Proposer secret key (hex).
    pub proposer_secret: String,
    /// Proposal title.
    pub title: String,
    /// Proposal description.
    pub description: String,
    /// Type of proposal (e.g. `"parameter_change"`).
    pub proposal_type: String,
    /// Name of the parameter to change, if applicable.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parameter_name: Option<String>,
    /// New value for the parameter, if applicable.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parameter_value: Option<String>,
    /// Fee in sats for submitting the proposal.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fee: Option<u64>,
}

// ---------------------------------------------------------------------------
// POST /vote
// ---------------------------------------------------------------------------

/// Request body for `POST /vote`.
#[derive(Debug, Clone, Serialize)]
pub struct VoteRequest {
    /// Voter secret key (hex).
    pub voter_secret: String,
    /// Proposal ID to vote on.
    pub proposal_id: u64,
    /// `true` for yes, `false` for no.
    pub vote: bool,
    /// Optional fee in sats.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fee: Option<u64>,
}
