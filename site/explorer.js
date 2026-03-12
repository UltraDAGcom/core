// UltraDAG Block Explorer
// Fetches and displays real network data

const API_URL = 'https://ultradag-node-1.fly.dev';

// State
let currentRound = 0;
let currentPage = 1;
const ROUNDS_PER_PAGE = 10;

// Utility functions
function formatUdag(sats) {
  return (sats / 100_000_000).toFixed(4);
}

function shortHash(hash) {
  if (!hash) return '—';
  return hash.substring(0, 8) + '...' + hash.substring(hash.length - 6);
}

function shortAddress(addr) {
  if (!addr) return '—';
  return addr.substring(0, 10) + '...' + addr.substring(addr.length - 8);
}

function timeAgo(timestamp) {
  const now = Math.floor(Date.now() / 1000);
  const diff = now - timestamp;
  if (diff < 60) return `${diff}s ago`;
  if (diff < 3600) return `${Math.floor(diff / 60)}m ago`;
  if (diff < 86400) return `${Math.floor(diff / 3600)}h ago`;
  return `${Math.floor(diff / 86400)}d ago`;
}

// Fetch network status
async function fetchStatus() {
  try {
    const response = await fetch(`${API_URL}/status`);
    const data = await response.json();
    
    currentRound = data.dag_round;
    
    document.getElementById('latest-round').textContent = data.dag_round.toLocaleString();
    document.getElementById('total-vertices').textContent = data.dag_vertices.toLocaleString();
    document.getElementById('total-supply').textContent = formatUdag(data.total_supply);
    document.getElementById('account-count').textContent = data.account_count.toLocaleString();
    
    return data;
  } catch (error) {
    console.error('Failed to fetch status:', error);
  }
}

// Fetch round data
async function fetchRound(roundNumber) {
  try {
    const response = await fetch(`${API_URL}/round/${roundNumber}`);
    if (!response.ok) {
      if (response.status === 404) return null;
      throw new Error(`HTTP ${response.status}`);
    }
    const data = await response.json();
    return data;
  } catch (error) {
    console.error(`Failed to fetch round ${roundNumber}:`, error);
    return null;
  }
}

// Fetch address balance
async function fetchAddress(address) {
  try {
    const response = await fetch(`${API_URL}/balance/${address}`);
    if (!response.ok) {
      if (response.status === 404) return null;
      throw new Error(`HTTP ${response.status}`);
    }
    const data = await response.json();
    return data;
  } catch (error) {
    console.error(`Failed to fetch address ${address}:`, error);
    return null;
  }
}

// Load latest rounds
async function loadRounds() {
  const tbody = document.getElementById('rounds-tbody');
  tbody.innerHTML = '<tr><td colspan="6" class="loading">Loading rounds</td></tr>';
  
  if (currentRound === 0) {
    await fetchStatus();
  }
  
  const startRound = Math.max(1, currentRound - (currentPage - 1) * ROUNDS_PER_PAGE);
  const endRound = Math.max(1, startRound - ROUNDS_PER_PAGE + 1);
  
  const rounds = [];
  for (let i = startRound; i >= endRound && i >= 1; i--) {
    const roundData = await fetchRound(i);
    if (roundData) {
      rounds.push({ round: i, data: roundData });
    }
  }
  
  if (rounds.length === 0) {
    tbody.innerHTML = '<tr><td colspan="6" class="empty">No rounds found</td></tr>';
    return;
  }
  
  tbody.innerHTML = rounds.map(({ round, data }) => {
    const vertexCount = data.length;
    const txCount = data.reduce((sum, v) => sum + v.tx_count, 0);
    const validators = new Set(data.map(v => v.validator)).size;
    const totalRewards = data.reduce((sum, v) => sum + v.reward, 0);
    
    return `
      <tr onclick="viewRound(${round})">
        <td><a href="#round-${round}">${round}</a></td>
        <td>${vertexCount}</td>
        <td>${txCount}</td>
        <td>${validators}</td>
        <td>${formatUdag(totalRewards)} UDAG</td>
        <td><span class="badge badge-success">Finalized</span></td>
      </tr>
    `;
  }).join('');
  
  renderPagination();
}

