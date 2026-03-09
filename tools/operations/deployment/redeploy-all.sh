#!/usr/bin/env bash
set -euo pipefail

# Redeploy all 4 UltraDAG nodes with new code
# This script builds the release binary and deploys to all Fly.io nodes

PROJECT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
cd "$PROJECT_DIR"

# Set Fly.io token
export FLY_API_TOKEN="FlyV1 fm2_lJPECAAAAAAACF4CxBALWiHG4Gt7uR26M+mFlRmwwrVodHRwczovL2FwaS5mbHkuaW8vdjGUAJLOAA1/5B8Lk7lodHRwczovL2FwaS5mbHkuaW8vYWFhL3YxxDw27fOPGr9orsDIlVin0jyDbyvCHcgAWi4+fdnTZgRe/0SCsEBknwPRodCMLm7ydWhdoJFGjr7+oJb9zR3ETpfJErfeFNECQ5Od20dgGmrHp5Tvdd03sLmQkzo5lczXY2spU6a1HSB4KVTr5DNbeu1uvywAMmVnBkYcOGFOb0CCz0mYfDRMAZGpv9xPrMQg5PWAb+17uRTo2T7mbU3pyqXGgTVpUCiyAVUtsI2ct7A=,fm2_lJPETpfJErfeFNECQ5Od20dgGmrHp5Tvdd03sLmQkzo5lczXY2spU6a1HSB4KVTr5DNbeu1uvywAMmVnBkYcOGFOb0CCz0mYfDRMAZGpv9xPrMQQFTqHw6zg8DKKgC/nn6FAIsO5aHR0cHM6Ly9hcGkuZmx5LmlvL2FhYS92MZgEks5pq+NJzwAAAAElpAFnF84ADRLmCpHOAA0S5gzEEA4EQC5ivNmjUNsWyLct3nTEILAGVdvZetlULjEjEC3Qiai0MVI8cMQyUCtZBsS0cmMG"

APPS=("ultradag-node-1" "ultradag-node-2" "ultradag-node-3" "ultradag-node-4")

echo "=========================================="
echo "  UltraDAG Testnet Redeployment"
echo "=========================================="
echo ""
echo "This will redeploy all 4 nodes with:"
echo "  - Unified Transaction enum (stake/unstake propagation fix)"
echo "  - Clean state (CLEAN_STATE=true)"
echo ""

# Build release binary
echo "=== Building release binary ==="
cargo build --release
echo "✓ Build complete"
echo ""

# Deploy to all nodes
for APP in "${APPS[@]}"; do
  echo "=== Deploying $APP ==="
  
  # Set CLEAN_STATE=true for fresh start with new transaction format
  echo "  Setting CLEAN_STATE=true..."
  flyctl secrets set CLEAN_STATE=true -a "$APP" 2>/dev/null || true
  
  # Deploy
  echo "  Deploying..."
  flyctl deploy -a "$APP" --strategy immediate --wait-timeout 300
  
  echo "✓ $APP deployed"
  echo ""
done

echo "=========================================="
echo "  All nodes deployed successfully!"
echo "=========================================="
echo ""
echo "Waiting 30 seconds for nodes to start..."
sleep 30

echo ""
echo "=== Node Status ==="
for APP in "${APPS[@]}"; do
  echo "Checking $APP..."
  STATUS=$(curl -s "https://$APP.fly.dev/status" | jq -r '.dag_round // "error"')
  echo "  DAG round: $STATUS"
done

echo ""
echo "=== Next Steps ==="
echo "1. Monitor logs: flyctl logs -a ultradag-node-1"
echo "2. Check status: curl https://ultradag-node-1.fly.dev/status | jq"
echo "3. Test stake propagation:"
echo "   curl -X POST https://ultradag-node-1.fly.dev/stake \\"
echo "     -H 'Content-Type: application/json' \\"
echo "     -d '{\"secret_key\":\"YOUR_KEY\",\"amount\":1000000000000}'"
echo "4. Verify on all nodes:"
echo "   curl https://ultradag-node-{1,2,3,4}.fly.dev/stake/YOUR_ADDRESS"
echo ""
