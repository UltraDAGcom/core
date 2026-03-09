#!/usr/bin/env bash
set -euo pipefail

# UltraDAG — Start a 4-node local testnet

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
BINARY="$PROJECT_DIR/target/release/ultradag-node"
TESTNET_DIR="/tmp/ultradag-testnet"

NUM_NODES=4
BASE_PORT=9333
ROUND_MS=5000

# --- Colors ---
if command -v tput &>/dev/null && [ -t 1 ]; then
    GREEN=$(tput setaf 2); YELLOW=$(tput setaf 3); RED=$(tput setaf 1); RESET=$(tput sgr0); BOLD=$(tput bold)
else
    GREEN='\033[0;32m'; YELLOW='\033[0;33m'; RED='\033[0;31m'; RESET='\033[0m'; BOLD='\033[1m'
fi
info()  { printf "${GREEN}%s${RESET}\n" "$*"; }
warn()  { printf "${YELLOW}%s${RESET}\n" "$*"; }
err()   { printf "${RED}%s${RESET}\n" "$*" >&2; }

# --- Help ---
usage() {
    cat <<EOF
Usage: $(basename "$0") [OPTIONS]

Start a 4-node local testnet for development.

Options:
  --nodes N          Number of nodes (default: 4)
  --base-port PORT   Starting port (default: 9333)
  --round-ms MS      Round duration in milliseconds (default: 5000)
  --dir DIR          Testnet data directory (default: /tmp/ultradag-testnet)
  -h, --help         Show this help

Each node gets:
  - Its own data directory under \$DIR/node{1..N}/
  - Sequential P2P ports: 9333, 9334, 9335, 9336
  - Sequential RPC ports: 10333, 10334, 10335, 10336
  - Node 1 is the seed; nodes 2-N connect to node 1

Press Ctrl+C to stop all nodes cleanly.
EOF
    exit 0
}

# --- Parse args ---
while [[ $# -gt 0 ]]; do
    case "$1" in
        --nodes)     NUM_NODES="$2"; shift 2 ;;
        --base-port) BASE_PORT="$2"; shift 2 ;;
        --round-ms)  ROUND_MS="$2"; shift 2 ;;
        --dir)       TESTNET_DIR="$2"; shift 2 ;;
        -h|--help)   usage ;;
        *) err "Unknown option: $1"; exit 1 ;;
    esac
done

# --- Check binary ---
if [[ ! -x "$BINARY" ]]; then
    err "Binary not found at $BINARY"
    echo "Run scripts/install.sh first."
    exit 1
fi

# --- Clean up on exit ---
PIDS=()
cleanup() {
    echo ""
    info "Shutting down testnet..."
    for pid in "${PIDS[@]}"; do
        if kill -0 "$pid" 2>/dev/null; then
            kill "$pid" 2>/dev/null || true
        fi
    done
    # Wait for clean shutdown
    for i in $(seq 1 5); do
        ALL_DEAD=true
        for pid in "${PIDS[@]}"; do
            if kill -0 "$pid" 2>/dev/null; then
                ALL_DEAD=false
                break
            fi
        done
        if $ALL_DEAD; then break; fi
        sleep 1
    done
    # Force kill stragglers
    for pid in "${PIDS[@]}"; do
        if kill -0 "$pid" 2>/dev/null; then
            kill -9 "$pid" 2>/dev/null || true
        fi
    done
    # Clean up PID files
    for i in $(seq 1 "$NUM_NODES"); do
        rm -f "$TESTNET_DIR/node$i/node.pid"
    done
    echo ""
    info "Testnet stopped."
}
trap cleanup EXIT INT TERM

# --- Start nodes ---
printf "${BOLD}Starting UltraDAG local testnet ($NUM_NODES nodes)${RESET}\n"
echo "  Base port:  $BASE_PORT"
echo "  Round:      ${ROUND_MS}ms"
echo "  Data dir:   $TESTNET_DIR"
echo ""

SEED_ADDR="127.0.0.1:$BASE_PORT"

