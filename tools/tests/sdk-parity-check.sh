#!/usr/bin/env bash
#
# Cross-SDK signable_bytes parity check.
#
# Runs the Rust canonical test, then Python, JavaScript, and Go SDK helpers
# with identical inputs. Compares all outputs against the Rust canonical hex.
#
# Usage:  bash tools/tests/sdk-parity-check.sh
#
# Prerequisites:
#   - cargo (Rust toolchain)
#   - python3 with blake3 and pynacl packages
#   - node (with tsx available globally, or npx tsx)
#   - go 1.21+
#
# Exit code 0 = all SDKs match. Non-zero = mismatch found.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/../.." && pwd)"

PASS=0
FAIL=0
SKIP=0

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# ---------------------------------------------------------------------------
# Step 1: Get canonical Rust values
# ---------------------------------------------------------------------------

echo "=== Step 1: Computing Rust canonical signable_bytes ==="

RUST_OUTPUT=$(cd "$ROOT_DIR" && cargo test -p ultradag-sim --test sdk_parity -- --nocapture 2>&1)

# Extract canonical hex values
extract_hex() {
    echo "$RUST_OUTPUT" | grep "SDK_PARITY:$1:" | head -1 | sed "s/SDK_PARITY:$1://"
}

RUST_TRANSFER=$(extract_hex "TRANSFER")
RUST_STAKE=$(extract_hex "STAKE")
RUST_DELEGATE=$(extract_hex "DELEGATE")
RUST_VOTE=$(extract_hex "VOTE")
RUST_UNSTAKE=$(extract_hex "UNSTAKE")
RUST_UNDELEGATE=$(extract_hex "UNDELEGATE")
RUST_SET_COMMISSION=$(extract_hex "SET_COMMISSION")
FROM_ADDRESS=$(extract_hex "FROM_ADDRESS")
PUBLIC_KEY=$(extract_hex "PUBLIC_KEY")
SECRET_SEED=$(extract_hex "SECRET_SEED")

echo "  From address:    ${FROM_ADDRESS}"
echo "  Transfer hex:    ${RUST_TRANSFER:0:40}..."
echo "  Stake hex:       ${RUST_STAKE:0:40}..."
echo "  Delegate hex:    ${RUST_DELEGATE:0:40}..."
echo "  Vote hex:        ${RUST_VOTE:0:40}..."
echo ""

compare() {
    local SDK_NAME="$1"
    local TX_TYPE="$2"
    local EXPECTED="$3"
    local ACTUAL="$4"

    if [ "$EXPECTED" = "$ACTUAL" ]; then
        echo -e "  ${GREEN}PASS${NC} ${SDK_NAME} ${TX_TYPE}"
        PASS=$((PASS + 1))
    else
        echo -e "  ${RED}FAIL${NC} ${SDK_NAME} ${TX_TYPE}"
        echo "    Expected: ${EXPECTED}"
        echo "    Actual:   ${ACTUAL}"
        FAIL=$((FAIL + 1))
    fi
}

# ---------------------------------------------------------------------------
# Step 2: Python SDK
# ---------------------------------------------------------------------------

echo "=== Step 2: Python SDK ==="

if command -v python3 &>/dev/null; then
    PYTHON_OUTPUT=$(python3 "$SCRIPT_DIR/sdk_parity_python.py" \
        "$SECRET_SEED" "$FROM_ADDRESS" "$PUBLIC_KEY" 2>&1) || {
        echo -e "  ${RED}Python script failed${NC}"
        echo "  $PYTHON_OUTPUT"
        FAIL=$((FAIL + 7))
        PYTHON_OUTPUT=""
    }

    if [ -n "$PYTHON_OUTPUT" ]; then
        py_extract() { echo "$PYTHON_OUTPUT" | grep "SDK_PARITY:$1:" | head -1 | sed "s/SDK_PARITY:$1://"; }

        compare "Python" "Transfer"       "$RUST_TRANSFER"       "$(py_extract TRANSFER)"
        compare "Python" "Stake"           "$RUST_STAKE"          "$(py_extract STAKE)"
        compare "Python" "Delegate"        "$RUST_DELEGATE"       "$(py_extract DELEGATE)"
        compare "Python" "Vote"            "$RUST_VOTE"           "$(py_extract VOTE)"
        compare "Python" "Unstake"         "$RUST_UNSTAKE"        "$(py_extract UNSTAKE)"
        compare "Python" "Undelegate"      "$RUST_UNDELEGATE"     "$(py_extract UNDELEGATE)"
        compare "Python" "SetCommission"   "$RUST_SET_COMMISSION" "$(py_extract SET_COMMISSION)"
    fi
else
    echo -e "  ${YELLOW}SKIP${NC} python3 not found"
    SKIP=$((SKIP + 7))
