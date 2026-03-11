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
      throw new Error(`Failed to get balance: ${response.statusText}`);
    }

    const data = await response.json();
    return data.balance;
  }

  // Get network status
  async getStatus() {
    const response = await fetch(`${this.apiUrl}/status`);
    if (!response.ok) {
      throw new Error(`Failed to get status: ${response.statusText}`);
    }
    return await response.json();
  }
}

// Make UltraDAG available globally for playground
window.UltraDAG = UltraDAG;

// Playground examples
const examples = {
  'send-transaction': {
    title: 'Send a Transaction',
    code: `// Generate a new wallet with testnet tokens
const wallet = new UltraDAG();
const account = await wallet.generateKeypair();

console.log('Address:', account.address);
console.log('Balance:', account.balance, 'UDAG');

// Send a transaction with a memo
const tx = await wallet.send({
  to: "udag1qqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqq5ce8xa",
  amount: 100,
  memo: "Hello from the playground!"
});

console.log('Transaction sent!');
console.log('Hash:', tx.hash);
console.log('Memo:', tx.memo);

return tx;`
  },
  'check-balance': {
    title: 'Check Balance',
    code: `// Create SDK instance
const sdk = new UltraDAG();

// Check any address balance
const balance = await sdk.getBalance(
  "udag1qqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqq5ce8xa"
);

console.log('Balance:', balance, 'UDAG');

return { balance };`
  },
  'network-status': {
    title: 'Get Network Status',
    code: `// Create SDK instance
const sdk = new UltraDAG();

// Get current network status
const status = await sdk.getStatus();

console.log('Current Round:', status.round);
console.log('Active Nodes:', status.nodes);
console.log('Total Supply:', status.supply, 'UDAG');
console.log('Finality Lag:', status.finality_lag, 'rounds');

return status;`
  },
  'multiple-transactions': {
    title: 'Send Multiple Transactions',
    code: `// Generate wallet with testnet tokens
const wallet = new UltraDAG();
await wallet.generateKeypair();

console.log('Sending 3 transactions...');

// Send multiple transactions
const txs = [];
for (let i = 1; i <= 3; i++) {
  const tx = await wallet.send({
    to: "udag1qqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqq5ce8xa",
    amount: 10 * i,
    memo: \`Transaction #\${i}\`
  });
  console.log(\`TX \${i} sent:\`, tx.hash);
  txs.push(tx);
}

console.log('All transactions sent!');
return txs;`
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
