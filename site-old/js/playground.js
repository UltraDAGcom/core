// UltraDAG Interactive Playground
(function() {
'use strict';

const API_URL = 'https://ultradag-node-1.fly.dev';

// Simple UltraDAG SDK for browser
class UltraDAG {
  constructor(apiUrl = API_URL) {
    this.apiUrl = apiUrl;
    this.secretKey = null;
    this.address = null;
  }

  // Generate a new keypair (simplified - uses random for demo)
  async generateKeypair() {
    const response = await fetch(`${this.apiUrl}/faucet`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({})
    });
    
    if (!response.ok) {
      if (response.status === 429) {
        const error = await response.json();
        throw new Error(`Rate limit: ${error.error || 'Faucet limited to 1 request per 10 minutes'}. Try the "Check Balance" or "Get Network Status" examples instead - they work without the faucet!`);
      }
      throw new Error(`Faucet request failed: ${response.statusText}`);
    }
    
    const data = await response.json();
    this.secretKey = data.secret_key;
    this.address = data.address;
    
    return {
      secretKey: this.secretKey,
      address: this.address,
      balance: data.balance
    };
  }

  // Send a transaction
  async send({ to, amount, memo = null }) {
    if (!this.secretKey) {
      throw new Error('No keypair loaded. Call generateKeypair() first.');
    }

    const payload = {
      secret_key: this.secretKey,
      to,
      amount,
      fee: 1
    };

    if (memo) {
      payload.memo = memo;
    }

    const response = await fetch(`${this.apiUrl}/send`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(payload)
    });

    if (!response.ok) {
      const error = await response.text();
      throw new Error(`Transaction failed: ${error}`);
    }

    const data = await response.json();
    return {
      hash: data.hash,
      from: this.address,
      to,
      amount,
      memo
    };
  }

  // Get account balance
  async getBalance(address = null) {
    const addr = address || this.address;
    if (!addr) {
      throw new Error('No address provided');
    }

    const response = await fetch(`${this.apiUrl}/balance/${addr}`);
    if (!response.ok) {
      const error = await response.text();
      throw new Error(`Failed to get balance: ${error}`);
    }

    const data = await response.json();
    return data.balance;
  }

  // Get network status
  async getStatus() {
    const response = await fetch(`${this.apiUrl}/status`);
    if (!response.ok) {
      const error = await response.text();
      throw new Error(`Failed to get status: ${error}`);
    }
    const data = await response.json();
    
    // Map API response to expected format
    return {
      round: data.dag_round,
      finalized: data.last_finalized_round,
      nodes: data.peer_count,
      supply: data.total_supply,
      finality_lag: data.dag_round - data.last_finalized_round
    };
  }
}

// Make UltraDAG available globally for playground
window.UltraDAG = UltraDAG;

// Playground examples
const examples = {
  'check-balance': {
    title: 'Check Balance',
    code: `// Create SDK instance
const sdk = new UltraDAG();

// Check any address balance (no faucet needed!)
// Using hex address format (64 chars)
const balance = await sdk.getBalance(
  "0000000000000000000000000000000000000000000000000000000000000000"
);

console.log('Balance:', balance, 'sats');
console.log('In UDAG:', (balance / 100_000_000).toFixed(2));

return { balance_sats: balance, balance_udag: (balance / 100_000_000).toFixed(2) };`
  },
  'network-status': {
    title: 'Get Network Status',
    code: `// Create SDK instance
const sdk = new UltraDAG();

// Get current network status (no faucet needed!)
const status = await sdk.getStatus();

console.log('Current Round:', status.round);
console.log('Finalized Round:', status.finalized);
console.log('Active Nodes:', status.nodes);
console.log('Total Supply:', (status.supply / 100_000_000).toFixed(2), 'UDAG');
console.log('Finality Lag:', status.finality_lag, 'rounds');

return status;`
  },
  'send-transaction': {
    title: 'Send a Transaction',
    code: `// NOTE: Faucet is rate-limited to 1 request per 10 minutes
// If you get a rate limit error, try the other examples first!

const wallet = new UltraDAG();
const account = await wallet.generateKeypair();

console.log('Generated address:', account.address);
console.log('Balance:', (account.balance / 100_000_000).toFixed(2), 'UDAG');

// Send a transaction with a memo
const tx = await wallet.send({
  to: "udag1qqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqq5ce8xa",
  amount: 100_000_000, // 1 UDAG
  memo: "Hello from the playground!"
});

console.log('Transaction sent!');
console.log('Hash:', tx.hash);
console.log('Memo:', tx.memo);

return tx;`
  },
  'multiple-queries': {
    title: 'Multiple API Queries',
    code: `// Query multiple endpoints without needing faucet
const sdk = new UltraDAG();

console.log('Fetching network data...');

// Get network status
const status = await sdk.getStatus();
console.log('Network Round:', status.round);
console.log('Finalized Round:', status.finalized);

// Check balance (using hex address format)
const addr1 = "0000000000000000000000000000000000000000000000000000000000000000";
const balance1 = await sdk.getBalance(addr1);
console.log('Balance:', (balance1 / 100_000_000).toFixed(2), 'UDAG');

// Calculate network stats
const supplyUDAG = status.supply / 100_000_000;
const maxSupply = 21_000_000;
const percentMinted = (supplyUDAG / maxSupply * 100).toFixed(2);

console.log('Total Supply:', supplyUDAG.toFixed(2), 'UDAG');
console.log('Percent minted:', percentMinted + '%');
console.log('Finality lag:', status.finality_lag, 'rounds');

return {
  round: status.round,
  finalized: status.finalized,
  supply_udag: supplyUDAG.toFixed(2),
  percent_minted: percentMinted + '%',
  finality_lag: status.finality_lag
};`
  }
};