fi

echo ""

# ---------------------------------------------------------------------------
# Step 3: JavaScript SDK
# ---------------------------------------------------------------------------

echo "=== Step 3: JavaScript SDK ==="

if command -v tsx &>/dev/null || command -v npx &>/dev/null; then
    TSX_CMD="tsx"
    if ! command -v tsx &>/dev/null; then
        TSX_CMD="npx tsx"
    fi

    JS_OUTPUT=$($TSX_CMD "$SCRIPT_DIR/sdk_parity_js.ts" \
        "$SECRET_SEED" "$FROM_ADDRESS" "$PUBLIC_KEY" 2>&1) || {
        echo -e "  ${RED}JavaScript script failed${NC}"
        echo "  $JS_OUTPUT"
        FAIL=$((FAIL + 7))
        JS_OUTPUT=""
    }

    if [ -n "$JS_OUTPUT" ]; then
        js_extract() { echo "$JS_OUTPUT" | grep "SDK_PARITY:$1:" | head -1 | sed "s/SDK_PARITY:$1://"; }

        compare "JS" "Transfer"       "$RUST_TRANSFER"       "$(js_extract TRANSFER)"
        compare "JS" "Stake"           "$RUST_STAKE"          "$(js_extract STAKE)"
        compare "JS" "Delegate"        "$RUST_DELEGATE"       "$(js_extract DELEGATE)"
        compare "JS" "Vote"            "$RUST_VOTE"           "$(js_extract VOTE)"
        compare "JS" "Unstake"         "$RUST_UNSTAKE"        "$(js_extract UNSTAKE)"
        compare "JS" "Undelegate"      "$RUST_UNDELEGATE"     "$(js_extract UNDELEGATE)"
        compare "JS" "SetCommission"   "$RUST_SET_COMMISSION" "$(js_extract SET_COMMISSION)"
    fi
else
    echo -e "  ${YELLOW}SKIP${NC} tsx/npx not found"
    SKIP=$((SKIP + 7))
fi

echo ""

# ---------------------------------------------------------------------------
# Step 4: Go SDK
# ---------------------------------------------------------------------------

echo "=== Step 4: Go SDK ==="

if command -v go &>/dev/null; then
    GO_OUTPUT=$(cd "$ROOT_DIR/sdk/go" && go run "$SCRIPT_DIR/sdk_parity_go.go" \
        "$SECRET_SEED" "$FROM_ADDRESS" "$PUBLIC_KEY" 2>&1) || {
        echo -e "  ${RED}Go script failed${NC}"
        echo "  $GO_OUTPUT"
        FAIL=$((FAIL + 7))
        GO_OUTPUT=""
    }

    if [ -n "$GO_OUTPUT" ]; then
        go_extract() { echo "$GO_OUTPUT" | grep "SDK_PARITY:$1:" | head -1 | sed "s/SDK_PARITY:$1://"; }

        compare "Go" "Transfer"       "$RUST_TRANSFER"       "$(go_extract TRANSFER)"
        compare "Go" "Stake"           "$RUST_STAKE"          "$(go_extract STAKE)"
        compare "Go" "Delegate"        "$RUST_DELEGATE"       "$(go_extract DELEGATE)"
        compare "Go" "Vote"            "$RUST_VOTE"           "$(go_extract VOTE)"
        compare "Go" "Unstake"         "$RUST_UNSTAKE"        "$(go_extract UNSTAKE)"
        compare "Go" "Undelegate"      "$RUST_UNDELEGATE"     "$(go_extract UNDELEGATE)"
        compare "Go" "SetCommission"   "$RUST_SET_COMMISSION" "$(go_extract SET_COMMISSION)"
    fi
else
    echo -e "  ${YELLOW}SKIP${NC} go not found"
    SKIP=$((SKIP + 7))
fi

echo ""

# ---------------------------------------------------------------------------
# Summary
# ---------------------------------------------------------------------------

echo "=== Summary ==="
echo -e "  ${GREEN}PASS: ${PASS}${NC}"
if [ "$FAIL" -gt 0 ]; then
    echo -e "  ${RED}FAIL: ${FAIL}${NC}"
fi
if [ "$SKIP" -gt 0 ]; then
    echo -e "  ${YELLOW}SKIP: ${SKIP}${NC}"
fi
echo ""

if [ "$FAIL" -gt 0 ]; then
    echo -e "${RED}Cross-SDK parity check FAILED${NC}"
    exit 1
fi

if [ "$PASS" -eq 0 ]; then
    echo -e "${YELLOW}No SDK checks ran (all skipped)${NC}"
    exit 2
fi

echo -e "${GREEN}All cross-SDK parity checks PASSED${NC}"
exit 0
