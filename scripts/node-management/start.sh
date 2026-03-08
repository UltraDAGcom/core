#!/usr/bin/env bash
set -euo pipefail

# UltraDAG — Start everything: build, testnet, and website

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
SITE_DIR="$PROJECT_DIR/site"
SITE_PORT=8080

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

Start the full UltraDAG stack: build, testnet, and website.

Options:
  --skip-build         Skip cargo build (use existing binary)
  --site-port PORT     Website port (default: 8080)
  --nodes N            Number of testnet nodes (default: 4)
  --round-ms MS        Round duration in ms (default: 5000)
  -h, --help           Show this help

Components started:
  1. Release binary (cargo build --release)
  2. 4-node local testnet (P2P: 9333-9336, RPC: 10333-10336)
  3. Website + wallet dashboard (http://127.0.0.1:8080)

Press Ctrl+C to stop everything.
EOF
    exit 0
}

# --- Parse args ---
SKIP_BUILD=false
EXTRA_TESTNET_ARGS=()
while [[ $# -gt 0 ]]; do
    case "$1" in
        --skip-build)  SKIP_BUILD=true; shift ;;
        --site-port)   SITE_PORT="$2"; shift 2 ;;
        --nodes)       EXTRA_TESTNET_ARGS+=(--nodes "$2"); shift 2 ;;
        --round-ms)    EXTRA_TESTNET_ARGS+=(--round-ms "$2"); shift 2 ;;
        -h|--help)     usage ;;
        *) err "Unknown option: $1"; exit 1 ;;
    esac
done

# --- Clean up on exit ---
SITE_PID=""
cleanup() {
    echo ""
    info "Shutting down..."

    # Stop website server
    if [[ -n "$SITE_PID" ]] && kill -0 "$SITE_PID" 2>/dev/null; then
        kill "$SITE_PID" 2>/dev/null || true
        info "  Website stopped"
    fi

    # Testnet cleanup is handled by testnet-local.sh's own trap
    # but kill any stragglers
    pkill -f ultradag-node 2>/dev/null || true
    sleep 1
    pkill -9 -f ultradag-node 2>/dev/null || true

    info "All stopped."
}
trap cleanup EXIT INT TERM

# --- Build ---
if [[ "$SKIP_BUILD" == false ]]; then
    printf "${BOLD}Building UltraDAG...${RESET}\n"
    cargo build --release -p ultradag-node 2>&1 | tail -3
    echo ""
fi

if [[ ! -x "$PROJECT_DIR/target/release/ultradag-node" ]]; then
    err "Binary not found. Run without --skip-build."
    exit 1
fi

# --- Start testnet ---
printf "${BOLD}Starting testnet...${RESET}\n"
bash "$SCRIPT_DIR/testnet-local.sh" ${EXTRA_TESTNET_ARGS[@]+"${EXTRA_TESTNET_ARGS[@]}"} &
TESTNET_PID=$!

# Wait for nodes to come up
sleep 12

# Verify at least node 1 is responding
if ! curl -s --max-time 3 "http://127.0.0.1:10333/status" >/dev/null 2>&1; then
    err "Testnet failed to start. Check /tmp/ultradag-testnet/node*/node.log"
    exit 1
fi

echo ""

# --- Start website ---
printf "${BOLD}Starting website on port ${SITE_PORT}...${RESET}\n"

# Kill anything on the site port first
lsof -ti :"$SITE_PORT" 2>/dev/null | xargs kill 2>/dev/null || true
sleep 0.5

cd "$SITE_DIR"
python3 -m http.server "$SITE_PORT" --bind 127.0.0.1 > /tmp/ultradag-site.log 2>&1 &
SITE_PID=$!
sleep 1

if ! kill -0 "$SITE_PID" 2>/dev/null; then
    err "Website failed to start on port $SITE_PORT"
    exit 1
fi

info "  Website running on http://127.0.0.1:${SITE_PORT}"

# --- Summary ---
echo ""
printf "${BOLD}=== UltraDAG Stack Running ===${RESET}\n"
echo ""
info "  Dashboard:  http://127.0.0.1:${SITE_PORT}/dashboard.html"
info "  Landing:    http://127.0.0.1:${SITE_PORT}/index.html"
echo ""
info "  Node 1 RPC: http://127.0.0.1:10333"
info "  Node 2 RPC: http://127.0.0.1:10334"
info "  Node 3 RPC: http://127.0.0.1:10335"
info "  Node 4 RPC: http://127.0.0.1:10336"
echo ""
echo "  curl http://127.0.0.1:10333/status | jq"
echo "  curl http://127.0.0.1:10333/keygen"
echo ""
info "Press Ctrl+C to stop everything."
echo ""

# --- Wait ---
wait "$TESTNET_PID" 2>/dev/null || true
