#!/usr/bin/env bash
set -euo pipefail

# UltraDAG — Tail node logs with color formatting

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# --- Defaults ---
DATA_DIR="$HOME/.ultradag/node"

# --- Help ---
usage() {
    cat <<EOF
Usage: $(basename "$0") [OPTIONS]

Tail the UltraDAG node log with color formatting.

Options:
  --data-dir DIR     Data directory (default: ~/.ultradag/node)
  -h, --help         Show this help

Colors:
  Green   - finalized rounds
  Yellow  - warnings, slow rounds
  Red     - errors, panics

The log file is at \$DATA_DIR/node.log (written by node.sh).
Press Ctrl+C to stop tailing.
EOF
    exit 0
}

# --- Parse args ---
while [[ $# -gt 0 ]]; do
    case "$1" in
        --data-dir) DATA_DIR="$2"; shift 2 ;;
        -h|--help)  usage ;;
        *) echo "Unknown option: $1" >&2; exit 1 ;;
    esac
done

LOG_FILE="$DATA_DIR/node.log"

if [[ ! -f "$LOG_FILE" ]]; then
    echo "Log file not found: $LOG_FILE" >&2
    echo "Is the node running? Start with: scripts/node.sh --data-dir $DATA_DIR"
    exit 1
fi

# --- Colors ---
if command -v tput &>/dev/null && [ -t 1 ]; then
    GREEN=$(tput setaf 2); YELLOW=$(tput setaf 3); RED=$(tput setaf 1); RESET=$(tput sgr0)
else
    GREEN='\033[0;32m'; YELLOW='\033[0;33m'; RED='\033[0;31m'; RESET='\033[0m'
fi

echo "Tailing $LOG_FILE (Ctrl+C to stop)"
echo "---"

tail -f "$LOG_FILE" | while IFS= read -r line; do
    if echo "$line" | grep -qiE "error|panic|fatal"; then
        printf "${RED}%s${RESET}\n" "$line"
    elif echo "$line" | grep -qiE "warn|slow|skip"; then
        printf "${YELLOW}%s${RESET}\n" "$line"
    elif echo "$line" | grep -qiE "finalized|finality"; then
        printf "${GREEN}%s${RESET}\n" "$line"
    else
        echo "$line"
    fi
done
