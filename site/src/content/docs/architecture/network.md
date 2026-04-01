---
title: "P2P Network"
description: "UltraDAG's P2P layer — Noise-encrypted transport, DAG vertex propagation, transaction gossip, and state synchronization."
order: 3
section: "architecture"
---

# P2P Network

UltraDAG's P2P layer handles encrypted communication between nodes, DAG vertex propagation, transaction gossip, and state synchronization. All traffic is encrypted using the Noise protocol framework.

---

## Transport Layer

### Noise Protocol Encryption

All connections use the **Noise_XX_25519_ChaChaPoly_BLAKE2s** handshake pattern:

- **XX pattern**: mutual authentication with static key exchange (3-message handshake)
- **X25519**: Diffie-Hellman key agreement (ephemeral + static keys)
- **ChaChaPoly**: AEAD symmetric encryption for application data
- **BLAKE2s**: hash function for key derivation

Every message after the handshake is encrypted with forward secrecy — compromising a node's long-term key does not decrypt past traffic.

See [Noise Encryption](/docs/technical/noise-protocol) for the full protocol specification.

### Connection Model

Each peer connection uses **split read/write** streams:

*Node A has separate Write and Read tasks. Node B also has separate Write and Read tasks. Node A's Write task sends encrypted data to Node B's Read task, and Node B's Write task sends encrypted data to Node A's Read task.*

- Connections are **bidirectional** — either side can send any message type
- Read and write are handled by separate async tasks for non-blocking I/O
- Connection lifecycle is managed with graceful shutdown on errors

### Wire Format

Messages use a simple framing protocol:

```
┌──────────────┬─────────────────────────┐
│ Length (4 B)  │ Payload (postcard bytes) │
└──────────────┴─────────────────────────┘
```

