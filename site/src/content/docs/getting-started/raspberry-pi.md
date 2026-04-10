---
title: "Raspberry Pi Zero 2 W"
description: "Cross-compile UltraDAG for ARM64, deploy to a $15 Raspberry Pi Zero 2 W, and run a full node on solar-friendly hardware. 15-minute bringup checklist."
order: 4
section: "getting-started"
---

# Raspberry Pi Zero 2 W

UltraDAG's full node cross-compiles to `aarch64-unknown-linux-gnu` and runs unchanged on a Raspberry Pi Zero 2 W — a $15, 512 MB, 1 GHz quad-core board the size of a stick of gum. This page is the real bringup checklist, written so you can follow it linearly on the day your board arrives.

**What this gets you:** a full UltraDAG validator on a solar-friendly SBC. Not a light client, not a signer — a real node that participates in consensus, holds the pruned DAG, and finalizes rounds.

**What this does NOT get you:** a full node on a classic Pi Zero (original or W). Those have ARMv6 cores (ARM11), which modern Rust stable does not support. You need the **2 W** specifically — the 2022+ model with a Cortex-A53 (ARMv8-A, 64-bit).

---

## Why the 2 W and not a bigger Pi?

| Board | Price | RAM | Cores | Runs UltraDAG? | Notes |
|---|---:|---:|---|---|---|
| Pi Zero (original / W) | $10 | 512 MB | 1× ARM11 (ARMv6) | ❌ | Rust stable does not support ARMv6 |
| **Pi Zero 2 W** | **$15** | **512 MB** | **4× Cortex-A53 @ 1 GHz** | **✅** | **Sweet spot for this chain** |
| Pi 3 A+ / B+ | $25-35 | 512 MB / 1 GB | 4× Cortex-A53 @ 1.4 GHz | ✅ | More headroom under load |
| Pi 4 / 5 | $35-80 | 1-8 GB | 4× Cortex-A72 / A76 | ✅ | Overkill — you'd feel bad running UltraDAG on this |

The 2 W is the smallest board where UltraDAG actually runs. The project's identity is "smallest real BFT chain" — it makes sense to ship and test on the smallest real Linux SBC.

---

## Prerequisites (on your dev machine)

You'll cross-compile on a macOS or Linux workstation and scp the binary to the Pi. Don't try to compile `ultradag-node` on the Pi itself — `cargo build --release` on a 1 GHz A53 with 512 MB RAM will take 30-90 minutes and may OOM during LLVM codegen.

### macOS host (Apple Silicon or Intel)

```bash
# 1. Rust stable with the aarch64 Linux target
rustup toolchain install stable
rustup target add --toolchain stable aarch64-unknown-linux-gnu

# 2. Zig as the cross-linker (Docker-free, single binary)
brew install zig
cargo install cargo-zigbuild
```

### Linux host

```bash
# Rust stable + the aarch64 target
rustup toolchain install stable
rustup target add --toolchain stable aarch64-unknown-linux-gnu

# Either the distro's cross-gcc...
sudo apt install gcc-aarch64-linux-gnu  # Debian/Ubuntu
# ...or zig (same approach as macOS)
sudo snap install zig --classic --beta
cargo install cargo-zigbuild
```

---

## Step 1: Cross-compile

From the workspace root:

```bash
cargo +stable zigbuild --release \
  -p ultradag-node \
  --target aarch64-unknown-linux-gnu
```

First build takes ~5 minutes cold (downloads + compiles all deps). Subsequent rebuilds are incremental.

Output binary:

```
target/aarch64-unknown-linux-gnu/release/ultradag-node
```

Expected size: **3.5 MB stripped**. Verify:

```bash
file target/aarch64-unknown-linux-gnu/release/ultradag-node
# ELF 64-bit LSB pie executable, ARM aarch64, version 1 (SYSV),
# dynamically linked, interpreter /lib/ld-linux-aarch64.so.1, stripped
```

Shared library deps should be glibc-only:

```bash
strings target/aarch64-unknown-linux-gnu/release/ultradag-node | grep -E '^lib.*\.so(\.[0-9]+)?$' | sort -u
# libc.so.6
# libdl.so.2
# libm.so.6
# libpthread.so.0
```

