# Security Policy

UltraDAG runs a **paid bug bounty program** for vulnerabilities in the
consensus, state, network, and RPC layers. Responsible disclosure is rewarded
in UDAG on the schedule below.

**If you have found a security issue, do NOT open a public GitHub issue.**
Use the private disclosure channel below.

---

## 🔒 Reporting a Vulnerability

### Preferred: GitHub Private Vulnerability Reporting

Open a private security advisory at:

**<https://github.com/UltraDAGcom/core/security/advisories/new>**

Or click the green **"Report a vulnerability"** button at
<https://github.com/UltraDAGcom/core/security>.

The advisory is visible only to you and the UltraDAG maintainers. It stays
private until a fix is shipped and the embargo lifts. You can attach files,
exchange follow-up messages, and (for valid reports) you'll be credited in
the eventual public advisory unless you ask to remain anonymous.

### Backup: encrypted email (if GitHub is unavailable)

If you cannot use GitHub for any reason, reach out via an out-of-band channel
listed on <https://ultradag.com> (the team contact) and ask for the current
security PGP key. We will respond within 24 hours and move the substantive
discussion to a GitHub advisory as soon as possible.

**Do not** post exploit details on public forums, Twitter/X, Discord,
Telegram, or the UltraDAG GitHub issue tracker before a fix has shipped.

---

## ⏱ Response Times

| Severity | Acknowledgment | Validation | Fix target |
|---|---|---|---|
| 🔴 Critical | 24 hours | 1–3 days | 7 days |
| 🟠 High     | 24 hours | 3–7 days | 14 days |
| 🟡 Medium   | 48 hours | 7 days   | 30 days |
| 🟢 Low      | 72 hours | 14 days  | 60 days |

"Acknowledgment" means a human has read the report and opened an internal
tracking ticket — not that the issue has been validated or accepted.

---

## 💰 Reward Tiers

Paid in UDAG to the testnet address you provide in the advisory, and converted
to mainnet UDAG at program maturity under the terms in
[`docs/security/bug-bounty/LEDGER.md`](docs/security/bug-bounty/LEDGER.md).

| Tier | Reward | Examples |
|---|---|---|
| 🔴 **Critical** | 10,000 – 50,000 UDAG | Consensus break, double-spend, supply inflation, cryptographic break, permanent network stall |
| 🟠 **High**     | 5,000 – 10,000 UDAG  | Node crash via crafted message, DoS of a validator, staking exploit, eclipse/partition attack |
| 🟡 **Medium**   | 1,000 – 5,000 UDAG   | RPC authentication bypass, rate-limit bypass, mempool censorship, DAG pruning bug |
| 🟢 **Low**      | 100 – 1,000 UDAG     | Missing input validation, inefficient algorithm, information leak in error messages |

See [`docs/security/bug-bounty/PROGRAM.md`](docs/security/bug-bounty/PROGRAM.md)
for the full program terms, scope, eligibility, and evaluation process.
See [`docs/security/bug-bounty/GUIDE.md`](docs/security/bug-bounty/GUIDE.md)
for a hunter quick-start with attack vectors, tooling, and a reporting
template.

---

## ✅ In Scope

- **Consensus**: DAG construction, finality tracking, validator set management (`crates/ultradag-coin/src/consensus/`)
- **State engine**: transaction application, balance accounting, supply invariants, tokenomics (`crates/ultradag-coin/src/state/`)
- **Transactions**: all tx types, signature verification, SmartAccount, WebAuthn, name registry (`crates/ultradag-coin/src/tx/`)
- **P2P network**: protocol, peer management, Noise transport, message handling (`crates/ultradag-network/`)
- **RPC**: endpoint validation, rate limiting, authentication, data leakage (`crates/ultradag-node/src/rpc.rs`)
- **Cryptography**: Ed25519, P-256, blake3, checkpoint hashing, state root
- **Genesis / bootstrap**: `new_with_genesis`, `GENESIS_CHECKPOINT_HASH` verification, council bootstrap
- **Bridge**: UDAG token contract, bridge attestations, replay protection (`bridge/`)
- **SDKs** (if the bug lets a user forge a valid transaction): `sdk/rust/`, `sdk/python/`, `sdk/javascript/`, `sdk/go/`

