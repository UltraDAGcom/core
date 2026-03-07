#!/usr/bin/env bash
set -euo pipefail

# UltraDAG 4-node testnet deployment on Fly.io
# Usage: ./scripts/fly-deploy.sh

APPS=("ultradag-node-1" "ultradag-node-2" "ultradag-node-3" "ultradag-node-4")
REGIONS=("ams" "fra" "lhr" "sin")
VALIDATORS=4
PROJECT_DIR="$(cd "$(dirname "$0")/.." && pwd)"

cd "$PROJECT_DIR"

# Check flyctl
if ! command -v flyctl &>/dev/null; then
  echo "Error: flyctl not installed. Run: brew install flyctl && flyctl auth login"
  exit 1
fi

if ! flyctl auth whoami &>/dev/null 2>&1; then
  echo "Error: Not authenticated. Run: flyctl auth login"
  exit 1
fi

echo "=== UltraDAG Testnet Deployment ==="
echo "Deploying ${#APPS[@]} nodes to Fly.io"
echo ""

# Create all apps and volumes first
for i in "${!APPS[@]}"; do
  APP="${APPS[$i]}"
  REGION="${REGIONS[$i]}"

  echo "--- Setting up $APP (region: $REGION) ---"

  if ! flyctl apps list 2>/dev/null | grep -q "^$APP "; then
    flyctl apps create "$APP" --org personal 2>/dev/null || echo "  App $APP may already exist"
  else
    echo "  App $APP exists"
  fi

  if ! flyctl volumes list -a "$APP" 2>/dev/null | grep -q "ultradag_data"; then
    echo "  Creating 1GB volume..."
    flyctl volumes create ultradag_data --size 1 --region "$REGION" -a "$APP" -y 2>/dev/null || echo "  Volume may already exist"
  else
    echo "  Volume exists"
  fi
done

# Helper: generate fly.toml for a given app/region
gen_fly_toml() {
  local app="$1" region="$2"
  cat > fly.toml <<EOF
app = "$app"
primary_region = "$region"

[build]

[env]
  RUST_LOG = "info"
  PORT = "9333"
  RPC_PORT = "10333"
  DATA_DIR = "/data"
  VALIDATORS = "$VALIDATORS"

[mounts]
  source = "ultradag_data"
  destination = "/data"

[[services]]
  protocol = "tcp"
  internal_port = 9333

  [[services.ports]]
    port = 9333

[[services]]
  protocol = "tcp"
  internal_port = 10333

  [[services.ports]]
    port = 10333
    handlers = ["http"]

[checks]
  [checks.health]
    grace_period = "60s"
    interval = "15s"
    method = "GET"
    path = "/status"
    port = 10333
    timeout = "5s"
    type = "http"
EOF
}

echo ""
echo "=== Deploying node-1 (seed node) ==="
gen_fly_toml "ultradag-node-1" "ams"
flyctl deploy -a ultradag-node-1 --strategy immediate --wait-timeout 300

# Allocate IP if needed and get it
echo ""
echo "Getting node-1 IP..."
if ! flyctl ips list -a ultradag-node-1 2>/dev/null | grep -q "v4"; then
  flyctl ips allocate-v4 -a ultradag-node-1 --shared
fi
NODE1_IP=$(flyctl ips list -a ultradag-node-1 2>/dev/null | grep "v4" | awk '{print $2}' | head -1)
echo "Node-1 IP: $NODE1_IP"
SEED_ADDR="${NODE1_IP}:9333"

echo "Waiting 15s for node-1 to start..."
sleep 15

# Deploy nodes 2-4 with seed
for i in 1 2 3; do
  APP="${APPS[$i]}"
  REGION="${REGIONS[$i]}"

  echo ""
  echo "=== Deploying $APP (region: $REGION) ==="
  gen_fly_toml "$APP" "$REGION"

  # Add SEED env var
  flyctl secrets set SEED="$SEED_ADDR" -a "$APP" 2>/dev/null || true
  flyctl deploy -a "$APP" --strategy immediate --wait-timeout 300
done

# Restore fly.toml to node-1 default
gen_fly_toml "ultradag-node-1" "ams"

echo ""
echo "=========================================="
echo "  UltraDAG Testnet Deployed Successfully  "
echo "=========================================="
echo ""
echo "Status URLs:"
for APP in "${APPS[@]}"; do
  echo "  https://$APP.fly.dev/status"
done
echo ""
echo "Logs:"
for APP in "${APPS[@]}"; do
  echo "  flyctl logs -a $APP"
done
echo ""
echo "Seed: $SEED_ADDR"
echo "Validators: $VALIDATORS"
