#!/bin/bash
# Deploy UltraDAG nodes 5-9 across different continents

set -e

echo "🌍 Deploying UltraDAG nodes globally..."
echo "========================================"
echo ""

# Node 5 - Tokyo, Japan (Asia)
echo "📍 Deploying Node 5 to Tokyo (nrt)..."
fly launch --config fly-node-5.toml --no-deploy --name ultradag-node-5 --region nrt || true
fly deploy --config fly-node-5.toml --ha=false
echo "✅ Node 5 deployed to Tokyo"
echo ""

# Node 6 - São Paulo, Brazil (South America)
echo "📍 Deploying Node 6 to São Paulo (gru)..."
fly launch --config fly-node-6.toml --no-deploy --name ultradag-node-6 --region gru || true
fly deploy --config fly-node-6.toml --ha=false
echo "✅ Node 6 deployed to São Paulo"
echo ""

# Node 7 - Sydney, Australia (Oceania)
echo "📍 Deploying Node 7 to Sydney (syd)..."
fly launch --config fly-node-7.toml --no-deploy --name ultradag-node-7 --region syd || true
fly deploy --config fly-node-7.toml --ha=false
echo "✅ Node 7 deployed to Sydney"
echo ""

# Node 8 - Johannesburg, South Africa (Africa)
echo "📍 Deploying Node 8 to Johannesburg (jnb)..."
fly launch --config fly-node-8.toml --no-deploy --name ultradag-node-8 --region jnb || true
fly deploy --config fly-node-8.toml --ha=false
echo "✅ Node 8 deployed to Johannesburg"
echo ""

# Node 9 - Seattle, USA (North America West)
echo "📍 Deploying Node 9 to Seattle (sea)..."
fly launch --config fly-node-9.toml --no-deploy --name ultradag-node-9 --region sea || true
fly deploy --config fly-node-9.toml --ha=false
echo "✅ Node 9 deployed to Seattle"
echo ""

echo "========================================"
echo "🎉 All 5 new nodes deployed!"
echo ""
echo "Global network coverage:"
echo "  • Europe: Amsterdam (nodes 1-4)"
echo "  • Asia: Tokyo (node 5)"
echo "  • South America: São Paulo (node 6)"
echo "  • Oceania: Sydney (node 7)"
echo "  • Africa: Johannesburg (node 8)"
echo "  • North America: Seattle (node 9)"
echo "  • VPS: Netherlands (84.247.10.2)"
echo ""
echo "Total validators: 10 (9 Fly.io + 1 VPS)"
echo ""
echo "Checking node status..."
for i in 5 6 7 8 9; do
    echo -n "Node $i: "
    fly status -a ultradag-node-$i | grep "STATE" | head -1 || echo "checking..."
done
