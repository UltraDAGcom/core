#!/usr/bin/env bash
set -euo pipefail

# UltraDAG — Show node status

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

# --- Help ---
usage() {
    cat <<EOF
Usage: $(basename "$0") [OPTIONS]

Show the status of a running UltraDAG node.

Options:
  --rpc URL          RPC endpoint (default: http://127.0.0.1:10333)
  -h, --help         Show this help

Requires curl and either jq or python3 for JSON parsing.
EOF
    exit 0
}

# --- Parse args ---
while [[ $# -gt 0 ]]; do
    case "$1" in
        --rpc)     RPC_URL="$2"; shift 2 ;;
        -h|--help) usage ;;
        *) err "Unknown option: $1"; exit 1 ;;
    esac
done

# --- Check dependencies ---
if ! command -v curl &>/dev/null; then
    err "curl is required but not found."
    exit 1
fi

# Find a JSON parser
parse_json() {
    local json="$1"
    local key="$2"
    if command -v jq &>/dev/null; then
        echo "$json" | jq -r ".$key // empty"
    elif command -v python3 &>/dev/null; then
        echo "$json" | python3 -c "import sys,json; d=json.load(sys.stdin); print(d.get('$key',''))"
    else
        err "Need jq or python3 for JSON parsing."
        exit 1
    fi
}

# --- Fetch status ---
RESPONSE=$(curl -s --max-time 5 "$RPC_URL/status" 2>/dev/null) || {
    err "Cannot reach node at $RPC_URL"
    echo "Is the node running? Start with: scripts/node.sh"
    exit 1
}

# Check for error
if echo "$RESPONSE" | grep -q '"error"'; then
    err "Node returned error: $RESPONSE"
    exit 1
fi

# --- Parse fields ---
DAG_ROUND=$(parse_json "$RESPONSE" "dag_round")
FINALIZED_ROUND=$(parse_json "$RESPONSE" "last_finalized_round")
VALIDATORS=$(parse_json "$RESPONSE" "validator_count")
PEERS=$(parse_json "$RESPONSE" "peer_count")
MEMPOOL=$(parse_json "$RESPONSE" "mempool_size")
SUPPLY=$(parse_json "$RESPONSE" "total_supply")
ACCOUNTS=$(parse_json "$RESPONSE" "account_count")
DAG_VERTICES=$(parse_json "$RESPONSE" "dag_vertices")
DAG_TIPS=$(parse_json "$RESPONSE" "dag_tips")
FINALIZED_COUNT=$(parse_json "$RESPONSE" "finalized_count")

# Calculate derived values
FINALIZED_ROUND="${FINALIZED_ROUND:-none}"
if [[ "$FINALIZED_ROUND" != "none" && "$FINALIZED_ROUND" != "null" && -n "$FINALIZED_ROUND" ]]; then
    LAG=$((DAG_ROUND - FINALIZED_ROUND))
    if [[ $LAG -le 3 ]]; then
        HEALTH="${GREEN}healthy${RESET} (lag: $LAG)"
    elif [[ $LAG -le 10 ]]; then
        HEALTH="${YELLOW}slow${RESET} (lag: $LAG)"
    else
        HEALTH="${RED}stalled${RESET} (lag: $LAG)"
    fi
else
    HEALTH="${YELLOW}no finality yet${RESET}"
    FINALIZED_ROUND="none"
fi

# Format supply as UDAG
if [[ -n "$SUPPLY" && "$SUPPLY" != "null" ]]; then
    if command -v python3 &>/dev/null; then
        SUPPLY_UDAG=$(python3 -c "print(f'{$SUPPLY / 100_000_000:,.4f}')")
    elif command -v bc &>/dev/null; then
        SUPPLY_UDAG=$(echo "scale=4; $SUPPLY / 100000000" | bc)
    else
        SUPPLY_UDAG="$SUPPLY sats"
    fi
else
    SUPPLY_UDAG="0"
fi

# --- Print ---
echo ""
printf "${BOLD}UltraDAG Node Status${RESET}\n"
echo "--------------------"
printf "  %-14s %s (finalized: %s)\n" "Round:" "$DAG_ROUND" "$FINALIZED_ROUND"
printf "  %-14s " "Finality:"
printf "$HEALTH\n"
printf "  %-14s %s\n" "Validators:" "$VALIDATORS"
printf "  %-14s %s\n" "Peers:" "$PEERS"
printf "  %-14s %s pending txs\n" "Mempool:" "$MEMPOOL"
printf "  %-14s %s UDAG\n" "Supply:" "$SUPPLY_UDAG"
printf "  %-14s %s\n" "Accounts:" "$ACCOUNTS"
printf "  %-14s %s (tips: %s, finalized: %s)\n" "DAG:" "$DAG_VERTICES vertices" "$DAG_TIPS" "$FINALIZED_COUNT"
printf "  %-14s %s\n" "RPC:" "$RPC_URL"
echo ""
