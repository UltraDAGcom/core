# Token Launch Checklist

Work top to bottom. Each item expands "About" section coverage in Uniswap, wallets, and aggregators.

## Today (30 minutes total)

- [ ] **Verify on Arbiscan** — https://arbiscan.io/verifyContract?a=0x9cFD2011DF13d9E394B5Bb59f0f7e7A5C512155b
  - Get a free API key: https://arbiscan.io/myapikey
  - Run `forge verify-contract` (command in `README.md` of this kit)
  - Without this, most aggregators will ignore the token

- [ ] **DexScreener** — https://dexscreener.com/arbitrum/0x9cfd2011df13d9e394b5bb59f0f7e7a5c512155b
  - Likely auto-indexed already; click "Update Info" to add logo + socials
  - Cost: ~0.5 ETH for Enhanced Token Info (optional, but makes it look professional)

- [ ] **Tweet it** from @ultradagcom with:
  - Contract address
  - Uniswap link
  - Arbiscan link
  - Active bug bounty mention

- [ ] **Post in Telegram** with same info

## This week

- [ ] **TrustWallet assets PR** — see `trustwallet/HOW_TO_SUBMIT.md`
  - Files ready: `trustwallet/logo.png` (256x256) and `trustwallet/info.json`
  - Approval: 3-5 days
  - Enables logo in MetaMask, Uniswap fallback, 100+ wallets

- [ ] **CoinGecko submission** — see `coingecko-submission.md`
  - All field values prepared
  - Approval: 7-14 days
  - Biggest visibility unlock — powers Uniswap "About" section

- [ ] **Uniswap default token list** — optional but nice
  - File ready (schema-validated): `ultradag.tokenlist.json`
  - Host at `https://ultradag.com/tokens.json` (copy to `site/public/tokens.json` and it'll be served)
  - Submit URL to https://tokenlists.org

## This month

- [ ] **CoinMarketCap** — see `coinmarketcap-submission.md` (2-4 weeks)

- [ ] **DEXTools** — https://www.dextools.io/app/en/arbitrum/pair-explorer (optional, ~$300-500 for immediate pro listing)

- [ ] **Listing on CMC DexScan, GeckoTerminal** — mostly automatic once you're in CG/CMC

## Files in this kit

```
token-launch-kit/
├── README.md                         — overview
├── CHECKLIST.md                      — this file
├── ultradag.tokenlist.json           — Uniswap token list (validated ✓)
├── coingecko-submission.md           — CG fields to paste
├── coinmarketcap-submission.md       — CMC fields to paste
└── trustwallet/
    ├── HOW_TO_SUBMIT.md              — step-by-step PR guide
    ├── info.json                     — TrustWallet metadata (validated ✓)
    └── logo.png                      — 256x256 logo
```

## Contract addresses reference

| Role | Address |
|---|---|
| UDAG Token | `0x9cFD2011DF13d9E394B5Bb59f0f7e7A5C512155b` |
| Bridge | `0xAb65098B184f24102F3C9306d8EB85bB60426F69` |
| Timelock | `0xa8Ff4C8e729bF86FE0bb38710A9FD4D7614e69Af` |
| Governor | `0xfcE87AAbAe4E304Bdfb4920a000fD1C8F1712356` |
| IDO / LP holder | `0x9aEcb515361af7980eaa16fE40c064f69738EbF9` |
