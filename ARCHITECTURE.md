# UltraDAG Architecture

This document explains the technical design of UltraDAG — why DAG-BFT was chosen, how the consensus mechanism works, what the epoch system does, and how the three-crate structure fits together.

## Table of Contents

- [Design Philosophy](#design-philosophy)
- [Why DAG-BFT?](#why-dag-bft)
- [Consensus Algorithm](#consensus-algorithm)
- [Epoch System](#epoch-system)
- [Optimistic Responsiveness](#optimistic-responsiveness)
- [State Engine](#state-engine)
- [Checkpoint System](#checkpoint-system)
- [Governance](#governance)
- [Network Protocol](#network-protocol)
- [Crate Structure](#crate-structure)
- [Data Flow](#data-flow)
- [Performance Characteristics](#performance-characteristics)

---

## Design Philosophy

UltraDAG is designed around three core principles:

1. **Simplicity over features** — The codebase should be readable by a competent Rust developer in a weekend. No exotic cryptography, no complex VM, no unnecessary abstractions.

2. **Community-first governance** — On-chain governance is not a feature bolted on later. It's baked into the state machine from genesis.

3. **Real decentralization** — Low hardware requirements (runs on a $5/month VPS), stake-weighted consensus (not hardware-weighted like PoW), and transparent validator selection.

These principles drive every architectural decision.

---

## Why DAG-BFT?

### The Problem with Traditional Blockchains

**Linear chains are slow:**
- Bitcoin: 10-minute blocks, 1-hour finality
- Ethereum: 12-second blocks, 15-minute finality (post-merge)

Why? Because linear chains enforce total ordering of all blocks. Block N+1 cannot be proposed until block N is finalized. This serializes consensus.

**Leader-based BFT has centralization risk:**
- PBFT, Tendermint, HotStuff all elect a leader per round
- Leaders can censor transactions
- Leader rotation adds latency

### The DAG-BFT Solution

**Directed Acyclic Graph (DAG):**
- Multiple validators propose blocks simultaneously
- Each block references multiple parent blocks
- No single leader — all validators participate equally
- Partial ordering is sufficient for most transactions

**Byzantine Fault Tolerance (BFT):**
- Tolerates up to f < n/3 Byzantine (malicious) validators
- Deterministic finality — no probabilistic confirmation
- No forks — finalized blocks are permanent

**Result:**
- 2.5-second round times
- 1-2 round finality (2.5-5 seconds)
- No leader bottleneck
- Censorship-resistant (any honest validator can include your transaction)

### Why Not Proof-of-Work?

PoW was considered and rejected:

**Energy waste:** Bitcoin uses ~150 TWh/year. UltraDAG uses ~1 kWh/year for the entire network.

**Centralization:** PoW mining is dominated by ASIC manufacturers and cheap electricity. UltraDAG validators need only a $5/month VPS.

**Slow finality:** PoW requires waiting for multiple confirmations (10-60 minutes). UltraDAG finalizes in 2.5-5 seconds.

**No governance:** PoW has no on-chain mechanism for protocol upgrades. UltraDAG has stake-weighted voting built in.

---

## Consensus Algorithm

UltraDAG implements a variant of DAG-BFT based on the Narwhal/Bullshark family of protocols.

### Block Structure

Each block (called a "vertex" in the code) contains:

```
DagVertex {
    round: u64,              // Consensus round number
    author: Address,         // Validator who created this vertex
    parents: Vec<[u8; 32]>,  // Hashes of parent vertices from previous round
    transactions: Vec<Tx>,   // Transactions included in this vertex
    timestamp: u64,          // Unix timestamp (milliseconds)
    signature: Signature,    // Ed25519 signature by author
}
```

### DAG Construction

**Round N:**
```
    [V1] [V2] [V3] [V4]  ← Round N (4 validators)
     ↓ ↘ ↓ ↘ ↓ ↘ ↓ ↘
    [V1] [V2] [V3] [V4]  ← Round N+1
     ↓ ↘ ↓ ↘ ↓ ↘ ↓ ↘
    [V1] [V2] [V3] [V4]  ← Round N+2
```

Each vertex in round N+1 references at least 2f+1 vertices from round N (where f = max Byzantine validators).

**Why 2f+1?** With n validators and f < n/3 Byzantine:
- n = 4 → f = 1 → need 3 parents (quorum)
- n = 21 → f = 7 → need 15 parents (quorum)

This ensures that if a vertex is accepted, at least one honest validator vouched for its parents.

### Finality Rule

A vertex is **finalized** when:

1. It has been referenced (directly or transitively) by 2f+1 vertices in round N+2
2. Those 2f+1 vertices are themselves accepted by the local node

**Example with 4 validators (f=1, quorum=3):**

```
Round 100: [A] [B] [C] [D]
            ↓   ↓   ↓   ↓
Round 101: [A] [B] [C] [D]  ← Each references 3+ parents from round 100
            ↓   ↓   ↓   ↓
Round 102: [A] [B] [C] [D]  ← Each references 3+ parents from round 101

When round 102 vertices are accepted, round 100 vertices are finalized.
```

**Finality lag:** Typically 1-2 rounds (2.5-5 seconds).

### Transaction Ordering

Within a finalized round, transactions are ordered deterministically:

1. Sort vertices by hash (lexicographic)
2. Process transactions in vertex order
3. Skip duplicate transactions (same hash seen earlier)

This gives a total ordering of all finalized transactions without requiring a leader.

### Byzantine Fault Scenarios

**Equivocation (double-signing):**
- Validator creates two vertices for the same round
- Detection: Other validators see conflicting vertices with same (round, author)
- Penalty: Immediate slashing (50% stake burned), removal from active set

**Censorship:**
- Malicious validator refuses to include certain transactions
- Mitigation: Other honest validators will include them
- Result: Transaction delayed by 1 round, not censored

**Network partition:**
- If < 2f+1 validators are reachable, consensus halts
- No finalization occurs (safety preserved)
- When partition heals, consensus resumes from last finalized round

---

## Epoch System

### What is an Epoch?

An **epoch** is a period of 210,000 rounds (~6 days at 2.5s/round) during which the active validator set is frozen.

**Why freeze the validator set?**
- Consensus requires knowing who the validators are
- Changing validators mid-consensus breaks safety guarantees
- Epochs provide a clean boundary for validator set updates

### Epoch Lifecycle

```
Epoch 0: Rounds 0 - 209,999
  ↓ (validator set recalculated at round 210,000)
Epoch 1: Rounds 210,000 - 419,999
  ↓ (validator set recalculated at round 420,000)
Epoch 2: Rounds 420,000 - 629,999
  ...
```

### Validator Selection

At each epoch boundary, the StateEngine:

1. Collects all stake accounts with `staked >= MIN_STAKE_SATS` (10,000 UDAG)
2. Sorts by stake amount (descending)
3. Takes top `MAX_ACTIVE_VALIDATORS` (currently 21)
4. This becomes the active validator set for the next epoch

**Observer rewards:**
- Validators ranked 22-100 are "observers"
- They don't participate in consensus but earn 20% of block rewards
- This incentivizes running nodes even if you're not in the top 21

### Stake Changes During an Epoch

**Staking:**
- New stake is added immediately to the stake account
- But does NOT affect the active validator set until next epoch
- This prevents validator set thrashing

**Unstaking:**
- Initiates a cooldown period (2,016 rounds ≈ 1.4 hours)
- Stake remains locked during cooldown
- After cooldown, stake is returned to liquid balance
- If unstaking drops you below MIN_STAKE_SATS, you're removed at next epoch

### Epoch Synchronization

When a new node joins:

1. It requests the latest checkpoint (includes epoch number)
2. It fast-syncs to the checkpoint state
3. It verifies the current epoch matches `round / EPOCH_LENGTH_ROUNDS`
4. If epochs don't match, it recalculates the active validator set

This ensures all nodes agree on who the validators are, even after downtime.

---

## Optimistic Responsiveness

### The Problem

Traditional BFT protocols have fixed timeout periods:
- "Wait 5 seconds for leader proposal"
- "Wait 3 seconds for votes"

This works but is inefficient:
- If network is fast, you wait unnecessarily
- If network is slow, you timeout and retry

### The Solution

UltraDAG uses **optimistic responsiveness**: proceed as fast as the network allows.

**How it works:**

1. Validator creates a vertex immediately when it has 2f+1 parents from the previous round
2. No waiting for a timeout
3. No leader election delay

**Result:**
- In good network conditions: 2.5-second rounds
- In degraded conditions: rounds slow down naturally
- No artificial delays

### Adaptive Round Duration

The `ROUND_DURATION_MS` constant (2500ms) is a *target*, not a hard requirement.

**Fast path:**
- If 2f+1 validators create vertices quickly → round completes in <2.5s
- Next round starts immediately

**Slow path:**
- If network is congested → some validators take >2.5s
- Round completes when 2f+1 vertices are received
- Next round starts when ready

This adapts to actual network conditions without manual tuning.

---

## State Engine

The StateEngine is the heart of UltraDAG. It maintains all account state and applies finalized transactions.

### State Components

```rust
StateEngine {
    accounts: HashMap<Address, AccountState>,
    stake_accounts: HashMap<Address, StakeAccount>,
    proposals: HashMap<u64, Proposal>,
    votes: HashMap<(u64, Address), bool>,
    total_supply: u64,
    last_finalized_round: Option<u64>,
    active_validator_set: Vec<Address>,
    current_epoch: u64,
    next_proposal_id: u64,
}
```

### Account State

```rust
AccountState {
    balance: u64,  // Liquid balance in sats
    nonce: u64,    // Transaction counter (prevents replay)
}
```

### Stake Account

```rust
StakeAccount {
    staked: u64,                  // Amount currently staked
    unlock_at_round: Option<u64>, // If unstaking, when it completes
}
```

### Transaction Processing

When a vertex is finalized:

```
apply_vertex(vertex):
  1. Verify vertex signature
  2. For each transaction in vertex:
     a. Verify transaction signature
     b. Check nonce (must equal current nonce)
     c. Check balance (sufficient for amount + fee)
     d. Apply state changes:
        - Transfer: debit sender, credit recipient
        - Stake: move balance → stake account
        - Unstake: start cooldown timer
        - CreateProposal: create proposal, deduct fee
        - Vote: record vote, deduct fee
     e. Increment sender nonce
  3. Credit block reward to vertex author
  4. Update last_finalized_round
  5. If epoch boundary, recalculate active validator set
  6. Tick governance (update proposal statuses)
```

### State Invariants

The StateEngine enforces these invariants at all times:

1. **Supply conservation:** `sum(balances) + sum(staked) == total_supply`
2. **Nonce ordering:** Transactions from an address must have sequential nonces
3. **No double-spend:** Balance checks prevent spending more than you have
4. **No double-vote:** Each address can vote once per proposal

These are verified in debug builds after every vertex application.

---

## Checkpoint System

Checkpoints enable fast-sync for new nodes without replaying the entire DAG history.

### What is a Checkpoint?

A checkpoint is a cryptographic commitment to the state at a specific round:

```rust
Checkpoint {
    round: u64,                    // Round number
    state_root: [u8; 32],          // Hash of StateSnapshot
    dag_tip: [u8; 32],             // Hash of latest finalized vertex
    total_supply: u64,             // Total UDAG in circulation
    signatures: Vec<(Address, Signature)>,  // Validator signatures
}
```

### Checkpoint Creation

Every `CHECKPOINT_INTERVAL` rounds (1,000 rounds ≈ 42 minutes):

1. Active validators create a StateSnapshot
2. Compute `state_root = hash(snapshot)`
3. Sign the checkpoint: `sign(round || state_root || dag_tip)`
4. Broadcast signature to other validators
5. When 2f+1 signatures collected → checkpoint is valid

### Fast-Sync Process

New node joining the network:

```
1. Request latest checkpoint from peers
2. Verify 2f+1 validator signatures
3. Download StateSnapshot from peer
4. Verify hash(snapshot) == state_root
5. Load snapshot into StateEngine
6. Download DAG vertices since checkpoint
7. Apply vertices to catch up to current round
```

**Time savings:**
- Full replay: ~1 hour per 100,000 rounds
- Fast-sync: ~30 seconds to download + verify checkpoint

### Checkpoint Security

**Can a malicious validator create a fake checkpoint?**

No. Checkpoints require 2f+1 signatures. If f < n/3 validators are Byzantine, at least f+1 signatures must come from honest validators. Honest validators only sign checkpoints they've verified.

**Can an attacker give you a fake StateSnapshot?**

No. The checkpoint includes `state_root = hash(snapshot)`. If the snapshot doesn't match the hash, verification fails.

**What if the checkpoint is old?**

The node downloads vertices since the checkpoint and applies them. As long as the checkpoint is valid (2f+1 signatures), the node will converge to the correct state.

---

## Governance

UltraDAG has on-chain governance built into the state machine. This is not a sidechain or a separate system — governance transactions are processed the same way as transfers.

### Proposal Lifecycle

```
1. CreateProposal transaction → Proposal created (status: Active)
2. Voting period: 120,960 rounds (~3.5 days)
3. Vote transactions → votes_for and votes_against accumulate
4. Voting ends:
   - If quorum reached (10% of staked supply) AND approval (66% yes):
     → status: PassedPending (execute_at_round = current + 2,016)
   - Else:
     → status: Rejected
5. Execution delay passes → status: Executed (manual execution required)
```

### Proposal Types

**TextProposal:**
- Non-binding community sentiment
- Example: "Should we prioritize mobile wallet development?"

**ParameterChange:**
- Binding change to a protocol parameter
- Example: Change `MIN_FEE_SATS` from 10,000 to 5,000
- Requires manual code deployment (governance cannot modify running code)

### Vote Weighting

Votes are weighted by staked amount:

```
Vote from address A with 100,000 UDAG staked:
  → Adds 100,000 to votes_for or votes_against

Vote from address B with 10,000 UDAG staked:
  → Adds 10,000 to votes_for or votes_against
```

**Why stake-weighted?**
- Aligns voting power with economic stake in the network
- Prevents Sybil attacks (creating many addresses doesn't help)
- Validators have the most to lose from bad decisions

### Governance Constants

```rust
MIN_STAKE_TO_PROPOSE = 50,000 UDAG  // Prevents spam proposals
GOVERNANCE_VOTING_PERIOD_ROUNDS = 120,960  // ~3.5 days
GOVERNANCE_QUORUM_NUMERATOR = 10  // 10% of staked supply must vote
GOVERNANCE_QUORUM_DENOMINATOR = 100
GOVERNANCE_APPROVAL_NUMERATOR = 66  // 66% of votes must be "yes"
GOVERNANCE_APPROVAL_DENOMINATOR = 100
GOVERNANCE_EXECUTION_DELAY_ROUNDS = 2,016  // ~1.4 hours safety buffer
MAX_ACTIVE_PROPOSALS = 20  // Prevents state bloat
```

### Governance State Persistence

Governance state is included in checkpoints:

```rust
StateSnapshot {
    // ... other fields ...
    proposals: Vec<(u64, Proposal)>,
    votes: Vec<((u64, Address), bool)>,
    next_proposal_id: u64,
}
```

This ensures governance history is preserved across node restarts and fast-sync.

---

## Network Protocol

UltraDAG uses a custom binary protocol over TCP for P2P communication.

### Message Types

```rust
enum Message {
    // Handshake
    Hello { version: u32, address: Address },
    HelloAck { peers: Vec<SocketAddr> },
    
    // Block propagation
    NewBlock(DagVertex),
    GetBlocks { from_round: u64 },
    Blocks(Vec<DagVertex>),
    
    // Transaction propagation
    NewTx(Transaction),
    
    // Checkpoint sync
    GetCheckpoint,
    CheckpointSync { checkpoint, vertices, state },
    CheckpointProposal(Checkpoint),
    CheckpointSignature { round, signature },
    
    // Peer discovery
    GetPeers,
    Peers(Vec<SocketAddr>),
    
    // Keepalive
    Ping,
    Pong,
}
```

### Message Encoding

Messages are length-prefixed bincode:

```
[4 bytes: message length] [N bytes: bincode-encoded message]
```

**Why bincode?**
- Fast: zero-copy deserialization
- Compact: smaller than JSON
- Type-safe: Rust compiler verifies encoding/decoding

### Connection Management

Each peer connection is split into reader/writer halves:

```
PeerConnection {
    reader: TcpStream (read half),
    writer: TcpStream (write half),
}
```

**Reader task:**
- Reads messages from peer
- Dispatches to appropriate handler
- Runs in separate async task

**Writer task:**
- Sends messages to peer
- Queues outgoing messages
- Runs in separate async task

This allows simultaneous send/receive without blocking.

### Peer Discovery

**Bootstrap:**
1. Node starts with `--seed` addresses (known peers)
2. Connects to seed peers
3. Sends `GetPeers` message
4. Receives list of other peers
5. Connects to those peers
6. Repeats until well-connected

**Ongoing:**
- Periodically send `GetPeers` to discover new nodes
- Maintain 8-20 active connections
- Disconnect from unresponsive peers

### Gossip Protocol

**Block propagation:**
1. Validator creates new vertex
2. Broadcasts `NewBlock(vertex)` to all peers
3. Peers validate and store vertex
4. Peers re-broadcast to their peers
5. Vertex reaches all nodes within 1-2 network hops

**Transaction propagation:**
- Same as block propagation
- Transactions gossip until included in a finalized vertex

---

## Crate Structure

### `ultradag-coin/` (Core Logic)

**Purpose:** State machine, consensus, and transaction processing.

**Key files:**
- `consensus/dag.rs` — DAG structure, vertex storage
- `consensus/finality.rs` — Finality tracking, validator set
- `consensus/epoch.rs` — Epoch management
- `state/engine.rs` — StateEngine implementation
- `tx/transaction.rs` — Transaction types
- `governance/mod.rs` — Governance types
- `governance/transactions.rs` — Proposal and vote transactions

**Dependencies:** Only `serde`, `blake3`, `ed25519-dalek`. No networking, no I/O.

**Testing:** Extensive unit and integration tests. Can test consensus logic without running a network.

### `ultradag-network/` (P2P Layer)

**Purpose:** TCP networking, message protocol, peer management.

**Key files:**
- `protocol/message.rs` — Message encoding/decoding
- `peer/connection.rs` — TCP connection handling
- `peer/registry.rs` — Peer registry, connection tracking
- `server.rs` — Main network event loop

**Dependencies:** `tokio` for async I/O, `bincode` for serialization.

**Testing:** Network tests use in-memory channels to simulate TCP connections.

### `ultradag-node/` (Binary)

**Purpose:** CLI, RPC server, wiring everything together.

**Key files:**
- `main.rs` — Argument parsing, initialization
- `rpc.rs` — HTTP RPC server
- `rate_limit.rs` — Rate limiting

**Dependencies:** `hyper` for HTTP, `clap` for CLI.

**Testing:** RPC endpoint tests, rate limiting tests.

---

## Data Flow

### Transaction Submission

```
User → RPC POST /tx
  ↓
RPC handler validates signature, nonce, balance
  ↓
Insert into Mempool
  ↓
Broadcast to peers (NewTx message)
  ↓
Block producer includes in next vertex
  ↓
Vertex finalized by consensus
  ↓
StateEngine applies transaction
  ↓
User queries /balance to see result
```

### Block Production

```
Validator's turn (every round):
  ↓
Collect 2f+1 parent hashes from previous round
  ↓
Select transactions from mempool (fee-sorted)
  ↓
Create DagVertex { round, author, parents, txs, timestamp }
  ↓
Sign vertex
  ↓
Broadcast to peers (NewBlock message)
  ↓
Store in local DAG
```

### Finalization

```
Receive vertex from peer
  ↓
Validate: signature, round, parents exist
  ↓
Store in DAG
  ↓
Check finality: does this vertex finalize any old vertices?
  ↓
If yes:
  ↓
  Apply finalized vertices to StateEngine (in round order)
  ↓
  Remove finalized vertices from memory (pruning)
  ↓
  Update last_finalized_round
```

### Checkpoint Sync

```
New node starts
  ↓
Request latest checkpoint from peers
  ↓
Verify 2f+1 signatures
  ↓
Download StateSnapshot
  ↓
Verify hash(snapshot) == checkpoint.state_root
  ↓
Load snapshot into StateEngine
  ↓
Download vertices since checkpoint
  ↓
Apply vertices to catch up
  ↓
Node is synced
```

---

## Performance Characteristics

### Throughput

**Theoretical maximum:**
- 4 validators × 1,000 tx/block × 0.4 blocks/sec = 1,600 tx/sec
- 21 validators × 1,000 tx/block × 0.4 blocks/sec = 8,400 tx/sec

**Actual (observed on testnet):**
- ~500-800 tx/sec sustained
- Bottleneck: signature verification (CPU-bound)

**Comparison:**
- Bitcoin: ~7 tx/sec
- Ethereum: ~15 tx/sec
- Solana: ~2,000 tx/sec (claimed), ~400 tx/sec (observed)

### Latency

**Transaction finality:**
- Best case: 2.5 seconds (1 round)
- Typical: 5 seconds (2 rounds)
- Worst case: 10 seconds (4 rounds, degraded network)

**Comparison:**
- Bitcoin: 60 minutes (6 confirmations)
- Ethereum: 15 minutes (post-merge)
- Solana: 13 seconds (optimistic), 30 seconds (confirmed)

### Storage

**DAG pruning:**
- Keep last 1,000 finalized rounds in memory
- Older rounds pruned automatically
- Checkpoints every 1,000 rounds

**Disk usage:**
- ~100 MB per 100,000 rounds (state snapshots)
- ~1 GB per 1,000,000 rounds
- Pruning keeps memory usage constant

### Network Bandwidth

**Per validator:**
- Outbound: ~50 KB/sec (broadcasting vertices)
- Inbound: ~200 KB/sec (receiving from 4-20 peers)
- Total: ~250 KB/sec = 2 Mbps

**Comparison:**
- Bitcoin: ~1 Mbps
- Ethereum: ~5 Mbps
- Solana: ~100 Mbps (requires dedicated hardware)

---

## Future Improvements

### Short-term (Next 6 Months)

- **Parallel signature verification** — Use Rayon to verify signatures in parallel
- **Mempool sharding** — Partition mempool by sender address for better concurrency
- **RPC batching** — Allow submitting multiple transactions in one request

### Medium-term (6-12 Months)

- **Smart contracts** — WASM-based VM for programmable transactions
- **Light clients** — SPV-style proofs for mobile wallets
- **Cross-shard transactions** — Partition state across multiple shards

### Long-term (12+ Months)

- **Zero-knowledge proofs** — Privacy-preserving transactions
- **Interoperability** — Bridge to Ethereum, Bitcoin
- **Formal verification** — Machine-checked proof of consensus safety

---

## References

**DAG-BFT:**
- Narwhal and Tusk: https://arxiv.org/abs/2105.11827
- Bullshark: https://arxiv.org/abs/2201.05677
- Aleph BFT: https://arxiv.org/abs/1908.05156

**Consensus:**
- PBFT: http://pmg.csail.mit.edu/papers/osdi99.pdf
- HotStuff: https://arxiv.org/abs/1803.05069

**Cryptography:**
- Ed25519: https://ed25519.cr.yp.to/
- BLAKE3: https://github.com/BLAKE3-team/BLAKE3-specs

---

## Questions?

If this document doesn't answer your question, please:
1. Open a GitHub issue with the "documentation" label
2. Ask in Discord #development
3. Submit a PR improving this document

UltraDAG's architecture is designed to be understandable. If it's not, that's a bug.
