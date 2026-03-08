# Security Policy

## Supported Versions

| Version | Status | Support |
|---------|--------|---------|
| Testnet | Active | ✅ Bug bounty active |
| Mainnet | Not launched | N/A |

## Reporting a Vulnerability

**We take security seriously.** If you discover a security vulnerability in UltraDAG, please report it responsibly.

### 🔒 Private Disclosure (Preferred)

**Use GitHub Security Advisories:**
1. Go to https://github.com/[your-org]/ultradag/security/advisories
2. Click "Report a vulnerability"
3. Fill out the form with detailed information
4. Submit privately

**What to include:**
- Clear description of the vulnerability
- Step-by-step reproduction instructions
- Proof-of-concept code or commands
- Your assessment of severity and impact
- Suggested fix (optional but appreciated)
- Your testnet address for bug bounty reward

### 💰 Bug Bounty Program

**Active rewards for testnet vulnerabilities!**

See [BUG_BOUNTY.md](./BUG_BOUNTY.md) for full details.

**Quick summary:**
- 🔴 Critical: 10,000 - 50,000 UDAG
- 🟠 High: 5,000 - 10,000 UDAG
- 🟡 Medium: 1,000 - 5,000 UDAG
- 🟢 Low: 100 - 1,000 UDAG

All rewards are tracked and will be honored with mainnet UDAG at launch.

### ⏱️ Response Timeline

- **Acknowledgment:** Within 24 hours
- **Initial assessment:** Within 7 days
- **Status updates:** Every 14 days until resolved
- **Fix deployment:** Varies by severity
  - Critical: 1-7 days
  - High: 7-30 days
  - Medium: 30-60 days
  - Low: 60-90 days

### 🚫 What NOT to Do

- ❌ Do not publicly disclose the vulnerability before we've had time to fix it
- ❌ Do not exploit the vulnerability for personal gain
- ❌ Do not attack the testnet maliciously or cause harm
- ❌ Do not test on mainnet (when launched) - that's illegal
- ❌ Do not share the vulnerability with others before disclosure

### ✅ What We Promise

- ✅ Acknowledge your report within 24 hours
- ✅ Keep you updated on our progress
- ✅ Credit you in release notes (if you want)
- ✅ Pay bug bounty rewards promptly
- ✅ Work with you on coordinated disclosure timing
- ✅ Not take legal action against good-faith researchers

## Disclosure Policy

### Our Commitment

We follow **coordinated disclosure**:
- 90-day embargo period for fixes
- We'll work with you on disclosure timing
- Public disclosure after fix is deployed
- CVE assignment for critical/high severity issues

### Your Rights

- Request anonymity (we'll honor it)
- Publish your findings after fix is deployed
- Receive credit in security advisories
- Appeal severity assessments

## Security Best Practices

### For Users

**Testnet:**
- Never use real private keys from other chains
- Don't store significant value (it's testnet!)
- Report suspicious activity

**Mainnet (future):**
- Use hardware wallets for large amounts
- Verify all transaction details before signing
- Keep software updated
- Use official releases only

### For Developers

**Contributing code:**
- Run `cargo test` before submitting PRs
- Add tests for security-critical code
- Follow Rust safety guidelines
- Review the codebase security model

**Running nodes:**
- Keep nodes updated
- Monitor resource usage
- Use firewalls and rate limiting
- Backup validator keys securely

## Known Security Considerations

### Current Testnet Limitations

**Not production-ready:**
- Testnet can be reset at any time
- No value should be stored
- Experimental features may be unstable
- Rate limiting is active but being tested

**Addressed vulnerabilities:**
- ✅ Quorum threshold overflow (fixed March 2026)
- ✅ Stall recovery oscillation (fixed March 2026)
- ✅ Validator round synchronization (fixed March 2026)
- ✅ Staking propagation (fixed March 2026)
- ✅ Rate limiting bypass (fixed March 2026)

See `CLAUDE.md` for detailed bug fix history.

## Security Architecture

### Threat Model

**What we protect against:**
- Double-spend attacks
- Finality violations
- Network partition attacks
- DoS and resource exhaustion
- State corruption
- Cryptographic attacks

**What we don't protect against (yet):**
- Quantum computing attacks (future work)
- Social engineering
- Physical attacks on infrastructure
- Compromised dependencies (trust chain)

### Security Features

**Consensus:**
- Byzantine fault tolerant DAG
- Fast finality (1-2 rounds)
- Validator quorum requirements
- Signature verification on all vertices

**Network:**
- P2P encryption (planned)
- Rate limiting per IP
- Connection limits
- Request size limits

**State:**
- Balance validation
- Nonce enforcement
- Fee requirements
- Staking constraints

**RPC:**
- Input validation
- Rate limiting
- Request throttling
- Error message sanitization

## Cryptography

**Algorithms used:**
- **Signatures:** Ed25519 (via `ed25519-dalek`)
- **Hashing:** SHA-256, BLAKE3
- **Address derivation:** RIPEMD-160 + Base58Check

**Key management:**
- Private keys never leave local storage
- Validator keys stored in `validator.key`
- No key escrow or recovery (your keys, your responsibility)

## Audit Status

**Current status:** Pre-audit (testnet phase)

**Planned audits:**
- Internal security review: Q2 2026
- External audit firm: Before mainnet launch
- Ongoing bug bounty: Active now

**Audit scope:**
- Consensus mechanism
- Cryptographic implementations
- State engine
- Network layer
- Economic model

## Contact

**Security Team:** security@ultradag.io (if available)  
**Bug Bounty:** Use GitHub Security Advisories  
**General Questions:** GitHub Discussions  

**PGP Key:** (Add your PGP public key here for encrypted communications)

---

**Last Updated:** March 8, 2026  
**Next Review:** April 8, 2026  
**Version:** 1.0
