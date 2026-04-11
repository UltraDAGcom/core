#!/usr/bin/env bash
# deploy-presale.sh — Deploy UDAGToken + UDAGBridgeValidator on Arbitrum
# with a one-shot IDO genesis allocation baked into the constructor.
#
# This is the presale-first deployment path: the token constructor mints
# the IDO pre-mine (up to 2,520,000 UDAG) directly to a liquidity
# distributor address, the bridge stays disabled because no validators
# are registered yet, and the ERC-20 is immediately transferable so it
# can be seeded into a Uniswap pool.
#
# USAGE
#   ./deploy-presale.sh sepolia                 # Arbitrum Sepolia, DRY RUN
#   ./deploy-presale.sh sepolia --broadcast     # Arbitrum Sepolia, broadcast
#   ./deploy-presale.sh mainnet --broadcast     # Arbitrum One (production)
#
# REQUIRED ENVIRONMENT
#   DEPLOYER_KEY        Private key that funds the deployment (hex, 0x...)
#   GOVERNOR_ADDRESS    Address that will own DEFAULT_ADMIN_ROLE via the
#                       TimelockController. STRONGLY RECOMMEND a Safe multisig,
#                       not an EOA.
#   GENESIS_RECIPIENT   Address that receives the IDO pre-mine. Set to your
#                       liquidity distributor / IDO multisig.
#   GENESIS_ALLOCATION  Amount to mint, in 8-decimal sats. 1 UDAG = 1e8 sats.
#                       Hard cap: 252000000000000 (2,520,000 UDAG = 12%).
#
# OPTIONAL ENVIRONMENT
#   RPC_URL             Override default public RPC for the chosen network.
#   ARBISCAN_API_KEY    Enables `--verify` so the contracts auto-verify.
#
# SAFETY
#   Without --broadcast the script runs forge in simulation mode and does
#   not send any transactions. Mainnet deploys require an explicit typed
#   confirmation prompt in addition to --broadcast.
#

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
cd "$SCRIPT_DIR"

# ─── Argument parsing ──────────────────────────────────────────────

NETWORK="${1:-}"
BROADCAST="${2:-}"

case "$NETWORK" in
    sepolia)
        CHAIN_ID=421614
        DEFAULT_RPC="https://sepolia-rollup.arbitrum.io/rpc"
        NETWORK_LABEL="Arbitrum Sepolia (testnet)"
        ;;
    mainnet)
        CHAIN_ID=42161
        DEFAULT_RPC="https://arb1.arbitrum.io/rpc"
        NETWORK_LABEL="Arbitrum One (PRODUCTION)"
        ;;
    "")
        echo "error: network argument required" >&2
        echo "usage: $0 <sepolia|mainnet> [--broadcast]" >&2
        exit 2
        ;;
    *)
        echo "error: unknown network '$NETWORK' (expected 'sepolia' or 'mainnet')" >&2
        exit 2
        ;;
esac

if [[ -n "${BROADCAST}" && "${BROADCAST}" != "--broadcast" ]]; then
    echo "error: unknown second argument '${BROADCAST}' (expected '--broadcast' or nothing)" >&2
    exit 2
fi

DRY_RUN=true
if [[ "${BROADCAST}" == "--broadcast" ]]; then
    DRY_RUN=false
fi

RPC_URL="${RPC_URL:-$DEFAULT_RPC}"

# ─── Required env checks ───────────────────────────────────────────

missing=()
for var in DEPLOYER_KEY GOVERNOR_ADDRESS GENESIS_RECIPIENT GENESIS_ALLOCATION; do
    if [[ -z "${!var:-}" ]]; then
        missing+=("$var")
    fi
done

