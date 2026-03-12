// Docs Interactive Playground Integration
(function() {
'use strict';

const API_URL = 'https://ultradag-node-1.fly.dev';

// Simple UltraDAG SDK for browser (same as playground.js)
class UltraDAG {
  constructor(apiUrl = API_URL) {
    this.apiUrl = apiUrl;
    this.secretKey = null;
    this.address = null;
  }

  async generateKeypair() {
    const response = await fetch(`${this.apiUrl}/faucet`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({})
    });
    
    if (!response.ok) {
      if (response.status === 429) {
        const error = await response.json();
        throw new Error(`Rate limit: ${error.error || 'Faucet limited to 1 request per 10 minutes'}. Try the "Check Balance" or "Network Status" examples instead - they work without the faucet!`);
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

window.UltraDAG = UltraDAG;

// Runnable code examples for docs
const runnableExamples = {
  'check-balance': `// Check balance of any address (no faucet needed!)
const sdk = new UltraDAG();

// Using hex address format (64 chars)
const balance = await sdk.getBalance(
  "0000000000000000000000000000000000000000000000000000000000000000"
);

console.log('Balance:', balance, 'sats');
console.log('In UDAG:', (balance / 100_000_000).toFixed(2));

return { balance_sats: balance, balance_udag: (balance / 100_000_000).toFixed(2) };`,

  'send-transaction': `// NOTE: Faucet is rate-limited to 1 request per 10 minutes
// If you see a rate limit error, try the network-status or check-balance examples instead!

const wallet = new UltraDAG();
const account = await wallet.generateKeypair();

console.log('Generated address:', account.address);
console.log('Initial balance:', (account.balance / 100_000_000).toFixed(2), 'UDAG');

// Send transaction with memo
const tx = await wallet.send({
  to: "udag1qqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqq5ce8xa",
  amount: 100_000_000, // 1 UDAG
  memo: "Test from docs!"
});

console.log('Transaction sent!');
console.log('Hash:', tx.hash);

return tx;`,

  'network-status': `// Get current network status (no faucet needed!)
const sdk = new UltraDAG();
const status = await sdk.getStatus();

console.log('Current Round:', status.round);
console.log('Finalized Round:', status.finalized);
console.log('Active Nodes:', status.nodes);
console.log('Total Supply:', (status.supply / 100_000_000).toFixed(2), 'UDAG');
console.log('Finality Lag:', status.finality_lag, 'rounds');

return status;`,

  'python-example': `// JavaScript equivalent of Python SDK example
// NOTE: This uses the faucet which is rate-limited
// Try network-status or check-balance examples if you hit rate limits

const wallet = new UltraDAG();
const kp = await wallet.generateKeypair();

console.log('Address:', kp.address);
console.log('Balance:', (kp.balance / 100_000_000).toFixed(2), 'UDAG');

// Send transaction
const tx = await wallet.send({
  to: "udag1qqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqq5ce8xa",
  amount: 50_000_000,
  memo: "From JS SDK"
});

console.log('TX Hash:', tx.hash);

return { address: kp.address, txHash: tx.hash };`,

  'weather-sensor': `// Weather sensor (Node A) sells data to drone (Node B)
const sdk = new UltraDAG();

// Node A: Weather station creates data package
const weatherData = {
  sensor_id: "weather_station_001",
  temperature: 23.5,
  humidity: 65,
  timestamp: Date.now(),
  location: { lat: 37.7749, lng: -122.4194 }
};

console.log('🌡️  Weather Station Data:');
console.log('Temperature:', weatherData.temperature, '°C');
console.log('Humidity:', weatherData.humidity, '%');
console.log('Location:', weatherData.location);

// Node B: Drone purchases weather data
const droneWallet = new UltraDAG();
const droneAccount = await droneWallet.generateKeypair();

console.log('🚁 Drone Wallet:', droneAccount.address);
console.log('Balance:', (droneAccount.balance / 100_000_000).toFixed(6), 'UDAG');

// Simulate data purchase transaction
const dataPrice = 1_000_000; // 0.01 UDAG in satoshis
const purchaseTx = await droneWallet.send({
  to: "udag1qqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqq5ce8xa", // Weather station
  amount: dataPrice,
  memo: `Purchase weather data: ${weatherData.sensor_id}`
});

console.log('💰 Data Purchase Transaction:');
console.log('Price:', (dataPrice / 100_000_000).toFixed(6), 'UDAG');
console.log('TX Hash:', purchaseTx.hash);
console.log('Data ID:', weatherData.sensor_id);

return {
  weatherData,
  purchase: {
    price: (dataPrice / 100_000_000).toFixed(6),
    txHash: purchaseTx.hash,
    sensorId: weatherData.sensor_id
  }
};`,

  'js-sdk-example': `// Get network status (no faucet needed!)
const sdk = new UltraDAG();
const status = await sdk.getStatus();

console.log('DAG Round:', status.round);
console.log('Finalized:', status.finalized);
console.log('Supply:', (status.supply / 100_000_000).toFixed(2), 'UDAG');
console.log('Nodes:', status.nodes);

return status;`
};

// Initialize docs playground
function initDocsPlayground() {
  // Add "Try it" buttons to ALL code examples
  const codeBlocks = document.querySelectorAll('pre');
  
  codeBlocks.forEach((pre, index) => {
    // Skip if already processed
    if (pre.closest('.code-example-container')) return;
    const text = pre.textContent;
    
    // Determine appropriate example based on content
    let exampleKey = 'weather-sensor'; // default
    
    if (text.includes('/balance/') && text.includes('curl')) {
      exampleKey = 'check-balance';
    } else if (text.includes('pip install ultradag') || text.includes('from ultradag import')) {
      exampleKey = 'python-example';
    } else if (text.includes('npm install ultradag') || text.includes("from 'ultradag'")) {
      exampleKey = 'js-sdk-example';
    } else if (text.includes('/tx') && text.includes('POST')) {
      exampleKey = 'send-transaction';
    } else if (text.includes('/status')) {
      exampleKey = 'network-status';
    } else if (text.includes('curl') && text.includes('submit_tx')) {
      exampleKey = 'send-transaction';
    } else if (text.includes('curl') && text.includes('stake')) {
      exampleKey = 'send-transaction';
    } else if (text.includes('npm install')) {
      exampleKey = 'js-sdk-example';
    } else if (text.includes('import') || text.includes('const')) {
      exampleKey = 'js-sdk-example';
    }
    
    // Always add the playground to every code block
    // Check if pre is inside a terminal
    const terminalParent = pre.closest('.terminal');
    let container;
    
    if (terminalParent) {
      // Wrap the entire terminal in container
      container = document.createElement('div');
      container.className = 'code-example-container terminal';
      terminalParent.parentNode.insertBefore(container, terminalParent);
      container.appendChild(terminalParent);
    } else {
      // Wrap pre in container
      container = document.createElement('div');
      container.className = 'code-example-container';
      pre.parentNode.insertBefore(container, pre);
      container.appendChild(pre);
    }
    
    // Add "Try it" button
    const btnContainer = document.createElement('div');
    btnContainer.className = 'code-example-actions';
    
    const tryBtn = document.createElement('button');
    tryBtn.className = 'try-it-btn';
    tryBtn.innerHTML = '▶ Try it live';
    tryBtn.dataset.example = exampleKey;
    
    btnContainer.appendChild(tryBtn);
    container.appendChild(btnContainer);
    
    // Add click handler
    tryBtn.addEventListener('click', () => openPlaygroundModal(exampleKey));
  });
}

// Playground modal
function openPlaygroundModal(exampleKey) {
  const code = runnableExamples[exampleKey];
  if (!code) return;
  
  // Create modal
  const modal = document.createElement('div');
  modal.className = 'playground-modal';
  modal.innerHTML = `
    <div class="playground-modal-overlay"></div>
    <div class="playground-modal-content">
      <div class="playground-modal-header">
        <h3>Interactive Playground</h3>
        <button class="playground-modal-close">&times;</button>
      </div>
      <div class="playground-modal-body">
        <div class="playground-editor-section">
          <div class="playground-label">Code</div>
          <textarea class="playground-code-editor" spellcheck="false">${code}</textarea>
        </div>
        <div class="playground-output-section">
          <div class="playground-label">Output</div>
          <div class="playground-output"></div>
          <div class="playground-result"></div>
        </div>
      </div>
      <div class="playground-modal-footer">
        <button class="playground-run-btn">Run Code</button>
      </div>
    </div>
  `;
  
  document.body.appendChild(modal);
  
  const closeBtn = modal.querySelector('.playground-modal-close');
  const overlay = modal.querySelector('.playground-modal-overlay');
  const runBtn = modal.querySelector('.playground-run-btn');
  const codeEditor = modal.querySelector('.playground-code-editor');
  const outputEl = modal.querySelector('.playground-output');
  const resultEl = modal.querySelector('.playground-result');
  
  // Close handlers
  const closeModal = () => {
    modal.classList.remove('show');
    setTimeout(() => modal.remove(), 300);
  };
  
  closeBtn.addEventListener('click', closeModal);
  overlay.addEventListener('click', closeModal);
  
  // Run code
  runBtn.addEventListener('click', async () => {
    outputEl.innerHTML = '';
    resultEl.innerHTML = '';
    runBtn.disabled = true;
    runBtn.textContent = 'Running...';
    
    const log = (msg, type = 'info') => {
      const line = document.createElement('div');
      line.className = `log-line log-${type}`;
      line.textContent = msg;
      outputEl.appendChild(line);
    };
    
    try {
      log('▶ Executing code...', 'info');
      
      // Capture console.log
      const originalLog = console.log;
      console.log = (...args) => {
        log(args.map(a => typeof a === 'object' ? JSON.stringify(a, null, 2) : String(a)).join(' '), 'log');
        originalLog(...args);
      };
      
      // Execute
      const AsyncFunction = Object.getPrototypeOf(async function(){}).constructor;
      const fn = new AsyncFunction(codeEditor.value);
      const result = await fn();
      
      console.log = originalLog;
      log('✓ Execution completed', 'success');
      
      if (result !== undefined) {
        resultEl.innerHTML = `
          <div class="result-header">Result:</div>
          <pre class="result-data">${JSON.stringify(result, null, 2)}</pre>
        `;
      }
    } catch (error) {
      console.log = console.log;
      log(`✗ Error: ${error.message}`, 'error');
    } finally {
      runBtn.disabled = false;
      runBtn.textContent = 'Run Code';
    }
  });
  
  // Show modal
  setTimeout(() => modal.classList.add('show'), 10);
}

// Initialize when DOM is ready
if (document.readyState === 'loading') {
  document.addEventListener('DOMContentLoaded', initDocsPlayground);
} else {
  initDocsPlayground();
}

})();
