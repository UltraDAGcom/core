# TrustWallet Assets Submission

This places your logo + metadata at the "truth source" used by MetaMask, Uniswap fallback, and many wallets.

## Steps

### 1. Fork the repo

Go to https://github.com/trustwallet/assets and click **Fork**.

### 2. Clone your fork

```bash
git clone https://github.com/YOUR_USERNAME/assets.git
cd assets
git checkout -b add-udag-arbitrum
```



### 3. Create the token folder

The path is exactly this (case-sensitive — matches EIP-55 checksum):

```bash
mkdir -p blockchains/arbitrum/assets/0x9cFD2011DF13d9E394B5Bb59f0f7e7A5C512155b
```

### 4. Copy the two files

```bash
# From the UltraDAG repo:
cp /path/to/ultradag/bridge/token-launch-kit/trustwallet/logo.png \
   blockchains/arbitrum/assets/0x9cFD2011DF13d9E394B5Bb59f0f7e7A5C512155b/

cp /path/to/ultradag/bridge/token-launch-kit/trustwallet/info.json \
   blockchains/arbitrum/assets/0x9cFD2011DF13d9E394B5Bb59f0f7e7A5C512155b/
```

### 5. Commit and push

```bash
git add blockchains/arbitrum/assets/0x9cFD2011DF13d9E394B5Bb59f0f7e7A5C512155b/
git commit -m "Add UDAG (Arbitrum)"
git push origin add-udag-arbitrum
```

### 6. Open PR

On GitHub, open a Pull Request against `trustwallet/assets:master`.

### 7. The bot will comment

A bot will auto-validate within ~1 minute. Common failures:
- **Logo size** — must be 256x256 (the one in the kit is 200x200; resize if needed)
- **Logo filesize** — must be under 100KB ✓
- **info.json** — must match their schema exactly ✓
- **Liquidity threshold** — they check that the token has active trading. You're live on Uniswap, so this should pass.

### 8. Merge

Human review follows bot validation. Usually merged within 3-5 days.

## Alternative: Resize logo to 256x256 first

If the bot rejects for logo size, use an image editor or:

```bash
# macOS — has sips built in
sips -z 256 256 logo.png --out logo_256.png && mv logo_256.png logo.png
```
