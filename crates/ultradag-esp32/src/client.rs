//! HTTP client for the UltraDAG node RPC.
//!
//! Uses `esp-idf-svc`'s blocking HTTP client wrapper. Each request opens a
//! fresh TCP connection (no keep-alive) — the RPC is not latency-critical
//! for a light client polling every ~10 s, and short connections keep
//! memory usage flat.
//!
//! Endpoint contracts come from `crates/ultradag-node/src/rpc.rs`:
//!   - `GET /status`           → `StatusResponse` (subset of fields used)
//!   - `GET /balance/{addr}`   → `BalanceResponse` (subset)
//!
//! We deliberately only deserialize the fields we display; serde ignores
//! unknown fields by default, so new server-side fields won't break us.

use anyhow::{anyhow, Context, Result};
use esp_idf_svc::http::client::{Configuration, EspHttpConnection};
use esp_idf_svc::http::Method;
use esp_idf_svc::io::Write as _;
use serde::Deserialize;

/// Subset of `ultradag-node::rpc::StatusResponse` we care about.
#[derive(Debug, Deserialize)]
pub struct StatusResponse {
    pub last_finalized_round: Option<u64>,
    pub peer_count: usize,
    pub dag_round: u64,
    pub validator_count: usize,
    pub total_supply: u64,
    pub mempool_size: usize,
}

/// Subset of `ultradag-node::rpc::ValidatorInfo` we care about.
/// Only the address matters for the trust-on-first-use set hash —
/// stake amounts are ignored because a legitimate top-up or partial
/// unstake would change the root even though the identities haven't.
#[derive(Debug, Deserialize)]
pub struct ValidatorInfo {
    pub address: String,
}

/// Response shape for `GET /validators`. `count` is redundant with
/// `validators.len()` but we keep it to log directly.
#[derive(Debug, Deserialize)]
pub struct ValidatorsResponse {
    pub count: usize,
    pub validators: Vec<ValidatorInfo>,
}

/// Subset of `ultradag-node::rpc::BalanceResponse` we care about.
#[derive(Debug, Deserialize)]
pub struct BalanceResponse {
    pub address: String,
    pub balance: u64,
    pub balance_udag: f64,
    pub nonce: u64,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub is_smart_account: bool,
    /// Number of authorized keys on the SmartAccount (None if the
    /// address isn't a SmartAccount yet). Used by the SmartTransfer
    /// bootstrap flow to decide whether we still need to submit an
    /// AddKeyTx to create the account + auto-register our key.
    #[serde(default)]
    pub authorized_key_count: Option<usize>,
}

/// Blocking HTTP client wrapped around ESP-IDF's native transport.
///
/// Each request builds its own `EspHttpConnection`. We intentionally do NOT
/// reuse connections across requests: `EspHttpConnection` is a strict state
/// machine, and if one request fails partway through (e.g. a read error
/// mid-response) the state can't be recovered — the next `initiate_request`
/// call will panic with "connection is not in initial phase". A fresh
/// connection per request costs ~1 s for the HTTPS handshake, which is
/// trivial for a light client polling on a ~10 s cadence.
pub struct UltraDagClient {
    base_url: String,
}

impl UltraDagClient {
    /// Build a client targeting `base_url` (no trailing slash). The actual
    /// HTTPS connection is created lazily, one per request.
    pub fn new(base_url: &str) -> Result<Self> {
        Ok(Self { base_url: base_url.trim_end_matches('/').to_string() })
    }

    /// GET `{base_url}/status` and deserialize the subset of StatusResponse
    /// fields we display.
    pub fn get_status(&self) -> Result<StatusResponse> {
        let body = self.get_json("/status")?;
        serde_json::from_slice::<StatusResponse>(&body)
            .with_context(|| format!("parsing /status body: {}", body_preview(&body)))
    }

    /// GET `{base_url}/balance/{addr_hex}` and deserialize the subset of
    /// BalanceResponse fields we display.
    pub fn get_balance(&self, address_hex: &str) -> Result<BalanceResponse> {
        let path = format!("/balance/{}", address_hex);
        let body = self.get_json(&path)?;
        serde_json::from_slice::<BalanceResponse>(&body)
            .with_context(|| format!("parsing /balance body: {}", body_preview(&body)))
    }

