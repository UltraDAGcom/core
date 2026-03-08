use reqwest::blocking::Client;
use serde_json::Value;

use crate::error::{Result, UltraDagError};
use crate::types::*;

/// Default RPC base URL.
const DEFAULT_BASE_URL: &str = "http://localhost:10333";

/// Synchronous HTTP client for an UltraDAG node's RPC API.
///
/// All amounts are denominated in sats (1 UDAG = 100,000,000 sats).
///
/// # Example
///
/// ```no_run
/// use ultradag_sdk::UltraDagClient;
///
/// let client = UltraDagClient::new("http://localhost:10333");
/// let status = client.status().unwrap();
/// println!("DAG round: {}", status.dag_round);
/// ```
pub struct UltraDagClient {
    base_url: String,
    http: Client,
}

impl UltraDagClient {
    /// Create a new client pointing at the given base URL.
    ///
    /// The URL should include the scheme and port, e.g. `"http://localhost:10333"`.
    pub fn new(base_url: &str) -> Self {
        let http = Client::builder()
            .no_proxy()
            .build()
            .expect("failed to build HTTP client");
        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            http,
        }
    }

    /// Create a client using the default base URL (`http://localhost:10333`).
    pub fn default_local() -> Self {
        Self::new(DEFAULT_BASE_URL)
    }

    /// Return the base URL this client is configured with.
    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    // -----------------------------------------------------------------------
    // GET endpoints
    // -----------------------------------------------------------------------

    /// `GET /health` — check node liveness.
    pub fn health(&self) -> Result<HealthResponse> {
        self.get("/health")
    }

    /// `GET /status` — node status including DAG metrics, supply, peers, etc.
    pub fn status(&self) -> Result<StatusResponse> {
        self.get("/status")
    }

    /// `GET /balance/{address}` — account balance and nonce.
    ///
    /// `address` is a 64-character hex string.
    pub fn balance(&self, address: &str) -> Result<BalanceResponse> {
        validate_hex_address(address)?;
        self.get(&format!("/balance/{address}"))
    }

    /// `GET /round/{round}` — list vertices produced in a specific round.
    pub fn round(&self, round: u64) -> Result<Vec<VertexResponse>> {
        self.get(&format!("/round/{round}"))
    }

    /// `GET /mempool` — list pending transactions (top 100 by fee).
    pub fn mempool(&self) -> Result<Vec<Value>> {
        self.get("/mempool")
    }

    /// `GET /keygen` — ask the node to generate a new keypair.
    ///
    /// For offline key generation, prefer [`crate::crypto::Keypair::generate`].
    pub fn keygen(&self) -> Result<KeygenResponse> {
        self.get("/keygen")
    }

    /// `GET /peers` — connected peers and bootstrap node info.
    pub fn peers(&self) -> Result<PeersResponse> {
        self.get("/peers")
    }

    /// `GET /validators` — active validator list with stake amounts.
    pub fn validators(&self) -> Result<ValidatorsResponse> {
        self.get("/validators")
    }

    /// `GET /stake/{address}` — staking info for an address.
    pub fn stake_info(&self, address: &str) -> Result<StakeInfoResponse> {
        validate_hex_address(address)?;
        self.get(&format!("/stake/{address}"))
    }

    /// `GET /governance/config` — governance configuration.
    pub fn governance_config(&self) -> Result<Value> {
        self.get("/governance/config")
    }

    /// `GET /proposals` — list governance proposals.
    pub fn proposals(&self) -> Result<ProposalsResponse> {
        self.get("/proposals")
    }

    /// `GET /proposal/{id}` — details of a single proposal.
    pub fn proposal(&self, id: u64) -> Result<Value> {
        self.get(&format!("/proposal/{id}"))
    }

    /// `GET /vote/{proposal_id}/{address}` — check a vote on a proposal.
    pub fn vote_info(&self, proposal_id: u64, address: &str) -> Result<Value> {
        validate_hex_address(address)?;
        self.get(&format!("/vote/{proposal_id}/{address}"))
    }

    // -----------------------------------------------------------------------
    // POST endpoints
    // -----------------------------------------------------------------------

    /// `POST /tx` — submit a signed transaction.
    ///
    /// `secret_key` is the sender's hex-encoded Ed25519 secret key.
    /// Amounts are in sats.
    pub fn send_tx(
        &self,
        secret_key: &str,
        to: &str,
        amount: u64,
        fee: u64,
    ) -> Result<TxResponse> {
        let body = SendTxRequest {
            from_secret: secret_key.to_string(),
            to: to.to_string(),
            amount,
            fee,
        };
        self.post("/tx", &body)
    }

    /// `POST /faucet` — request testnet tokens.
    pub fn faucet(&self, address: &str, amount: u64) -> Result<Value> {
        let body = FaucetRequest {
            address: address.to_string(),
            amount,
        };
        self.post("/faucet", &body)
    }

    /// `POST /stake` — stake UDAG as a validator.
    ///
    /// `secret_key` is the staker's hex-encoded Ed25519 secret key.
    /// `amount` is in sats.
    pub fn stake(&self, secret_key: &str, amount: u64) -> Result<Value> {
        let body = StakeRequest {
            secret_key: secret_key.to_string(),
            amount,
        };
        self.post("/stake", &body)
    }

    /// `POST /unstake` — begin unstake cooldown.
    pub fn unstake(&self, secret_key: &str) -> Result<Value> {
        let body = UnstakeRequest {
            secret_key: secret_key.to_string(),
        };
        self.post("/unstake", &body)
    }

    /// `POST /proposal` — submit a governance proposal.
    pub fn submit_proposal(&self, req: &ProposalRequest) -> Result<Value> {
        self.post("/proposal", req)
    }

    /// `POST /vote` — cast a vote on a governance proposal.
    pub fn vote(&self, req: &VoteRequest) -> Result<Value> {
        self.post("/vote", req)
    }

    // -----------------------------------------------------------------------
    // Internal helpers
    // -----------------------------------------------------------------------

    fn get<T: serde::de::DeserializeOwned>(&self, path: &str) -> Result<T> {
        let url = format!("{}{}", self.base_url, path);
        let resp = self.http.get(&url).send()?;
        handle_response(resp)
    }

    fn post<B: serde::Serialize, T: serde::de::DeserializeOwned>(
        &self,
        path: &str,
        body: &B,
    ) -> Result<T> {
        let url = format!("{}{}", self.base_url, path);
        let resp = self.http.post(&url).json(body).send()?;
        handle_response(resp)
    }
}

