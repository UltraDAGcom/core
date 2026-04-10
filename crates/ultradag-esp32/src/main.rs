//! UltraDAG ESP32 light client — `main` entry point.
//!
//! Boots the ESP-IDF runtime, connects to WiFi, and enters a polling
//! loop that talks to an UltraDAG node's HTTP RPC. On each successful
//! poll the onboard blue LED (GPIO2) blinks once; on error it stays on.
//!
//! This is NOT a full node. See README.md for what "full node on ESP32"
//! actually means on this hardware (TL;DR: it does not fit).

use std::thread::sleep;
use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use esp_idf_svc::eventloop::EspSystemEventLoop;
use esp_idf_svc::hal::gpio::{Output, PinDriver};
use esp_idf_svc::hal::peripherals::Peripherals;
use esp_idf_svc::nvs::EspDefaultNvsPartition;

mod client;
mod config;
mod display;
mod finality;
mod sign;
mod storage;
mod wallet;
mod wifi;

use client::UltraDagClient;
use sign::{SignedAddKey, SignedSmartTransfer, SignedTransfer};
use storage::Storage;

/// Amount the ESP32 asks the faucet for (sats). 10 UDAG is plenty to
/// cover at least one transfer plus the 10,000-sat minimum fee with
/// change left over for a human-readable balance.
const FAUCET_AMOUNT_SATS: u64 = 10 * 100_000_000; // 10 UDAG

/// Amount the ESP32 sends in its demo transfer (sats). Small enough that
/// one faucet drip funds many reboots, large enough that the balance
/// change is obvious in logs.
const DEMO_TRANSFER_SATS: u64 = 50_000_000; // 0.5 UDAG

/// Minimum fee the node will accept — must match `MIN_FEE_SATS` in
/// `ultradag-coin/src/constants.rs`. If this drifts, the tx gets rejected
/// with "fee too low".
const MIN_FEE_SATS: u64 = 10_000;

/// Max rounds to wait for a funding tx to finalize before giving up.
const FAUCET_WAIT_ROUNDS: u32 = 18; // ≈ 3 minutes at 10 s per poll

/// If `true`, the ESP32 will bootstrap a SmartAccount via `AddKeyTx` on
/// first boot and then sign its demo transfer as a `SmartTransferTx`
/// (the modern path, byte-exact with the Rust node's `smart_transfer`
/// tag). If `false`, the chip keeps signing the legacy `TransferTx`
/// which doesn't require a SmartAccount.
///
/// The cost of enabling this: one extra AddKeyTx (fee: MIN_FEE_SATS)
/// per chip lifetime. The benefit: the same code path that a phone
/// wallet uses — multi-key, recovery-capable, policy-capable. For a
/// single-key IoT device neither benefit materializes, so this is
/// mostly ceremonial.
const USE_SMART_TRANSFER: bool = true;

