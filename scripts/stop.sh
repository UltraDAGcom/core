#!/usr/bin/env bash
set -euo pipefail

# UltraDAG — Stop a running node cleanly

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
ALL=false

# --- Help ---
usage() {
    cat <<EOF
Usage: $(basename "$0") [OPTIONS]

Stop a running UltraDAG node.

Options:
  --data-dir DIR     Data directory containing node.pid (default: ~/.ultradag/node)
  --all              Stop all UltraDAG nodes (kills all ultradag-node processes)
  -h, --help         Show this help

The script sends SIGTERM for a clean shutdown, waits up to 10 seconds,
then sends SIGKILL if the process hasn't exited.
EOF
    exit 0
}

# --- Parse args ---
while [[ $# -gt 0 ]]; do
    case "$1" in
        --data-dir) DATA_DIR="$2"; shift 2 ;;
        --all)      ALL=true; shift ;;
        -h|--help)  usage ;;
        *) err "Unknown option: $1"; exit 1 ;;
    esac
done

# --- Stop all ---
if $ALL; then
    PIDS=$(pgrep -f "ultradag-node" 2>/dev/null || true)
    if [[ -z "$PIDS" ]]; then
        info "No UltraDAG nodes running."
        exit 0
    fi
    info "Stopping all UltraDAG nodes..."
    for pid in $PIDS; do
        echo "  Sending SIGTERM to PID $pid"
        kill "$pid" 2>/dev/null || true
    done
    # Wait for clean shutdown
    for i in $(seq 1 10); do
        REMAINING=$(pgrep -f "ultradag-node" 2>/dev/null || true)
        if [[ -z "$REMAINING" ]]; then
            info "All nodes stopped cleanly."
            # Clean up any PID files in /tmp/ultradag-testnet
            rm -f /tmp/ultradag-testnet/node*/node.pid 2>/dev/null || true
            exit 0
        fi
        sleep 1
    done
    # Force kill stragglers
    warn "Some nodes didn't stop in 10 seconds — sending SIGKILL"
    for pid in $(pgrep -f "ultradag-node" 2>/dev/null || true); do
        kill -9 "$pid" 2>/dev/null || true
    done
    rm -f /tmp/ultradag-testnet/node*/node.pid 2>/dev/null || true
    info "All nodes stopped."
    exit 0
fi

# --- Stop single node ---
stop_node() {
    local data_dir="$1"
    local pid_file="$data_dir/node.pid"

    if [[ ! -f "$pid_file" ]]; then
        err "No PID file found at $pid_file"
        echo "Is the node running? Check with: ps aux | grep ultradag-node"
        return 1
    fi

    local pid
    pid=$(cat "$pid_file")

    if ! kill -0 "$pid" 2>/dev/null; then
        warn "Process $pid is not running. Cleaning up stale PID file."
        rm -f "$pid_file"
        return 0
    fi

    info "Stopping node (PID $pid)..."
    kill "$pid"

    # Wait up to 10 seconds for clean shutdown
    for i in $(seq 1 10); do
        if ! kill -0 "$pid" 2>/dev/null; then
            rm -f "$pid_file"
            info "Node stopped cleanly."
            return 0
        fi
        sleep 1
    done

    # Force kill
    warn "Node didn't stop in 10 seconds — sending SIGKILL"
    kill -9 "$pid" 2>/dev/null || true
    rm -f "$pid_file"
    info "Node killed."
}

stop_node "$DATA_DIR"
