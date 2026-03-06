#!/usr/bin/env bash
set -euo pipefail

# UltraDAG — Generate a new keypair

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
RPC_URL="http://127.0.0.1:10333"
OUTPUT="./ultradag-key.json"

# --- Help ---
usage() {
    cat <<EOF
Usage: $(basename "$0") [OPTIONS]

Generate a new Ed25519 keypair via the node RPC.

Options:
  --rpc URL          RPC endpoint (default: http://127.0.0.1:10333)
  --output FILE      Output file (default: ./ultradag-key.json)
  -h, --help         Show this help

The keypair is saved to the output file with restricted permissions (600).
The file contains the secret key and address in JSON format.

Requires a running node for key generation.
EOF
    exit 0
}

# --- Parse args ---
while [[ $# -gt 0 ]]; do
    case "$1" in
        --rpc)    RPC_URL="$2"; shift 2 ;;
        --output) OUTPUT="$2"; shift 2 ;;
        -h|--help) usage ;;
        *) err "Unknown option: $1"; exit 1 ;;
    esac
done

# --- Check dependencies ---
if ! command -v curl &>/dev/null; then
    err "curl is required but not found."
    exit 1
fi

# --- Check if output file exists ---
if [[ -f "$OUTPUT" ]]; then
    warn "Output file already exists: $OUTPUT"
    printf "Overwrite? [y/N] "
    read -r CONFIRM
    if [[ "$CONFIRM" != "y" && "$CONFIRM" != "Y" ]]; then
        info "Aborted."
        exit 0
    fi
fi

# --- Generate keypair ---
RESPONSE=$(curl -s --max-time 5 "$RPC_URL/keygen" 2>/dev/null) || {
    err "Cannot reach node at $RPC_URL"
    echo "Is the node running? Start with: scripts/node.sh"
    exit 1
}

# Check for error
if echo "$RESPONSE" | grep -q '"error"'; then
    err "Node returned error: $RESPONSE"
    exit 1
fi

# --- Save to file ---
echo "$RESPONSE" > "$OUTPUT"
chmod 600 "$OUTPUT"

# --- Parse address ---
if command -v jq &>/dev/null; then
    ADDRESS=$(echo "$RESPONSE" | jq -r '.address')
elif command -v python3 &>/dev/null; then
    ADDRESS=$(echo "$RESPONSE" | python3 -c "import sys,json; print(json.load(sys.stdin)['address'])")
else
    ADDRESS="(install jq or python3 to display)"
fi

echo ""
info "Keypair generated!"
echo "  Address:  $ADDRESS"
echo "  Saved to: $OUTPUT"
echo ""
warn "IMPORTANT: Back up this file! The secret key cannot be recovered."
warn "Keep it safe and never share it."
