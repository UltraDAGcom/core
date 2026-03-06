# UltraDAG

A minimal DAG-BFT cryptocurrency. Built in Rust.

**Ed25519 signatures. DAG-BFT consensus. Blake3 hashing. 21M max supply. P2P networking.**

## Quick Start

```bash
# Run a validator node (RPC on port 10333)
cargo run --release -p ultradag-node -- --port 9333 --validate

# Connect a second validator
cargo run --release -p ultradag-node -- --port 9334 --seed 127.0.0.1:9333 --validate

# Custom round duration (default 5000ms)
cargo run --release -p ultradag-node -- --port 9335 --seed 127.0.0.1:9333 --validate --round-ms 3000

# Query chain status
curl http://127.0.0.1:10333/status

# Generate a keypair
curl http://127.0.0.1:10333/keygen

# Check balance
curl http://127.0.0.1:10333/balance/<address>

# Send UDAG
curl -X POST http://127.0.0.1:10333/tx \
  -d '{"from_secret":"<secret_key_hex>","to":"<address_hex>","amount":1000000000,"fee":100000}'
```

## Architecture

```
ultradag-coin     Crypto primitives: Ed25519 keys, DAG-BFT consensus, blockchain
ultradag-network  TCP P2P: peer discovery, block/tx/DAG relay, fork resolution
ultradag-node     Full node binary: round-based validator, HTTP RPC, peer management
```

## Crates

| Crate | Description |
|---|---|
| `ultradag-coin` | DAG-BFT consensus cryptocurrency. Ed25519 signatures, multi-parent DAG, BFT finality, 21M max supply, Blake3 hashing, account-based ledger. |
| `ultradag-network` | TCP P2P networking. Peer discovery, block/tx/DAG relay, split reader/writer connections, fork resolution via chain reorg. |
| `ultradag-node` | Full node binary. Round-based DAG-BFT validator, Ed25519-signed vertices, finality tracking, HTTP RPC with CORS. |

## Consensus

UltraDAG uses DAG-BFT consensus:

- **DAG structure**: each vertex references all known tips (multiple parents)
- **Round-based**: validators produce one signed vertex per round (configurable interval)
- **Ed25519 signed**: every vertex is signed by the proposing validator
- **BFT finality**: a vertex is finalized when 2/3+ validators have descendants
- **Parallel blocks**: multiple validators produce vertices concurrently
- **Fork resolution**: longest valid chain wins, with automatic reorg

## Tokenomics

- Max supply: 21,000,000 UDAG (1 UDAG = 100,000,000 sats)
- Initial reward: 50 UDAG per block
- Halving: every 210,000 blocks
- Target round time: 5 seconds (configurable via `--round-ms`)

## Cryptography

- **Signing**: Ed25519 via `ed25519-dalek`
- **Addresses**: `blake3(ed25519_public_key)` — 32 bytes
- **Transactions**: carry sender's public key for on-chain verification
- **DAG vertices**: Ed25519-signed by the proposing validator
- **Block hashing**: Blake3
- **Merkle trees**: Blake3-based for transaction commitment

## License

MIT OR Apache-2.0
