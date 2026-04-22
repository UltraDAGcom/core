# Security Advisories

This directory contains published security advisories for vulnerabilities that have been fixed in UltraDAG.

## Published Advisories

- [GHSA-9chc-gjfr-6hrq](GHSA-9chc-gjfr-6hrq.md) — Critical — Spending-policy bypass via pockets. SmartAccount policies (daily limit, vault threshold, whitelist, per-key limit) were not enforced on transfers originating from pocket sub-addresses. Reported by Sumitshah00, fixed 2026-04-21.
- [INTERNAL-2026-04-22-pocket-keyreg](INTERNAL-2026-04-22-pocket-keyreg.md) — Critical — Pocket key-injection enables pocket drain. `auto_register_ed25519_key` did not require the pubkey to derive to the target address, so any attacker could plant their key on a victim's pocket and then spend from it. Found by internal review, fixed 2026-04-22.
- [INTERNAL-2026-04-22-pocket-persist](INTERNAL-2026-04-22-pocket-persist.md) — Critical — Pocket persistence gap. `pocket_to_parent` reverse-index was never rebuilt on node restart; after any restart every pocket became unspendable and the GHSA-9chc policy-bypass silently regressed. Found by second-pass internal review, fixed 2026-04-22.

## Disclosure Timeline

We follow a **90-day coordinated disclosure** policy:

1. Vulnerability reported privately
2. Fix developed and tested (1-90 days depending on severity)
3. Fix deployed to testnet
4. Coordinated disclosure with reporter
5. Public advisory published here

## Advisory Format

Each advisory includes:
- **CVE ID** (if assigned)
- **Severity** (Critical/High/Medium/Low)
- **Affected versions**
- **Description** of the vulnerability
- **Impact** assessment
- **Mitigation** steps
- **Credits** to the reporter
- **Timeline** of discovery, fix, and disclosure

## Reporting Vulnerabilities

**Do not report vulnerabilities here publicly!**

Use GitHub Security Advisories for private reporting:
- Go to the Security tab
- Click "Report a vulnerability"
- Follow the bug bounty program guidelines

See [../POLICY.md](../POLICY.md) for full details.

---

**Last Updated:** March 8, 2026
