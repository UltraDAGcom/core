# UltraDAG Bridge Contracts

Solidity contracts for bridging UDAG between Arbitrum and the UltraDAG native
chain, secured by the **UltraDAG validator set** itself (no external relayers).

## Overview

The UltraDAG bridge enables transfer of UDAG between:

- **Arbitrum**: ERC-20 representation (8 decimals)
- **UltraDAG Native**: Native UDAG (8 decimals = 1 UDAG)

Unlike traditional multi-sig-relayer bridges, attestations are produced by the
same validators that already run DAG consensus. The Arbitrum contract knows the
current validator set and verifies a strict `⌊2n/3⌋ + 1` (BFT-safe) threshold of
ECDSA signatures on every `claimWithdrawal()` call.

## Contracts

| Contract                 | File                           | Description                                                              |
| ------------------------ | ------------------------------ | ------------------------------------------------------------------------ |
| `UDAGToken`              | `src/UDAGToken.sol`            | ERC-20, 8 decimals, 21M cap, optional one-shot IDO genesis pre-mine.      |
| `UDAGBridgeValidator`    | `src/UDAGBridgeValidator.sol`  | Validator-federation bridge with BFT threshold signatures.                |
| `TimelockController`     | (OpenZeppelin)                 | 1-day governance timelock; owns `DEFAULT_ADMIN_ROLE` on the token.        |

## Features

### `UDAGToken`

- **8 decimals** to match the native chain (1 UDAG = 100,000,000 sats).
- **21M hard cap**, enforced in every `mint()` path.
- **Bridge is the sole ongoing minter.** `MINTER_ROLE` is locked at construction
  time — its admin is set to a dead role no address can hold, so nobody can
  grant new minters via `AccessControl.grantRole`.
- **2-day timelock bridge migration.** If the bridge is ever compromised, the
  admin can propose a replacement and execute after a 2-day cooldown. A
  previously-proposed migration can be cancelled before execution.
- **One-shot IDO genesis allocation** (optional, constructor-only): mint up to
  2,520,000 UDAG (12% of the cap) to a liquidity / IDO distributor address
  during deployment. After the constructor returns there is no code path to
  mint again except `mint()`, which is MINTER_ROLE-gated to the bridge.
- **Pausable** (admin-gated unpause, PAUSER_ROLE for emergency pause).
- **Irreversible admin renounce** via `renounceAdminRole()` as the final
  decentralisation step.

### `UDAGBridgeValidator`

- **Validator federation:** bridge signers are the DAG's active validators, not
  an external relayer set. The Rust node derives a secp256k1 signing key from
  each validator's Ed25519 consensus key for Solidity compatibility.
- **BFT threshold:** `⌊2n/3⌋ + 1` signatures required, where `n` is the
  registered validator count. Threshold is recomputed on every add/remove.
- **Auto-enable** at `MIN_VALIDATORS = 3`. `MAX_VALIDATORS = 100`.
- **Strict signature verification:**
  - Every signature must recover to a registered validator — non-validators
    cause a revert, not a silent skip (prevents gap-based replay attacks).
  - Signatures must be sorted in ascending recovered-address order — no
    duplicates, no reorderings.
  - Malleable (high-s) signatures are rejected.
  - EIP-191 personal-sign prefix is applied inside the contract.
- **Internal message hash:** the contract never accepts a user-supplied hash.
  It derives its own hash via `keccak256(abi.encode(...))` including
  `address(this)` and `block.chainid` for deployment and chain separation.
- **Rate limiting:** 100,000 UDAG per transaction, 500,000 UDAG per rolling
  24-hour window, applied to both deposits and withdrawals.
- **Replay protection:** per-nonce `usedWithdrawalNonces` map; monotonic
  `depositNonceCounter`.
- **Two-step governor transfer** to prevent typo-bricked ownership.
- **Pausable** with migration escape hatch (`migrateToNewBridge` while paused).

## Bridge Flow

### Arbitrum → UltraDAG Native (deposit)

```
User (Arbitrum)                    Bridge Contract              Validators (DAG)
     │                                    │                           │
     │─ approve(bridge, amount) ────────▶ │                           │
     │─ deposit(nativeRecipient, amount)▶ │                           │
     │                                    │─ transferFrom to escrow   │
     │                                    │─ emit DepositMade(nonce)  │
     │                                    │──────────────────────────▶│ observe event
     │                                    │                           │─ submit BridgeReleaseTx
     │                                    │                           │  (one tx per validator)
     │                                    │                           │─ ⌈2n/3⌉ threshold reached
     │                                    │                           │─ credit native recipient
```

### UltraDAG Native → Arbitrum (withdrawal)

```
User (DAG)                       State Engine                  Bridge Contract
    │                                  │                              │
    │─ BridgeDepositTx ───────────────▶│                              │
    │                                  │─ debit sender                │
    │                                  │─ credit bridge_reserve       │
    │                                  │─ create BridgeAttestation    │
    │                                  │                              │
    │                          Validators each sign the attestation   │
    │                          with their derived secp256k1 key as    │
    │                          part of normal block production.       │
    │                                  │                              │
    │  ─── off-chain: gather ⌊2n/3⌋+1 signatures into a BridgeProof ─ │
    │                                                                  │
    │─ claimWithdrawal(nativeSender, recipient, amount, nonce, sigs)─▶│
    │                                                                 │─ ecrecover × N
    │                                                                 │─ check sorted, unique
    │                                                                 │─ rate-limit check
    │                                                                 │─ token.mint(recipient)
```

