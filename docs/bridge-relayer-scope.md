# Bridge Relayer Daemon — Scoping Doc

**Status:** Not yet implemented. Tracked post-mainnet-launch (launch was 2026-04-16).
**Blocker for:** Arbitrum ERC-20 UDAG ↔ UltraDAG mainnet native UDAG round-trips.
**Not a blocker for:** Anything currently shipped. Arbitrum ERC-20 trades on Camelot; native UDAG is earned via staking/validator emission.

## What exists today

Consensus layer is ready on both sides:

- **UltraDAG mainnet**: `BridgeDepositTx` and `BridgeReleaseTx` fully implemented in `crates/ultradag-coin/src/tx/bridge.rs`, with quorum enforcement, deposit-nonce tracking, and `MIN_BRIDGE_VALIDATORS=4` / `MIN_BRIDGE_QUORUM=3` floors (GHSA-6gwf-frh8-ppw7 fix).
- **Arbitrum**: `UDAGBridgeValidator` contract deployed; validator signature aggregation and replay protection in place.

What's missing is the off-chain daemon that:
1. Watches the Arbitrum `UDAGBridge` contract for `Deposit(from, amount, unique_id)` events.
2. Aggregates validator signatures (Ed25519 from the UltraDAG active set) over `(chain_id, deposit_nonce, recipient, amount)`.
3. Submits `BridgeReleaseTx` to UltraDAG mainnet with those signatures.
4. Reverse direction: watches UltraDAG for `BridgeBurnTx`, aggregates, submits Arbitrum-side release.

## Scope estimate

- **Language:** Rust (reuses `ultradag-coin` types + `ultradag-sdk`).
- **Size:** ~2-3k SLOC new daemon + ~500 SLOC validator-side handler mod.
- **Key design decisions (open):**
  - How do validators know to sign? Auto-sign on deposit observation, or require operator action?
  - Where does the daemon run? Founder-operated first, then permissioned per validator, then permissionless?
  - Signature aggregation: threshold BLS, Ed25519 multi-sig list, or something else?
- **Bridge-hardening pass (reporter's recommendation from GHSA-6gwf-frh8-ppw7 #2):** deposit-nonce ↔ source-chain-proof binding. Still open; should land in the same sprint.

## Why not now

1. **No user pressure.** ERC-20 UDAG on Arbitrum has its own liquidity and trades fine. Native UDAG is earned via validator emission. Nobody is currently trying to round-trip.
2. **Surface-area.** A live bridge daemon is the highest-risk component in a crypto system — history is littered with 8-figure bridge exploits. Shipping it in a rush right after the nuclear restart would be reckless.
3. **Genesis-clean.** With the 2026-04-16 hard-fork restart, the bridge deposit counter on both sides starts at 0. No legacy state to reconcile.

## Suggested next steps

1. **Week 1:** design doc — which signature scheme, who runs the daemon, how are validators compensated for signing.
2. **Week 2-3:** implement the happy-path daemon (founder-operated, single instance, manual sig collection).
3. **Week 4:** bounty-program-hardened — let Sumitshah00 and others have a crack at it on testnet before mainnet.
4. **Week 5+:** mainnet bridge enabled.

Until then, the UI at `/bridge` remains correctly labeled "Bridge contracts not yet deployed for round-trips" (check `dashboard/src/pages/BridgePage.tsx`).