# Kill any stale ultradag-node processes from previous runs
STALE_PIDS=$(pgrep -f ultradag-node 2>/dev/null || true)
if [[ -n "$STALE_PIDS" ]]; then
    warn "Killing stale ultradag-node processes: $STALE_PIDS"
    pkill -f ultradag-node 2>/dev/null || true
    sleep 1
    # Force kill any that didn't exit
    pkill -9 -f ultradag-node 2>/dev/null || true
    sleep 1
fi

# Clean old data to prevent stale validators from previous runs
if [[ -d "$TESTNET_DIR" ]]; then
    warn "Cleaning previous testnet data: $TESTNET_DIR"
    rm -rf "$TESTNET_DIR"
fi

for i in $(seq 1 "$NUM_NODES"); do
    PORT=$((BASE_PORT + i - 1))
    RPC_PORT=$((PORT + 1000))
    NODE_DIR="$TESTNET_DIR/node$i"
    mkdir -p "$NODE_DIR"

    CMD=("$BINARY" --port "$PORT" --round-ms "$ROUND_MS" --validate --data-dir "$NODE_DIR" --validators "$NUM_NODES" --no-bootstrap)
    # Each node seeds from all previous nodes for mesh topology
    # (skip for node 1 which has no previous nodes)
    if [[ $i -gt 1 ]]; then
        for j in $(seq 1 $((i - 1))); do
            SEED_PORT=$((BASE_PORT + j - 1))
            CMD+=(--seed "127.0.0.1:$SEED_PORT")
        done
    fi

    nohup "${CMD[@]}" >> "$NODE_DIR/node.log" 2>&1 &
    NODE_PID=$!
    PIDS+=("$NODE_PID")
    echo "$NODE_PID" > "$NODE_DIR/node.pid"

    # Wait for the listener to be ready before starting the next node.
    # This prevents "Connection refused" when seeds aren't listening yet.
    sleep 1
    if ! kill -0 "$NODE_PID" 2>/dev/null; then
        err "  Node $i FAILED to start (port=$PORT may be in use). Check $NODE_DIR/node.log"
        exit 1
    fi

    info "  Node $i: port=$PORT rpc=$RPC_PORT pid=$NODE_PID"
done

echo ""
info "All $NUM_NODES nodes started. Waiting 3 seconds for connections..."
sleep 3

# --- Print status of all nodes ---
echo ""
printf "${BOLD}Node Status${RESET}\n"
echo "-----------"
for i in $(seq 1 "$NUM_NODES"); do
    PORT=$((BASE_PORT + i - 1))
    RPC_PORT=$((PORT + 1000))
    RPC_URL="http://127.0.0.1:$RPC_PORT"

    STATUS=$(curl -s --max-time 2 "$RPC_URL/status" 2>/dev/null) || STATUS=""
    if [[ -n "$STATUS" ]]; then
        if command -v jq &>/dev/null; then
            PEERS=$(echo "$STATUS" | jq -r '.peer_count // 0')
            ROUND=$(echo "$STATUS" | jq -r '.dag_round // 0')
        elif command -v python3 &>/dev/null; then
            PEERS=$(echo "$STATUS" | python3 -c "import sys,json; print(json.load(sys.stdin).get('peer_count',0))")
            ROUND=$(echo "$STATUS" | python3 -c "import sys,json; print(json.load(sys.stdin).get('dag_round',0))")
        else
            PEERS="?"
            ROUND="?"
        fi
        info "  Node $i: peers=$PEERS round=$ROUND rpc=$RPC_URL"
    else
        warn "  Node $i: not responding yet (rpc=$RPC_URL)"
    fi
done

echo ""
info "Testnet running. Press Ctrl+C to stop."
echo ""
echo "Useful commands:"
echo "  scripts/status.sh --rpc http://127.0.0.1:$((BASE_PORT + 1000))"
echo "  curl http://127.0.0.1:$((BASE_PORT + 1000))/keygen"
echo "  curl http://127.0.0.1:$((BASE_PORT + 1000))/status | jq"
echo ""

# --- Wait forever ---
wait