fn main() -> Result<()> {
    // Required incantation: patch the ESP-IDF environment so Rust std can
    // talk to FreeRTOS primitives. Must be the very first thing in main.
    esp_idf_svc::sys::link_patches();

    // Route `log::*` macros into ESP-IDF's native logger — output shows up
    // over the UART that `espflash monitor` is reading.
    esp_idf_svc::log::EspLogger::initialize_default();

    log::info!("UltraDAG ESP32 light client v{} starting", env!("CARGO_PKG_VERSION"));
    log::info!("  node  = {}", config::NODE_URL);
    log::info!("  watch = {} addresses", config::WATCH_ADDRESSES.len());
    for (i, addr) in config::WATCH_ADDRESSES.iter().enumerate() {
        log::info!("          [{}] {}", i, addr);
    }
    log::info!("  poll  = {}s", config::POLL_INTERVAL_SECS);

    // ── Take hardware peripherals ────────────────────────────────────────
    let peripherals = Peripherals::take().context("Peripherals::take() failed — already taken?")?;
    let sysloop = EspSystemEventLoop::take().context("take sysloop")?;
    let nvs_partition = EspDefaultNvsPartition::take().context("take nvs")?;

    // GPIO2 → onboard blue LED on most ESP32-DevKitC boards. Active-high.
    // If your board routes it differently, edit this line.
    let mut led = PinDriver::output(peripherals.pins.gpio2).context("gpio2 as output")?;
    let _ = led.set_low();

    // Optional OLED display on I²C (SDA=GPIO21, SCL=GPIO22). If no
    // panel is wired up the init fails silently and we run headless —
    // the firmware works either way so the same binary boots on a
    // bare chip and on one with a display.
    let mut oled = display::OledDisplay::try_init(
        peripherals.i2c0,
        peripherals.pins.gpio21,
        peripherals.pins.gpio22,
    );
    if let Some(d) = oled.as_mut() {
        d.show_lines(&["UltraDAG ESP32", "booting...", ""]);
    }

    // ── Persistent storage (NVS) ─────────────────────────────────────────
    // WiFi needs the NvsPartition handle too (it stores calibration data
    // in NVS internally), so we clone it before handing our own handle
    // to the Storage wrapper.
    let mut storage = Storage::open(nvs_partition.clone()).context("open NVS storage")?;

    // Resolve WiFi credentials: prefer values persisted in NVS, fall
    // back to the compile-time config so a freshly-flashed chip still
    // works out of the box. The first time we boot, we COPY the config
    // values into NVS so the next boot can be re-configured at runtime
    // (via `Storage::set_wifi_credentials`) without a reflash.
    let (ssid, password, source) = resolve_wifi_credentials(&mut storage)?;
    log::info!("wifi: using SSID '{}' (from {})", ssid, source);

    if let Some(d) = oled.as_mut() {
        d.show_lines(&["UltraDAG ESP32", &format!("wifi: {}", ssid), "connecting..."]);
    }

    // ── WiFi ─────────────────────────────────────────────────────────────
    let _wifi = wifi::connect(
        peripherals.modem,
        sysloop,
        nvs_partition,
        &ssid,
        &password,
    )
    .context("WiFi connect failed")?;

    if let Some(d) = oled.as_mut() {
        d.show_lines(&["UltraDAG ESP32", "wifi up", "loading wallet..."]);
    }

    // ── HTTP client + wallet flow + poll loop ────────────────────────────
    let client = UltraDagClient::new(config::NODE_URL).context("HTTP client init failed")?;

    // TOFU validator-set check. On first boot we snapshot the active
    // validator address set and persist its blake3 root to NVS. On
    // every subsequent boot we re-fetch and compare. This is NOT a
    // cryptographic finality check — the current RPC doesn't expose
    // signed checkpoints. It catches RPC takeover, not response-level
    // lying. See `finality.rs` for the full honesty disclaimer.
    match finality::check_validator_set(&client, &mut storage) {
        Ok(finality::ValidatorCheckResult::FirstBoot { validator_count, root_hex }) => {
            log::info!("finality: first-boot TOFU — trusted {} validators", validator_count);
            log::info!("finality: validator-set root = {}", root_hex);
        }
        Ok(finality::ValidatorCheckResult::Match { validator_count, root_hex }) => {
            log::info!(
                "finality: validator set unchanged ({} validators, root={}…)",
                validator_count, &root_hex[..16]
            );
        }
        Ok(finality::ValidatorCheckResult::Mismatch {
            stored_root_hex,
            live_root_hex,
            live_validator_count,
        }) => {
            log::error!("finality: ⚠ VALIDATOR SET CHANGED SINCE TOFU ⚠");
            log::error!("finality:   stored root = {}", stored_root_hex);
            log::error!("finality:   live root   = {}", live_root_hex);
            log::error!("finality:   live count  = {}", live_validator_count);
            log::error!("finality:   this could be legitimate churn on a small testnet");
            log::error!("finality:   OR a compromised RPC — do NOT trust balances until verified");
        }
        Err(e) => {
            log::warn!("finality: validator set check failed (continuing): {:#}", e);
        }
    }

    // Load (or on first boot, generate) the chip's Ed25519 signing key
    // from NVS. This gives every chip a unique, persistent identity
    // without baking a key into the firmware image.
    let sk = wallet::load_or_create(&mut storage).context("load wallet key")?;
    let wallet_addr_hex = wallet::address_hex_of(&sk);
    log::info!("┈┈ wallet ┈┈");
    log::info!("  address = {}", wallet_addr_hex);
    log::warn!("  SECURITY: NVS partition is NOT encrypted — anyone who dumps the");
    log::warn!("            flash can recover the Ed25519 seed. TESTNET ONLY.");

    // Run the once-per-boot wallet flow. If anything fails we log and
    // continue into the observer loop — a failed send doesn't make the
    // rest of the firmware useless.
    match run_wallet_flow(&client, &sk, &wallet_addr_hex) {
        Ok(()) => log::info!("wallet flow complete"),
        Err(e) => log::warn!("wallet flow failed (continuing as observer): {:#}", e),
    }

    let mut tick: u64 = 0;
    let mut last_round: Option<u64> = None;

    // Show the wallet address on the OLED once the wallet flow is done
    // and we're about to enter the polling loop. This line stays while
    // the first tick is inflight so the display isn't blank.
    if let Some(d) = oled.as_mut() {
        let short_addr = format!("{}…{}", &wallet_addr_hex[..6], &wallet_addr_hex[wallet_addr_hex.len() - 4..]);
        d.show_lines(&[
            "UltraDAG ESP32",
            &format!("wallet {}", short_addr),
            "waiting for tick...",
        ]);
    }

    loop {
        tick = tick.wrapping_add(1);
        log::info!("── tick {} ─────────────────────────────────", tick);

        match poll_once(&client, &mut led, &mut last_round, oled.as_mut(), &wallet_addr_hex, tick) {
            Ok(()) => {}
            Err(e) => {
                // Leave the LED on to signal an error state visually until
                // the next successful poll clears it.
                let _ = led.set_high();
                log::error!("poll failed: {:#}", e);
                if let Some(d) = oled.as_mut() {
                    d.show_lines(&[
                        "ERROR",
                        &format!("tick {}", tick),
                        "see serial log",
                    ]);
                }
            }
        }

        sleep(Duration::from_secs(config::POLL_INTERVAL_SECS));
    }
}

