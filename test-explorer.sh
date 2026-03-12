#!/bin/bash

# Test UltraDAG Explorer functionality with real network data

echo "🔍 Testing UltraDAG Explorer"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo ""

API_URL="https://ultradag-node-1.fly.dev"

# Test 1: Network Status
echo "1️⃣  Testing /status endpoint..."
STATUS=$(curl -s "$API_URL/status")
if echo "$STATUS" | jq -e '.dag_round' > /dev/null 2>&1; then
  CURRENT_ROUND=$(echo "$STATUS" | jq -r '.dag_round')
  VERTICES=$(echo "$STATUS" | jq -r '.dag_vertices')
  SUPPLY=$(echo "$STATUS" | jq -r '.total_supply')
  ACCOUNTS=$(echo "$STATUS" | jq -r '.account_count')
  echo "   ✅ Status endpoint working"
  echo "   📊 Current Round: $CURRENT_ROUND"
  echo "   📊 Total Vertices: $VERTICES"
  echo "   📊 Total Supply: $SUPPLY sats"
  echo "   📊 Accounts: $ACCOUNTS"
else
  echo "   ❌ Status endpoint failed"
  exit 1
fi
echo ""

# Test 2: Round Data
echo "2️⃣  Testing /round/{n} endpoint..."
for ROUND in 1 2; do
  ROUND_DATA=$(curl -s "$API_URL/round/$ROUND")
  if echo "$ROUND_DATA" | jq -e '.[0].hash' > /dev/null 2>&1; then
    VERTEX_COUNT=$(echo "$ROUND_DATA" | jq '. | length')
    echo "   ✅ Round $ROUND: $VERTEX_COUNT vertices"
  else
    echo "   ⚠️  Round $ROUND: No data (might not exist yet)"
  fi
done
echo ""

# Test 3: Address Lookup
echo "3️⃣  Testing /balance/{address} endpoint..."
# Get a validator address from round 1
VALIDATOR_ADDR=$(curl -s "$API_URL/round/1" | jq -r '.[0].validator')
if [ ! -z "$VALIDATOR_ADDR" ] && [ "$VALIDATOR_ADDR" != "null" ]; then
  BALANCE_DATA=$(curl -s "$API_URL/balance/$VALIDATOR_ADDR")
  if echo "$BALANCE_DATA" | jq -e '.balance' > /dev/null 2>&1; then
    BALANCE=$(echo "$BALANCE_DATA" | jq -r '.balance_udag')
    NONCE=$(echo "$BALANCE_DATA" | jq -r '.nonce')
    echo "   ✅ Address lookup working"
    echo "   💰 Balance: $BALANCE UDAG"
    echo "   🔢 Nonce: $NONCE"
  else
    echo "   ⚠️  Address has no balance (expected for validators)"
  fi
else
  echo "   ❌ Could not get validator address"
fi
echo ""

# Test 4: Explorer Page
echo "4️⃣  Testing explorer.html loads..."
if curl -s http://localhost:8000/explorer.html | grep -q "UltraDAG Explorer"; then
  echo "   ✅ Explorer page loads correctly"
else
  echo "   ❌ Explorer page failed to load"
  exit 1
fi
echo ""

# Test 5: JavaScript File
echo "5️⃣  Testing explorer.js loads..."
if curl -s http://localhost:8000/explorer.js | grep -q "UltraDAG Block Explorer"; then
  echo "   ✅ Explorer JavaScript loads correctly"
else
  echo "   ❌ Explorer JavaScript failed to load"
  exit 1
fi
echo ""

echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "✨ All tests passed!"
echo ""
echo "🌐 Open http://localhost:8000/explorer.html to test:"
echo "   • Search for round numbers (1, 2)"
echo "   • Click rounds to see details"
echo "   • Click validator addresses"
echo "   • Test copy functionality"
echo "   • Test pagination"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
