# UltraDAG Adversarial Test Results

**Date**: 2026-03-06T17:52:40Z
**Target**: Fly.io testnet (4 nodes, ams region)
**Results**: 30 PASS / 2 FAIL / 3 SKIP / 35 total

## Summary Table

| Test | Result | Details |
|------|--------|---------|
| A1-finalized-round-agreement | PASS | rounds=[16 16 16 16] spread=0 (<=1) |
| A2-supply-agreement | PASS | supplies=[100035000000000 100035000000000 100035000000000 100035000000000] spread=0 sats (<= 20000000000) |
| A3-validator-count | PASS | validator_count=4 |
| A4-dag-round-liveness | PASS | round 19 -> 27 (+8 in 30s, expected >=4) |
| A5-finality-liveness | PASS | finalized 16 -> 24 (+8 in 30s, expected >=3) |
| A6-finality-lag-bounded | PASS | dag_round=27 finalized=24 lag=3 (<=5) |
| B1-peer-count | PASS | node1=13 node2=11 node3=13 node4=11 (all >= 3) |
| B2-bootstrap-visibility | PASS | connected=13 peers visible |
| B3-cross-node-tx-propagation | FAIL | tx on node-3, balance=0 on node-2 (expected >0) |
| B4-mempool-propagation | SKIP | Faucet balance not available (sender_bal=0) |
| C1-insufficient-balance | PASS | error='insufficient balance: need 1000000000100000 sats (incl. 0 pending), have 0 sats (0.0000 UDAG)' |
| C2-invalid-address | PASS | error='from_secret must be 64 hex chars (32 bytes)' |
| C3-invalid-secret-key | PASS | error='invalid hex in from_secret' |
| C4-nonce-increment | SKIP | Faucet balance not available (balance=) |
| C5-faucet-propagation | PASS | Faucet tx propagated: node1= node2= node3= node4=  |
| D1-duplicate-tx-rejected | SKIP | Faucet balance not available |
| D2-mempool-drain | FAIL | mempool_size= (unbounded growth?) |
| E1-balance-update | PASS | receiver=1000000000 sender=98999900000 (expected ~98999900000) |
| E2-account-count | PASS | account_count=18 (>= 1) |
| E3-supply-positive | PASS | total_supply=100250000000000 sats (1002500.00 UDAG) |
| E4-account-count-agreement | PASS | accounts=[18 18 18 18] spread=0 (<=5) |
| F1-rpc-latency | PASS | latency=376ms (<2000ms) |
| F2-burst-submission | PASS | accepted=50 rejected=0 elapsed=18172ms rate=2.8 tx/s |
| F3-burst-finalization | PASS | receiver balance=5000000 sats (>= 4000000 expected) |
| G1-zero-amount-tx | PASS | zero-amount handled: {
  "hash": "ea0d061ea3565df9d1b4975dada859d2ec5a78cd23c07e6010f408725ada4b52",
  "from": "579c0f6e0 |
| G2-self-send | PASS | self-send handled: {
  "hash": "51871f1ae7f9a08e38630b318805f892b4f751857ab090a722ad98fc11af9271",
  "from": "579c0f6e0 |
| G3-empty-faucet-body | PASS | empty body rejected |
| G4-invalid-json-tx | PASS | invalid JSON rejected |
| G5-404-endpoint | PASS | 404 returned for /nonexistent |
| G6-cors-headers | PASS | CORS headers present |
| H1-dag-vertices | PASS | node1=99 node2=99 node3=99 node4=99 (all >= 10) |
| H2-dag-tips | PASS | node1=1 node2=1 node3=1 node4=1 (all >= 1) |
| H3-finalized-count | PASS | node1=97 node2=97 node3=97 node4=97  |
| H4-round-endpoint | PASS | round 96 has 1 vertices |
| H5-keygen-valid | PASS | secret_key=64 chars, address=64 chars |

## Categories

- **A: Consensus Safety** — Finalized round agreement, supply consistency, liveness, finality lag
- **B: Network & Peer Connectivity** — Peer mesh, bootstrap, cross-node propagation, mempool gossip
- **C: Transaction Validity** — Balance checks, address validation, nonce sequencing, faucet propagation
- **D: Mempool** — Duplicate handling, drain after finalization
- **E: State Machine** — Balance updates, account tracking, supply growth, cross-node agreement
- **F: Performance Under Stress** — RPC latency, burst submission, finalization throughput
- **G: Protocol Edge Cases** — Zero amount, self-send, malformed input, CORS, 404 handling
- **H: Bootstrap and Sync** — DAG vertices, tips, finalized count, round endpoint, keygen

## Verdict

**MOSTLY PASSING** — 2 test(s) failed. Review details above.
