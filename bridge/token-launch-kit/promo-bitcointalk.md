# BitcoinTalk Announcement Post

**Section:** Altcoin Announcements (ANN) — https://bitcointalk.org/index.php?board=159.0
**Title suggestion:** `[ANN][UDAG] UltraDAG — DAG-BFT Consensus for IoT · Live on Arbitrum`

BitcoinTalk uses BBCode, not Markdown. Paste the block below directly into the forum editor.

---

```bbcode
[center]
[size=24pt][b]UltraDAG (UDAG)[/b][/size]
[i]DAG-BFT consensus for IoT and machine-to-machine micropayments[/i]

[size=14pt][b]Live on Arbitrum · 21M max supply · 10-second finality[/b][/size]
[/center]

[hr]

[size=16pt][b]What is UltraDAG?[/b][/size]

UltraDAG is a directed-acyclic-graph BFT blockchain written in Rust. Instead of a linear chain of blocks, every validator produces one vertex per round, each referencing the previous round's tips. Finality is deterministic — once a vertex has descendants from 2/3+ of validators, it is permanent. There is no proof of work.

The design target: tiny embedded devices that need to send money to each other. A full node binary is under 4 MB stripped and runs on a $15 Raspberry Pi Zero 2 W. A hardware-wallet variant runs on an ESP32.

[hr]

[size=16pt][b]Token details[/b][/size]

[list]
[li][b]Contract (Arbitrum One):[/b] [url=https://arbiscan.io/token/0x9cFD2011DF13d9E394B5Bb59f0f7e7A5C512155b]0x9cFD2011DF13d9E394B5Bb59f0f7e7A5C512155b[/url][/li]
[li][b]Symbol:[/b] UDAG[/li]
[li][b]Decimals:[/b] 8 (matches native chain's sats-per-UDAG)[/li]
[li][b]Max supply:[/b] 21,000,000 UDAG[/li]
[li][b]Circulating at launch:[/b] 2,520,000 UDAG (12% IDO pre-mine)[/li]
[li][b]Native emission:[/b] 18,480,000 UDAG over time via per-round consensus rewards[/li]
[li][b]Primary DEX:[/b] [url=https://app.uniswap.org/explore/tokens/arbitrum/0x9cfd2011df13d9e394b5bb59f0f7e7a5c512155b]Uniswap v4 (Arbitrum)[/url][/li]
[/list]

[hr]

[size=16pt][b]Tokenomics — 7-bucket emission[/b][/size]

Per-round rewards split across 7 buckets. The numbers below are the per-round percentages; over time they compound into the 18.48M emission cap.

[list]
[li][b]44%[/b] validators (proportional to stake)[/li]
[li][b]10%[/b] Council of 21 (1-vote-per-seat governance)[/li]
[li][b]16%[/b] DAO treasury (spent via governance proposals)[/li]
[li][b]5%[/b] founder allocation[/li]
[li][b]8%[/b] ecosystem fund[/li]
[li][b]5%[/b] reserve[/li]
[li][b]12%[/b] IDO genesis pre-mine (already minted — this is what's live on Uniswap)[/li]
[/list]

Validator rewards include a passive-staker share (50% of proportional reward) so holders who aren't running nodes still earn.

[hr]

[size=16pt][b]What's novel[/b][/size]

[b]1. Passkey-native wallets.[/b] Accounts are secured by your device's FIDO2 / WebAuthn credential — the same passkey you use on every modern phone and laptop. Hardware-backed signing, no seed phrase to lose, no browser extension to phish. SmartAccount transactions verify P256 signatures natively in the consensus engine.

[b)2. Sub-4 MB full-node binary.[/b] Strip + LTO + panic=abort. You can literally run an UltraDAG validator on a [url=https://www.raspberrypi.com/products/raspberry-pi-zero-2-w/]$15 Pi Zero 2 W[/url]. A separate crate targets ESP32 as a light-client hardware wallet.

[b]3. DAG-BFT with BitVec descendants.[/b] Finality checks are O(1) per vertex via a per-vertex BitVec of descendant validators — O(256x) memory reduction vs. HashSet<Address>. Enables the chain to scale to thousands of validators while staying responsive on weak hardware.

[b]4. Formal verification.[/b] Consensus invariants are specified in TLA+ and model-checked (see [url=https://github.com/UltraDAGcom/core/tree/main/formal]/formal[/url] in the repo).

[b]5. Validator-federation bridge.[/b] The Arbitrum ERC-20 is backed 1:1 by locked UDAG on the native chain. Deposits lock; withdrawals require signatures from a registered validator federation. At launch the bridge is [b]disabled[/b] (zero validators registered) — the ERC-20 is tradable but new mints require the bridge to be activated via governance.

[hr]

[size=16pt][b]Where to buy[/b][/size]

[list]
[li][b]Uniswap v4 (Arbitrum One):[/b] [url=https://app.uniswap.org/explore/tokens/arbitrum/0x9cfd2011df13d9e394b5bb59f0f7e7a5c512155b?inputCurrency=NATIVE]app.uniswap.org[/url][/li]
[/list]

No centralized exchanges listed. CoinGecko and CoinMarketCap submissions in review.

[hr]

[size=16pt][b]Bug bounty[/b][/size]

[b]500,000 UDAG active pool.[/b]

[list]
[li]Critical: 10,000 – 50,000 UDAG[/li]
[li]High: 5,000 – 10,000 UDAG[/li]
[li]Medium: 1,000 – 5,000 UDAG[/li]
[li]Low: 100 – 1,000 UDAG[/li]
[/list]

Reports filed privately via [url=https://github.com/UltraDAGcom/core/security/advisories/new]GitHub Security Advisories[/url]. First bounty already awarded (15,000 UDAG) for a fatal-halt vulnerability in the SmartOp authorization path — fix committed, regression tests added, advisory publication pending.

All commitments are tracked in an append-only [url=https://github.com/UltraDAGcom/core/blob/main/docs/security/bug-bounty/LEDGER.md]ledger[/url]. Testnet resets do not affect any reward promise.

[hr]

[size=16pt][b]Links[/b][/size]

[list]
[li][b]Website:[/b] [url=https://ultradag.com]ultradag.com[/url][/li]
[li][b]Whitepaper:[/b] [url=https://ultradag.com/whitepaper]ultradag.com/whitepaper[/url][/li]
[li][b]Documentation:[/b] [url=https://ultradag.com/docs]ultradag.com/docs[/url][/li]
[li][b]Architecture explorer (interactive):[/b] [url=https://ultradag.com/explore]ultradag.com/explore[/url][/li]
[li][b]Source code:[/b] [url=https://github.com/UltraDAGcom/core]github.com/UltraDAGcom/core[/url][/li]
[li][b]Block explorer:[/b] [url=https://ultradag.com/explorer]ultradag.com/explorer[/url][/li]
[li][b]Live network status:[/b] [url=https://ultradag.com/network]ultradag.com/network[/url][/li]
[li][b]Dashboard:[/b] [url=https://ultradag.com/dashboard]ultradag.com/dashboard[/url][/li]
[li][b]Telegram:[/b] [url=https://t.me/ultradagcom]t.me/ultradagcom[/url][/li]
[li][b]Twitter / X:[/b] [url=https://x.com/ultradagcom]x.com/ultradagcom[/url][/li]
[/list]

[hr]

[size=16pt][b]Contract addresses[/b][/size]

[table][tr][td][b]Role[/b][/td][td][b]Address[/b][/td][/tr]
[tr][td]UDAGToken (Arbitrum One)[/td][td][tt]0x9cFD2011DF13d9E394B5Bb59f0f7e7A5C512155b[/tt][/td][/tr]
[tr][td]Bridge contract[/td][td][tt]0xAb65098B184f24102F3C9306d8EB85bB60426F69[/tt][/td][/tr]
[tr][td]Timelock (24h delay)[/td][td][tt]0xa8Ff4C8e729bF86FE0bb38710A9FD4D7614e69Af[/tt][/td][/tr]
[tr][td]Governor[/td][td][tt]0xfcE87AAbAe4E304Bdfb4920a000fD1C8F1712356[/tt][/td][/tr]
[tr][td]IDO / LP holder[/td][td][tt]0x9aEcb515361af7980eaa16fE40c064f69738EbF9[/tt][/td][/tr][/table]

[hr]

[size=16pt][b]Honest disclosures[/b][/size]

[list]
[li][b]Solo developer project.[/b] Built and maintained by one person. Community contributions welcome via the GitHub repo.[/li]
[li][b]No external audit yet.[/b] The Rust consensus engine has been heavily fuzz-tested and runs 1000+ property-based tests. The Solidity contracts passed a Slither static-analysis pass, have 42 Foundry tests including invariants, and mirror a well-known OpenZeppelin AccessControl pattern. An external audit is planned but not yet scheduled.[/li]
[li][b)Thin launch liquidity.[/b] The Uniswap v4 pool seeded at launch is intentionally small. Expect volatility until volume builds.[/li]
[li][b]BUSL-1.1 license.[/b] Source is open, commercial use has restrictions for the first 2 years. See [url=https://github.com/UltraDAGcom/core/blob/main/LICENSE]LICENSE[/url].[/li]
[/list]

[hr]

[size=16pt][b]How to run a validator[/b][/size]

The full node binary auto-downloads from GitHub Releases. On any Linux machine:

[code]
docker run -d \
  -p 9333:9333 -p 10333:10333 \
  -v /var/ultradag:/data \
  -e PKEY=0xyour_validator_key \
  ghcr.io/ultradagcom/core:latest
[/code]

Full guide: [url=https://ultradag.com/docs/getting-started/validator]ultradag.com/docs/getting-started/validator[/url]

A Raspberry Pi Zero 2 W running a validator draws ~0.6 watts. At residential electricity prices (~$0.12/kWh), that's less than $0.65/year to run.

[hr]

[center][size=14pt]Feedback, critiques, and adversarial questions welcome below.[/size][/center]
```

---

## Posting tips

- **Read BitcoinTalk's ANN rules** before posting (they're strict about promo — keep the tone factual, not hype)
- **Bookmark the thread URL** and cross-post it from X once the thread is up — "also available on BitcoinTalk: [link]"
- **Monitor replies daily** for the first two weeks. Answering good-faith technical questions builds credibility
- **Don't reply to obvious FUD accounts** — it amplifies them
- **Update the OP** when milestones happen (CG listing, CMC listing, first bridge activation, etc.)