/// Once-per-boot wallet flow: check balance → request faucet if empty
/// → wait for funds to land → sign and submit a small transfer → verify
/// the nonce advanced. Any step failing logs a warning but does not abort
/// the caller — the observer loop always runs regardless.
fn run_wallet_flow(
    client: &UltraDagClient,
    sk: &ed25519_dalek::SigningKey,
    wallet_addr_hex: &str,
) -> Result<()> {
    // Step 1: peek at the current on-chain balance of this wallet.
    log::info!("wallet: fetching current balance…");
    let bal = client
        .get_balance(wallet_addr_hex)
        .context("wallet: initial balance fetch failed")?;
    log::info!(
        "wallet: current balance = {} UDAG ({} sats, nonce={})",
        bal.balance_udag, bal.balance, bal.nonce
    );

    // Step 2: if the wallet has too little to afford a transfer + fee,
    // hit the testnet faucet. The faucet rate limit is 1 req / 10 min per
    // IP — if we recently drained it, we'll get HTTP 429 here, so log and
    // continue rather than panicking.
    let needed = DEMO_TRANSFER_SATS.saturating_add(MIN_FEE_SATS);
    if bal.balance < needed {
        log::info!(
            "wallet: balance {} sats < {} sats needed; requesting {} UDAG from faucet…",
            bal.balance, needed, FAUCET_AMOUNT_SATS / 100_000_000
        );
        match client.post_faucet(wallet_addr_hex, FAUCET_AMOUNT_SATS) {
            Ok(resp) => {
                log::info!(
                    "wallet: faucet accepted: tx_hash={} nonce={}",
                    resp.get("tx_hash").and_then(|v| v.as_str()).unwrap_or("?"),
                    resp.get("nonce").and_then(|v| v.as_u64()).unwrap_or(0)
                );
            }
            Err(e) => {
                // Most common: 429 rate-limited. Also possible: server down.
                log::warn!("wallet: faucet request failed: {:#}", e);
                log::warn!("wallet: (if you just asked, the faucet limits to 1/10min per IP)");
                return Err(anyhow!("faucet unavailable"));
            }
        }

        // Step 3: wait for funds to land. Poll balance once per
        // POLL_INTERVAL_SECS until we see enough for the transfer.
        log::info!("wallet: waiting for funds to finalize…");
        let mut waited = 0u32;
        let funded = loop {
            if waited >= FAUCET_WAIT_ROUNDS {
                log::warn!("wallet: gave up waiting after {} polls", waited);
                return Err(anyhow!("faucet tx didn't finalize in time"));
            }
            sleep(Duration::from_secs(config::POLL_INTERVAL_SECS));
            waited += 1;
            match client.get_balance(wallet_addr_hex) {
                Ok(b) => {
                    log::info!(
                        "wallet: poll {}/{} → balance {} sats",
                        waited, FAUCET_WAIT_ROUNDS, b.balance
                    );
                    if b.balance >= needed {
                        break b;
                    }
                }
                Err(e) => log::warn!("wallet: poll failed: {:#}", e),
            }
        };
        log::info!("wallet: funded! balance={} sats nonce={}", funded.balance, funded.nonce);
    } else {
        log::info!("wallet: already funded, skipping faucet");
    }

    // Step 4: re-fetch the balance to get the latest nonce before signing.
    // This matters because between the faucet wait loop and here, more txes
    // could have landed, incrementing the account's nonce.
    let latest = client
        .get_balance(wallet_addr_hex)
        .context("wallet: pre-sign balance fetch failed")?;
    let nonce = latest.nonce;
    log::info!("wallet: signing transfer with nonce={}", nonce);

    // Step 5: build, sign, and submit a transfer to the first watch
    // address. Signing happens entirely on-chip — the private key never
    // leaves the ESP32 and never touches the network. The transfer is
    // always directed at WATCH_ADDRESSES[0] so that subsequent observer
    // polls will show the balance change on an address we're watching.
    let to_str = config::WATCH_ADDRESSES
        .first()
        .ok_or_else(|| anyhow!("WATCH_ADDRESSES is empty — need at least one entry"))?;
    let to_bytes = parse_hex_address(to_str)
        .with_context(|| format!("wallet: invalid WATCH_ADDRESS[0] {:?}", to_str))?;

    // SmartTransfer bootstrap (optional): if USE_SMART_TRANSFER is on and
    // the account isn't a SmartAccount yet, submit an AddKeyTx first to
    // create the SmartAccount via `auto_register_ed25519_key`. The
    // AddKeyTx registers our primary pubkey as "default" AND adds a
    // deterministic secondary key — we need a distinct secondary because
    // AddKey rejects duplicates, but the secondary never signs anything.
    let use_smart = USE_SMART_TRANSFER
        && bootstrap_smart_account_if_needed(client, sk, wallet_addr_hex, &latest)?;

    let (envelope, label) = if use_smart {
        // Re-fetch nonce — the AddKeyTx we just submitted (if any) will
        // have incremented it, and signing with a stale nonce is the
        // most common reason SmartTransfer rejects.
        let fresh = client
            .get_balance(wallet_addr_hex)
            .context("wallet: post-bootstrap balance fetch failed")?;
        let n = fresh.nonce;
        log::info!("wallet: signing SmartTransfer with nonce={}", n);
        let signed = SignedSmartTransfer::build(sk, to_bytes, DEMO_TRANSFER_SATS, MIN_FEE_SATS, n);
        (
            serde_json::json!({ "SmartTransfer": signed.to_json() }),
            "SmartTransfer",
        )
    } else {
        let signed = SignedTransfer::build(sk, to_bytes, DEMO_TRANSFER_SATS, MIN_FEE_SATS, nonce);
        (serde_json::json!({ "Transfer": signed.to_json() }), "Transfer")
    };

    log::info!(
        "wallet: signed {} → to={} amount={} sats fee={} sats",
        label, to_str, DEMO_TRANSFER_SATS, MIN_FEE_SATS
    );

    log::info!("wallet: submitting to /tx/submit…");
    let submit_resp = client
        .submit_tx(&envelope)
        .context("wallet: /tx/submit failed")?;
    // The `/tx/submit` handler returns `{"status": "pending", "tx_hash": "..."}`
    // (unlike `/tx` which uses `hash`), so grab `tx_hash` here.
    log::info!(
        "wallet: submitted! status={} tx_hash={}",
        submit_resp.get("status").and_then(|v| v.as_str()).unwrap_or("?"),
        submit_resp.get("tx_hash").and_then(|v| v.as_str()).unwrap_or("?"),
    );

    // Step 6: briefly verify the nonce advanced. Not strictly necessary
    // — the server already validated the signature and admitted the tx
    // to the mempool — but it's a nice visible confirmation that the
    // balance path works end-to-end.
    sleep(Duration::from_secs(config::POLL_INTERVAL_SECS));
    match client.get_balance(wallet_addr_hex) {
        Ok(post) => log::info!(
            "wallet: post-submit balance={} sats nonce={} (expected nonce >= {})",
            post.balance, post.nonce, nonce + 1
        ),
        Err(e) => log::warn!("wallet: post-submit balance check failed: {:#}", e),
    }

    Ok(())
}

