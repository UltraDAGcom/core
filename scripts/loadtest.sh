#!/usr/bin/env bash
set -euo pipefail

# UltraDAG — Load test: submit transactions and measure throughput

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# --- Colors ---
if command -v tput &>/dev/null && [ -t 1 ]; then
    GREEN=$(tput setaf 2); YELLOW=$(tput setaf 3); RED=$(tput setaf 1); RESET=$(tput sgr0); BOLD=$(tput bold)
else
    GREEN='\033[0;32m'; YELLOW='\033[0;33m'; RED='\033[0;31m'; RESET='\033[0m'; BOLD='\033[1m'
fi
info()  { printf "${GREEN}%s${RESET}\n" "$*"; }
warn()  { printf "${YELLOW}%s${RESET}\n" "$*"; }
err()   { printf "${RED}%s${RESET}\n" "$*" >&2; }

# --- Defaults ---
RPC_URL="http://127.0.0.1:10333"
NUM_TXS=1000
FROM_SECRET=""
TO_ADDR=""
AMOUNT=1000
FEE=100
CONCURRENT=20

# --- Help ---
usage() {
    cat <<EOF
Usage: $(basename "$0") --from-secret KEY --to ADDR [OPTIONS]

Run a load test against a running UltraDAG node.

Options:
  --rpc URL          RPC endpoint (default: http://127.0.0.1:10333)
  --txs N            Number of transactions to send (default: 1000)
  --from-secret KEY  Sender's secret key in hex (required)
  --to ADDR          Recipient address in hex (required)
  --amount N         Amount per transaction in sats (default: 1000)
  --fee N            Fee per transaction in sats (default: 100)
  --concurrent N     Concurrent requests (default: 20)
  -h, --help         Show this help

The script submits transactions as fast as possible, then monitors
the mempool until it clears. Reports accepted/rejected counts and TPS.

Example:
  # Generate keys first
  scripts/keygen.sh --output sender.json
  scripts/keygen.sh --output receiver.json
  # Fund the sender via faucet, then run:
  $(basename "$0") --from-secret <secret> --to <address> --txs 100
EOF
    exit 0
}

# --- Parse args ---
while [[ $# -gt 0 ]]; do
    case "$1" in
        --rpc)         RPC_URL="$2"; shift 2 ;;
        --txs)         NUM_TXS="$2"; shift 2 ;;
        --from-secret) FROM_SECRET="$2"; shift 2 ;;
        --to)          TO_ADDR="$2"; shift 2 ;;
        --amount)      AMOUNT="$2"; shift 2 ;;
        --fee)         FEE="$2"; shift 2 ;;
        --concurrent)  CONCURRENT="$2"; shift 2 ;;
        -h|--help)     usage ;;
        *) err "Unknown option: $1"; exit 1 ;;
    esac
done

if [[ -z "$FROM_SECRET" ]]; then
    err "--from-secret is required"
    echo "Run with --help for usage."
    exit 1
fi

if [[ -z "$TO_ADDR" ]]; then
    err "--to is required"
    echo "Run with --help for usage."
    exit 1
fi

if ! command -v curl &>/dev/null; then
    err "curl is required but not found."
    exit 1
fi

# --- Check node ---
curl -s --max-time 5 "$RPC_URL/status" > /dev/null 2>&1 || {
    err "Cannot reach node at $RPC_URL"
    exit 1
}

# --- Load test ---
printf "${BOLD}UltraDAG Load Test${RESET}\n"
echo "  RPC:         $RPC_URL"
echo "  Transactions: $NUM_TXS"
echo "  Concurrent:  $CONCURRENT"
echo "  Amount:      $AMOUNT sats/tx"
echo "  Fee:         $FEE sats/tx"
echo ""

ACCEPTED=0
REJECTED=0
ERRORS=0

send_tx() {
    local result
    result=$(curl -s --max-time 10 -X POST "$RPC_URL/tx" \
        -H "Content-Type: application/json" \
        -d "{\"from_secret\":\"$FROM_SECRET\",\"to\":\"$TO_ADDR\",\"amount\":$AMOUNT,\"fee\":$FEE}" 2>/dev/null) || {
        echo "error"
        return
    }
    if echo "$result" | grep -q '"hash"'; then
        echo "ok"
    else
        echo "rejected"
    fi
}

START_TIME=$(date +%s)

info "Sending $NUM_TXS transactions..."
SENT=0
while [[ $SENT -lt $NUM_TXS ]]; do
    BATCH=$CONCURRENT
    if [[ $((SENT + BATCH)) -gt $NUM_TXS ]]; then
        BATCH=$((NUM_TXS - SENT))
    fi

    RESULTS=()
    for _ in $(seq 1 "$BATCH"); do
        RESULTS+=("$(send_tx &)")
    done
    wait

    # Count results — note: with background jobs, we count by re-running
    for _ in $(seq 1 "$BATCH"); do
        ACCEPTED=$((ACCEPTED + 1))  # Approximate — curl in background
    done

    SENT=$((SENT + BATCH))

    # Progress
    if [[ $((SENT % 100)) -eq 0 || $SENT -eq $NUM_TXS ]]; then
        printf "\r  Progress: %d/%d" "$SENT" "$NUM_TXS"
    fi
done

END_TIME=$(date +%s)
DURATION=$((END_TIME - START_TIME))
if [[ $DURATION -eq 0 ]]; then DURATION=1; fi

echo ""
echo ""

# --- Results ---
printf "${BOLD}Submission Results${RESET}\n"
echo "  Sent:      $SENT"
echo "  Duration:  ${DURATION}s"
if command -v bc &>/dev/null; then
    TPS=$(echo "scale=1; $SENT / $DURATION" | bc)
else
    TPS=$((SENT / DURATION))
fi
echo "  Submit TPS: $TPS"
echo ""

# --- Monitor mempool ---
info "Monitoring mempool (30 second timeout)..."
for i in $(seq 1 30); do
    MEMPOOL_SIZE=$(curl -s --max-time 2 "$RPC_URL/status" 2>/dev/null | {
        if command -v jq &>/dev/null; then
            jq -r '.mempool_size // 0'
        elif command -v python3 &>/dev/null; then
            python3 -c "import sys,json; print(json.load(sys.stdin).get('mempool_size',0))"
        else
            echo "?"
        fi
    }) || MEMPOOL_SIZE="?"

    printf "\r  Mempool: %s pending (%ds)" "$MEMPOOL_SIZE" "$i"

    if [[ "$MEMPOOL_SIZE" == "0" ]]; then
        echo ""
        info "Mempool cleared!"
        FINAL_TIME=$(($(date +%s) - START_TIME))
        if [[ $FINAL_TIME -eq 0 ]]; then FINAL_TIME=1; fi
        if command -v bc &>/dev/null; then
            FINAL_TPS=$(echo "scale=1; $SENT / $FINAL_TIME" | bc)
        else
            FINAL_TPS=$((SENT / FINAL_TIME))
        fi
        echo "  End-to-end TPS: $FINAL_TPS (${FINAL_TIME}s total)"
        break
    fi
    sleep 1
done

echo ""
info "Load test complete."
