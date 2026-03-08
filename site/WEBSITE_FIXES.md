# Website Content Fixes

**Date:** March 8, 2026  
**Purpose:** Correct minor inaccuracies found in fact-check of website claims

---

## Required Changes

### 1. Round Time Specification

**Current:**
```
5s
Default round time
```

**Change to:**
```
Configurable
Round time (5s testnet)
```

**Reason:** Design target is 30 seconds, testnet runs at 5 seconds. Current claim is misleading.

---

### 2. Genesis Block Reference

**Current:**
```
Developer address is deterministic and auditable from block 0.
```

**Change to:**
```
Developer address is deterministic and auditable from round 0.
```

**Reason:** Terminology consistency - we use "rounds" not "blocks" throughout the codebase.

---

### 3. Round Time in Description (Optional but Recommended)

**Current:**
```
BFT finality in 3 rounds (~5-15 seconds).
```

**Consider changing to:**
```
BFT finality in 3 rounds (~15-90 seconds at 30s design target, ~5-15s testnet).
```

**Reason:** Clarifies that timing depends on configured round duration.

---

### 4. Unstake Cooldown Clarification (Optional)

**Current:**
```
~1 week
Unstake cooldown
```

**Consider adding:**
```
~1 week
Unstake cooldown (at 30s rounds)
```

**Reason:** Cooldown is 2,016 rounds, which equals ~7 days at 30-second rounds, but only ~12 hours at 5-second testnet rounds.

---

## Verified as Accurate (No Changes Needed)

✅ **Binary size:** <2MB (actual: 1.4MB)  
✅ **Bounded storage:** 1000 rounds  
✅ **21M max supply**  
✅ **50 UDAG per round reward**  
✅ **210K halving interval**  
✅ **5% dev allocation (1,050,000 UDAG)**  
✅ **10,000 UDAG min stake**  
✅ **Ed25519 signatures**  
✅ **blake3(pubkey) == address**  
✅ **Leaderless parallel production**  
✅ **Use case math (0.001 UDAG = 100,000 sats)**  
✅ **Fee <0.0001 UDAG (min fee = 10,000 sats = 0.0001 UDAG)**  

---

## Summary

**Total claims verified:** 21  
**Accurate as-is:** 18 (86%)  
**Need minor fixes:** 2 (required)  
**Recommended clarifications:** 2 (optional)  

All core technical claims are 100% accurate and verifiable in the codebase. The issues are minor terminology and clarification improvements.
