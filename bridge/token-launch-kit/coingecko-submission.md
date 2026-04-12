# CoinGecko Submission

**Submit here:** https://www.coingecko.com/en/coins/new

Paste each field from the sections below.

---

## Basic Information

| Field | Value |
|---|---|
| Coin Name | `UltraDAG` |
| Coin Symbol | `UDAG` |
| Coin Type | `ERC20 Token` (CoinGecko will ask chain — select **Arbitrum One**) |
| Contract Address | `0x9cFD2011DF13d9E394B5Bb59f0f7e7A5C512155b` |
| Contract Decimals | `8` |
| Max Supply | `21000000` |
| Circulating Supply | `2520000` |
| Total Supply | `2520000` (will increase via cross-chain bridge mints) |

---

## Project Info

**Project description (paste the whole thing):**

```
UltraDAG (UDAG) is a DAG-BFT consensus protocol built for IoT, sensor networks, and machine-to-machine micropayments. Instead of a linear blockchain, UltraDAG uses a directed acyclic graph where multiple validators produce vertices in parallel every round, achieving deterministic finality in 2-3 rounds (~10 seconds) without proof-of-work.

Key properties:
— 21,000,000 UDAG max supply (8 decimals)
— Sub-4 MB stripped full-node binary — runs on a $15 Raspberry Pi Zero 2 W
— Passkey-native WebAuthn smart accounts (FIDO2, hardware-backed)
— 7-bucket emission: 44% validators / 10% council / 16% treasury / 5% founder / 8% ecosystem / 5% reserve / 12% IDO genesis pre-mine
— Council of 21 governance (1-vote-per-seat, adjustable via on-chain proposals)
— Validator federation bridge to Arbitrum (ERC-20 backed 1:1 with native UDAG)
— BLAKE3 hashing, Ed25519 signatures, Noise XX encrypted P2P, redb ACID persistence

The ERC-20 on Arbitrum represents UDAG that was bridged from the native chain. The bridge is disabled at launch (zero validators registered), with the initial 2.52M UDAG pre-minted in the constructor as a one-shot IDO allocation for Uniswap v3 liquidity. Native emission is capped at 18.48M UDAG distributed over years via per-round block rewards to validators, council members, the DAO treasury, and ecosystem buckets.

UltraDAG is built and maintained by a solo developer with a 500,000 UDAG active bug bounty program (reports filed via GitHub Security Advisories). The full consensus engine and cryptographic primitives are open source under BUSL-1.1.
```

---

## URLs

| Field | Value |
|---|---|
| Website | `https://ultradag.com` |
| Whitepaper URL | `https://ultradag.com/whitepaper` |
| Source Code | `https://github.com/UltraDAGcom/core` |
| Explorer | `https://arbiscan.io/token/0x9cFD2011DF13d9E394B5Bb59f0f7e7A5C512155b` |
| Twitter | `https://twitter.com/ultradagcom` |
| Telegram | `https://t.me/ultradagcom` |
| Bug Bounty | `https://github.com/UltraDAGcom/core/blob/main/docs/security/bug-bounty/PROGRAM.md` |

---

## Market Data

| Field | Value |
|---|---|
| First DEX Pool | Uniswap v4 on Arbitrum One |
| Pool URL | `https://app.uniswap.org/explore/tokens/arbitrum/0x9cfd2011df13d9e394b5bb59f0f7e7a5c512155b` |

---

## Logo

Upload: `bridge/token-launch-kit/trustwallet/logo.png` (256x256 PNG)

---

## Tips for faster approval

- **Active pool volume helps.** CoinGecko auto-reviews pools with 7-day volume above some threshold. Organic trades or a few $100-500 self-trades over a week will qualify.
- **Verify the contract on Arbiscan before submitting.** They check for it.
- **Real-looking socials.** Post something on Twitter before submitting — they peek at the account.
- **Don't spam.** One submission per token. If rejected, fix the issue noted and resubmit once.

Typical timeline: **7-14 days** for automatic approval. Faster if you have active volume and complete metadata.
