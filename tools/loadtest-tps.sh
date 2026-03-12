#!/usr/bin/env bash
# TPS Load Test for UltraDAG testnet
set -euo pipefail

NODE_URL="${1:-https://ultradag-node-1.fly.dev}"
NUM_TXS="${2:-200}"
CONCURRENCY="${3:-20}"

echo "=== UltraDAG TPS Load Test ==="
echo "Node: $NODE_URL"
echo "Target txs: $NUM_TXS (concurrency: $CONCURRENCY)"
echo ""

# Generate test keypairs
KEYGEN=$(curl -s "$NODE_URL/keygen")
SECRET=$(echo "$KEYGEN" | python3 -c "import sys,json; print(json.load(sys.stdin)['secret_key'])")
ADDRESS=$(echo "$KEYGEN" | python3 -c "import sys,json; print(json.load(sys.stdin)['address'])")
RECV=$(curl -s "$NODE_URL/keygen" | python3 -c "import sys,json; print(json.load(sys.stdin)['address'])")
echo "Sender: ${ADDRESS:0:16}..."
echo "Receiver: ${RECV:0:16}..."

# Fund sender
echo ""
echo "--- Funding sender (100 UDAG via faucet) ---"
curl -s -X POST "$NODE_URL/faucet" -H "Content-Type: application/json" \
  -d "{\"address\":\"$ADDRESS\",\"amount\":10000000000}" > /dev/null
echo "Waiting 15s for finalization..."
sleep 15

BALANCE=$(curl -s "$NODE_URL/balance/$ADDRESS" | python3 -c "import sys,json; print(json.load(sys.stdin)['balance'])")
echo "Balance: $(python3 -c "print(f'{$BALANCE/100000000:.2f}')") UDAG"

if [ "$BALANCE" -lt 100000 ] 2>/dev/null; then
  echo "ERROR: Insufficient balance"
  exit 1
fi

# Pre-test state
PRE=$(curl -s "$NODE_URL/status")
PRE_FIN=$(echo "$PRE" | python3 -c "import sys,json; print(json.load(sys.stdin)['last_finalized_round'])")
echo ""
echo "--- Pre-test: finalized round = $PRE_FIN ---"

# Create temp dir for results
TMPDIR=$(mktemp -d)
trap "rm -rf $TMPDIR" EXIT

# Function to send one tx
send_tx() {
  local idx=$1
  local code
  code=$(curl -s -o "$TMPDIR/$idx.body" -w "%{http_code}" --max-time 10 \
    -X POST "$NODE_URL/tx" \
    -H "Content-Type: application/json" \
    -d "{\"from_secret\":\"$SECRET\",\"to\":\"$RECV\",\"amount\":1000,\"fee\":10000}")
  echo "$code" > "$TMPDIR/$idx.code"
}
export -f send_tx
export NODE_URL SECRET RECV TMPDIR

echo ""
echo "--- Sending $NUM_TXS transactions (concurrency=$CONCURRENCY) ---"
START_TS=$(python3 -c "import time; print(time.time())")

# Launch in batches
SENT=0
while [ $SENT -lt $NUM_TXS ]; do
  BATCH_END=$((SENT + CONCURRENCY))
  if [ $BATCH_END -gt $NUM_TXS ]; then BATCH_END=$NUM_TXS; fi

  for i in $(seq $SENT $((BATCH_END - 1))); do
    send_tx $i &
  done
  wait

  SENT=$BATCH_END
  # Show progress
  OK_SO_FAR=$(grep -c "200" "$TMPDIR"/*.code 2>/dev/null || echo 0)
  echo "  Progress: $SENT/$NUM_TXS sent ($OK_SO_FAR accepted)"
done

END_TS=$(python3 -c "import time; print(time.time())")

# Results
OK=$(grep -c "200" "$TMPDIR"/*.code 2>/dev/null || echo 0)
REJECTED=$((NUM_TXS - OK))
DURATION=$(python3 -c "print(f'{$END_TS - $START_TS:.2f}')")
SUBMIT_TPS=$(python3 -c "ok=$OK; dur=$END_TS-$START_TS; print(f'{ok/dur:.1f}') if dur > 0 else print('inf')")

echo ""
echo "Submission: $OK accepted, $REJECTED rejected in ${DURATION}s"
echo "Submission TPS: $SUBMIT_TPS tx/s"

# Sample some rejection reasons
if [ "$REJECTED" -gt 0 ]; then
  echo "Sample rejections:"
  for f in $(ls "$TMPDIR"/*.code 2>/dev/null | head -500); do
    CODE=$(cat "$f")
    if [ "$CODE" != "200" ]; then
      IDX=$(basename "$f" .code)
      BODY=$(cat "$TMPDIR/$IDX.body" 2>/dev/null | head -c 100)
      echo "  HTTP $CODE: $BODY"
      break
    fi
  done
fi

# Wait for finalization
echo ""
echo "--- Waiting 30s for finalization ---"
sleep 30

# Post-test
POST=$(curl -s "$NODE_URL/status")
POST_FIN=$(echo "$POST" | python3 -c "import sys,json; print(json.load(sys.stdin)['last_finalized_round'])")
POST_ROUND=$(echo "$POST" | python3 -c "import sys,json; print(json.load(sys.stdin)['dag_round'])")
MEMPOOL=$(echo "$POST" | python3 -c "import sys,json; print(json.load(sys.stdin)['mempool_size'])")
RECV_BAL=$(curl -s "$NODE_URL/balance/$RECV" | python3 -c "import sys,json; print(json.load(sys.stdin)['balance'])")
FIN_TXS=$((RECV_BAL / 1000))
ROUNDS=$((POST_FIN - PRE_FIN))

echo ""
echo "=========================================="
echo "         TPS LOAD TEST RESULTS"
echo "=========================================="
echo ""
echo "Network:  6 validators, 5s rounds"
echo ""
echo "Submitted:      $OK transactions"
echo "Finalized:      $FIN_TXS transactions"
echo "Mempool left:   $MEMPOOL"
echo "Rounds elapsed: $ROUNDS ($PRE_FIN → $POST_FIN)"
echo ""
echo "SUBMISSION TPS:    $SUBMIT_TPS tx/s"

if [ "$ROUNDS" -gt 0 ] && [ "$FIN_TXS" -gt 0 ]; then
  TXR=$(python3 -c "print(f'{$FIN_TXS / $ROUNDS:.1f}')")
  FTPS=$(python3 -c "print(f'{$FIN_TXS / ($ROUNDS * 5):.1f}')")
  echo "FINALIZATION TPS:  $FTPS tx/s ($TXR tx/round)"
fi

echo ""
echo "THEORETICAL MAX:   2,000 tx/s"
echo "  (10K tx/vertex, 6 validators deduped,"
echo "   effective 10K unique tx/round ÷ 5s)"
echo "=========================================="

# Per-round breakdown
echo ""
echo "--- Per-round tx counts ---"
START_R=$((PRE_FIN + 1))
END_R=$POST_FIN
if [ $((END_R - START_R)) -gt 15 ]; then START_R=$((END_R - 14)); fi
for r in $(seq $START_R $END_R); do
  curl -s "$NODE_URL/round/$r" 2>/dev/null | python3 -c "
import sys,json
try:
    d=json.load(sys.stdin)
    verts=d.get('vertices',[])
    txs=sum(v.get('tx_count',0) for v in verts)
    print(f'  Round {d[\"round\"]:>4}: {len(verts)} vertices, {txs} txs')
except: pass
" 2>/dev/null
done