## ❌ Out of Scope

- Third-party dependencies (please report upstream; we'll update and credit you for the discovery)
- Social engineering against the team or community
- Physical attacks on infrastructure
- Spam on public endpoints that's already rate-limited as designed
- Theoretical issues without a proof-of-concept
- Known issues already tracked in our internal audit ledger
- Anything in `_archives/`, `node_modules/`, or `target/` — those are frozen / generated

---

## 📦 Coordinated Disclosure Policy

- **Private by default.** Reports are embargoed until a fix ships and affected
  networks have upgraded.
- **90-day embargo** is our standard. We will publish the advisory (with credit)
  no later than 90 days after acknowledgment, even if a full fix isn't ready —
  at which point we'll publish a mitigation advisory instead.
- **Credit** is given to the reporter in the public advisory unless you ask to
  remain anonymous.
- **CVE assignment** for Critical and High severity findings.
- We will **not** take legal action against good-faith researchers who follow
  this policy.

---

## What happens after you submit

1. **Acknowledgment** (24h for Critical/High): a maintainer replies in the advisory thread.
2. **Validation** (1–14 days depending on severity): we reproduce the issue from your PoC. We may ask clarifying questions.
3. **Severity assignment**: we propose a tier and reward range; you can push back if you disagree.
4. **Fix development**: we branch, write tests, implement the fix, review internally.
5. **Testnet deploy + soak**: the fix runs on testnet under your test case before mainnet.
6. **Mainnet deploy**: coordinated upgrade across all mainnet nodes.
7. **Reward payout**: testnet UDAG transferred to the address you specified. Recorded in [`docs/security/bug-bounty/LEDGER.md`](docs/security/bug-bounty/LEDGER.md) with a git-tracked audit trail.
8. **Public disclosure** (after embargo): advisory is published to the GitHub Security tab, crediting you (optionally), and a fix release note lands in the main changelog.

---

## Security assumptions this project makes

Context that may inform your findings:

- **Consensus model**: DAG-BFT, 2–3 round deterministic finality, tolerates ⌊(n-1)/3⌋ Byzantine validators.
- **Max active validators**: 100 (top stakers by effective stake).
- **Supply invariant**: `liquid + staked + delegated + treasury + bridge + streamed == total_supply ≤ 21M UDAG`. Any observed violation is automatically a supply-integrity issue.
- **Minimum fee**: 10,000 sats (`MIN_FEE_SATS` in `constants.rs`). Stake / unstake / delegate / undelegate / vote / smart-op are fee-exempt.
- **Slash severity**: 50% of stake burned on equivocation, governable 10–100% via `slash_percent` param.
- **Genesis pre-mine**: 2.52M UDAG to the IDO distributor address (`udag1rvdfs928eu7trrc33wj2edwctdkt08gdkmhppx`); all other buckets start at zero.
- **Runtime genesis check**: mainnet nodes abort at startup if the computed genesis hash doesn't match the hardcoded `GENESIS_CHECKPOINT_HASH` — a stale constant or runtime override is considered a fatal misconfiguration.

---

## Not a security issue?

If you're unsure whether something is in scope, err on the side of reporting
it privately. We would rather triage a false positive than miss a real one.
If it turns out to be low-impact or out of scope we'll tell you so, and you
can still open a normal public issue if appropriate.

---

**Maintainer response commitment:** we take security reports seriously. Even
if you're the first person to find a bug in a new project with a small
footprint, we will read your report, reproduce it, pay out if valid, and
credit you. No PR, no social proof, no stake required — just a working PoC.

Thanks for helping keep UltraDAG safe.
