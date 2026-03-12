#!/bin/sh
set -e

# Clean state on startup if requested (for fresh testnet resets).
# Uses a marker file to prevent double-wipe on subsequent restarts
# when CLEAN_STATE is still set in the deployed env.
MARKER="${DATA_DIR:-/data}/.clean_state_done"
if [ "${CLEAN_STATE:-}" = "true" ] || [ "${CLEAN_STATE:-}" = "1" ]; then
  if [ -f "$MARKER" ]; then
    echo "CLEAN_STATE: already wiped this session (marker exists), skipping..."
  else
    echo "CLEAN_STATE: removing persisted state files..."
    # Remove DAG/state data, checkpoints, and high-water mark, but keep validator.key
    rm -f "${DATA_DIR:-/data}/dag.json" "${DATA_DIR:-/data}/finality.json" \
          "${DATA_DIR:-/data}/state.json" "${DATA_DIR:-/data}/mempool.json" \
          "${DATA_DIR:-/data}/high_water_mark.json" \
          "${DATA_DIR:-/data}/wal.jsonl" "${DATA_DIR:-/data}/wal_header.json"
    rm -rf "${DATA_DIR:-/data}/checkpoints"
    rm -rf "${DATA_DIR:-/data}/checkpoint_states"
    # Create marker so subsequent restarts don't wipe again
    touch "$MARKER"
  fi
else
  # CLEAN_STATE not set — remove marker if it exists (for future clean deploys)
  rm -f "$MARKER"
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

if [ "${NO_BOOTSTRAP:-}" = "true" ] || [ "${NO_BOOTSTRAP:-}" = "1" ]; then
  ARGS="$ARGS --no-bootstrap"
fi

if [ -n "${PRUNING_DEPTH:-}" ]; then
  ARGS="$ARGS --pruning-depth $PRUNING_DEPTH"
fi

if [ "${ARCHIVE:-}" = "true" ] || [ "${ARCHIVE:-}" = "1" ]; then
  ARGS="$ARGS --archive"
fi

if [ "${SKIP_FAST_SYNC:-}" = "true" ] || [ "${SKIP_FAST_SYNC:-}" = "1" ]; then
  ARGS="$ARGS --skip-fast-sync"
fi

exec ultradag-node $ARGS