- **Length prefix**: 4-byte big-endian u32
- **Payload**: [postcard](https://docs.rs/postcard)-encoded binary message
- **Maximum message size**: 4 MB (4,194,304 bytes)

<div class="callout callout-note"><div class="callout-title">Why postcard?</div>Postcard is a compact, deterministic binary serialization format built on serde. It produces smaller payloads than JSON or bincode with minimal overhead, which matters for IoT nodes on constrained networks.</div>

---

## Message Types

| Message | Direction | Purpose |
|---------|-----------|---------|
| `Hello` / `HelloAck` | Bidirectional | Initial handshake, exchange version, height, listen port |
| `DagProposal` | Broadcast | New DAG vertex produced by this validator |
| `GetDagVertices` | Request | Request vertices by round range `{from_round, max_count}` |
| `DagVertices` | Response | Batch of requested vertices |
| `GetParents` | Request | Request specific vertices by hash (for resolving missing parents) |
| `ParentVertices` | Response | Requested parent vertices |
| `NewTx` | Broadcast | New transaction for mempool inclusion |
| `GetPeers` / `Peers` | Bidirectional | Peer discovery via gossip |
| `GetRoundHashes` / `RoundHashes` | Request/Response | Round-level hash comparison for sync |
| `CheckpointProposal` | Broadcast | Propose a new checkpoint |
| `CheckpointSignatureMsg` | Broadcast | Co-sign a checkpoint: `{round, checkpoint_hash, signature}` |
| `GetCheckpoint` | Request | Request latest checkpoint for fast-sync |
| `CheckpointSync` | Response | Full checkpoint with state snapshot and suffix vertices |
| `EquivocationEvidence` | Broadcast | Two conflicting vertices from same validator+round |
| `Ping` / `Pong` | Bidirectional | Keepalive and latency measurement |

### Hello Message

The `Hello` message is exchanged immediately after the Noise handshake:

```rust
struct Hello {
    version: u32,
    height: u64,        // Current DAG round
    listen_port: u16,   // P2P listen port for reverse connections
}
```

Network domain separation is handled at the Noise handshake level via `NETWORK_ID` — testnet and mainnet nodes use different identity binding prefixes.

---

## DAG Sync Protocol

When a new node joins or a node falls behind, it must synchronize the DAG. There are two sync mechanisms:

### Incremental Sync (DAG Catch-Up)

For nodes that are slightly behind (within the pruning horizon):

*Sync sequence: The new node sends Hello (my round = 50), the peer responds with Hello (my round = 200). The new node then requests GetDagVertices for rounds 51-200. The peer responds with multiple DagVertices batches covering rounds 51-100, 101-150, and 151-200. The new node inserts vertices and resolves parents.*

**Orphan resolution**: If a received vertex references a parent hash that is not yet known:

1. The vertex is held in an orphan buffer (capped at 1000 entries / 50MB, with per-peer cap of 100)
2. A `GetParents { hashes }` request is sent for the missing parent hashes (capped at 32 per request)
3. The peer responds with `ParentVertices` containing the requested vertices
4. When the parent arrives, `resolve_orphans()` attempts to flush buffered orphans
5. This recurses until all ancestors are resolved or found in local state

### Checkpoint Sync (Fast-Sync)

For nodes that are far behind (beyond the pruning horizon) or joining fresh:

1. Request the latest checkpoint from a peer
2. Receive the checkpoint including:
    - State snapshot (full account/stake/governance state)
    - Checkpoint signatures (>2/3 validator co-signatures)
    - Suffix vertices (recent DAG vertices since the checkpoint)
3. Verify the checkpoint signatures
4. Load the state snapshot
5. Apply suffix vertices to catch up to the current round

<div class="callout callout-tip"><div class="callout-title">Fast-sync vs full sync</div>Fast-sync takes seconds instead of potentially hours. New nodes default to checkpoint sync. Use <code>--skip-fast-sync</code> to force full DAG sync from genesis (only useful for verification purposes).</div>

See [Checkpoint Protocol](/docs/technical/checkpoints) for full details.

---

## Noise Handshake Flow

The XX pattern requires 3 messages:

*Handshake sequence: The Initiator generates an ephemeral X25519 keypair and sends Message 1 (ephemeral public key) to the Responder. The Responder generates its own ephemeral keypair and sends Message 2 (ephemeral + static keys) back. The Initiator sends Message 3 (static key + proof). After this, transport keys are established and all subsequent messages are encrypted.*

After the handshake:

- Both parties have authenticated static keys
- Forward-secret transport keys are derived from ephemeral DH
- Validators additionally bind their Ed25519 identity to the Noise static key

---

## Rate Limiting

UltraDAG implements multi-layer rate limiting to prevent abuse:

### Per-Peer Aggregate Limit

| Parameter | Value |
|-----------|-------|
| Max messages per peer | 500 |
| Window | 60 seconds |

A peer exceeding 500 messages in any 60-second window is temporarily throttled.

### Per-Message-Type Cooldowns

| Message Type | Cooldown |
|-------------|----------|
| `GetDagVertices` | 2 seconds per peer |
| `GetRoundHashes` | 10 seconds per peer |
| `DagProposal` | 1 per round per validator |

### Violation Handling

Peers exceeding the aggregate limit (500 messages / 60 seconds) are disconnected with a warning log. There is no progressive ban system — the connection is closed immediately and the peer must reconnect.

---

## Bootstrap Nodes

New nodes discover the network through bootstrap nodes. The testnet has 4 hardcoded bootstrap addresses (dedicated IPv4):

```
206.51.242.223:9333   # ultradag-node-1
137.66.57.226:9333    # ultradag-node-2
169.155.54.169:9333   # ultradag-node-3
169.155.55.151:9333   # ultradag-node-4
```

After connecting to bootstrap nodes, the node learns additional peer addresses through `GetPeers`/`Peers` gossip. The `--no-bootstrap` flag disables automatic bootstrap connections (useful for isolated local testnets). Exponential backoff retry (2, 4, 8, 16, 32 seconds) is used for bootstrap connections.

---

## Connection Management

### Peer Discovery

Peers are discovered through:

1. **Bootstrap nodes**: hardcoded addresses for initial connection
2. **Peer exchange**: nodes share known peer addresses during `Hello`
3. **Incoming connections**: any node can connect to a listening node

### Connection Limits

| Parameter | Default |
|-----------|---------|
| Max incoming connections | 16 (`MAX_INBOUND_PEERS`) |
| Handshake timeout | 10 seconds |
| Read timeout | 30 seconds (per message) |

### Reconnection

If a peer disconnects:

1. Wait 5 seconds before attempting reconnection
2. Exponential backoff up to 60 seconds
3. After 10 failed attempts, remove peer from known list
4. Bootstrap nodes are always retried regardless of failure count

---

## Network Security

### Eclipse Attack Prevention

An eclipse attack isolates a node by surrounding it with malicious peers. UltraDAG mitigates this through:

- **Checkpoint verification**: fast-sync checkpoints require >2/3 validator signatures
- **Diverse peer selection**: connects to peers across different IP ranges
- **Bootstrap diversity**: multiple independent bootstrap nodes

### Message Validation

All received messages are validated before processing:

- **Signatures**: Ed25519 signatures are verified with `verify_strict`
- **Round bounds**: vertices from the far future or deep past are rejected
- **Parent existence**: referenced parents must exist or be requested
- **Duplicate detection**: duplicate vertex hashes are discarded

See [Security Model](/docs/security/model) for the full threat analysis.

---

## Next Steps

- [Noise Encryption](/docs/technical/noise-protocol) — detailed Noise protocol specification
- [Checkpoint Protocol](/docs/technical/checkpoints) — fast-sync and checkpoint co-signing
- [State Engine](/docs/architecture/state-engine) — how synced vertices become account state
