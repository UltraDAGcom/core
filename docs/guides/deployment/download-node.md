# Download & Run UltraDAG Node

## Quick Install

### Linux (x86_64)
```bash
curl -L https://github.com/UltraDAGcom/core/releases/download/latest/ultradag-node-linux-x86_64.tar.gz | tar xz
chmod +x ultradag-node-linux-x86_64
./ultradag-node-linux-x86_64 --port 9333
```

### macOS (Apple Silicon)
```bash
curl -L https://github.com/UltraDAGcom/core/releases/download/latest/ultradag-node-macos-arm64.tar.gz | tar xz
chmod +x ultradag-node-macos-arm64
./ultradag-node-macos-arm64 --port 9333
```

### macOS (Intel)
```bash
curl -L https://github.com/UltraDAGcom/core/releases/download/latest/ultradag-node-macos-x86_64.tar.gz | tar xz
chmod +x ultradag-node-macos-x86_64
./ultradag-node-macos-x86_64 --port 9333
```

## Run as Validator

To produce blocks and earn UDAG emission rewards:

```bash
./ultradag-node-linux-x86_64 --port 9333 --validate
```

### With Your Own Key

```bash
./ultradag-node-linux-x86_64 --port 9333 --validate --pkey <your-64-char-hex-private-key>
```

### Auto-Stake on Startup

```bash
./ultradag-node-linux-x86_64 --port 9333 --validate --auto-stake 10000
```

## CLI Reference

| Flag | Default | Description |
|------|---------|-------------|
| `--port <PORT>` | 9333 | P2P listen port |
| `--rpc-port <PORT>` | port+1000 | HTTP RPC port |
| `--validate` | false | Enable block production |
| `--pkey <HEX>` | auto | 64-char hex private key |
| `--auto-stake <UDAG>` | - | Auto-stake after sync |
| `--seed <host:port>` | bootstrap | Seed peer address |
| `--validators <N>` | auto | Expected validator count |
| `--round-ms <MS>` | 5000 | Round duration |
| `--data-dir <PATH>` | ~/.ultradag/node | Data directory |
| `--pruning-depth <N>` | 1000 | Rounds to keep |
| `--archive` | false | Keep full history |
| `--no-bootstrap` | false | Skip bootstrap nodes |
| `--testnet` | true | Enable testnet endpoints |

## Build from Source

```bash
git clone https://github.com/UltraDAGcom/core.git
cd core
cargo build --release -p ultradag-node
./target/release/ultradag-node --port 9333 --validate
```

## Docker

```bash
docker run -p 9333:9333 -p 10333:10333 ghcr.io/ultradagcom/ultradag-node:latest
```

## Verify Installation

After starting, check health:
```bash
curl http://localhost:10333/health
# {"status":"ok"}

curl http://localhost:10333/status
# Shows round, finality, supply, peers
```
