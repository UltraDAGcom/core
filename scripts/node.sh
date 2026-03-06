#!/usr/bin/env bash
set -euo pipefail

# UltraDAG — Start a persistent validator node

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
BINARY="$PROJECT_DIR/target/release/ultradag-node"

# --- Colors ---
if command -v tput &>/dev/null && [ -t 1 ]; then
    GREEN=$(tput setaf 2); YELLOW=$(tput setaf 3); RED=$(tput setaf 1); RESET=$(tput sgr0)
else
    GREEN='\033[0;32m'; YELLOW='\033[0;33m'; RED='\033[0;31m'; RESET='\033[0m'
fi
info()  { printf "${GREEN}%s${RESET}\n" "$*"; }
warn()  { printf "${YELLOW}%s${RESET}\n" "$*"; }
err()   { printf "${RED}%s${RESET}\n" "$*" >&2; }

# --- Defaults ---
PORT=9333
DATA_DIR="$HOME/.ultradag/node"
SEED=""
ROUND_MS=5000
NAME=""

# --- Help ---
usage() {
    cat <<EOF
Usage: $(basename "$0") [OPTIONS]

Start a persistent UltraDAG validator node.

Options:
  --port PORT        P2P port (default: 9333)
  --data-dir DIR     Data directory (default: ~/.ultradag/node)
  --seed ADDR        Seed peer address (e.g. 1.2.3.4:9333)
  --round-ms MS      Round duration in milliseconds (default: 5000)
  --name NAME        Display name for this node (optional)
  -h, --help         Show this help

The node runs in the background via nohup. Logs are written to
\$DATA_DIR/node.log. A PID file is stored at \$DATA_DIR/node.pid.

The RPC server runs on P2P port + 1000 (e.g. port 9333 -> RPC 10333).

Examples:
  $(basename "$0")                                  # Start on default port
  $(basename "$0") --port 9334 --seed 127.0.0.1:9333
  $(basename "$0") --data-dir /var/ultradag --round-ms 3000
EOF
    exit 0
}

# --- Parse args ---
while [[ $# -gt 0 ]]; do
    case "$1" in
        --port)     PORT="$2"; shift 2 ;;
        --data-dir) DATA_DIR="$2"; shift 2 ;;
        --seed)     SEED="$2"; shift 2 ;;
        --round-ms) ROUND_MS="$2"; shift 2 ;;
        --name)     NAME="$2"; shift 2 ;;
        -h|--help)  usage ;;
        *) err "Unknown option: $1"; echo "Run with --help for usage."; exit 1 ;;
    esac
done

# --- Check binary ---
if [[ ! -x "$BINARY" ]]; then
    err "Binary not found at $BINARY"
    echo "Run scripts/install.sh first."
    exit 1
fi

# --- Create data directory ---
mkdir -p "$DATA_DIR"

PID_FILE="$DATA_DIR/node.pid"
LOG_FILE="$DATA_DIR/node.log"
RPC_PORT=$((PORT + 1000))

# --- Check if already running ---
if [[ -f "$PID_FILE" ]]; then
    OLD_PID=$(cat "$PID_FILE")
    if kill -0 "$OLD_PID" 2>/dev/null; then
        err "Node already running (PID $OLD_PID)"
        echo "Stop it first: scripts/stop.sh --data-dir $DATA_DIR"
        exit 1
    else
        warn "Stale PID file found (process $OLD_PID not running). Cleaning up."
        rm -f "$PID_FILE"
    fi
fi

# --- Build command ---
CMD=("$BINARY" --port "$PORT" --round-ms "$ROUND_MS" --validate)
if [[ -n "$SEED" ]]; then
    CMD+=(--seed "$SEED")
fi

# --- Start node ---
NODE_NAME="${NAME:-node:$PORT}"
info "Starting UltraDAG node '$NODE_NAME'..."
echo "  P2P port:  $PORT"
echo "  RPC port:  $RPC_PORT"
echo "  Data dir:  $DATA_DIR"
echo "  Round:     ${ROUND_MS}ms"
[[ -n "$SEED" ]] && echo "  Seed:      $SEED"
echo ""

nohup "${CMD[@]}" >> "$LOG_FILE" 2>&1 &
NODE_PID=$!
echo "$NODE_PID" > "$PID_FILE"

info "Node started (PID $NODE_PID)"
echo "  Log:  $LOG_FILE"
echo "  PID:  $PID_FILE"
echo "  RPC:  http://127.0.0.1:$RPC_PORT"
echo ""

# --- Tail log briefly ---
info "Tailing log for 5 seconds..."
echo "---"
timeout 5 tail -f "$LOG_FILE" 2>/dev/null || true
echo "---"
echo ""
info "Node is running in the background."
echo "  Status:  scripts/status.sh --rpc http://127.0.0.1:$RPC_PORT"
echo "  Stop:    scripts/stop.sh --data-dir $DATA_DIR"
