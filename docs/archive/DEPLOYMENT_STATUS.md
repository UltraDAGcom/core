# Deployment Status — March 7, 2026

## ✅ Deployment Complete

All 4 nodes successfully redeployed with unified Transaction enum (staking fix):
- **ultradag-node-1** (ams) - Deployed ✅
- **ultradag-node-2** (fra) - Deployed ✅
- **ultradag-node-3** (lhr) - Deployed ✅
- **ultradag-node-4** (sin) - Deployed ✅

**Build:** Release binary with staking propagation fix
**State:** Clean state (CLEAN_STATE=true) - fresh start with new transaction format
**Image size:** 29 MB per node

---

## ⚠️ Current Issue: P2P Connectivity

**Symptoms:**
- Nodes are producing vertices (DAG rounds advancing)
- No finalization happening (`last_finalized_round: null`)
- Peers not connecting (`peers_connected: 0`)
- Node 1 showing "Broken pipe" errors in logs

**Current Status (as of 13:31 UTC):**
```
Node 1: Not responding to HTTP (load balancer errors)
Node 2: DAG round 80, no finalization, 0 peers
Node 3: DAG round 59, no finalization, 0 peers
Node 4: DAG round 42, no finalization, 0 peers
```

**Root Cause:**
After clean state deployment, nodes need to rediscover each other via:
1. Seed node (node-1) needs to be accessible
2. Other nodes need SEED environment variable pointing to node-1
3. P2P connections need to establish on port 9333

---

## 🔧 Recommended Fix

### Option 1: Restart Node 1 (Quick Fix)

Node 1 appears to be in a bad state. Restart it:

```bash
export FLY_API_TOKEN="<token>"
flyctl apps restart ultradag-node-1
```

Wait 30 seconds, then check if other nodes connect.

### Option 2: Verify SEED Configuration

Check if nodes 2-4 have the correct SEED environment variable:

```bash
flyctl secrets list -a ultradag-node-2
flyctl secrets list -a ultradag-node-3
flyctl secrets list -a ultradag-node-4
```

Should show: `SEED=<node-1-ip>:9333`

If missing, get node-1 IP and set:

```bash
NODE1_IP=$(flyctl ips list -a ultradag-node-1 | grep "v4" | awk '{print $2}' | head -1)
flyctl secrets set SEED="${NODE1_IP}:9333" -a ultradag-node-2
flyctl secrets set SEED="${NODE1_IP}:9333" -a ultradag-node-3
flyctl secrets set SEED="${NODE1_IP}:9333" -a ultradag-node-4
```

### Option 3: Full Redeploy with Proper Seed Setup

Use the original `fly-deploy.sh` script which handles seed node setup correctly:

```bash
cd /Users/johan/Projects/15_UltraDAG
./scripts/fly-deploy.sh
```

This will:
1. Deploy node-1 first
2. Get its IP address
3. Set SEED env var on nodes 2-4
4. Deploy nodes 2-4 with seed configuration

---

## 📊 What to Expect After Fix

Once P2P connectivity is restored:

1. **Within 10 seconds:**
   - Nodes discover each other
   - Peer count increases to 3 per node
   - Vertices start propagating

2. **Within 30 seconds:**
   - Finalization begins
   - `last_finalized_round` starts advancing
   - Finality lag stabilizes at ~3 rounds

3. **Healthy State:**
   ```json
   {
     "dag_round": 150,
     "last_finalized_round": 147,
     "finality_lag": 3,
     "peers_connected": 3,
     "validator_count": 4
   }
   ```

---

## 🧪 Testing Stake Propagation (After P2P Fix)

Once nodes are syncing properly, test the staking fix:

### 1. Generate Test Key

```bash
# Generate a test keypair
SECRET_KEY="0000000000000000000000000000000000000000000000000000000000000001"
# Address: derived from blake3(ed25519_pubkey)
```

### 2. Fund Address via Faucet

