# Testing the 3D Network Globe

## Option 1: Use Dashboard (Easiest)

1. Open `site/dashboard.html` in your browser
2. Create a wallet or import one with funds
3. Send 50 small transactions manually or use the console:

```javascript
// Run this in browser console on dashboard page
async function sendTestTransactions(count = 50) {
  for (let i = 0; i < count; i++) {
    try {
      // Use dashboard's existing send function
      await sendTransaction({
        to: '0000000000000000000000000000000000000000000000000000000000000001',
        amount: 100,
        fee: 10000,
        memo: `Globe test ${i+1}`
      });
      console.log(`✅ Sent transaction ${i+1}/${count}`);
      await new Promise(r => setTimeout(r, 500)); // Wait 500ms between tx
    } catch (e) {
      console.error(`❌ Failed transaction ${i+1}:`, e);
      break;
    }
  }
}

// Run it
sendTestTransactions(50);
```

## Option 2: Use Existing Account

If you have a secret key for an account with funds, run:

```bash
./test-transactions.sh
```

But first edit the script and replace the keygen section with your actual secret key:

```bash
# Replace this line:
SECRET_KEY=$(echo "$KEYGEN_RESPONSE" | jq -r '.secret_key')

# With your actual key:
SECRET_KEY="your_64_char_hex_secret_key_here"
FROM_ADDRESS="your_address_here"
```

## Option 3: Watch Natural Network Activity

The globe already shows real transaction activity! Just:

1. Open `site/index.html` in your browser
2. Scroll to the globe section
3. Watch for green particles when the network processes transactions

The globe polls `/status` every 2 seconds and shows particles whenever `dag_round` increases.

## What You Should See

- 4 blue glowing nodes (NYC, London, Tokyo, Sydney)
- Green particles flowing between nodes when transactions happen
- Smooth animations as the globe rotates
- Interactive controls (drag to rotate, scroll to zoom)