if (( ${#missing[@]} > 0 )); then
    echo "error: missing required environment variables:" >&2
    for v in "${missing[@]}"; do echo "  - $v" >&2; done
    echo "see the header of this script for documentation" >&2
    exit 2
fi

# ─── Sanity checks ─────────────────────────────────────────────────

# Normalize DEPLOYER_KEY: forge's `vm.envUint` is strict and rejects
# hex strings without the `0x` prefix. Users frequently paste keys
# raw, so prepend the prefix when missing rather than error out.
# Also validate it's the right length (32 bytes = 64 hex chars,
# optionally preceded by `0x`).
case "$DEPLOYER_KEY" in
    0x*) _key_hex="${DEPLOYER_KEY#0x}" ;;
    *)   _key_hex="$DEPLOYER_KEY" ;;
esac

if [[ ${#_key_hex} -ne 64 ]]; then
    echo "error: DEPLOYER_KEY must be 64 hex characters (32 bytes), got ${#_key_hex}" >&2
    echo "       example format: 0xabcd...1234 (with or without 0x prefix)" >&2
    exit 2
fi

if ! [[ "$_key_hex" =~ ^[0-9a-fA-F]{64}$ ]]; then
    echo "error: DEPLOYER_KEY contains non-hex characters" >&2
    exit 2
fi

# Always re-export with the 0x prefix so downstream tools (forge, cast)
# accept it regardless of how the user supplied it.
DEPLOYER_KEY="0x$_key_hex"
unset _key_hex

# Cap: 2,520,000 UDAG in 8-decimal sats = 252_000_000_000_000.
MAX_GENESIS_SATS=252000000000000

if ! [[ "$GENESIS_ALLOCATION" =~ ^[0-9]+$ ]]; then
    echo "error: GENESIS_ALLOCATION must be a positive integer (in sats), got '$GENESIS_ALLOCATION'" >&2
    exit 2
fi

if (( GENESIS_ALLOCATION > MAX_GENESIS_SATS )); then
    echo "error: GENESIS_ALLOCATION=$GENESIS_ALLOCATION exceeds cap $MAX_GENESIS_SATS" >&2
    echo "       (max is 2,520,000 UDAG = 12% of MAX_SUPPLY)" >&2
    exit 2
fi

if (( GENESIS_ALLOCATION == 0 )); then
    echo "warning: GENESIS_ALLOCATION=0 — no pre-mine will be minted." >&2
    echo "         This script is meant for presale deployments; you probably" >&2
    echo "         want a non-zero allocation. Proceeding anyway." >&2
fi

if ! command -v forge >/dev/null 2>&1; then
    echo "error: forge not found in PATH. Install Foundry:" >&2
    echo "  curl -L https://foundry.paradigm.xyz | bash && foundryup" >&2
    exit 2
fi

if [[ ! -d lib/openzeppelin-contracts || ! -d lib/forge-std ]]; then
    echo "error: forge dependencies not installed. Run:" >&2
    echo "  forge install openzeppelin/openzeppelin-contracts foundry-rs/forge-std" >&2
    exit 2
fi

# ─── Pretty-print a human summary ──────────────────────────────────

echo "┌─────────────────────────────────────────────────────────────────"
echo "│  UltraDAG Presale Deployment"
echo "│"
echo "│  Network        : $NETWORK_LABEL"
echo "│  Chain ID       : $CHAIN_ID"
echo "│  RPC URL        : $RPC_URL"
echo "│  Governor       : $GOVERNOR_ADDRESS"
echo "│  Genesis addr   : $GENESIS_RECIPIENT"
printf "│  Genesis amount : %'d sats (%.6f UDAG)\n" "$GENESIS_ALLOCATION" "$(awk -v x="$GENESIS_ALLOCATION" 'BEGIN { print x / 100000000 }')"
echo "│  Mode           : $([ "$DRY_RUN" = true ] && echo 'DRY RUN (no broadcast)' || echo 'LIVE BROADCAST')"
echo "└─────────────────────────────────────────────────────────────────"
echo

# ─── Mainnet double-confirm ────────────────────────────────────────

if [[ "$NETWORK" == "mainnet" && "$DRY_RUN" == "false" ]]; then
    cat <<'EOF'
⚠️  MAINNET BROADCAST WARNING

You are about to deploy production bridge contracts to Arbitrum One and
mint the IDO genesis allocation to the recipient above. This spends real
gas, publishes code at a real address, and irreversibly mints tokens.

Verify:
  - GOVERNOR_ADDRESS is a Safe multisig (not an EOA)
  - GENESIS_RECIPIENT is the correct liquidity distributor address
  - GENESIS_ALLOCATION matches your presale plan
  - You have read DEPLOYMENT_GUIDE.md and completed the pre-deployment checklist
  - An external audit of UDAGToken.sol and UDAGBridgeValidator.sol has happened

Type "DEPLOY MAINNET" (exactly) to proceed:
EOF
    read -r confirmation
    if [[ "$confirmation" != "DEPLOY MAINNET" ]]; then
        echo "aborted (confirmation did not match)" >&2
        exit 1
    fi
fi

# ─── Build forge command ───────────────────────────────────────────

FORGE_ARGS=(
    script script/Deploy.s.sol:DeployScript
    --rpc-url "$RPC_URL"
    --private-key "$DEPLOYER_KEY"
    -vvvv
)

if [[ "$DRY_RUN" == "false" ]]; then
    FORGE_ARGS+=(--broadcast)
    if [[ -n "${ARBISCAN_API_KEY:-}" ]]; then
        FORGE_ARGS+=(--verify --etherscan-api-key "$ARBISCAN_API_KEY")
    else
        echo "note: ARBISCAN_API_KEY not set — skipping automatic contract verification"
        echo "      (you can run 'forge verify-contract' manually afterwards)"
    fi
fi

# Deploy.s.sol reads these from the environment via vm.envOr / vm.envAddress.
export GENESIS_RECIPIENT
export GENESIS_ALLOCATION
export GOVERNOR_ADDRESS
export DEPLOYER_KEY

echo "running: forge ${FORGE_ARGS[*]}"
echo
forge "${FORGE_ARGS[@]}"

# ─── Post-deploy summary ───────────────────────────────────────────

if [[ "$DRY_RUN" == "false" && -f deployment-output.json ]]; then
    echo
    echo "┌─────────────────────────────────────────────────────────────────"
    echo "│  DEPLOYMENT COMPLETE"
    echo "└─────────────────────────────────────────────────────────────────"
    echo
    cat deployment-output.json | python3 -m json.tool || cat deployment-output.json
    echo
    TOKEN_ADDR=$(python3 -c "import json; print(json.load(open('deployment-output.json'))['token'])" 2>/dev/null || echo "")
    BRIDGE_ADDR=$(python3 -c "import json; print(json.load(open('deployment-output.json'))['bridge'])" 2>/dev/null || echo "")

    if [[ -n "$TOKEN_ADDR" ]]; then
        echo "Next steps:"
        echo
        echo "1. Verify the genesis allocation landed in the liquidity address:"
        echo "   cast call $TOKEN_ADDR 'balanceOf(address)(uint256)' $GENESIS_RECIPIENT --rpc-url $RPC_URL"
        echo "   (expected: $GENESIS_ALLOCATION)"
        echo
        echo "2. Verify totalSupply equals the genesis allocation:"
        echo "   cast call $TOKEN_ADDR 'totalSupply()(uint256)' --rpc-url $RPC_URL"
        echo
        echo "3. Confirm the bridge is the sole minter:"
        echo "   cast call $TOKEN_ADDR 'isMinter(address)(bool)' $BRIDGE_ADDR --rpc-url $RPC_URL"
        echo "   (expected: true)"
        echo
        echo "4. Seed a Uniswap v3 pool from GENESIS_RECIPIENT:"
        echo "   - Pair UDAG with WETH (0x82aF49447D8a07e3bd95BD0d56f35241523fBab1 on Arbitrum One)"
        echo "   - Use the Uniswap interface or v3-periphery NonfungiblePositionManager"
        echo "   - Start with a narrow price range; widen as volume confirms"
        echo
        echo "5. Save these addresses in the bridge/README.md 'Contract Addresses' table."
    fi
fi
