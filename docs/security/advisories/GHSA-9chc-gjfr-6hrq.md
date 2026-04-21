# GHSA-9chc-gjfr-6hrq — Spending-policy bypass via pockets

**Severity:** Critical
**Component:** `ultradag-coin` — `StateEngine` SmartAccount policy enforcement
**Reporter:** Sumitshah00
**Status:** Fixed

## Summary

Pockets (derived sub-addresses under a SmartAccount) inherit the parent's
authorization keys but did not inherit the parent's spending policy. A
transfer originating from a pocket was authorized against the parent's keys
but policy-checked against the pocket address — which has no
`SmartAccountConfig` of its own, so `check_spending_policy` fell through to
`Ok(None)` and bypassed every constraint on the account.

## Impact

Any holder of an authorized key — including a low-security, daily-limited key
whose entire purpose is to cap blast radius — could drain every pocket on the
account by routing the transfer through the pocket surface. The following
were all bypassed for pocket-held funds:

- Account-level daily spending limit
- Vault threshold + time-locked withdrawal delay
- Whitelist-only restrictions
- Per-authorized-key daily limit

The parent-account surface was unaffected; only transfers whose `tx.from` was
a pocket escaped enforcement.

## Root cause

Authorization in `verify_smart_transfer` already resolved
`pocket_to_parent` → parent, but enforcement in `check_spending_policy` and in
`apply_smart_transfer_tx` keyed directly on `tx.from`. With no config at the
pocket address, enforcement silently no-op'd.

## Fix

- `check_spending_policy` now resolves `pocket_to_parent` → parent before
  loading the policy config.
- `apply_smart_transfer_tx` threads the resolved `policy_owner` through the
  per-key daily-limit check and pushes any pending vault transfer onto the
  parent's config.
- `PendingVaultTransfer` gained a `from` field so `apply_cancel_vault_tx`
  refunds to the origin surface. Cancelling a pocket-originated vault
  otherwise silently rehomed the balance onto the parent account.

Five regression tests in `crates/ultradag-coin/tests/smart_account_policy_pocket.rs`
cover the daily-limit, vault-routing, cancel-refund destination, per-key
limit, and whitelist-inheritance paths.

## Credits

Reported by Sumitshah00 via GitHub Security Advisory with a standalone PoC
reproducing the bypass against a 1 UDAG daily-limit policy.