    /// GET `{base_url}/validators` — returns the active validator set
    /// (address + stake per validator). Used by the TOFU validator-set
    /// check in the finality module. Note: this is the live active set,
    /// not a historical checkpoint — validators that leave mid-epoch
    /// will disappear here before a "real" finality endpoint would
    /// reflect the change.
    pub fn get_validators(&self) -> Result<ValidatorsResponse> {
        let body = self.get_json("/validators")?;
        serde_json::from_slice::<ValidatorsResponse>(&body)
            .with_context(|| format!("parsing /validators body: {}", body_preview(&body)))
    }

    /// `POST /faucet` — testnet-only faucet. Body: `{"address": ..., "amount": <sats>}`.
    /// Returns the raw JSON body on success so the caller can extract
    /// `tx_hash`, `nonce`, etc. Rate limited to 1 request / 10 min per IP.
    pub fn post_faucet(&self, address_hex: &str, amount_sats: u64) -> Result<serde_json::Value> {
        let body = serde_json::json!({
            "address": address_hex,
            "amount":  amount_sats,
        });
        let body_bytes = serde_json::to_vec(&body)?;
        let resp = self.post_json("/faucet", &body_bytes)?;
        serde_json::from_slice::<serde_json::Value>(&resp)
            .with_context(|| format!("parsing /faucet response: {}", body_preview(&resp)))
    }

    /// `POST /tx/submit` — submit a pre-signed transaction as JSON. The
    /// caller passes the already-wrapped `{"Transfer": {...}}` envelope so
    /// this method stays tx-type-agnostic. Returns the parsed server
    /// response as JSON (typically `{tx_hash, from, to, amount, fee, nonce}`).
    pub fn submit_tx(&self, tx_envelope: &serde_json::Value) -> Result<serde_json::Value> {
        let body_bytes = serde_json::to_vec(tx_envelope)?;
        let resp = self.post_json("/tx/submit", &body_bytes)?;
        serde_json::from_slice::<serde_json::Value>(&resp)
            .with_context(|| format!("parsing /tx/submit response: {}", body_preview(&resp)))
    }

    // ── internals ─────────────────────────────────────────────────────────