// Render pagination
function renderPagination() {
  const pagination = document.getElementById('rounds-pagination');
  const maxPages = Math.ceil(currentRound / ROUNDS_PER_PAGE);
  
  let html = '';
  
  // Previous button
  html += `<button class="page-btn" ${currentPage === 1 ? 'disabled' : ''} onclick="changePage(${currentPage - 1})">← Prev</button>`;
  
  // Page numbers
  const startPage = Math.max(1, currentPage - 2);
  const endPage = Math.min(maxPages, currentPage + 2);
  
  if (startPage > 1) {
    html += `<button class="page-btn" onclick="changePage(1)">1</button>`;
    if (startPage > 2) html += `<span style="color:var(--muted)">...</span>`;
  }
  
  for (let i = startPage; i <= endPage; i++) {
    html += `<button class="page-btn ${i === currentPage ? 'active' : ''}" onclick="changePage(${i})">${i}</button>`;
  }
  
  if (endPage < maxPages) {
    if (endPage < maxPages - 1) html += `<span style="color:var(--muted)">...</span>`;
    html += `<button class="page-btn" onclick="changePage(${maxPages})">${maxPages}</button>`;
  }
  
  // Next button
  html += `<button class="page-btn" ${currentPage >= maxPages ? 'disabled' : ''} onclick="changePage(${currentPage + 1})">Next →</button>`;
  
  pagination.innerHTML = html;
}

// Change page
window.changePage = function(page) {
  currentPage = page;
  loadRounds();
};

// View round details
window.viewRound = async function(roundNumber) {
  const detailView = document.getElementById('detail-view');
  detailView.style.display = 'block';
  detailView.innerHTML = '<div class="loading">Loading round details</div>';
  
  // Scroll to detail view
  detailView.scrollIntoView({ behavior: 'smooth' });
  
  const roundData = await fetchRound(roundNumber);
  if (!roundData) {
    detailView.innerHTML = '<div class="error">Round not found</div>';
    return;
  }
  
  const vertexCount = roundData.length;
  const txCount = roundData.reduce((sum, v) => sum + v.tx_count, 0);
  const validators = new Set(roundData.map(v => v.validator));
  const totalRewards = roundData.reduce((sum, v) => sum + v.reward, 0);
  
  let html = `
    <div class="detail-card">
      <div class="detail-header">
        <div class="detail-title">Round ${roundNumber}</div>
        <button onclick="closeDetail()" style="background:var(--bg3);border:1px solid var(--border);color:var(--subtle);padding:8px 16px;border-radius:2px;cursor:pointer;font-family:'DM Mono',monospace;font-size:11px">Close</button>
      </div>
      
      <div class="detail-grid">
        <div class="detail-label">Round Number</div>
        <div class="detail-value">${roundNumber}</div>
        
        <div class="detail-label">Vertices</div>
        <div class="detail-value">${vertexCount}</div>
        
        <div class="detail-label">Transactions</div>
        <div class="detail-value">${txCount}</div>
        
        <div class="detail-label">Validators</div>
        <div class="detail-value">${validators.size}</div>
        
        <div class="detail-label">Total Rewards</div>
        <div class="detail-value">${formatUdag(totalRewards)} UDAG</div>
        
        <div class="detail-label">Status</div>
        <div class="detail-value"><span class="badge badge-success">Finalized</span></div>
      </div>
    </div>
    
    <div class="detail-card">
      <h3 style="font-family:'Cormorant',serif;font-size:24px;color:var(--white);margin-bottom:24px">Vertices in Round ${roundNumber}</h3>
      <div class="table-container">
        <table class="table">
          <thead>
            <tr>
              <th>Hash</th>
              <th>Validator</th>
              <th>Transactions</th>
              <th>Parents</th>
              <th>Reward</th>
            </tr>
          </thead>
          <tbody>
            ${roundData.map(v => `
              <tr>
                <td class="hash"><a href="#vertex-${v.hash}">${shortHash(v.hash)}</a></td>
                <td class="hash"><a href="#address-${v.validator}" onclick="viewAddress('${v.validator}')">${shortAddress(v.validator)}</a></td>
                <td>${v.tx_count}</td>
                <td>${v.parent_count}</td>
                <td>${formatUdag(v.reward)} UDAG</td>
              </tr>
            `).join('')}
          </tbody>
        </table>
      </div>
    </div>
  `;
  
  detailView.innerHTML = html;
};