/// Decode a 40-character hex address into a 20-byte array. Rejects
/// anything that isn't exactly the right length to avoid building an
/// all-zero `to` field from a malformed string.
fn parse_hex_address(s: &str) -> Result<[u8; 20]> {
    let trimmed = s.trim_start_matches("0x");
    if trimmed.len() != 40 {
        return Err(anyhow!("expected 40 hex chars, got {}", trimmed.len()));
    }
    let bytes = hex::decode(trimmed).context("hex decode failed")?;
    let mut addr = [0u8; 20];
    addr.copy_from_slice(&bytes);
    Ok(addr)
}

/// Ensure a SmartAccount exists at `sk`'s address with the primary key
/// registered. Returns `Ok(true)` if SmartTransfer is ready to use after
/// this call, `Ok(false)` if something went wrong and the caller should
/// fall back to legacy TransferTx.
///
/// Contract with the Rust node:
///   - If `is_smart_account` is already true AND `authorized_key_count`
///     is >= 1, we assume our key is the registered "default" (because
///     `auto_register_ed25519_key` was called the last time this
///     account was touched by a smart-account-aware tx, and we're the
///     only signer). Skip the bootstrap.
///   - Otherwise, submit an AddKeyTx. The node's engine calls
///     `ensure_smart_account_at_round` + `auto_register_ed25519_key`
///     BEFORE validating `new_key`, so even the act of submitting
///     AddKey creates the SmartAccount and registers us. The
///     `new_key` we add is a deterministic "throwaway" derived from
///     the primary seed via blake3 — it's a real pubkey that the node
///     will validate, but the chip never uses it again.
///
/// On error, logs and returns `Ok(false)` — the caller gracefully
/// falls back to `TransferTx` so the tick isn't a total wash.
fn bootstrap_smart_account_if_needed(
    client: &UltraDagClient,
    sk: &ed25519_dalek::SigningKey,
    wallet_addr_hex: &str,
    prior_balance: &client::BalanceResponse,
) -> Result<bool> {
    // Fast path: account already set up.
    if prior_balance.is_smart_account
        && prior_balance.authorized_key_count.unwrap_or(0) >= 1
    {
        log::info!(
            "wallet: SmartAccount already bootstrapped ({} keys) — skipping AddKey",
            prior_balance.authorized_key_count.unwrap_or(0)
        );
        return Ok(true);
    }

    log::info!("wallet: SmartAccount not bootstrapped — submitting AddKeyTx");

    // Derive a deterministic throwaway secondary pubkey. We only need
    // its PUBLIC key bytes for AddKey — the private half doesn't exist
    // because we never sign with it. Constructing one from raw bytes
    // isn't possible without a full key gen, so derive a second seed
    // from the primary and use its verifying key as the "new_key".
    let primary_seed: [u8; 32] = sk.to_bytes();
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"ultradag-esp32-secondary-v1");
    hasher.update(&primary_seed);
    let secondary_seed: [u8; 32] = *hasher.finalize().as_bytes();
    let secondary_sk = ed25519_dalek::SigningKey::from_bytes(&secondary_seed);
    let secondary_pubkey: [u8; 32] = secondary_sk.verifying_key().to_bytes();
    log::info!(
        "wallet: derived throwaway secondary pubkey: {}",
        hex::encode(secondary_pubkey)
    );

    // AddKey carries its own MIN_FEE_SATS — it's NOT fee-exempt at the
    // legacy-tx layer (only the SmartOpType::AddKey variant is fee-exempt).
    let nonce = prior_balance.nonce;
    let add_key = SignedAddKey::build(
        sk,
        secondary_pubkey,
        "esp32-secondary",
        MIN_FEE_SATS,
        nonce,
    );
    let envelope = serde_json::json!({ "AddKey": add_key.to_json() });
    log::info!("wallet: submitting AddKeyTx (nonce={}, fee={} sats)…", nonce, MIN_FEE_SATS);

    let resp = match client.submit_tx(&envelope) {
        Ok(r) => r,
        Err(e) => {
            log::warn!("wallet: AddKey submission failed: {:#}", e);
            log::warn!("wallet: falling back to legacy TransferTx path");
            return Ok(false);
        }
    };
    log::info!(
        "wallet: AddKey submitted: status={} tx_hash={}",
        resp.get("status").and_then(|v| v.as_str()).unwrap_or("?"),
        resp.get("tx_hash").and_then(|v| v.as_str()).unwrap_or("?")
    );

    // Poll /balance until authorized_key_count >= 1 (AddKey finalized).
    log::info!("wallet: waiting for AddKey to finalize…");
    for i in 1..=FAUCET_WAIT_ROUNDS {
        sleep(Duration::from_secs(config::POLL_INTERVAL_SECS));
        match client.get_balance(wallet_addr_hex) {
            Ok(b) => {
                let keys = b.authorized_key_count.unwrap_or(0);
                log::info!(
                    "wallet: bootstrap poll {}/{} → smart={} keys={} nonce={}",
                    i, FAUCET_WAIT_ROUNDS, b.is_smart_account, keys, b.nonce
                );
                if b.is_smart_account && keys >= 1 {
                    log::info!("wallet: SmartAccount bootstrapped — ready for SmartTransfer");
                    return Ok(true);
                }
            }
            Err(e) => log::warn!("wallet: bootstrap poll failed: {:#}", e),
        }
    }

    log::warn!("wallet: AddKey didn't finalize in time — falling back to Transfer");
    Ok(false)
}