```bash
curl -X POST https://ultradag-node-1.fly.dev/faucet \
  -H 'Content-Type: application/json' \
  -d '{"address":"YOUR_ADDRESS","amount":2000000000000}'
```

### 3. Submit Stake Transaction

```bash
curl -X POST https://ultradag-node-1.fly.dev/stake \
  -H 'Content-Type: application/json' \
  -d "{\"secret_key\":\"$SECRET_KEY\",\"amount\":1000000000000}"
```

Expected response:
```json
{
  "status": "pending",
  "tx_hash": "...",
  "address": "...",
  "amount": 1000000000000,
  "amount_udag": 10000.0,
  "nonce": 1,
  "note": "Stake transaction added to mempool. Will be applied when included in a finalized vertex."
}
```

### 4. Verify Propagation

**Check mempool on all nodes (within 1 second):**
```bash
for i in 1 2 3 4; do
  echo "=== Node $i mempool ==="
  curl -s "https://ultradag-node-$i.fly.dev/mempool" | jq '.[] | select(.type=="stake")'
done
```

Expected: Stake transaction appears in all 4 mempools

**Check stake state after finalization (within 10 seconds):**
```bash
for i in 1 2 3 4; do
  echo "=== Node $i stake state ==="
  curl -s "https://ultradag-node-$i.fly.dev/stake/YOUR_ADDRESS" | jq
done
```

Expected: All nodes show:
```json
{
  "address": "...",
  "staked": 1000000000000,
  "staked_udag": 10000.0,
  "active": true,
  "unlock_at_round": null
}
```

---

## 📝 Summary

**Deployment:** ✅ Complete - All 4 nodes running new code with staking fix
**P2P Network:** ⚠️ Issue - Nodes not connecting, need to fix seed configuration
**Next Step:** Restart node-1 or reconfigure SEED environment variables

**The staking propagation fix is deployed and ready to test once P2P connectivity is restored.**

---

## Quick Commands

```bash
# Set Fly.io token
export FLY_API_TOKEN="FlyV1 fm2_lJPECAAAAAAACF4CxBALWiHG4Gt7uR26M+mFlRmwwrVodHRwczovL2FwaS5mbHkuaW8vdjGUAJLOAA1/5B8Lk7lodHRwczovL2FwaS5mbHkuaW8vYWFhL3YxxDw27fOPGr9orsDIlVin0jyDbyvCHcgAWi4+fdnTZgRe/0SCsEBknwPRodCMLm7ydWhdoJFGjr7+oJb9zR3ETpfJErfeFNECQ5Od20dgGmrHp5Tvdd03sLmQkzo5lczXY2spU6a1HSB4KVTr5DNbeu1uvywAMmVnBkYcOGFOb0CCz0mYfDRMAZGpv9xPrMQg5PWAb+17uRTo2T7mbU3pyqXGgTVpUCiyAVUtsI2ct7A=,fm2_lJPETpfJErfeFNECQ5Od20dgGmrHp5Tvdd03sLmQkzo5lczXY2spU6a1HSB4KVTr5DNbeu1uvywAMmVnBkYcOGFOb0CCz0mYfDRMAZGpv9xPrMQQFTqHw6zg8DKKgC/nn6FAIsO5aHR0cHM6Ly9hcGkuZmx5LmlvL2FhYS92MZgEks5pq+NJzwAAAAElpAFnF84ADRLmCpHOAA0S5gzEEA4EQC5ivNmjUNsWyLct3nTEILAGVdvZetlULjEjEC3Qiai0MVI8cMQyUCtZBsS0cmMG"

# Check status
for i in 1 2 3 4; do echo "Node $i:" && curl -s "https://ultradag-node-$i.fly.dev/status" | jq '{dag_round, last_finalized_round, peers}'; done

# Restart node 1
flyctl apps restart ultradag-node-1

# Check logs
flyctl logs -a ultradag-node-1 -n

# Full redeploy with proper seed setup
./scripts/fly-deploy.sh
```