// View address details
window.viewAddress = async function(address) {
  const detailView = document.getElementById('detail-view');
  detailView.style.display = 'block';
  detailView.innerHTML = '<div class="loading">Loading address details</div>';
  
  detailView.scrollIntoView({ behavior: 'smooth' });
  
  const addressData = await fetchAddress(address);
  if (!addressData) {
    detailView.innerHTML = '<div class="error">Address not found or has no balance</div>';
    return;
  }
  
  let html = `
    <div class="detail-card">
      <div class="detail-header">
        <div class="detail-title">Address Details</div>
        <button onclick="closeDetail()" style="background:var(--bg3);border:1px solid var(--border);color:var(--subtle);padding:8px 16px;border-radius:2px;cursor:pointer;font-family:'DM Mono',monospace;font-size:11px">Close</button>
      </div>
      
      <div class="detail-grid">
        <div class="detail-label">Address</div>
        <div class="detail-value">${addressData.address}</div>
        
        <div class="detail-label">Balance</div>
        <div class="detail-value">${addressData.balance_udag} UDAG (${addressData.balance.toLocaleString()} sats)</div>
        
        <div class="detail-label">Nonce</div>
        <div class="detail-value">${addressData.nonce}</div>
      </div>
    </div>
  `;
  
  detailView.innerHTML = html;
};

// Close detail view
window.closeDetail = function() {
  document.getElementById('detail-view').style.display = 'none';
};

// Search functionality
async function performSearch() {
  const query = document.getElementById('search-input').value.trim();
  if (!query) return;
  
  // Check if it's a round number
  if (/^\d+$/.test(query)) {
    const roundNum = parseInt(query);
    await viewRound(roundNum);
    return;
  }
  
  // Check if it's a hex address (64 chars)
  if (/^[0-9a-fA-F]{64}$/.test(query)) {
    await viewAddress(query.toLowerCase());
    return;
  }
  
  // Otherwise show error
  const detailView = document.getElementById('detail-view');
  detailView.style.display = 'block';
  detailView.innerHTML = '<div class="error">Invalid search query. Please enter a round number or address (64 hex characters).</div>';
  detailView.scrollIntoView({ behavior: 'smooth' });
}

// Tab switching
function switchTab(tabName) {
  // Update tab buttons
  document.querySelectorAll('.tab').forEach(tab => {
    tab.classList.remove('active');
  });
  document.querySelector(`[data-tab="${tabName}"]`).classList.add('active');
  
  // Update tab content
  document.querySelectorAll('.tab-content').forEach(content => {
    content.style.display = 'none';
  });
  document.getElementById(`${tabName}-tab`).style.display = 'block';
  
  // Load data for the tab
  if (tabName === 'rounds') {
    loadRounds();
  } else if (tabName === 'transactions') {
    loadTransactions();
  }
}

// Load transactions (placeholder - would need tx history API)
async function loadTransactions() {
  const tbody = document.getElementById('transactions-tbody');
  tbody.innerHTML = '<tr><td colspan="6" class="empty">Transaction history requires additional API endpoint</td></tr>';
}

// Mobile menu toggle
document.getElementById('hamburger').addEventListener('click', () => {
  document.getElementById('mobile-menu').classList.toggle('active');
});

// Search button
document.getElementById('search-btn').addEventListener('click', performSearch);
document.getElementById('search-input').addEventListener('keypress', (e) => {
  if (e.key === 'Enter') performSearch();
});

// Tab buttons
document.querySelectorAll('.tab').forEach(tab => {
  tab.addEventListener('click', () => {
    switchTab(tab.dataset.tab);
  });
});

// Initialize
(async function init() {
  await fetchStatus();
  await loadRounds();
  
  // Refresh status every 5 seconds
  setInterval(fetchStatus, 5000);
})();