/// Parse a response, mapping non-2xx status codes to [`UltraDagError::Api`].
fn handle_response<T: serde::de::DeserializeOwned>(
    resp: reqwest::blocking::Response,
) -> Result<T> {
    let status = resp.status();
    if !status.is_success() {
        let code = status.as_u16();
        let text = resp.text().unwrap_or_default();
        return Err(UltraDagError::Api {
            status: code,
            message: text,
        });
    }
    let text = resp.text()?;
    let value: T = serde_json::from_str(&text)?;
    Ok(value)
}

/// Validate that an address string looks like 64 hex characters.
fn validate_hex_address(address: &str) -> Result<()> {
    if address.len() != 64 {
        return Err(UltraDagError::InvalidAddress(format!(
            "expected 64 hex chars, got {}",
            address.len()
        )));
    }
    if !address.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(UltraDagError::InvalidAddress(
            "contains non-hex characters".into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_hex_address_ok() {
        let addr = "a".repeat(64);
        assert!(validate_hex_address(&addr).is_ok());
    }

    #[test]
    fn validate_hex_address_too_short() {
        assert!(validate_hex_address("abcd").is_err());
    }

    #[test]
    fn validate_hex_address_non_hex() {
        let bad = format!("{}zz", "a".repeat(62));
        assert!(validate_hex_address(&bad).is_err());
    }

    #[test]
    fn default_local_url() {
        let c = UltraDagClient::default_local();
        assert_eq!(c.base_url(), "http://localhost:10333");
    }

    #[test]
    fn trailing_slash_stripped() {
        let c = UltraDagClient::new("http://example.com:10333/");
        assert_eq!(c.base_url(), "http://example.com:10333");
    }
}
