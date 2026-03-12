#!/bin/bash

# Generate 50 test transactions to visualize on the 3D globe
# This will create activity that shows up as animated particles

NODE_URL="https://ultradag-node-1.fly.dev"
NUM_TRANSACTIONS=50

echo "🚀 Generating $NUM_TRANSACTIONS test transactions..."
echo "📡 Target: $NODE_URL"
echo ""

# Generate a keypair for sending transactions
echo "🔑 Generating keypair..."
KEYGEN_RESPONSE=$(curl -s "$NODE_URL/keygen")
SECRET_KEY=$(echo "$KEYGEN_RESPONSE" | jq -r '.secret_key')
FROM_ADDRESS=$(echo "$KEYGEN_RESPONSE" | jq -r '.address')

echo "Address: $FROM_ADDRESS"
echo "Secret Key: ${SECRET_KEY:0:16}..."
echo ""

# Generate a recipient address
RECIPIENT_RESPONSE=$(curl -s "$NODE_URL/keygen")
TO_ADDRESS=$(echo "$RECIPIENT_RESPONSE" | jq -r '.address')

echo "Recipient: $TO_ADDRESS"
echo ""
echo "📤 Sending transactions..."

SUCCESS_COUNT=0
FAIL_COUNT=0

for i in $(seq 1 $NUM_TRANSACTIONS); do
  # Send a transaction using the /tx endpoint
  RESPONSE=$(curl -s -X POST "$NODE_URL/tx" \
    -H "Content-Type: application/json" \
    -d "{
      \"secret_key\": \"$SECRET_KEY\",
      \"to\": \"$TO_ADDRESS\",
      \"amount\": 100,
      \"fee\": 10000,
      \"memo\": \"Globe test $i\"
    }")
  
  # Check if transaction was successful
  if echo "$RESPONSE" | jq -e '.hash' > /dev/null 2>&1; then
    TX_HASH=$(echo "$RESPONSE" | jq -r '.hash')
    echo "✅ Transaction $i/$NUM_TRANSACTIONS sent - Hash: ${TX_HASH:0:16}..."
    ((SUCCESS_COUNT++))
  else
    ERROR=$(echo "$RESPONSE" | jq -r '.error // "unknown error"')
    echo "❌ Transaction $i/$NUM_TRANSACTIONS failed - Error: $ERROR"
    ((FAIL_COUNT++))
    
    # If we get insufficient balance, stop
    if echo "$ERROR" | grep -q "insufficient balance"; then
      echo ""
      echo "⚠️  Insufficient balance - stopping after $i transactions"
      break
    fi
  fi
  
  # Small delay to avoid rate limiting
  sleep 0.3
done

echo ""
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "✨ Done!"
echo "📊 Success: $SUCCESS_COUNT | Failed: $FAIL_COUNT"
echo ""
echo "🌍 Open your website and watch the 3D globe!"
echo "   The green particles should be flowing between nodes."
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
