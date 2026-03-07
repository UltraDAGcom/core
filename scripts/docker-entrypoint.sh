#!/bin/sh
set -e

# Clean state on startup if requested (for fresh testnet resets)
if [ "${CLEAN_STATE:-}" = "true" ] || [ "${CLEAN_STATE:-}" = "1" ]; then
  echo "CLEAN_STATE: removing persisted state files..."
  # Remove DAG/state data but keep validator.key (node identity must survive resets
  # so addresses continue to match the permissioned validator allowlist)
  rm -f "${DATA_DIR:-/data}/dag.json" "${DATA_DIR:-/data}/finality.json" \
        "${DATA_DIR:-/data}/state.json" "${DATA_DIR:-/data}/mempool.json"
fi

ARGS="--port ${PORT:-9333} --rpc-port ${RPC_PORT:-10333} --data-dir ${DATA_DIR:-/data} --validate"

if [ -n "${VALIDATORS:-}" ]; then
  ARGS="$ARGS --validators $VALIDATORS"
fi

if [ -n "${SEED:-}" ]; then
  for s in $SEED; do
    ARGS="$ARGS --seed $s"
  done
fi

if [ -n "${ROUND_MS:-}" ]; then
  ARGS="$ARGS --round-ms $ROUND_MS"
fi

VALIDATOR_KEY_FILE="${VALIDATOR_KEY_FILE:-/etc/ultradag/validators.txt}"
if [ -f "$VALIDATOR_KEY_FILE" ]; then
  ARGS="$ARGS --validator-key $VALIDATOR_KEY_FILE"
fi

if [ "${NO_BOOTSTRAP:-}" = "true" ] || [ "${NO_BOOTSTRAP:-}" = "1" ]; then
  ARGS="$ARGS --no-bootstrap"
fi

exec ultradag-node $ARGS
