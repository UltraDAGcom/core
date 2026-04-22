# INTERNAL-2026-04-22 — Pocket persistence gap re-exposes pocket-policy bypass after restart

**Severity:** Critical
**Component:** `ultradag-coin` — StateEngine persistence (redb load path)
**Disclosure:** Internal review (second-pass audit; no bounty payout)
**Status:** Fixed

## Summary

`pocket_to_parent: HashMap<Address, Address>` is the authoritative reverse
index that policy enforcement and signature authorization depend on for
every pocket-originated transfer. It is **derived** state, maintained
incrementally on `CreatePocket` / `RemovePocket`, and deliberately *not*
persisted to redb — the authoritative data is the `pockets: Vec<String>`
list on each parent's `SmartAccountConfig`.

The `rebuild_pocket_map()` helper that reconstructs this index from those
lists was defined on `StateEngine` (line 376) but was **never called
anywhere in the codebase**. A source-level comment at the `from_parts`
constructor even stated the invariant ("Rebuilt via rebuild_pocket_map()
after loading") — the call simply wasn't wired. After any node restart,
`pocket_to_parent` was empty.

## Impact

On every node restart:

1. **Every pocket becomes unspendable.** `verify_smart_transfer` checks
   the pocket's own config first (empty) then falls back to
   `pocket_to_parent.get(&tx.from)` — which returns `None` with no map
   loaded. The signature does not verify against either surface and the
   transfer is rejected. Real user funds are locked on the live pocket
   address.

2. **The GHSA-9chc-gjfr-6hrq fix silently regresses.** Once
   `pocket_to_parent` is empty, `check_spending_policy`'s `unwrap_or(*from)`
   falls through to the pocket's own `SmartAccountConfig`, which has no
   policy. Every account-level limit (daily, vault threshold, whitelist)
   that a user had configured on their parent account stops being enforced
   for any of their pockets.

Combined with INTERNAL-2026-04-22-pocket-keyreg (the key-injection fix
landed earlier today in commit `826e2a28`), these three together form a
complete defense for pockets. This bug would have undermined the other
two on the first restart.

## Root cause

A function that is load-bearing for correctness was defined but never
called. Runtime mutations to the map stayed correct because `CreatePocket`
and `RemovePocket` update it incrementally. The gap only opened at startup
— exactly when no test suite happened to exercise a save/load round-trip
with an active pocket.

## Fix

One-line: call `engine.rebuild_pocket_map()` in
`crates/ultradag-coin/src/state/db.rs::load_from_redb` immediately after
`engine.restore_smart_accounts(smart_accounts_vec)`.

Three regression tests in
`crates/ultradag-coin/tests/pocket_persistence.rs`:

- `pocket_to_parent_map_rebuilt_on_reload` — direct assertion that the
  index is non-empty after save/load.
- `pocket_is_spendable_after_reload` — `verify_smart_transfer` succeeds
  for a pocket-originated transfer after reload.
- `parent_policy_still_enforced_on_pocket_after_reload` — the parent's
  daily_limit continues to apply to pocket-originated transfers after
  reload; a follow-up transfer past the cap is rejected.

## Credits

Found by the second-pass state-engine audit launched after the first two
pocket fixes landed (commit `fb6ef59d` for GHSA-9chc-gjfr-6hrq, commit
`826e2a28` for INTERNAL-2026-04-22-pocket-keyreg).