## Quick Start

### Install Foundry

```bash
curl -L https://foundry.paradigm.xyz | bash
foundryup
```

### Install dependencies

```bash
cd bridge
forge install openzeppelin/openzeppelin-contracts
forge install foundry-rs/forge-std
```

### Build

```bash
forge build
```

### Run tests

```bash
forge test
```

Expected: **42 tests passing** (27 in `UDAGTokenTest` + 15 in `UDAGBridgeValidatorTest`).

## Deployment

See [`DEPLOYMENT_GUIDE.md`](./DEPLOYMENT_GUIDE.md) for the full walkthrough.
Short version:

```bash
# Required environment
export RPC_URL="https://sepolia-rollup.arbitrum.io/rpc"   # or arb1.arbitrum.io for mainnet
export DEPLOYER_KEY="0x..."
export GOVERNOR_ADDRESS="0x..."                           # owns the timelock

# Optional: IDO / presale genesis pre-mine (mint 2.52M UDAG in the constructor)
export GENESIS_RECIPIENT="0x..."                          # liquidity distributor multisig
export GENESIS_ALLOCATION="252000000000000"               # 2,520,000 UDAG in 8-decimal sats

forge script script/Deploy.s.sol:DeployScript \
  --rpc-url "$RPC_URL" \
  --private-key "$DEPLOYER_KEY" \
  --broadcast \
  --verify \
  -vvvv
```

`Deploy.s.sol` deploys the `TimelockController`, the token, and the bridge in a
single transaction batch, using CREATE-nonce prediction so the token's
constructor can grant `MINTER_ROLE` to the as-yet-undeployed bridge.

After deployment, register the initial validator set with `ConfigureBridge.s.sol`:

```bash
export BRIDGE_ADDRESS="0x..."                             # from deployment-output.json
export GOVERNOR_KEY="0x..."                               # must match TimelockController's proposer
export VALIDATOR_ADDRESSES="0xaaa...,0xbbb...,0xccc..."   # ≥ 3 addresses

forge script script/ConfigureBridge.s.sol:ConfigureBridgeScript \
  --rpc-url "$RPC_URL" \
  --private-key "$GOVERNOR_KEY" \
  --broadcast \
  -vvvv
```

The bridge auto-enables as soon as the third validator is added.

## IDO / Presale Use Case

For a presale-first launch (before the native DAG chain is open to the public):

1. Deploy the contracts with `GENESIS_RECIPIENT` set to your liquidity
   distributor / IDO multisig and `GENESIS_ALLOCATION` set to the desired
   pre-mine amount (capped at 2,520,000 UDAG = 12% of supply).
2. The distributor address receives the tokens atomically during deployment.
3. Seed a Uniswap v3 pool from that address.
4. The bridge need not be "enabled" (no validators registered yet) for the
   ERC-20 itself to trade — `UDAGBridgeValidator.bridgeEnabled` only gates
   `deposit()` / `claimWithdrawal()` calls. Transfers are fully functional.
5. Once the native chain opens and the bridge's validator set is registered,
   ERC-20 holders can bridge to native at will.

This matches the 7-bucket tokenomics on the native side, where the 12% IDO
bucket is explicitly a genesis pre-mine and is **not** minted via per-round
emission. The Solidity pre-mine mirrors the native pre-mine so the 21M cap is
respected across both chains.

## Security

- External audit **required** before mainnet deployment and activation.
- Run a bug bounty in parallel with mainnet rollout.
- Monitoring and alerting on `DepositMade`, `WithdrawalClaimed`,
  `BridgePaused`, and `BridgeMigration` events.
- Emergency pause: any governor call to `pause()` halts all bridge operations.
- If the bridge contract itself is compromised, the token's
  `proposeBridgeMigration()` / `executeBridgeMigration()` path provides a
  2-day recovery window that swaps `MINTER_ROLE` to a fresh bridge.

## Contract Addresses

| Network                | Token   | Bridge  | Timelock |
| ---------------------- | ------- | ------- | -------- |
| Arbitrum Sepolia       | _TBD_   | _TBD_   | _TBD_    |
| Arbitrum One (mainnet) | _TBD_   | _TBD_   | _TBD_    |

## Layout

```
bridge/
├── src/
│   ├── UDAGToken.sol              # ERC-20 with optional IDO genesis mint
│   └── UDAGBridgeValidator.sol    # validator-federation bridge
├── script/
│   ├── Deploy.s.sol               # deploys timelock + token + bridge
│   └── ConfigureBridge.s.sol      # registers the initial validator set
├── test/
│   ├── UDAGToken.t.sol            # 27 tests
│   └── UDAGBridgeValidator.t.sol  # 15 tests
├── foundry.toml
├── DEPLOYMENT_GUIDE.md
└── VALIDATOR_FEDERATION_BRIDGE.md # architecture + Rust-side integration notes
```

## License

BUSL-1.1
