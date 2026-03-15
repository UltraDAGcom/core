# UltraDAG Integration Guide

**Version:** 1.0  
**Last Updated:** March 2026  
**Target Audience:** Application developers, wallet developers, exchange integrators

---

## Table of Contents

1. [Overview](#overview)
2. [Quick Start](#quick-start)
3. [Wallet Integration](#wallet-integration)
4. [Exchange Integration](#exchange-integration)
5. [DApp Development](#dapp-development)
6. [Testing](#testing)
7. [Production Deployment](#production-deployment)
8. [Best Practices](#best-practices)
9. [Code Examples](#code-examples)

---

## Overview

This guide helps developers integrate UltraDAG into applications, wallets, exchanges, and decentralized applications (DApps). It covers everything from basic transaction submission to advanced features like staking and governance.

**What You'll Learn:**
- Connect to UltraDAG nodes
- Create and sign transactions
- Query balances and state
- Handle confirmations
- Implement wallet functionality
- Integrate with exchanges

**Prerequisites:**
- Basic understanding of blockchain concepts
- Familiarity with Ed25519 cryptography
- Programming experience (JavaScript, Python, or Rust)

---

## Quick Start

### 1. Connect to a Node

**Testnet RPC Endpoint:**
```
http://testnet.ultradag.io:10333
```

**Local Node:**
```
http://localhost:10333
```

**Test Connection:**
```bash
curl http://localhost:10333/health
# {"status":"ok"}
```

### 2. Generate a Wallet

**Using RPC (testing only):**
```bash
curl http://localhost:10333/keygen
```

**Response:**
```json
{
  "secret_key": "abc123...",
  "public_key": "def456...",
  "address": "789xyz..."
}
```

**⚠️ Production:** Generate keys offline using a secure library.

### 3. Get Testnet Tokens

```bash
curl -X POST http://localhost:10333/faucet \
  -H "Content-Type: application/json" \
  -d '{"address": "YOUR_ADDRESS"}'
```

### 4. Check Balance

```bash
curl http://localhost:10333/balance/YOUR_ADDRESS
```

**Response:**
```json
{
  "address": "YOUR_ADDRESS",
  "balance": 100000000,
  "nonce": 0
}
```

### 5. Send a Transaction

See [Wallet Integration](#wallet-integration) for complete transaction signing.

---

## Wallet Integration

### Key Management

**Generate Keys (JavaScript):**
```javascript
const nacl = require('tweetnacl');
const blake3 = require('blake3');

// Generate keypair
const keypair = nacl.sign.keyPair();
const secretKey = keypair.secretKey;
const publicKey = keypair.publicKey;

// Derive address
const address = blake3.hash(publicKey);

console.log({
  secretKey: Buffer.from(secretKey).toString('hex'),
  publicKey: Buffer.from(publicKey).toString('hex'),
  address: Buffer.from(address).toString('hex')
});
```

**Generate Keys (Python):**
```python
from nacl.signing import SigningKey
from hashlib import blake2b

# Generate keypair
signing_key = SigningKey.generate()
verify_key = signing_key.verify_key

# Derive address (using blake3 library)
import blake3
address = blake3.blake3(bytes(verify_key)).digest()

print({
    'secret_key': signing_key.encode().hex(),
    'public_key': bytes(verify_key).hex(),
    'address': address.hex()
})
```

**Store Keys Securely:**
- Encrypt with user password (AES-256-GCM)
- Use hardware wallets (Ledger, Trezor)
- Never store plaintext keys
- Implement key derivation (BIP39/BIP44 for HD wallets)

### Transaction Creation

**1. Fetch Current Nonce:**
```javascript
async function getCurrentNonce(address) {
  const response = await fetch(`http://localhost:10333/balance/${address}`);
  const data = await response.json();
  return data.nonce;
}
```

**2. Build Transaction:**
```javascript
function buildTransaction(from, to, amount, fee, nonce) {
  return {
    from: from,
    to: to,
    amount: amount,
    fee: fee,
    nonce: nonce
  };
}
```

**3. Sign Transaction:**
```javascript
const nacl = require('tweetnacl');

function signTransaction(tx, secretKey, publicKey) {
  // Build signable bytes
  const NETWORK_ID = Buffer.from('ultradag-testnet-v1', 'utf8');
  const from = Buffer.from(tx.from, 'hex');
  const to = Buffer.from(tx.to, 'hex');
  const amount = Buffer.alloc(8);
  amount.writeBigUInt64LE(BigInt(tx.amount));
  const fee = Buffer.alloc(8);
  fee.writeBigUInt64LE(BigInt(tx.fee));
  const nonce = Buffer.alloc(8);
  nonce.writeBigUInt64LE(BigInt(tx.nonce));
  
  const signable = Buffer.concat([NETWORK_ID, from, to, amount, fee, nonce]);
  
  // Sign
  const signature = nacl.sign.detached(signable, secretKey);
  
  return {
    ...tx,
    pub_key: Buffer.from(publicKey).toString('hex'),
    signature: Buffer.from(signature).toString('hex')
  };
}
```

**4. Submit Transaction:**
```javascript
async function submitTransaction(signedTx) {
  const response = await fetch('http://localhost:10333/tx', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(signedTx)
  });
  
  return await response.json();
}
```

**Complete Example:**
```javascript
async function sendUDAG(from, to, amount, secretKey, publicKey) {
  // 1. Get current nonce
  const nonce = await getCurrentNonce(from);
  
  // 2. Build transaction
  const tx = buildTransaction(from, to, amount, 10000, nonce);
  
  // 3. Sign transaction
  const signedTx = signTransaction(tx, secretKey, publicKey);
  
  // 4. Submit transaction
  const result = await submitTransaction(signedTx);
  
  console.log('Transaction submitted:', result);
  return result;
}
```

### Transaction Confirmation

**Poll for Confirmation:**
```javascript
async function waitForConfirmation(address, expectedNonce, timeout = 30000) {
  const startTime = Date.now();
  
  while (Date.now() - startTime < timeout) {
    const balance = await fetch(`http://localhost:10333/balance/${address}`);
    const data = await balance.json();
    
    if (data.nonce > expectedNonce) {
      return true; // Transaction confirmed
    }
    
    await new Promise(resolve => setTimeout(resolve, 1000));
  }
  
  return false; // Timeout
}

// Usage
const nonce = await getCurrentNonce(address);
await sendUDAG(from, to, amount, secretKey, publicKey);
const confirmed = await waitForConfirmation(address, nonce);
```

**Check Transaction in Mempool:**
```javascript
async function isInMempool(txHash) {
  const response = await fetch('http://localhost:10333/mempool');
  const txs = await response.json();
  return txs.some(tx => tx.hash === txHash);
}
```

### Balance Monitoring

**Real-time Balance Updates:**
```javascript
class BalanceMonitor {
  constructor(address, interval = 5000) {
    this.address = address;
    this.interval = interval;
    this.callbacks = [];
  }
  
  async start() {
    this.timer = setInterval(async () => {
      const response = await fetch(`http://localhost:10333/balance/${this.address}`);
      const data = await response.json();
      
      this.callbacks.forEach(cb => cb(data));
    }, this.interval);
  }
  
  stop() {
    clearInterval(this.timer);
  }
  
  onChange(callback) {
    this.callbacks.push(callback);
  }
}

// Usage
const monitor = new BalanceMonitor('YOUR_ADDRESS');
monitor.onChange(balance => {
  console.log('Balance updated:', balance);
});
monitor.start();
```

---

## Exchange Integration

### Deposit Handling

**1. Generate Deposit Addresses:**
```javascript
// Generate unique address per user
function generateDepositAddress(userId) {
  const keypair = nacl.sign.keyPair();
  const address = blake3.hash(keypair.publicKey);
  
  // Store in database
  db.saveDepositAddress(userId, {
    address: Buffer.from(address).toString('hex'),
    publicKey: Buffer.from(keypair.publicKey).toString('hex'),
    secretKey: Buffer.from(keypair.secretKey).toString('hex') // Encrypt!
  });
  
  return Buffer.from(address).toString('hex');
}
```

**2. Monitor Deposits:**
```javascript
class DepositMonitor {
  constructor(addresses) {
    this.addresses = addresses; // Array of addresses to monitor
    this.lastNonces = new Map();
  }
  
  async checkDeposits() {
    for (const address of this.addresses) {
      const response = await fetch(`http://localhost:10333/balance/${address}`);
      const data = await response.json();
      
      const lastNonce = this.lastNonces.get(address) || -1;
      
      if (data.nonce > lastNonce) {
        // New transaction(s) received
        const deposited = data.balance;
        await this.processDeposit(address, deposited);
        this.lastNonces.set(address, data.nonce);
      }
    }
  }
  
  async processDeposit(address, amount) {
    const userId = await db.getUserByAddress(address);
    await db.creditUser(userId, amount);
    console.log(`Credited ${amount} to user ${userId}`);
  }
  
  start(interval = 10000) {
    this.timer = setInterval(() => this.checkDeposits(), interval);
  }
  
  stop() {
    clearInterval(this.timer);
  }
}
```

**3. Confirmations:**

UltraDAG has fast finality (2-3 rounds, ~10-15 seconds):
- **1 confirmation:** Transaction in finalized vertex (safe for most uses)
- **3 confirmations:** Extra safety margin (recommended for large amounts)

```javascript
async function getConfirmations(address, txNonce) {
  const response = await fetch(`http://localhost:10333/balance/${address}`);
  const data = await response.json();
  return data.nonce - txNonce;
}
```

### Withdrawal Handling

**1. Validate Withdrawal Request:**
```javascript
function validateWithdrawal(userId, amount, destinationAddress) {
  // Check user balance
  const userBalance = db.getUserBalance(userId);
  if (userBalance < amount) {
    throw new Error('Insufficient balance');
  }
  
  // Validate destination address
  if (!/^[0-9a-f]{64}$/.test(destinationAddress)) {
    throw new Error('Invalid address format');
  }
  
  // Check minimum withdrawal
  const MIN_WITHDRAWAL = 1000000; // 0.01 UDAG
  if (amount < MIN_WITHDRAWAL) {
    throw new Error('Below minimum withdrawal');
  }
  
  return true;
}
```

**2. Process Withdrawal:**
```javascript
async function processWithdrawal(userId, amount, destinationAddress) {
  // Validate
  validateWithdrawal(userId, amount, destinationAddress);
  
  // Get hot wallet address
  const hotWallet = await db.getHotWallet();
  
  // Get nonce
  const nonce = await getCurrentNonce(hotWallet.address);
  
  // Build and sign transaction
  const tx = buildTransaction(
    hotWallet.address,
    destinationAddress,
    amount,
    10000, // fee
    nonce
  );
  
  const signedTx = signTransaction(tx, hotWallet.secretKey, hotWallet.publicKey);
  
  // Submit
  const result = await submitTransaction(signedTx);
  
  // Record in database
  await db.recordWithdrawal(userId, amount, destinationAddress, result.hash);
  
  // Debit user balance
  await db.debitUser(userId, amount);
  
  return result;
}
```

**3. Hot/Cold Wallet Management:**
```javascript
class WalletManager {
  constructor() {
    this.hotWallet = null;
    this.coldWallets = [];
    this.HOT_WALLET_LIMIT = 100000000000; // 1000 UDAG
  }
  
  async checkHotWalletBalance() {
    const balance = await fetch(`http://localhost:10333/balance/${this.hotWallet.address}`);
    const data = await balance.json();
    
    if (data.balance > this.HOT_WALLET_LIMIT) {
      await this.sweepToColdStorage(data.balance - this.HOT_WALLET_LIMIT);
    }
  }
  
  async sweepToColdStorage(amount) {
    const coldAddress = this.getNextColdWallet();
    await processWithdrawal('system', amount, coldAddress);
    console.log(`Swept ${amount} to cold storage`);
  }
}
```

### Fee Management

**Dynamic Fee Estimation:**
```javascript
async function estimateFee() {
  const mempool = await fetch('http://localhost:10333/mempool');
  const txs = await mempool.json();
  
  if (txs.length === 0) {
    return 10000; // Minimum fee
  }
  
  // Calculate median fee
  const fees = txs.map(tx => tx.fee).sort((a, b) => a - b);
  const medianFee = fees[Math.floor(fees.length / 2)];
  
  // Add 10% buffer
  return Math.ceil(medianFee * 1.1);
}
```

### Delegation

UltraDAG supports delegated staking, allowing users to delegate UDAG to validators and earn passive rewards without running a node.

**Delegate to a Validator:**

```bash
curl -X POST http://localhost:10333/delegate \
  -H "Content-Type: application/json" \
  -d '{
    "secret_key": "YOUR_SECRET_KEY",
    "validator": "VALIDATOR_ADDRESS",
    "amount": 10000000000
  }'
```

```python
import requests

response = requests.post("http://localhost:10333/delegate", json={
    "secret_key": "YOUR_SECRET_KEY",
    "validator": "VALIDATOR_ADDRESS",
    "amount": 10000000000  # 100 UDAG
})
print(response.json())
```

```javascript
const response = await fetch('http://localhost:10333/delegate', {
  method: 'POST',
  headers: { 'Content-Type': 'application/json' },
  body: JSON.stringify({
    secret_key: 'YOUR_SECRET_KEY',
    validator: 'VALIDATOR_ADDRESS',
    amount: 10000000000 // 100 UDAG
  })
});
const result = await response.json();
```

**Check Delegation Status:**

```bash
curl http://localhost:10333/delegation/YOUR_ADDRESS
```

**Response:**
```json
{
  "delegator": "YOUR_ADDRESS",
  "validator": "VALIDATOR_ADDRESS",
  "amount": 10000000000,
  "amount_udag": 100.0
}
```

**List a Validator's Delegators:**

```bash
curl http://localhost:10333/validator/VALIDATOR_ADDRESS/delegators
```

**Response:**
```json
{
  "validator": "VALIDATOR_ADDRESS",
  "delegators": [
    {
      "address": "DELEGATOR_1",
      "amount": 10000000000,
      "amount_udag": 100.0
    },
    {
      "address": "DELEGATOR_2",
      "amount": 50000000000,
      "amount_udag": 500.0
    }
  ],
  "total_delegated": 60000000000,
  "total_delegated_udag": 600.0
}
```

**Undelegate:**

```bash
curl -X POST http://localhost:10333/undelegate \
  -H "Content-Type: application/json" \
  -d '{"secret_key": "YOUR_SECRET_KEY"}'
```

```python
response = requests.post("http://localhost:10333/undelegate", json={
    "secret_key": "YOUR_SECRET_KEY"
})
print(response.json())
# Funds return after 2,016 rounds (~2.8 hours) cooldown
```

```javascript
const response = await fetch('http://localhost:10333/undelegate', {
  method: 'POST',
  headers: { 'Content-Type': 'application/json' },
  body: JSON.stringify({ secret_key: 'YOUR_SECRET_KEY' })
});
const result = await response.json();
// Funds return after 2,016 rounds (~2.8 hours) cooldown
```

**Set Validator Commission:**

Validators can set their commission rate (percentage taken from delegator rewards):

```bash
curl -X POST http://localhost:10333/set-commission \
  -H "Content-Type: application/json" \
  -d '{
    "secret_key": "VALIDATOR_SECRET_KEY",
    "commission_percent": 15
  }'
```

```python
response = requests.post("http://localhost:10333/set-commission", json={
    "secret_key": "VALIDATOR_SECRET_KEY",
    "commission_percent": 15  # 15%, default is 10%, max 100%
})
print(response.json())
```

```javascript
const response = await fetch('http://localhost:10333/set-commission', {
  method: 'POST',
  headers: { 'Content-Type': 'application/json' },
  body: JSON.stringify({
    secret_key: 'VALIDATOR_SECRET_KEY',
    commission_percent: 15 // 15%, default is 10%, max 100%
  })
});
const result = await response.json();
```

---

## DApp Development

### Smart Contract Alternative

UltraDAG doesn't have smart contracts, but you can build DApps using:
1. **On-chain governance** for protocol-level logic
2. **Off-chain computation** with on-chain settlement
3. **State channels** for high-frequency interactions

### Example: Decentralized Exchange (DEX)

**Order Book (Off-chain):**
```javascript
class OrderBook {
  constructor() {
    this.orders = [];
  }
  
  addOrder(order) {
    // Verify signature
    if (!this.verifyOrderSignature(order)) {
      throw new Error('Invalid signature');
    }
    
    this.orders.push(order);
  }
  
  matchOrders() {
    // Match buy and sell orders
    // Execute settlement on-chain
  }
  
  async settleOrder(buyOrder, sellOrder) {
    // Submit on-chain transactions
    await submitTransaction(buyOrder.settlementTx);
    await submitTransaction(sellOrder.settlementTx);
  }
}
```

### Example: Token Standard

**Token Registry (On-chain via Governance):**
```javascript
// Propose new token registration
async function proposeToken(tokenName, totalSupply) {
  const proposal = {
    proposer: YOUR_ADDRESS,
    proposal_type: {
      Text: {
        title: `Register ${tokenName} token`,
        description: `Total supply: ${totalSupply}`
      }
    },
    nonce: await getCurrentNonce(YOUR_ADDRESS),
    pub_key: YOUR_PUBLIC_KEY,
    signature: SIGNATURE
  };
  
  await submitProposal(proposal);
}
```

**Token Balances (Off-chain Database):**
```javascript
class TokenLedger {
  constructor() {
    this.balances = new Map();
  }
  
  transfer(from, to, amount, signature) {
    // Verify signature
    // Update balances
    this.balances.set(from, this.balances.get(from) - amount);
    this.balances.set(to, this.balances.get(to) + amount);
    
    // Periodically settle on-chain
  }
}
```

---

## Testing

### Local Testnet

**Start Local Node:**
```bash
ultradag-node \
  --data-dir ./test-data \
  --listen 127.0.0.1:9333 \
  --rpc-addr 127.0.0.1:10333 \
  --validators 1 \
  --validator \
  --secret-key YOUR_TEST_KEY
```

**Reset Testnet:**
```bash
rm -rf ./test-data
```

### Unit Tests

**JavaScript (Jest):**
```javascript
const { signTransaction, buildTransaction } = require('./wallet');

describe('Transaction Signing', () => {
  test('signs transaction correctly', () => {
    const tx = buildTransaction(
      'a1b2c3...',
      'f6e5d4...',
      100000000,
      10000,
      0
    );
    
    const signed = signTransaction(tx, secretKey, publicKey);
    
    expect(signed.signature).toHaveLength(128); // 64 bytes hex
    expect(signed.pub_key).toHaveLength(64); // 32 bytes hex
  });
  
  test('derives correct address', () => {
    const address = deriveAddress(publicKey);
    expect(address).toMatch(/^[0-9a-f]{64}$/);
  });
});
```

**Python (pytest):**
```python
import pytest
from wallet import sign_transaction, build_transaction

def test_sign_transaction():
    tx = build_transaction(
        from_addr='a1b2c3...',
        to_addr='f6e5d4...',
        amount=100000000,
        fee=10000,
        nonce=0
    )
    
    signed = sign_transaction(tx, secret_key, public_key)
    
    assert len(signed['signature']) == 128
    assert len(signed['pub_key']) == 64
```

### Integration Tests

**End-to-End Transaction:**
```javascript
describe('E2E Transaction', () => {
  test('sends and confirms transaction', async () => {
    // Generate wallets
    const alice = generateWallet();
    const bob = generateWallet();
    
    // Fund Alice from faucet
    await requestFaucet(alice.address);
    await sleep(5000);
    
    // Check Alice balance
    const aliceBalance = await getBalance(alice.address);
    expect(aliceBalance.balance).toBeGreaterThan(0);
    
    // Send to Bob
    await sendUDAG(alice.address, bob.address, 50000000, alice.secretKey, alice.publicKey);
    
    // Wait for confirmation
    await sleep(10000);
    
    // Verify Bob received
    const bobBalance = await getBalance(bob.address);
    expect(bobBalance.balance).toBe(50000000);
  });
});
```

---

## Production Deployment

### Infrastructure

**Load Balancer:**
```nginx
upstream ultradag_rpc {
    least_conn;
    server node1.ultradag.io:10333 max_fails=3 fail_timeout=30s;
    server node2.ultradag.io:10333 max_fails=3 fail_timeout=30s;
    server node3.ultradag.io:10333 max_fails=3 fail_timeout=30s;
}

server {
    listen 443 ssl http2;
    server_name api.yourapp.com;
    
    location / {
        proxy_pass http://ultradag_rpc;
        proxy_next_upstream error timeout http_503;
        proxy_connect_timeout 5s;
        proxy_send_timeout 10s;
        proxy_read_timeout 10s;
    }
}
```

**Health Checks:**
```javascript
async function checkNodeHealth(nodeUrl) {
  try {
    const response = await fetch(`${nodeUrl}/health/detailed`, {
      timeout: 5000
    });
    const health = await response.json();
    
    return health.status === 'healthy';
  } catch (error) {
    return false;
  }
}

// Periodic health monitoring
setInterval(async () => {
  const nodes = ['http://node1:10333', 'http://node2:10333'];
  
  for (const node of nodes) {
    const healthy = await checkNodeHealth(node);
    if (!healthy) {
      console.error(`Node ${node} is unhealthy`);
      // Alert operations team
    }
  }
}, 30000);
```

### Error Handling

**Retry Logic:**
```javascript
async function submitWithRetry(tx, maxRetries = 3) {
  for (let i = 0; i < maxRetries; i++) {
    try {
      return await submitTransaction(tx);
    } catch (error) {
      if (i === maxRetries - 1) throw error;
      
      console.log(`Retry ${i + 1}/${maxRetries}`);
      await sleep(1000 * (i + 1)); // Exponential backoff
    }
  }
}
```

**Error Classification:**
```javascript
function classifyError(error) {
  if (error.message.includes('insufficient balance')) {
    return 'USER_ERROR'; // Show to user
  }
  if (error.message.includes('rate limit')) {
    return 'RATE_LIMIT'; // Retry later
  }
  if (error.message.includes('503')) {
    return 'NODE_UNAVAILABLE'; // Try different node
  }
  return 'UNKNOWN'; // Log and investigate
}
```

### Monitoring

**Application Metrics:**
```javascript
const metrics = {
  transactionsSent: 0,
  transactionsConfirmed: 0,
  transactionsFailed: 0,
  averageConfirmationTime: 0
};

function recordTransaction(status, confirmationTime) {
  if (status === 'confirmed') {
    metrics.transactionsConfirmed++;
    metrics.averageConfirmationTime = 
      (metrics.averageConfirmationTime + confirmationTime) / 2;
  } else if (status === 'failed') {
    metrics.transactionsFailed++;
  }
  
  // Export to Prometheus/Grafana
  exportMetrics(metrics);
}
```

---

## Best Practices

### Security

1. **Never expose secret keys**
   - Store encrypted
   - Use environment variables
   - Implement key rotation

2. **Validate all inputs**
   - Address format
   - Amount ranges
   - Nonce values

3. **Use HTTPS**
   - Encrypt RPC communication
   - Verify SSL certificates

4. **Implement rate limiting**
   - Prevent abuse
   - Protect your infrastructure

5. **Monitor for anomalies**
   - Unusual transaction patterns
   - Failed authentication attempts
   - Balance discrepancies

### Performance

1. **Cache balances**
   - Reduce RPC calls
   - Update on transactions

2. **Batch operations**
   - Group multiple queries
   - Use connection pooling

3. **Optimize nonce management**
   - Track locally
   - Sync periodically

4. **Use WebSockets** (if available)
   - Real-time updates
   - Reduced polling

### Reliability

1. **Multiple nodes**
   - Redundancy
   - Failover

2. **Transaction tracking**
   - Store in database
   - Retry failed transactions

3. **Reconciliation**
   - Periodic balance checks
   - Audit logs

4. **Graceful degradation**
   - Handle node failures
   - Queue operations

---

## Code Examples

### Complete Wallet Implementation (JavaScript)

See `examples/wallet/` directory for full implementation:
- Key generation and storage
- Transaction signing
- Balance monitoring
- QR code generation
- Transaction history

### Exchange Integration (Python)

See `examples/exchange/` directory for:
- Deposit monitoring
- Withdrawal processing
- Hot/cold wallet management
- Reconciliation scripts

### React DApp Template

See `examples/dapp-template/` for:
- React frontend
- Web3-style wallet connection
- Transaction UI components
- Balance display

---

## Additional Resources

- **RPC API Reference:** [docs/reference/api/rpc-endpoints.md](../../reference/api/rpc-endpoints.md)
- **Transaction Format:** [docs/reference/specifications/transaction-format.md](../../reference/specifications/transaction-format.md)
- **Node Operator Guide:** [docs/guides/operations/node-operator-guide.md](../operations/node-operator-guide.md)
- **GitHub Examples:** https://github.com/UltraDAGcom/examples

---

## Support

**Developer Support:**
- GitHub Discussions: https://github.com/UltraDAGcom/core/discussions
- Discord: https://discord.gg/ultradag
- Email: developers@ultradag.io

**Bug Reports:**
- GitHub Issues: https://github.com/UltraDAGcom/core/issues

---

**Last Updated:** March 10, 2026  
**Document Version:** 1.0  
**Maintainer:** UltraDAG Core Team
