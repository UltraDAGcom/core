---
title: UltraDAG — Lightweight DAG-BFT for IoT Micropayments
---

# UltraDAG

**A lightweight DAG-BFT cryptocurrency purpose-built for IoT and machine-to-machine micropayments.** UltraDAG delivers deterministic finality in two rounds (~10 seconds), runs on hardware as small as a $5/month VPS, and ships as a single binary under 2 MB. No bloat, no complexity — just fast, verifiable value transfer for the machine economy.

---

## At a Glance

<div class="grid cards" markdown>

-   **< 2 MB Binary**

    ---

    Single static binary. Runs on Raspberry Pi, edge gateways, and $5/month cloud instances.

-   **2-Round Finality**

    ---

    DAG-BFT consensus finalizes transactions in ~10 seconds with 5-second rounds. Deterministic, not probabilistic.

-   **21M Max Supply**

    ---

    Bitcoin-style fixed supply with halving every ~1.66 years. Sound money for machines.

-   **836 Tests Passing**

    ---

    Comprehensive test suite including 14 Jepsen fault-injection tests and TLA+ formal verification over 32.6 million states.

</div>

---

## Core Features

### Fast Finality
Leaderless DAG-BFT consensus with parallel vertex production. Transactions finalize when >2/3 of validators have observed them — typically within two rounds. No leader election, no view changes, no wasted rounds.

### Bounded Storage
Automatic pruning keeps node storage bounded at ~1000 rounds of DAG history. New nodes join via checkpoint fast-sync rather than replaying the full history. Run a validator without ever-growing disk requirements.

### IoT-Ready
The entire node compiles to a sub-2 MB binary with minimal dependencies. Ed25519 signatures and Blake3 hashing keep CPU overhead low. The Noise protocol encrypts all P2P traffic with forward secrecy — critical for untrusted network environments.

### Formally Verified
The consensus protocol is specified in TLA+ and model-checked across 32.6 million states with zero invariant violations. Six safety properties are verified including BFT safety, equivocation detection, and round monotonicity.

---

## Quick Links

| Goal | Where to Go |
|------|------------|
| Run your first node in 5 minutes | [Quick Start](getting-started/quickstart.md) |
| Deploy with Docker | [Docker Guide](getting-started/docker.md) |
| Become a validator | [Run a Validator](getting-started/validator.md) |
| Integrate via RPC | [API Reference](api/rpc.md) |
| Use an SDK | [SDKs](api/sdks.md) |
| Understand the consensus | [DAG-BFT Consensus](architecture/consensus.md) |
| Read the security model | [Security Model](security/model.md) |

---

## Live Testnet

!!! info "5-Node Testnet on Fly.io (Amsterdam)"

    The public testnet is live with 5 validator nodes:

    | Node | Endpoint |
    |------|----------|
    | Node 1 | `ultradag-node-1.fly.dev` |
    | Node 2 | `ultradag-node-2.fly.dev` |
    | Node 3 | `ultradag-node-3.fly.dev` |
    | Node 4 | `ultradag-node-4.fly.dev` |
    | Node 5 | `ultradag-node-5.fly.dev` |

    Check node status:

    ```bash
    curl https://ultradag-node-1.fly.dev/status
    ```

    Get free testnet UDAG:

    ```bash
    curl -X POST https://ultradag-node-1.fly.dev/faucet -H "Content-Type: application/json" -d '{"address":"YOUR_ADDRESS","amount":100000000}'
    ```

---

## Project Structure

UltraDAG is organized as a Rust workspace with five crates:

| Crate | Purpose |
|-------|---------|
| `ultradag-coin` | Consensus engine, state machine, tokenomics |
| `ultradag-network` | P2P transport, Noise encryption, DAG sync |
| `ultradag-node` | Binary entry point, RPC server, CLI |
| `ultradag-sim` | Deterministic simulation harness |
| `ultradag-sdk` | Rust SDK for integration |

Four additional SDKs are available for [Python, JavaScript, Go, and Rust](api/sdks.md).

---

## Contributing

UltraDAG is open source under the BUSL-1.1 (Business Source License). Contributions are welcome:

1. Fork the [repository](https://github.com/UltraDAGcom/core)
2. Create a feature branch
3. Ensure all 836 tests pass: `cargo test --workspace`
4. Submit a pull request

See the [Bug Bounty](security/bug-bounty.md) program for security-related contributions.
