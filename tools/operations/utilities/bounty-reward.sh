#!/bin/bash
# Bug bounty reward distribution script
# Usage: ./bounty_reward.sh <hunter_address> <amount> <severity> <description>

set -e

if [ $# -lt 4 ]; then
    echo "Usage: $0 <hunter_address> <amount> <severity> <description>"
    echo "Example: $0 udag1abc123... 5000 High \"DoS vulnerability in RPC\""
    exit 1
fi

HUNTER_ADDRESS=$1
AMOUNT=$2
SEVERITY=$3
DESCRIPTION=$4

# Validate severity
if [[ ! "$SEVERITY" =~ ^(Critical|High|Medium|Low)$ ]]; then
    echo "Error: Severity must be Critical, High, Medium, or Low"
    exit 1
fi

# Get next bounty ID
YEAR=$(date +%Y)
LAST_ID=$(grep -E "^ID: BB-${YEAR}-" ../BOUNTY_LEDGER.md 2>/dev/null | tail -1 | sed 's/ID: BB-'${YEAR}'-//' || echo "0000")
NEXT_NUM=$(printf "%04d" $((10#$LAST_ID + 1)))
BOUNTY_ID="BB-${YEAR}-${NEXT_NUM}"

echo "=== UltraDAG Bug Bounty Reward ==="
echo "Bounty ID: $BOUNTY_ID"
echo "Hunter: $HUNTER_ADDRESS"
echo "Amount: $AMOUNT UDAG (mainnet promise)"
echo "Severity: $SEVERITY"
echo "Description: $DESCRIPTION"
echo ""

# Send testnet UDAG
echo "Sending testnet UDAG to hunter..."
RESPONSE=$(curl -s -X POST "https://ultradag-node-1.fly.dev/faucet" \
    -H "Content-Type: application/json" \
    -d "{\"address\":\"$HUNTER_ADDRESS\",\"amount\":$AMOUNT}")

if echo "$RESPONSE" | grep -q "error"; then
    echo "Warning: Testnet distribution may have failed: $RESPONSE"
    echo "Manual distribution may be required."
else
    echo "✅ Testnet UDAG sent successfully"
fi

# Add entry to bounty ledger
echo ""
echo "Adding entry to BOUNTY_LEDGER.md..."

ENTRY="
---

### $BOUNTY_ID

**Date:** $(date +%Y-%m-%d)  
**Hunter:** $HUNTER_ADDRESS  
**Severity:** $SEVERITY  
**Reward:** $(printf "%'d" $AMOUNT) UDAG (mainnet promise)  
**Testnet Paid:** $(printf "%'d" $AMOUNT) UDAG  
**Issue:** $DESCRIPTION  
**Status:** Paid (Testnet)  
**Timestamp:** $(date -u +%Y-%m-%dT%H:%M:%SZ)  

*Mainnet conversion: 25% at launch, 75% vested over 12 months*
"

# Insert after "## Active Bounties" section
sed -i.bak '/## Active Bounties/a\
'"$ENTRY"'
' ../BOUNTY_LEDGER.md

# Update statistics
TOTAL_AWARDED=$(grep -E "^\*\*Reward:\*\*" ../BOUNTY_LEDGER.md | sed 's/[^0-9]//g' | awk '{sum+=$1} END {print sum}')
sed -i.bak "s/\*\*Total Awarded:\*\* [0-9,]* UDAG/**Total Awarded:** $(printf "%'d" $TOTAL_AWARDED) UDAG/" ../BOUNTY_LEDGER.md
sed -i.bak "s/\*\*Total Paid (Testnet):\*\* [0-9,]* UDAG/**Total Paid (Testnet):** $(printf "%'d" $TOTAL_AWARDED) UDAG/" ../BOUNTY_LEDGER.md

rm ../BOUNTY_LEDGER.md.bak 2>/dev/null || true

echo "✅ Bounty ledger updated"
echo ""
echo "Next steps:"
echo "1. Review the entry in BOUNTY_LEDGER.md"
echo "2. Commit the changes: git add ../BOUNTY_LEDGER.md && git commit -m 'Award bounty $BOUNTY_ID'"
echo "3. (Optional) GPG sign the commit: git commit --amend -S"
echo "4. Notify the hunter via GitHub"
echo ""
echo "Bounty $BOUNTY_ID awarded successfully!"
