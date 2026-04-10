# ultradag-esp32

UltraDAG **light client + minimal wallet** for ESP32-WROOM-32 (the classic
4 MB / 520 KB SRAM / no-PSRAM module). Written in Rust, targets
`xtensa-esp32-espidf`, builds against ESP-IDF v5.1.

## What this is

A tiny firmware that:

1. Connects to a 2.4 GHz WiFi network using WPA2-PSK. Credentials are
   read from the **NVS** partition, falling back to the compile-time
   values in `config.rs` on the first boot of a freshly-flashed chip
   (and then persisted into NVS so future boots come from storage).
2. Performs a **trust-on-first-use (TOFU) validator-set check** by
   fetching `/validators` and comparing a `blake3` hash of the sorted
   address set against a root saved in NVS. Catches RPC takeover;
   doesn't catch response-level lying (see `finality.rs` for the
   honesty disclaimer — real cryptographic finality verification
   requires a `/checkpoint` endpoint the current RPC doesn't expose).
3. Loads its **per-chip Ed25519 signing key from NVS**. On the very
   first boot the NVS value is empty, so we generate a fresh 32-byte
   seed from ESP-IDF's hardware RNG (`esp_fill_random`) and persist it.
   Every chip gets a unique, stable on-chain identity without the key
   ever appearing in the firmware image or the git repo.
4. Runs a **once-per-boot wallet flow** that actually transacts on chain:
   - Checks the wallet's balance.
   - If empty, hits `POST /faucet` to self-fund 10 UDAG on testnet.
   - On first boot, **bootstraps a SmartAccount** via `AddKeyTx`: the
     node's `auto_register_ed25519_key` side effect registers the
     primary key as "default" and we add a deterministic throwaway
     secondary as the `new_key`, yielding `smart=true keys=2`.
   - Signs a real **`SmartTransferTx`** with `ed25519-dalek` entirely
     on-chip (byte-exact with the Rust node's `smart_transfer`
     signable-bytes layout), or falls back to legacy `TransferTx` if
     the bootstrap fails.
   - Submits via `POST /tx/submit` and verifies the nonce advanced.
5. Enters a read-only polling loop every N seconds:
   - `GET /status` — current round, finalized round, peer / validator count, mempool size.
   - `GET /balance/{address}` for **every** address in the
     `WATCH_ADDRESSES` list (configurable, cap at ~8 before the per-tick
     HTTPS latency gets unbearable).
6. Optionally drives an **SSD1306 128×32 OLED** over I²C (SDA=GPIO21,
   SCL=GPIO22, address 0x3C). The init is non-fatal — if no panel is
   wired up the firmware logs a warning and runs headless. Same binary
   works both ways.
7. Logs everything over UART (visible via `espflash monitor`).
8. Blinks the onboard blue LED (GPIO2) on each successful poll; leaves it on when the last poll failed.

The signing path is **byte-exact** with the Rust node: the
`transfer_signable_bytes` function in `src/sign.rs` mirrors
`ultradag-coin::tx::TransferTx::signable_bytes` field-for-field, and local
self-verification via `verify_strict` runs before every submission to
catch any drift immediately rather than failing silently in the mempool.

