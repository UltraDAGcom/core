#!/bin/bash
# Check VPS node status

VPS="84.247.10.2"
VPS_USER="johanmichel"

echo "🔍 Checking UltraDAG node on VPS..."
echo "===================================="
echo ""

# Check if node is responding via HTTP
echo "1. Checking HTTP API (port 10333)..."
STATUS=$(curl -s --max-time 5 "http://$VPS:10333/status" 2>/dev/null)

if [ -n "$STATUS" ]; then
    echo "✅ Node is responding!"
    echo ""
    echo "$STATUS" | jq '.'
    echo ""
    
    ROUND=$(echo "$STATUS" | jq -r '.dag_round')
    SUPPLY=$(echo "$STATUS" | jq -r '.total_supply')
    PEERS=$(echo "$STATUS" | jq -r '.peer_count')
    
    echo "Summary:"
    echo "  Round: $ROUND"
    echo "  Supply: $SUPPLY sats"
    echo "  Peers: $PEERS"
else
    echo "❌ Node is not responding on HTTP"
    echo ""
    echo "2. Checking systemd service status..."
    echo "   (You'll need to enter VPS password)"
    echo ""
    
    ssh "$VPS_USER@$VPS" "sudo systemctl status ultradag --no-pager -l"
    
    echo ""
    echo "3. Checking recent logs..."
    ssh "$VPS_USER@$VPS" "sudo journalctl -u ultradag -n 50 --no-pager"
fi
