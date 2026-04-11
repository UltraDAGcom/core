# UltraDAG Bridge Deployment Guide

Complete walkthrough for deploying the UltraDAG validator-federation bridge to
Arbitrum (Sepolia and mainnet), with notes on the presale-first deployment
variant.

> **Architecture note**: this bridge is secured by the UltraDAG DAG validator
> set itself. There is **no external relayer network**. Every withdrawal claim
> on Arbitrum is verified against the BFT signature threshold of the registered
> validator set. See [`VALIDATOR_FEDERATION_BRIDGE.md`](./VALIDATOR_FEDERATION_BRIDGE.md)
> for the architectural details.

## Prerequisites

- [Foundry](https://book.getfoundry.sh/getting-started/installation) (`forge` 1.5+)
- An Arbitrum RPC endpoint (Infura, Alchemy, or a self-hosted node)
- Funded deployer address (gas on the target network)
- Governor address (will own the 1-day `TimelockController` that administers
  the token and the bridge) — **recommend a Safe multisig**, not an EOA
- For the full bi-directional bridge: at least 3 registered UltraDAG validators
  with published secp256k1 bridge addresses

## Phase 0: Install Dependencies

```bash
cd bridge
forge install openzeppelin/openzeppelin-contracts
forge install foundry-rs/forge-std
forge build
forge test
```

All 42 tests must pass before going further.

## Phase 1: Testnet (Arbitrum Sepolia)

### 1.1 Environment Variables

Create `.env`:

```bash
# ── Required ─────────────────────────────────────────────────────────
RPC_URL=https://sepolia-rollup.arbitrum.io/rpc
DEPLOYER_KEY=0x<deployer_private_key>
GOVERNOR_ADDRESS=0x<safe_or_timelock_owner>

# ── Optional: IDO / Presale Pre-mine ─────────────────────────────────
# If set, the token constructor mints the IDO allocation in one shot to
# GENESIS_RECIPIENT. Capped at 2,520,000 UDAG (12% of MAX_SUPPLY).
# Amount is in 8-decimal sats: 1 UDAG = 100_000_000 sats.
#
# Example: mint the full 2.52M UDAG IDO bucket to the liquidity multisig
# GENESIS_RECIPIENT=0x<liquidity_multisig_address>
# GENESIS_ALLOCATION=252000000000000

# ── Filled after deployment ──────────────────────────────────────────
BRIDGE_ADDRESS=
TOKEN_ADDRESS=
TIMELOCK_ADDRESS=
```

Load it: `source .env` (or use `forge script`'s automatic `.env` loading).

### 1.2 Deploy

```bash
forge script script/Deploy.s.sol:DeployScript \
  --rpc-url "$RPC_URL" \
  --private-key "$DEPLOYER_KEY" \
  --broadcast \
  --verify \
  -vvvv
```

The script deploys, in one transaction batch:

1. `TimelockController` with a 1-day delay, `GOVERNOR_ADDRESS` as the sole
   proposer and executor.
2. `UDAGToken`, with:
   - `admin = TimelockController`
   - `bridge = <CREATE-predicted bridge address>`
   - `genesisRecipient = $GENESIS_RECIPIENT` (or `0x0` to skip)
   - `genesisAllocation = $GENESIS_ALLOCATION` (or `0` to skip)
3. `UDAGBridgeValidator(token, timelock)`.

Deployment artifacts are written to `deployment-output.json`:

```json
{
  "network": 421614,
  "token": "0x...",
  "bridge": "0x...",
  "timelock": "0x...",
  "governor": "0x...",
  "timelockDelay": 86400,
  "genesisRecipient": "0x...",
  "genesisAllocation": 252000000000000
}
```

### 1.3 Verify Deployment

```bash
# Token basics
cast call "$TOKEN_ADDRESS" "name()(string)" --rpc-url "$RPC_URL"
cast call "$TOKEN_ADDRESS" "symbol()(string)" --rpc-url "$RPC_URL"
cast call "$TOKEN_ADDRESS" "decimals()(uint8)" --rpc-url "$RPC_URL"
cast call "$TOKEN_ADDRESS" "MAX_SUPPLY()(uint256)" --rpc-url "$RPC_URL"

# Genesis allocation (should equal GENESIS_ALLOCATION if set)
cast call "$TOKEN_ADDRESS" "genesisAllocation()(uint256)" --rpc-url "$RPC_URL"
cast call "$TOKEN_ADDRESS" "genesisRecipient()(address)" --rpc-url "$RPC_URL"
cast call "$TOKEN_ADDRESS" "totalSupply()(uint256)" --rpc-url "$RPC_URL"

# Bridge is sole minter
cast call "$TOKEN_ADDRESS" "isMinter(address)(bool)" "$BRIDGE_ADDRESS" --rpc-url "$RPC_URL"

# Bridge starts disabled (no validators yet)
cast call "$BRIDGE_ADDRESS" "bridgeEnabled()(bool)" --rpc-url "$RPC_URL"
cast call "$BRIDGE_ADDRESS" "getValidatorCount()(uint256)" --rpc-url "$RPC_URL"
```

### 1.4 Register Initial Validators

The bridge starts disabled. It auto-enables the moment the third validator is
registered. Each validator's on-chain address is the Ethereum address derived
from their Ed25519-derived secp256k1 bridge key (see
`crates/ultradag-coin/src/bridge/mod.rs::eth_address_from_secp_key`).

```bash
export BRIDGE_ADDRESS="0x..."
export GOVERNOR_KEY="0x..."                         # proposer on the TimelockController
export VALIDATOR_ADDRESSES="0xv1,0xv2,0xv3"         # ≥ 3 comma-separated addresses

forge script script/ConfigureBridge.s.sol:ConfigureBridgeScript \
  --rpc-url "$RPC_URL" \
  --private-key "$GOVERNOR_KEY" \
  --broadcast \
  -vvvv
```

> **NB**: `ConfigureBridge.s.sol` calls `addValidator()` directly as the
> bridge's governor. In a production deployment where the governor is the
> TimelockController, each `addValidator` call must instead be scheduled
> through the timelock (1-day delay). Either batch them via a single
> `schedule` / `execute` pair, or use a Safe + Zodiac timelock module.

### 1.5 Smoke Tests

After validators are registered and `bridgeEnabled() == true`:

```bash
# Native → Arbitrum (claimWithdrawal) — requires a real BridgeProof from the
# UltraDAG node. The raw workflow is in VALIDATOR_FEDERATION_BRIDGE.md.

# Arbitrum → Native (deposit)
cast send "$TOKEN_ADDRESS" "approve(address,uint256)" \
  "$BRIDGE_ADDRESS" 100000000 \
  --rpc-url "$RPC_URL" --private-key "$USER_KEY"

cast send "$BRIDGE_ADDRESS" \
  "deposit(bytes20,uint256)" \
  0x<20_byte_native_address> 100000000 \
  --rpc-url "$RPC_URL" --private-key "$USER_KEY"
```

Tail the event stream:

```bash
cast logs --address "$BRIDGE_ADDRESS" --from-block latest --rpc-url "$RPC_URL"
```

You should see `DepositMade`. UltraDAG validators observing the Arbitrum
event will each submit a `BridgeReleaseTx` on the native chain; once
`⌈2n/3⌉` agreeing votes are in, the recipient on the native side is credited.

### 1.6 Run on Testnet for 2–4 Weeks

- Monitor bridge uptime, latency, and volume.
- Exercise every rate-limit edge case (per-tx cap, daily cap rollover).
- Test `pause()` / `unpause()`, governor transfer, validator add/remove.
- Dry-run the `proposeBridgeMigration` / `executeBridgeMigration` recovery path.
- Exercise the off-chain relayer daemon (see the **Off-chain integration**
  section below; this is the part of the system that does not yet ship in
  this repo).

## Phase 2: Mainnet (Arbitrum One)

### 2.1 Pre-Deployment Checklist

- [ ] Testnet ran cleanly for ≥ 2 weeks with representative traffic.
- [ ] External audit complete. Fixes merged and re-audited as needed.
- [ ] Bug bounty program launched.
- [ ] Monitoring and alerting live.
- [ ] Emergency runbook documented (pause, migrate, refund).
- [ ] Governor is a Safe multisig, not an EOA.
- [ ] Deployer address is gas-funded on Arbitrum One.

### 2.2 Deploy

```bash
export RPC_URL="https://arb1.arbitrum.io/rpc"
# ... rest of .env identical except mainnet values ...

forge script script/Deploy.s.sol:DeployScript \
  --rpc-url "$RPC_URL" \
  --private-key "$DEPLOYER_KEY" \
  --broadcast \
  --verify \
  -vvvv
```

Save the `deployment-output.json`.

### 2.3 Post-Deployment Verification

- All verification commands from §1.3, against mainnet.
- Contracts verified on Arbiscan.
- If a genesis allocation was configured, confirm on-chain:
  - `totalSupply()` equals `GENESIS_ALLOCATION`.
  - `balanceOf(GENESIS_RECIPIENT)` equals `GENESIS_ALLOCATION`.
  - `isMinter(admin) == false` (admin cannot mint, only the bridge can).
- Transfer `DEFAULT_ADMIN_ROLE` to the production Safe / timelock if it is
  not already there.

### 2.4 Register Mainnet Validators

Same command as §1.4, now via the production timelock / Safe multisig and
using the **production** validator secp256k1 addresses.

### 2.5 Monitor Go-Live

- First `DepositMade` event → confirm the native chain credits the recipient
  within the expected time budget.
- First `WithdrawalClaimed` event → confirm the submitted `BridgeProof` hash
  matches the Rust `solidity_message_hash()` exactly (one mismatched byte =
  dead bridge).

## Presale-First Deployment (Bridge Unregistered)

If the goal is to get UDAG tradable on a DEX *before* the native DAG chain is
publicly reachable:

1. Deploy with `GENESIS_RECIPIENT` = liquidity distributor multisig and
   `GENESIS_ALLOCATION` = the desired pre-mine (up to 2,520,000 UDAG).
2. **Do not register any validators yet.** The bridge remains disabled; its
   `deposit()` and `claimWithdrawal()` functions revert with `BridgeNotEnabled`.
3. The ERC-20 is still fully transferable — `bridgeEnabled` only gates
   bridge-specific methods, not standard ERC-20 transfers.
4. The distributor seeds a Uniswap v3 (or v4) pool from the genesis allocation.
5. When you are ready to bring the bridge online, register the validator set
   via `ConfigureBridge.s.sol`. Once `≥ 3` validators are registered, the
   bridge auto-enables and bi-directional transfers begin.

This workflow keeps the 21M supply cap intact across both chains: the 12%
Solidity pre-mine is exactly the 12% IDO bucket from the native 7-bucket
tokenomics, and neither chain ever mints that bucket via emission.

## Off-chain Integration (Not Yet Shipped in This Repo)

The contracts and the Rust state engine both expose the primitives needed for
the bridge, but the glue is not yet committed:

| Direction            | Missing component                                                                 |
| -------------------- | --------------------------------------------------------------------------------- |
| Arbitrum → Native    | A validator-operated daemon that reads `DepositMade` events from Arbitrum,       |
|                      | constructs a signed `BridgeReleaseTx`, and submits it to the UltraDAG RPC.       |
| Native → Arbitrum    | A helper (CLI or server-side) that queries the UltraDAG RPC for a finalized      |
|                      | `BridgeProof`, encodes the threshold signatures, and submits                     |
|                      | `claimWithdrawal()` on Arbitrum. This can be end-user-initiated.                 |

Both pieces are small in terms of code but are **security-critical**: any bug
in either direction can strand or double-spend funds. They must be built and
audited before mainnet traffic flows.

## Operations

### Daily
- Monitor validator uptime and bridge signature coverage.
- Watch for rate-limit approaches (`getDailyDepositRemaining`,
  `getDailyWithdrawalRemaining`).
- Alert on any `BridgePaused` event.

### Weekly
- Reconcile on-chain `bridge_reserve` (native) against escrow balance (Arbitrum).
- Review daily volume histograms against configured limits.
- Audit the relayer daemon logs for signature failures.

### Monthly
- Rotate individual validator bridge keys if any operator requests it (add
  new, remove old, both via governor with proper timelock).
- Test the emergency pause / unpause drill.
- Review security posture against new disclosed vulnerabilities.

## Emergency Procedures

### Pause the Bridge

```bash
cast send "$BRIDGE_ADDRESS" "pause()" \
  --rpc-url "$RPC_URL" --private-key "$GOVERNOR_KEY"
```

Only the governor can pause. Once paused, `deposit()` and `claimWithdrawal()`
both revert.

### Unpause the Bridge

```bash
cast send "$BRIDGE_ADDRESS" "unpause()" \
  --rpc-url "$RPC_URL" --private-key "$GOVERNOR_KEY"
```

### Migrate to a New Bridge Contract

If the current bridge is compromised:

1. `bridge.pause()` — halt further damage.
2. Deploy a new `UDAGBridgeValidator`.
3. `bridge.migrateToNewBridge(newBridge, <escrow_balance>)` — moves the
   escrowed UDAG to the new bridge. Only callable while paused.
4. `token.proposeBridgeMigration(newBridge)` — start the 2-day timelock.
5. After 2 days: `token.executeBridgeMigration()` — swaps `MINTER_ROLE`
   from the old bridge to the new one.
6. Re-register validators on the new bridge via `ConfigureBridge.s.sol`.

## Troubleshooting

### `BridgeNotEnabled`

The bridge has fewer than 3 registered validators. Register more via
`ConfigureBridge.s.sol` until the auto-enable threshold fires.

### `TooFewSignatures(provided, required)`

The `BridgeProof` didn't contain enough validator signatures. Required count
is the current `threshold()`, i.e. `⌊2n/3⌋ + 1` for `n` validators.

### `SignerNotValidator(index, signer)`

One of the signatures in the proof recovered to an address that is not on
the registered validator list. Common causes:
- The validator was removed between signing and submission.
- The Rust-side `eth_address_from_secp_key` output does not match the address
  that was passed to `addValidator()`.
- The signer used a different secp256k1 key for signing than for registration.

### `SignersNotSorted(index, current, previous)`

Signatures must be sorted in strictly ascending order by recovered
Ethereum address. The Rust helper
`BridgeProof::encode_signatures()` handles this automatically — if you see
this error, the off-chain assembler is probably not using the helper.

### `MalleableSignature(index)`

The submitted signature's `s` value is in the upper half of the curve order.
Use `sign_for_bridge()` from the Rust module; it always produces low-s
signatures via `sign_prehash_recoverable()`.

### `AmountAboveMaximum` / `DailyLimitExceeded`

Hit the rate limits: 100,000 UDAG/tx or 500,000 UDAG per rolling 24h window.
These are intentionally tight for the initial rollout. They can be raised by
governance once the bridge has accumulated operational history, but doing so
without a pressing business reason simply widens the blast radius of any
exploit.

## Support

- Documentation: https://ultradag.com/docs/bridge
- Security contact: see [`SECURITY.md`](../SECURITY.md) at the repo root for
  the private vulnerability disclosure path.
