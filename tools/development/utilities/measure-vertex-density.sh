#!/bin/bash
# Measure vertex density per round to verify validator synchronization fix

NODE="https://ultradag-node-2.fly.dev"

# Get current round
CURRENT=$(curl -s "$NODE/status" | jq -r '.dag_round')
echo "Current round: $CURRENT"
echo ""
echo "Measuring vertex density in recent rounds..."
echo "Round | Validators | Vertex Hashes"
echo "------|------------|---------------"

# Check last 20 rounds
START=$((CURRENT - 20))
if [ $START -lt 1 ]; then
  START=1
fi

for round in $(seq $START $CURRENT); do
  # Get vertices for this round
  RESPONSE=$(curl -s "$NODE/round/$round" 2>/dev/null)
  
  if [ -z "$RESPONSE" ] || [ "$RESPONSE" = "null" ]; then
    COUNT=0
    HASHES=""
  else
    COUNT=$(echo "$RESPONSE" | jq 'length' 2>/dev/null || echo "0")
    HASHES=$(echo "$RESPONSE" | jq -r '.[].hash' 2>/dev/null | cut -c1-8 | tr '\n' ' ' || echo "")
  fi
  
  printf "%5d | %10d | %s\n" "$round" "$COUNT" "$HASHES"
done

echo ""
echo "Summary:"
TOTAL_ROUNDS=$((CURRENT - START + 1))
MULTI_VALIDATOR=$(for round in $(seq $START $CURRENT); do
  RESPONSE=$(curl -s "$NODE/round/$round" 2>/dev/null)
  COUNT=$(echo "$RESPONSE" | jq 'length' 2>/dev/null || echo "0")
  if [ "$COUNT" -ge 3 ]; then
    echo "1"
  fi
done | wc -l | tr -d ' ')

echo "Rounds checked: $TOTAL_ROUNDS"
echo "Rounds with 3+ validators: $MULTI_VALIDATOR"
PERCENT=$((MULTI_VALIDATOR * 100 / TOTAL_ROUNDS))
echo "Success rate: $PERCENT%"

if [ $PERCENT -ge 80 ]; then
  echo "✅ PASS: Validators are synchronized (80%+ rounds have 3+ validators)"
else
  echo "❌ FAIL: Validators are still drifting (only $PERCENT% rounds have 3+ validators)"
fi