// Initialize playground
function initPlayground() {
  const playgroundEl = document.getElementById('playground');
  if (!playgroundEl) return;

  const exampleSelect = document.getElementById('example-select');
  const codeEditor = document.getElementById('code-editor');
  const runBtn = document.getElementById('run-code');
  const outputEl = document.getElementById('output');
  const resultEl = document.getElementById('result');

  // Populate examples
  Object.keys(examples).forEach(key => {
    const option = document.createElement('option');
    option.value = key;
    option.textContent = examples[key].title;
    exampleSelect.appendChild(option);
  });

  // Load example
  function loadExample(key) {
    const example = examples[key];
    if (example) {
      codeEditor.value = example.code;
      clearOutput();
    }
  }

  // Clear output
  function clearOutput() {
    outputEl.innerHTML = '';
    resultEl.innerHTML = '';
  }

  // Log to output
  function log(message, type = 'info') {
    const line = document.createElement('div');
    line.className = `log-line log-${type}`;
    line.textContent = message;
    outputEl.appendChild(line);
    outputEl.scrollTop = outputEl.scrollHeight;
  }

  // Override console.log for playground
  const originalLog = console.log;
  let captureLog = false;

  // Run code
  async function runCode() {
    clearOutput();
    runBtn.disabled = true;
    runBtn.textContent = 'Running...';

    try {
      log('▶ Executing code...', 'info');
      
      // Capture console.log
      captureLog = true;
      console.log = (...args) => {
        log(args.map(a => typeof a === 'object' ? JSON.stringify(a, null, 2) : String(a)).join(' '), 'log');
        originalLog(...args);
      };

      // Execute code
      const AsyncFunction = Object.getPrototypeOf(async function(){}).constructor;
      const fn = new AsyncFunction(codeEditor.value);
      const result = await fn();

      // Restore console.log
      console.log = originalLog;
      captureLog = false;

      log('✓ Execution completed', 'success');

      // Display result
      if (result !== undefined) {
        resultEl.innerHTML = `
          <div class="result-header">Result:</div>
          <pre class="result-data">${JSON.stringify(result, null, 2)}</pre>
        `;
      }

    } catch (error) {
      console.log = originalLog;
      captureLog = false;
      log(`✗ Error: ${error.message}`, 'error');
      console.error(error);
    } finally {
      runBtn.disabled = false;
      runBtn.textContent = 'Run Code';
    }
  }

  // Event listeners
  exampleSelect.addEventListener('change', (e) => {
    loadExample(e.target.value);
  });

  runBtn.addEventListener('click', runCode);

  // Load first example
  loadExample('send-transaction');
}

// Initialize when DOM is ready
if (document.readyState === 'loading') {
  document.addEventListener('DOMContentLoaded', initPlayground);
} else {
  initPlayground();
}

})();