All four are part of the glibc package that ships with Raspberry Pi OS. No additional packages needed.

### Optional: smoke test without a Pi

If you have Docker Desktop on Apple Silicon (or Docker on an x86_64 Linux host with qemu-user binfmt installed), you can execute the binary before you own the board:

```bash
docker run --rm --platform linux/arm64 \
  -v "$(pwd)/target/aarch64-unknown-linux-gnu/release:/app:ro" \
  debian:bookworm-slim \
  /app/ultradag-node --help
```

On Apple Silicon this runs natively (no emulation) because Docker Desktop ships an arm64 Linux VM. The expected output is the full `--help` listing with all 20+ command-line flags. If you see that, the binary is good.

---

## Step 2: Flash Raspberry Pi OS to the SD card

Use the official [Raspberry Pi Imager](https://www.raspberrypi.com/software/) and pick:

- **OS**: Raspberry Pi OS Lite (64-bit) — Bookworm or newer. You don't need a desktop environment.
- **Storage**: a decent microSD card. **A2-rated is strongly recommended** — redb (UltraDAG's storage engine) fsyncs on commit, and a slow SD card can cause commit lag that stalls consensus under load. Avoid the "free with purchase" cards that come in bundles.
- **Customize settings** (the gear icon): set hostname, enable SSH, set your WiFi SSID + password, set the user/password. Do this now — it's much faster than doing it on the Pi afterward.

Eject, insert into the Pi, plug in USB power. Give it ~60 seconds to boot and join WiFi, then find it:

```bash
# On your dev machine
ping ultradag-pi.local              # if you set hostname = ultradag-pi
# or
arp -a | grep b8:27:eb               # Pi 3 MAC prefix
arp -a | grep dc:a6:32               # Pi 4 / Zero 2 W MAC prefix
```

SSH in:

```bash
ssh pi@ultradag-pi.local
```

---

## Step 3: Copy the binary and run

From your dev machine:

```bash
scp target/aarch64-unknown-linux-gnu/release/ultradag-node pi@ultradag-pi.local:~/
```

On the Pi:

```bash
# Install it where PATH can find it
sudo mv ultradag-node /usr/local/bin/
sudo chmod +x /usr/local/bin/ultradag-node

# Sanity check: does it start?
ultradag-node --help

# First real run — join the testnet as an observer (not yet producing vertices)
ultradag-node \
  --port 9333 \
  --seed ultradag-node-1.fly.dev:9333 \
  --testnet
```

Expected within ~30 seconds:

- Log line `Round duration: 5000ms`
- Fast-sync pulling checkpoints from the testnet
- `round: N finalized: N-1` advancing once per ~5 seconds

In another SSH session (or with `curl` from your dev machine if you exposed the RPC port):

```bash
curl http://ultradag-pi.local:10333/status | jq
```

You should see the Pi's local view of the testnet: round, finalized round, peer count, validators, mempool, and total supply.

---

## Step 4: Verify memory and CPU budget

While the node is running, in another SSH session:

```bash
# Memory: should settle well under 256 MB after fast-sync completes.
ps -o pid,rss,vsz,cmd -C ultradag-node

# CPU: should idle around 5-20% of a single core while tracking rounds.
top -b -n 1 | grep ultradag-node
```

**Acceptable ranges after the node has caught up:**

- **RSS** (resident memory): 50-200 MB steady-state
- **CPU**: 5-25% of one core under normal testnet load (the Pi Zero 2 W has 4 cores, so this is 1-6% aggregate)
- **Disk usage**: `~/.ultradag/node/` should settle around 80-150 MB depending on activity — if it grows without bound, something is wrong with pruning

If memory climbs past 300 MB and keeps going, that's a bug — capture `journalctl` output and open an issue. The target is "bounded storage forever."

---

## Step 5: Promote to validator (optional)

Once the observer is happily tracking rounds, you can turn it into a real validator. **Only do this on testnet for now** — mainnet validator setup has different security requirements (offline key generation, hardware wallet, etc. — see the [validator docs](/docs/getting-started/validator)).

```bash
# Stop the observer
sudo systemctl stop ultradag-node   # if running as a service
# or just Ctrl-C in the terminal

# Start in validator mode
ultradag-node \
  --port 9333 \
  --seed ultradag-node-1.fly.dev:9333 \
  --validate \
  --testnet

# In another terminal: note the validator address from the startup logs,
# then get testnet UDAG from the faucet
curl -X POST http://ultradag-pi.local:10333/faucet \
  -H 'Content-Type: application/json' \
  -d '{"address":"YOUR_VALIDATOR_ADDRESS","amount":1000000000000}'

# Stake 2,000 UDAG (the minimum)
curl -X POST http://ultradag-pi.local:10333/stake \
  -H 'Content-Type: application/json' \
  -d '{"secret_key":"YOUR_SECRET_KEY_HEX","amount":200000000000}'

# After a few rounds, you should see your address in the active set
curl http://ultradag-pi.local:10333/status | jq '.active_stakers'
```

The secret key is saved at `~/.ultradag/node/validator.key` — back it up, don't share it. For mainnet, generate keys offline on a different machine and never let them touch a network-connected server.

---

## Step 6: Run it as a systemd service

For "always-on" deployment:

```bash
# /etc/systemd/system/ultradag-node.service
sudo tee /etc/systemd/system/ultradag-node.service <<'EOF'
[Unit]
Description=UltraDAG full node
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
User=pi
ExecStart=/usr/local/bin/ultradag-node \
  --port 9333 \
  --seed ultradag-node-1.fly.dev:9333 \
  --testnet
Restart=on-failure
RestartSec=10
LimitNOFILE=65536

[Install]
WantedBy=multi-user.target
EOF

sudo systemctl daemon-reload
sudo systemctl enable --now ultradag-node
sudo systemctl status ultradag-node
```

Logs:

```bash
journalctl -u ultradag-node -f
```

---

## Known caveats on the Zero 2 W specifically

1. **WiFi only, 2.4 GHz only.** The Pi Zero 2 W has no Ethernet port and its radio is 2.4 GHz-only 802.11n, shared with Bluetooth. This is fine for a testnet participant but under household congestion you may see occasional peer disconnects. For production, use a USB-OTG Ethernet adapter.
2. **SD card is the bottleneck.** If commits are slow, it's almost always the SD card. Use A2-rated, avoid no-name bulk cards.
3. **Enable swap, but keep it small.** Bookworm defaults to 100 MB of swap which is fine. Don't grow it past 512 MB — if you're hitting swap regularly, the node is genuinely too big for the board and you should step up to a Pi 3 A+.
4. **The default CPU governor is `ondemand`.** For consistent block production latency, switch to `performance`: `sudo cpufreq-set -g performance` (requires `cpufrequtils`).
5. **Power it properly.** "Any USB-C charger" is NOT a valid power source for a Pi Zero 2 W under sustained load. Use a real 5V/2.5A PSU. Brownouts cause data corruption on the SD card.

---

## If something goes wrong

**Binary won't start**: check `file ultradag-node` on the Pi — if it says "aarch64" you're fine. If it says "arm" (without 64) you grabbed a 32-bit binary (wrong target) and need to rebuild with `--target aarch64-unknown-linux-gnu`.

**"exec format error"**: you flashed a 32-bit Raspberry Pi OS image. Reflash with Raspberry Pi OS **Lite 64-bit**.

**Node starts but never syncs past round 0**: check `--seed` is reachable (`nc -vz ultradag-node-1.fly.dev 9333`) and that your firewall / carrier isn't blocking outbound 9333.

**Node syncs but `/status` shows `last_finalized: null`**: wait another ~30 seconds. First-time sync via checkpoints takes a bit even on healthy hardware.

**RSS memory climbs past 400 MB**: this is not normal. Capture the full log and open an issue — we care about this. The design target is bounded memory forever.

---

## Why this page exists

The homepage claims UltraDAG full nodes run on a $15 Raspberry Pi Zero 2 W. A claim like that is worth nothing without a real bringup path that a user can follow without making judgment calls at every step. This page is that path. If you follow it and something is wrong, that's a documentation bug — please open an issue with the exact command that failed.
