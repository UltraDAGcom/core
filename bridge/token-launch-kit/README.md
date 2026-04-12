# UDAG Token Launch Kit

Everything you need to make the UDAG token look "launched and legit" across listing sites, wallets, and DEX aggregators.

**Token contract (Arbitrum One):** `0x9cFD2011DF13d9E394B5Bb59f0f7e7A5C512155b`

---

## Priority order (do in this sequence)

### 1. Verify source code on Arbiscan — **do this first, takes 5 minutes**

Go to: https://arbiscan.io/verifyContract?a=0x9cFD2011DF13d9E394B5Bb59f0f7e7A5C512155b

If it says "Contract Source Code Verified" already, skip ahead. Otherwise:

Fastest path is to re-run your deploy script with `ARBISCAN_API_KEY` set. Get a free API key at https://arbiscan.io/myapikey and then:

```bash
# Grab the constructor args from deployment-output.json
forge verify-contract \
  --chain arbitrum \
  --watch \
  --etherscan-api-key YOUR_ARBISCAN_KEY \
  --constructor-args $(cast abi-encode "constructor(address,address,address,uint256)" \
    0xa8Ff4C8e729bF86FE0bb38710A9FD4D7614e69Af \
    0xAb65098B184f24102F3C9306d8EB85bB60426F69 \
    0x9aEcb515361af7980eaa16fE40c064f69738EbF9 \
    252000000000000) \
  0x9cFD2011DF13d9E394B5Bb59f0f7e7A5C512155b \
  src/UDAGToken.sol:UDAGToken
```

### 2. TrustWallet assets PR — powers logos in MetaMask, Uniswap, etc.

See `trustwallet/` folder in this kit.

### 3. Uniswap token list — signals you as the canonical issuer

See `ultradag.tokenlist.json` in this kit.

### 4. CoinGecko submission — biggest visibility unlock

See `coingecko-submission.md` for the fill-in-the-blanks content.

### 5. CoinMarketCap submission

See `coinmarketcap-submission.md`.

### 6. DexScreener (free info update)

https://dexscreener.com/arbitrum/0x9cFD2011DF13d9E394B5Bb59f0f7e7A5C512155b — find the pool, click "Update info" or "Claim token".

---

## What Uniswap's "About" section actually reads

Uniswap pulls token metadata from (in order):
1. **CoinGecko API** — this is the big one. Description, image, links all come from here.
2. **Uniswap default token list**
3. **TrustWallet assets** (legacy, still works for logos)

So the **biggest single action** is getting CoinGecko to approve UDAG. Everything else is supporting.