/// Decide which WiFi credentials to use: NVS if set, config.rs fallback
/// otherwise. On the very first boot of a freshly-flashed chip, NVS is
/// empty so we fall through to the compile-time values — and then copy
/// them INTO NVS so future boots read from persisted storage and can be
/// re-configured at runtime (e.g. by a future provisioning flow) without
/// a reflash.
///
/// Returns `(ssid, password, source)` where `source` is "NVS" or
/// "config.rs" — tracked explicitly because the SSID value itself
/// doesn't tell you the source (NVS and config usually hold the same
/// string after the first boot).
fn resolve_wifi_credentials(storage: &mut Storage) -> Result<(String, String, &'static str)> {
    let nvs_ssid = storage.get_wifi_ssid().context("reading NVS ssid")?;
    let nvs_pass = storage.get_wifi_password().context("reading NVS password")?;

    match (nvs_ssid, nvs_pass) {
        (Some(ssid), Some(pass)) => Ok((ssid, pass, "NVS")),
        _ => {
            // Copy the compile-time values into NVS so they survive into
            // future boots — next boot will take the NVS path.
            log::info!("wifi: NVS empty, priming from config.rs and persisting");
            if let Err(e) = storage.set_wifi_credentials(config::WIFI_SSID, config::WIFI_PASSWORD) {
                log::warn!("wifi: failed to persist creds to NVS: {:#} (using config.rs anyway)", e);
            }
            Ok((
                config::WIFI_SSID.to_string(),
                config::WIFI_PASSWORD.to_string(),
                "config.rs",
            ))
        }
    }
}