It is **not** a full node, a validator, a P2P peer, or a block producer.
See [Why not a full node?](#why-not-a-full-node) below.

## ⚠ Wallet security

The Ed25519 seed is now **generated on first boot** from the ESP-IDF
hardware RNG and persisted in NVS — it is NOT in the git repo or in
any source file, and every chip has a unique identity.

However, the NVS partition itself is **not encrypted** by default.
Anyone who physically gets the chip and dumps its flash with
`espflash read-flash` can recover the Ed25519 seed and WiFi password
in plaintext. If you care about that threat model, turn on ESP-IDF
flash encryption:

```
# One-time per chip, IRREVERSIBLE.
# Burns eFuses to generate a device-unique AES key and forces the
# bootloader to only run encrypted firmware from then on.
espefuse.py --port /dev/cu.usbserial-0001 burn_efuse FLASH_CRYPT_CNT 1
```

Warning — this is a one-way operation. Once flash encryption is on,
you can no longer flash unencrypted firmware, and debugging requires
extra tooling. Do it only on a chip you're ready to commit to
production, NOT on a dev board you're iterating on.

Still: **never use this wallet to hold real mainnet UDAG.** It's a
testnet light client.

## Hardware targeted

| Spec | Your ESP32 |
| --- | --- |
| Chip | ESP32 (rev v1.0) — dual-core Xtensa LX6 @ 240 MHz |
| Flash | 4 MB |
| SRAM | 520 KB (no PSRAM) |
| WiFi | 2.4 GHz only |
| USB-UART | CH340 / CP2102 (appears as `/dev/cu.usbserial-0001` on macOS) |

The above is the stock ESP32-WROOM-32 on the cheapest DevKitC boards. Anything else (S3, C3, S6, C6) will need the target and a few kconfig knobs retuned.

## Prerequisites

Install once, per-machine:

```bash
# 1. Rustup + cargo (if you don't already have them).
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# 2. espup — the Espressif Rust fork installer.
cargo install espup

# 3. Install the Xtensa Rust toolchain + GCC + rust-src for this target.
#    This adds the `esp` rustup channel and ~500 MB of tooling.
espup install --targets esp32

# 4. Flasher + build-linker shim.
cargo install espflash ldproxy
```

After `espup install` finishes it prints a path to an `export-esp.sh` script. Source it in each new shell *before* building:

```bash
source ~/export-esp.sh
```

(Or add the source line to your `~/.zshrc` / `~/.bash_profile` to do it automatically.)

Verify it worked:

```bash
rustup toolchain list            # should list "esp"
ls ~/.rustup/toolchains/esp/lib/rustlib/   # should contain xtensa-esp32-espidf
```

## Configure

Edit `src/config.rs` with your details:

```rust
pub const WIFI_SSID:     &str = "your-wifi-ssid";
pub const WIFI_PASSWORD: &str = "your-wifi-password";
pub const NODE_URL:      &str = "http://192.168.1.42:10333";
pub const WATCH_ADDRESS: &str = "0000000000000000000000000000000000000000";
pub const POLL_INTERVAL_SECS: u64 = 10;
```

### About `NODE_URL`

You have two realistic choices for what the ESP32 talks to:

1. **A node running on your laptop, over plain HTTP.** Easiest. Run
   `cargo run -p ultradag-node --release -- --port 9333 --rpc-port 10333 --data-dir /tmp/udag-dev` on your Mac, find your LAN IP (`ipconfig getifaddr en0`), then set `NODE_URL = "http://<that-ip>:10333"`. No TLS, no certs — just works as long as the ESP32 and laptop are on the same WiFi.

2. **A public HTTPS endpoint** like `https://your-node.example.com`. The firmware already bundles the mbedTLS root certificate store (see `sdkconfig.defaults`) so any public CA-signed cert will validate. Costs ~60 KB flash but otherwise transparent — just put `https://` in the URL.

The fly.io testnet nodes (`ultradag-node-*.fly.dev`) redirect HTTP → HTTPS at the edge, so for those you **must** use `https://`, not plain HTTP.

## Build and flash

Plug the ESP32 into USB. It should show up as `/dev/cu.usbserial-0001` on macOS.

```bash
cd crates/ultradag-esp32
source ~/export-esp.sh          # if you haven't already for this shell

# One-shot: compile, flash, and open the serial monitor.
cargo run --release
```

`cargo run` invokes the `espflash flash --monitor` runner defined in `.cargo/config.toml`, which auto-detects the serial port. If you have multiple boards plugged in, pass `--port`:

```bash
cargo run --release -- --port /dev/cu.usbserial-0001
```

To just build without flashing:

```bash
cargo build --release
```

The resulting ELF lives at `target/xtensa-esp32-espidf/release/ultradag-esp32`. `espflash` will convert it to the ESP32 bootable image format on the fly during flashing.

### First build will be slow

Expect the **first** `cargo build` to take 15-30 minutes. `embuild` has to:
- Download ESP-IDF v5.1.4 from github (~500 MB).
- Download the Xtensa GCC toolchain.
- Build the ESP-IDF C components from source (mbedTLS, lwIP, wifi driver, FreeRTOS, …).
- Compile `std` for Xtensa via `-Z build-std`.

Subsequent incremental rebuilds of *your* code are fast (seconds). A full `cargo clean && cargo build --release` of *just* this crate is ~3-5 minutes because embuild caches ESP-IDF in `target/.embuild/`.

## What you should see on the serial monitor

First-boot output against the public testnet, with an empty demo wallet:

```
I (705)  UltraDAG ESP32 light client v0.1.0 starting
I (715)    node  = https://ultradag-node-1.fly.dev
I (715)    watch = 0000000000000000000000000000000000000001
I (725)    poll  = 10s
I (965)  WiFi: radio started, associating with 'YOUR_WIFI_SSID'…
I (3455) WiFi: associated, waiting for DHCP lease…
I (4455) sta ip: 10.114.29.24, mask: 255.255.255.0, gw: 10.114.29.227
I (4455) WiFi: up. ip=10.114.29.24 netmask=24 gw=10.114.29.227
I (4485) ┈┈ demo wallet ┈┈
I (4485)   address = 92c2a778009bc5ad5ee87ca1721bc009bff041dd
W (4485)   SECURITY: demo seed is hardcoded in wallet.rs — TESTNET ONLY
I (4495) wallet: fetching current demo balance…
I (6805) wallet: current balance = 0 UDAG (0 sats, nonce=0)
I (6805) wallet: balance 0 sats < 50010000 sats needed; requesting 10 UDAG from faucet…
I (9105) wallet: faucet accepted: tx_hash=fd839dffc40dcbba52b3fe370a975218463ea7acb1f0bc46c499ec2e2648e710 nonce=5
I (9105) wallet: waiting for funds to finalize…
I (20935) wallet: poll 1/18 → balance 1000000000 sats
I (20945) wallet: funded! balance=1000000000 sats nonce=0
I (22705) wallet: signing transfer with nonce=0
I (22825) wallet: signed transfer → to=0000…0001 amount=50000000 sats fee=10000 sats
I (22825) wallet: submitting to /tx/submit…
I (24685) wallet: submitted! status=pending tx_hash=1a35ca73375a203437a00604dfcf0e6a97185288ca77049c46f0b30fe5d2ebeb
I (36915) wallet: post-submit balance=949990000 sats nonce=1 (expected nonce >= 1)
I (36915) wallet flow complete
I (36915) ── tick 1 ─────────────────────────────────
I (38965) status:  round=15252 finalized=Some(15251) peers=7 validators=5 mempool=0 supply_sats=101525055000000
I (41215) balance: 0000…0001 = 0.5 UDAG (50000000 sats, nonce=0, smart=true)
I (51295) ── tick 2 ─────────────────────────────────
…
```

On subsequent reboots the wallet is already funded and the flow is
idempotent — the faucet is skipped, nonce is read from chain, and a
fresh transfer is signed with `nonce = state_nonce`:

```
I (4475) wallet: current balance = 8.9998 UDAG (899980000 sats, nonce=2)
I (4475) wallet: already funded, skipping faucet
I (8545) wallet: signing transfer with nonce=2
I (10645) wallet: submitted! status=pending tx_hash=<hash>
I (22765) wallet: post-submit balance=849970000 sats nonce=3
```

Balance math is exact: `899980000 − 50000000 − 10000 = 849970000`. The
blue LED flashes once per successful observer-loop poll.

## Flash / RAM footprint

Measured on the first successful release build of this crate (`cargo +esp build --release`, esp-idf-svc 0.51, ESP-IDF v5.1.4):

| | |
| --- | --- |
| Version | App size | % of 4 MB |
| --- | --- | --- |
| v0.1 pure observer (status + balance only) | 1,198,384 B ≈ 1.14 MB | 29.03 % |
| v0.2 + demo wallet (ed25519-dalek, blake3, hex, TransferTx) | 1,346,912 B ≈ 1.28 MB | 32.62 % |
| v0.3 final (multi-addr, NVS, SmartTransfer, finality TOFU, SSD1306) | **1,406,784 B ≈ 1.34 MB** | **34.07 %** |

Incremental cost breakdown of the final feature set versus the pure observer:
- `ed25519-dalek + blake3 + hex` → ~148 KB (signing)
- NVS + sign.rs SmartTransfer additions → ~15 KB
- SSD1306 + embedded-graphics + display.rs → ~60 KB
- Misc refactoring (multi-address loop, finality module) → ~5 KB

Still comfortably under half the 4 MB flash budget with all features on.

If you don't need a specific feature, remove its module + dependencies:
the `ssd1306` + `embedded-graphics` pair alone is ~60 KB, and dropping
`ed25519-dalek` + the entire wallet flow drops another ~150 KB if you
only want the read-only observer.

Plenty of headroom on a 4 MB WROOM-32. Re-run `espflash save-image --chip esp32 target/xtensa-esp32-espidf/release/ultradag-esp32 /tmp/ultradag-esp32.bin` to re-measure after changes. Free internal SRAM after boot is only visible at runtime — the monitor output logs it on the first successful poll tick.

If you ever run out of flash, the easy wins are: turn off HTTPS (unset `CONFIG_MBEDTLS_CERTIFICATE_BUNDLE` in `sdkconfig.defaults`, saves ~60 KB, and switch `NODE_URL` back to `http://`) and/or drop `CONFIG_LOG_DEFAULT_LEVEL_INFO=y` to `=3` → Warning (~20 KB).

## Why not a full node?

"Full UltraDAG node" means: maintains the DAG, verifies signatures on every vertex, runs finality voting, accepts P2P connections, persists state to a KV store, serves HTTP RPC. The node crate's dependency graph (`tokio` full, `hyper`, `reqwest`, `redb`, `snow`, `bitvec`, `dashmap`, `ed25519-dalek`, `k256`, `p256`, `serde_json`, `blake3`, plus ESP-IDF + lwIP + WiFi) blows past both the 1.9 MB app-partition budget and the ~300 KB usable RAM on a WROOM-32.

If you genuinely need a full validating node on embedded hardware:
- **ESP32-S3-DevKitC-1-N16R8** (16 MB flash, 8 MB PSRAM) is the minimum viable board. A follower-mode port to that chip is a real project but a meaningfully larger undertaking than this light client. Open an issue if you need it.
- Anything smaller than that (plain ESP32, ESP32-C3) is not an option for a full node. It's physically impossible to fit the working set.

A light client (this crate) is the right shape for a 4 MB ESP32: it verifies nothing on-chain by itself, but it trusts a node you control over a network you control, which is exactly how every phone wallet on every blockchain works.

## Troubleshooting

**`espup install` fails with "could not install xtensa-esp-elf"**
Network flake, usually. Re-run it. If it still fails, delete `~/.espressif` and retry.

**`cargo build` fails with `ldproxy: command not found`**
Run `cargo install ldproxy`. The `.cargo/config.toml` for this crate hard-requires it as the linker.

**`cargo run` fails with "failed to open serial port"**
Your Mac user doesn't have access to `/dev/cu.usbserial-0001`. Nothing to do on macOS — if it happens, unplug and re-plug the ESP32. On Linux, add yourself to the `dialout` group and log out / back in.

**WiFi associates but never gets a DHCP lease**
The AP is probably giving you an IPv6-only lease or has a DHCP snooping ACL. The firmware explicitly waits for IPv4 via `wait_netif_up`. Check your router.

**HTTP requests time out even though WiFi is up**
First check: can your laptop `curl http://<LAN IP>:10333/status` from the same WiFi? If no, the node isn't listening on `0.0.0.0` — by default it binds `[::]:10333` which is both IPv4 and IPv6, so this is unusual. If yes, check the ESP32's IP with `ping` and confirm it's on the same subnet.

**`serde_json` panics on parse**
The node's JSON response shape changed. Update `src/client.rs::StatusResponse` / `BalanceResponse` to match. These structs deliberately deserialize only the fields we use, so extra fields won't break us — but renamed / removed fields will.
