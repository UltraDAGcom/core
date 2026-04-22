# INTERNAL-2026-04-22 — Pocket key-injection enables pocket drain

**Severity:** Critical
**Component:** `ultradag-coin` — `StateEngine` key auto-registration
**Disclosure:** Internal review (not externally reported; no bounty payout)
**Status:** Fixed

## Summary

`auto_register_ed25519_key` gated key registration on
`authorized_keys.is_empty()` but did NOT require that the supplied pubkey
actually derives to the target address. For regular EOAs (`addr =
blake3(pubkey)[..20]`) this was implicitly safe because only the legitimate
keyholder can produce a pubkey that derives to the address. Pockets broke
the assumption: a pocket's address is computed from `(parent_address,
label)` and no pubkey can satisfy the derivation, so the implicit trust was
never crosschecked.

Any attacker could submit a policy, key, or recovery tx targeting a
victim's pocket with their own pubkey as the signer. The auto-registration
path would plant the attacker's key on the pocket's `SmartAccountConfig`;
the subsequent authorization check (`is_key_authorized`) would then find
the just-added key and pass. Because `verify_smart_transfer` checks the
pocket's own config before falling back to the parent, the attacker's
planted key then authorized `SmartTransferTx` from the pocket — draining
the balance in a second tx.

Sister path: `auto_register_key` (pub; used by the relay endpoint)
unconditionally pushed a key to any target. The one live caller passes a
pubkey-derived address so there was no exploitable site today, but the
function itself offered no defense-in-depth.

## Impact

Same attack class as GHSA-9chc-gjfr-6hrq but more severe — grants
arbitrary attacker-key authorization on a victim's pocket rather than
merely bypassing a limit. Every pocket on the network with any balance
was drainable by any funded address in two txs (one to plant the key,
one to spend).

## Root cause

`auto_register_ed25519_key` did not enforce the invariant that a key can
only be auto-registered on an address that cryptographically derives from
it. The guard `authorized_keys.is_empty()` only prevented overwriting an
already-owned account — not hostile first-time registration on a non-EOA
address.

## Fix

- `auto_register_ed25519_key`: early-return unless `Address::from_pubkey(pub_key)
  == *addr`. Non-EOA addresses (pockets today, any future non-derived
  address type) now refuse arbitrary key injection. The existing
  `is_empty()` gate stays in place.
- `auto_register_key` (pub): rejects pocket addresses outright with a
  ValidationError. Defense in depth — ensures the RPC relay path can't be
  weaponized if a future code path passes a pocket address by accident.

The fix restores the correct invariant: **pockets never hold their own
authorized keys**. Authorization for pocket-originated transfers flows
exclusively through `pocket_to_parent` → parent config →
`verify_smart_transfer` fallback.

Five regression tests in
`crates/ultradag-coin/tests/smart_account_pocket_key_injection.rs` cover
each attack surface (`SetPolicyTx`, `AddKeyTx`, `SetRecoveryTx`, full
drain chain, and direct `auto_register_key` rejection).

## Credits

Internal review during a 5-domain parallel audit (consensus / state /
network / economic / crypto). Found by the state-engine review which
flagged the unguarded pocket surface immediately after the
GHSA-9chc-gjfr-6hrq remediation.