/// One full polling iteration — fetches `/status`, then `/balance/{addr}`,
/// logs both, and blinks the LED. Also pushes the latest status to the
/// OLED if one is connected. Returns `Err` if either request fails so
/// the outer loop can drive the error indicator.
fn poll_once(
    client: &UltraDagClient,
    led: &mut PinDriver<'_, esp_idf_svc::hal::gpio::Gpio2, Output>,
    last_round: &mut Option<u64>,
    oled: Option<&mut display::OledDisplay>,
    wallet_addr_hex: &str,
    tick: u64,
) -> Result<()> {
    // /status
    let status = client.get_status().context("GET /status")?;
    log::info!(
        "status: round={} finalized={:?} peers={} validators={} mempool={} supply_sats={}",
        status.dag_round,
        status.last_finalized_round,
        status.peer_count,
        status.validator_count,
        status.mempool_size,
        status.total_supply
    );

    // Only announce a round change, to keep the log readable.
    if *last_round != Some(status.dag_round) {
        if let Some(prev) = *last_round {
            log::info!("round advanced: {} → {}", prev, status.dag_round);
        }
        *last_round = Some(status.dag_round);
    }

    // /balance for every watched address. We fetch sequentially (ESP32
    // only has one connection at a time and each HTTPS handshake is
    // ~3 s), so N addresses → N × 3 s added to every tick. Keep
    // WATCH_ADDRESSES small.
    //
    // Also remember the LAST balance we saw so we can push it to the
    // OLED after the loop — we show the first watch address's balance,
    // since the display only has 3 lines of text.
    let mut first_balance_udag: Option<f64> = None;
    for (i, addr) in config::WATCH_ADDRESSES.iter().enumerate() {
        let bal = client
            .get_balance(addr)
            .with_context(|| format!("GET /balance for WATCH_ADDRESSES[{}]={}", i, addr))?;
        let name_part = bal
            .name
            .as_deref()
            .map(|n| format!(" (@{})", n))
            .unwrap_or_default();
        log::info!(
            "balance[{}]: {}{} = {} UDAG ({} sats, nonce={}, smart={})",
            i,
            bal.address,
            name_part,
            bal.balance_udag,
            bal.balance,
            bal.nonce,
            bal.is_smart_account
        );
        if i == 0 {
            first_balance_udag = Some(bal.balance_udag);
        }
    }

    // Push to OLED if present. Three lines at 6×10 font on 128×32.
    // Line 1: round + finalized + tick
    // Line 2: short wallet address
    // Line 3: watched balance (first entry)
    if let Some(d) = oled {
        let short_addr = format!(
            "{}…{}",
            &wallet_addr_hex[..6],
            &wallet_addr_hex[wallet_addr_hex.len() - 4..]
        );
        let fin_str = match status.last_finalized_round {
            Some(r) => format!("fin{}", r),
            None => "fin?".to_string(),
        };
        let line1 = format!("r{} {} t{}", status.dag_round, fin_str, tick);
        let line2 = format!("me:{}", short_addr);
        let line3 = match first_balance_udag {
            Some(u) => format!("watch: {:.4} UDAG", u),
            None => "watch: (no addrs)".to_string(),
        };
        d.show_lines(&[&line1, &line2, &line3]);
    }

    // Success blink — on for 80 ms, off.
    let _ = led.set_high();
    sleep(Duration::from_millis(80));
    let _ = led.set_low();

    Ok(())
}
