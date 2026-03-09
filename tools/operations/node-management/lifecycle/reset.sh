#!/usr/bin/env bash
set -euo pipefail

# UltraDAG — Wipe node data and start fresh

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

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
DATA_DIR="$HOME/.ultradag/node"

# --- Help ---
usage() {
    cat <<EOF
Usage: $(basename "$0") [OPTIONS]

Wipe the node data directory and start fresh.

Options:
  --data-dir DIR     Data directory to wipe (default: ~/.ultradag/node)
  -h, --help         Show this help

This will:
  1. Stop the node if it is running
  2. Delete the data directory (logs, state, PID file)
  3. The node will re-sync from the network on next start

Requires typing "yes" to confirm. Cannot be undone.
EOF
    exit 0
}

# --- Parse args ---
while [[ $# -gt 0 ]]; do
    case "$1" in
        --data-dir) DATA_DIR="$2"; shift 2 ;;
        -h|--help)  usage ;;
        *) err "Unknown option: $1"; exit 1 ;;
    esac
done

# --- Check directory exists ---
if [[ ! -d "$DATA_DIR" ]]; then
    info "Data directory does not exist: $DATA_DIR"
    echo "Nothing to reset."
    exit 0
fi

# --- Show what will be deleted ---
echo ""
warn "WARNING: This will delete all local node data."
echo ""
echo "  Directory: $DATA_DIR"
if [[ -d "$DATA_DIR" ]]; then
    echo "  Contents:"
    ls -la "$DATA_DIR" 2>/dev/null | head -20 | sed 's/^/    /'
fi
echo ""
warn "The node will need to re-sync from the network on next start."
echo ""

# --- Confirm ---
printf "Type 'yes' to confirm: "
read -r CONFIRM
if [[ "$CONFIRM" != "yes" ]]; then
    info "Aborted."
    exit 0
fi

# --- Stop node if running ---
PID_FILE="$DATA_DIR/node.pid"
if [[ -f "$PID_FILE" ]]; then
    PID=$(cat "$PID_FILE")
    if kill -0 "$PID" 2>/dev/null; then
        info "Stopping running node (PID $PID)..."
        kill "$PID" 2>/dev/null || true
        for i in $(seq 1 10); do
            if ! kill -0 "$PID" 2>/dev/null; then break; fi
            sleep 1
        done
        if kill -0 "$PID" 2>/dev/null; then
            kill -9 "$PID" 2>/dev/null || true
        fi
    fi
fi

# --- Delete data ---
rm -rf "$DATA_DIR"
info "Data directory deleted: $DATA_DIR"
echo ""
echo "To start a fresh node:"
echo "  scripts/node.sh --data-dir $DATA_DIR"
