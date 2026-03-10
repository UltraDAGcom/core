# Fly.io Network Configuration for UltraDAG

**Last Updated:** March 10, 2026

---

## Overview

UltraDAG nodes deployed on Fly.io use **internal WireGuard mesh networking** (`.internal` DNS) instead of public bootstrap nodes. This is the recommended and expected configuration for Fly.io deployments.

---

## Network Architecture

### Internal Mesh Network

Fly.io provides a private WireGuard network that connects all apps within the same organization. UltraDAG validators use this for peer-to-peer communication:

```
┌─────────────────────────────────────────────────────┐
│           Fly.io WireGuard Mesh Network             │
│                                                     │
│  ultradag-node-1.internal ←→ ultradag-node-2.internal │
│         ↕                           ↕               │
│  ultradag-node-3.internal ←→ ultradag-node-4.internal │
│                                                     │
└─────────────────────────────────────────────────────┘
```

**Advantages:**
- ✅ **Reliable:** No TCP proxy interference
- ✅ **Low Latency:** Direct WireGuard connections
- ✅ **Secure:** Private network, not exposed to internet
- ✅ **Stable:** No "early eof" or "Connection reset by peer" errors

### Public Bootstrap Nodes

Public bootstrap nodes are configured but **not connected** when using internal mesh:

```json
{
  "bootstrap_nodes": [
    {"addr": "206.51.242.223:9333", "connected": false},
    {"addr": "137.66.57.226:9333", "connected": false},
    {"addr": "169.155.54.169:9333", "connected": false},
    {"addr": "169.155.55.151:9333", "connected": false}
  ]
}
```

**This is expected behavior** - the nodes use `.internal` DNS instead.

---

## Configuration

### Seed Peers (fly-node-N.toml)

Each validator is configured with internal DNS names as seed peers:

```toml
# fly-node-1.toml
[env]
SEED_1 = "ultradag-node-2.internal:9333"
SEED_2 = "ultradag-node-3.internal:9333"
SEED_3 = "ultradag-node-4.internal:9333"
```

### Command Line

The equivalent command-line configuration:

```bash
ultradag-node \
  --port 9333 \
  --seed ultradag-node-2.internal:9333 \
  --seed ultradag-node-3.internal:9333 \
  --seed ultradag-node-4.internal:9333
```

---

## Why Not Use Public IPs?

### Previous Issues with Dedicated IPv4

Early deployments attempted to use Fly.io dedicated IPv4 addresses:

```toml
# ❌ OLD CONFIGURATION (caused issues)
SEED_1 = "137.66.57.226:9333"
SEED_2 = "169.155.54.169:9333"
```

**Problems Encountered:**
- TCP proxy kills persistent connections
- "early eof" errors
- "Connection reset by peer" errors
- Unstable peer connectivity

### Solution: Internal DNS

```toml
# ✅ CURRENT CONFIGURATION (stable)
SEED_1 = "ultradag-node-2.internal:9333"
SEED_2 = "ultradag-node-3.internal:9333"
```

**Benefits:**
- Direct WireGuard connections (no TCP proxy)
- Stable, long-lived connections
- Lower latency
- No connection resets

---

## Monitoring

### Expected /peers Response

```json
{
  "connected": 3,
  "peers": [
    "ultradag-node-2.internal:9333",
    "[fdaa:12:2aca:a7b:331:94d9:9e0c:2]:35046",
    "ultradag-node-3.internal:9333"
  ],
  "bootstrap_nodes": [
    {"addr": "206.51.242.223:9333", "connected": false},
    {"addr": "137.66.57.226:9333", "connected": false},
    {"addr": "169.155.54.169:9333", "connected": false},
    {"addr": "169.155.55.151:9333", "connected": false}
  ]
}
```

**What to Check:**
- ✅ `connected` should be 3 (for 4-validator network)
- ✅ `peers` should contain `.internal` DNS names
- ✅ `bootstrap_connected: false` is **expected and correct**

### Health Indicators

**Healthy Network:**
```json
{
  "peer_count": 3,
  "bootstrap_connected": false  // ← This is CORRECT for Fly.io
}
```