    /// Build a one-shot HTTPS connection and issue a GET. Returns the
    /// response body as bytes; caller deserializes. The connection is
    /// dropped when this function returns — including in the error path —
    /// so no state leaks between calls.
    ///
    /// Caps the body at 16 KB — anything bigger is almost certainly the
    /// wrong endpoint and we don't want to OOM a 4 MB chip.
    fn get_json(&self, path: &str) -> Result<Vec<u8>> {
        const MAX_BODY: usize = 16 * 1024;
        let url = format!("{}{}", self.base_url, path);

        let cfg = Configuration {
            // Attach the ESP-IDF crt_bundle so HTTPS to public hosts works
            // without shipping our own CA store. This is enabled in
            // sdkconfig.defaults via CONFIG_MBEDTLS_CERTIFICATE_BUNDLE.
            crt_bundle_attach: Some(esp_idf_svc::sys::esp_crt_bundle_attach),
            timeout: Some(std::time::Duration::from_secs(10)),
            buffer_size: Some(4096),
            buffer_size_tx: Some(1024),
            ..Default::default()
        };
        let mut conn = EspHttpConnection::new(&cfg).context("EspHttpConnection::new failed")?;

        let headers = [
            ("accept", "application/json"),
            ("connection", "close"),
        ];

        // EspHttpConnection is a state machine — `initiate_request` writes
        // the request headers and transitions into the "request body" state,
        // then `initiate_response` flushes the body (empty for GET) and
        // switches into "response reading" state.
        conn.initiate_request(Method::Get, &url, &headers)
            .with_context(|| format!("HTTP initiate GET {} failed", url))?;
        conn.initiate_response()
            .with_context(|| format!("HTTP initiate_response for {} failed", url))?;

        let status = conn.status();
        if !(200..=299).contains(&status) {
            return Err(anyhow!("HTTP {} from {}", status, url));
        }

        // Drain the body into a Vec. We use a small stack buffer to keep
        // stack usage bounded — ESP-IDF main task gets 16 KB of stack and
        // mbedTLS has already eaten a chunk of it during the TLS handshake.
        let mut buf = [0u8; 512];
        let mut body = Vec::with_capacity(1024);
        loop {
            match conn.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => {
                    if body.len() + n > MAX_BODY {
                        return Err(anyhow!("response body exceeded {} bytes for {}", MAX_BODY, url));
                    }
                    body.extend_from_slice(&buf[..n]);
                }
                Err(e) => return Err(anyhow!("read error on {}: {:?}", url, e)),
            }
        }
        Ok(body)
    }

    /// Build a fresh HTTPS connection, POST a JSON body, and return the
    /// response body. Same one-shot pattern as `get_json` — no connection
    /// reuse because `EspHttpConnection` can't be reset after a failure.
    ///
    /// On non-2xx responses this returns `Err` with the server's error
    /// body embedded so the caller (and the serial log) can see WHY the
    /// submission was rejected (e.g. "fee too low", "insufficient
    /// balance", "rate limit exceeded").
    fn post_json(&self, path: &str, json_body: &[u8]) -> Result<Vec<u8>> {
        const MAX_BODY: usize = 16 * 1024;
        let url = format!("{}{}", self.base_url, path);

        let cfg = Configuration {
            crt_bundle_attach: Some(esp_idf_svc::sys::esp_crt_bundle_attach),
            timeout: Some(std::time::Duration::from_secs(15)),
            buffer_size: Some(4096),
            buffer_size_tx: Some(4096), // bigger TX buffer — tx JSON can be ~500 B
            ..Default::default()
        };
        let mut conn = EspHttpConnection::new(&cfg).context("EspHttpConnection::new failed")?;

        // Content-Length must be a &str living as long as the headers array.
        // Stash it in a String so the &str reference stays alive.
        let content_length = json_body.len().to_string();
        let headers = [
            ("content-type", "application/json"),
            ("accept", "application/json"),
            ("content-length", content_length.as_str()),
            ("connection", "close"),
        ];

        conn.initiate_request(Method::Post, &url, &headers)
            .with_context(|| format!("HTTP initiate POST {} failed", url))?;

        // Write the request body. `EspHttpConnection` implements
        // `embedded_io::Write`, so we bring the `Write` trait into scope
        // at the top of the file and call `write_all` here.
        conn.write_all(json_body)
            .with_context(|| format!("HTTP write body on POST {} failed", url))?;
        conn.flush()
            .with_context(|| format!("HTTP flush body on POST {} failed", url))?;

        conn.initiate_response()
            .with_context(|| format!("HTTP initiate_response for {} failed", url))?;

        let status = conn.status();

        // Drain the body regardless of status — we want to see server errors.
        let mut buf = [0u8; 512];
        let mut body = Vec::with_capacity(1024);
        loop {
            match conn.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => {
                    if body.len() + n > MAX_BODY {
                        return Err(anyhow!("response body exceeded {} bytes for POST {}", MAX_BODY, url));
                    }
                    body.extend_from_slice(&buf[..n]);
                }
                Err(e) => return Err(anyhow!("read error on POST {}: {:?}", url, e)),
            }
        }

        if !(200..=299).contains(&status) {
            return Err(anyhow!(
                "HTTP {} from POST {} — body: {}",
                status,
                url,
                body_preview(&body)
            ));
        }
        Ok(body)
    }
}

/// Truncated preview of a body for error messages — avoids dumping a
/// potentially-huge JSON blob into the serial log.
fn body_preview(body: &[u8]) -> String {
    let n = body.len().min(200);
    let s = String::from_utf8_lossy(&body[..n]);
    if body.len() > n {
        format!("{}… ({} bytes)", s, body.len())
    } else {
        s.into_owned()
    }
}
