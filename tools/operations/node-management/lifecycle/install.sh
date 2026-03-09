#!/usr/bin/env bash
set -euo pipefail

# UltraDAG — Build from source
# Checks for Rust, installs if missing, builds the node binary.

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

# --- Help ---
if [[ "${1:-}" == "--help" || "${1:-}" == "-h" ]]; then
    cat <<EOF
Usage: $(basename "$0")

Builds the UltraDAG node binary from source.

Steps:
  1. Checks for Rust toolchain (cargo). Installs via rustup if missing.
  2. Runs cargo build --release -p ultradag-node
  3. Verifies the binary at target/release/ultradag-node

This script is idempotent — running it multiple times is safe.
EOF
    exit 0
fi

# --- Check / install Rust ---
if command -v cargo &>/dev/null; then
    info "Rust toolchain found: $(rustc --version)"
else
    warn "Rust not found. Installing via rustup..."
    if command -v curl &>/dev/null; then
        curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
    elif command -v wget &>/dev/null; then
        wget -qO- https://sh.rustup.rs | sh -s -- -y
    else
        err "Neither curl nor wget found. Install Rust manually: https://rustup.rs"
        exit 1
    fi
    # Source cargo env for this session
    # shellcheck disable=SC1091
    source "$HOME/.cargo/env"
    info "Rust installed: $(rustc --version)"
fi

# --- Build ---
info "Building ultradag-node (release mode)..."
cd "$PROJECT_DIR"
cargo build --release -p ultradag-node

# --- Verify ---
if [[ -x "$BINARY" ]]; then
    info "Build successful!"
    echo ""
    info "Binary: $BINARY"
    echo ""
    info "Next steps:"
    echo "  ./scripts/testnet-local.sh   # Start a 4-node local testnet"
    echo "  ./scripts/node.sh            # Start a single node"
    echo "  ./scripts/node.sh --help     # See all node options"
else
    err "Build failed — binary not found at $BINARY"
    exit 1
fi