**Unhealthy Network:**
```json
{
  "peer_count": 0,  // ← Problem: no peers connected
  "bootstrap_connected": false
}
```

---

## Troubleshooting

### No Peer Connections

**Symptom:** `peer_count: 0`

**Diagnosis:**
```bash
# Check if internal DNS resolves
fly ssh console -a ultradag-node-1
nslookup ultradag-node-2.internal
```

**Solutions:**
1. Verify all nodes are in same Fly.io organization
2. Check that WireGuard network is enabled
3. Restart nodes to re-establish connections
4. Verify firewall rules allow port 9333

### IPv6 Connections

**Symptom:** Peers show IPv6 addresses like `[fdaa:12:2aca:a7b:...]`

**Explanation:** This is normal - Fly.io uses IPv6 for WireGuard mesh. Both IPv4 `.internal` DNS and direct IPv6 connections work.

**No Action Required** ✅

### Bootstrap Nodes Not Connected

**Symptom:** All bootstrap nodes show `connected: false`

**Explanation:** This is **expected behavior** for Fly.io deployments using internal mesh.

**No Action Required** ✅

---

## Deployment Checklist

When deploying UltraDAG validators on Fly.io:

- [ ] Use `.internal` DNS for seed peers
- [ ] Do NOT use dedicated IPv4 addresses for seeds
- [ ] Verify all nodes are in same organization
- [ ] Check peer count is N-1 (where N = total validators)
- [ ] Confirm `bootstrap_connected: false` (this is correct)
- [ ] Monitor for `.internal` DNS names in peer list
- [ ] Accept IPv6 addresses in peer list (normal)

---

## Migration from Public IPs

If migrating from public IP configuration:

**1. Update fly-node-N.toml:**
```diff
  [env]
- SEED_1 = "137.66.57.226:9333"
+ SEED_1 = "ultradag-node-2.internal:9333"
```

**2. Deploy updated configuration:**
```bash
fly deploy -a ultradag-node-1
```

**3. Verify connections:**
```bash
curl https://ultradag-node-1.fly.dev/peers | jq '.peers'
```

**4. Confirm stability:**
- Monitor for 1 hour
- Check for connection resets
- Verify finality lag remains low

---

## External Node Connectivity

### Connecting External Nodes to Fly.io Network

External nodes (not on Fly.io) can connect via public IPs:

```bash
# External node configuration
ultradag-node \
  --seed 206.51.242.223:9333 \
  --seed 137.66.57.226:9333
```

**Note:** External nodes will connect via TCP proxy, which may be less stable than internal mesh.

### Hybrid Network

```
┌──────────────────────────────────────┐
│      Fly.io Internal Mesh            │
│  node-1 ←→ node-2 ←→ node-3 ←→ node-4│
└──────────────┬───────────────────────┘
               │ (public IP)
               ↓
         External Node
       (via TCP proxy)
```

---

## Best Practices

### For Fly.io Deployments

1. ✅ **Always use `.internal` DNS** for seed peers
2. ✅ **Accept `bootstrap_connected: false`** as normal
3. ✅ **Monitor peer count** (should be N-1)
4. ✅ **Use WireGuard mesh** for all validator communication
5. ✅ **Keep nodes in same organization**

### For Mixed Deployments

1. ⚠️ **Fly.io nodes:** Use `.internal` DNS for each other
2. ⚠️ **External nodes:** Use public IPs to connect to Fly.io
3. ⚠️ **Monitor stability:** TCP proxy may cause issues
4. ⚠️ **Consider all-Fly.io or all-external** for best stability

---

## References

- [Fly.io Private Networking](https://fly.io/docs/reference/private-networking/)
- [UltraDAG Node Operator Guide](../guides/operations/node-operator-guide.md)
- [UltraDAG Testnet Report](../../TESTNET_REPORT.md)

---

**Summary:** For Fly.io deployments, `bootstrap_connected: false` is **expected and correct**. The nodes use internal WireGuard mesh (`.internal` DNS) which is more stable and reliable than public IP connections.
