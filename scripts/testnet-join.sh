#!/usr/bin/env bash
set -euo pipefail

# UltraDAG — Join an existing testnet as a validator

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

# --- Help ---
usage() {
    cat <<EOF
Usage: $(basename "$0") --seed ADDR [OPTIONS]

Join an existing UltraDAG testnet as a validator.

Options:
  --seed ADDR        Seed node address (required, e.g. 1.2.3.4:9333)
  --port PORT        P2P port (default: 9333)
  --data-dir DIR     Data directory (default: ~/.ultradag/node)
  --round-ms MS      Round duration (default: 5000)
  -h, --help         Show this help

The node runs in the foreground with live log output so you can see
sync progress. Press Ctrl+C to stop.

Note: Joining as a validator requires the network to recognize your
node. In the current implementation, validators are auto-registered
when they produce their first vertex.

Examples:
  $(basename "$0") --seed 1.2.3.4:9333
  $(basename "$0") --seed 1.2.3.4:9333 --port 9334
EOF
    exit 0
}

# --- Parse args ---
while [[ $# -gt 0 ]]; do
    case "$1" in
        --seed)     SEED="$2"; shift 2 ;;
        --port)     PORT="$2"; shift 2 ;;
        --data-dir) DATA_DIR="$2"; shift 2 ;;
        --round-ms) ROUND_MS="$2"; shift 2 ;;
        -h|--help)  usage ;;
        *) err "Unknown option: $1"; exit 1 ;;
    esac
done

if [[ -z "$SEED" ]]; then
    err "--seed is required"
    echo "Usage: $(basename "$0") --seed ADDR"
    exit 1
fi

# --- Check binary ---
if [[ ! -x "$BINARY" ]]; then
    err "Binary not found at $BINARY"
    echo "Run scripts/install.sh first."
    exit 1
fi

# --- Setup ---
mkdir -p "$DATA_DIR"
RPC_PORT=$((PORT + 1000))

info "Joining UltraDAG testnet"
echo "  Seed:      $SEED"
echo "  P2P port:  $PORT"
echo "  RPC port:  $RPC_PORT"
echo "  Data dir:  $DATA_DIR"
echo "  Round:     ${ROUND_MS}ms"
echo ""
warn "Running in foreground. Press Ctrl+C to stop."
echo ""

# --- Run in foreground ---
exec "$BINARY" --port "$PORT" --seed "$SEED" --round-ms "$ROUND_MS" --validate
