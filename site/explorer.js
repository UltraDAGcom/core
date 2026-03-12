// UltraDAG Block Explorer
// Fetches and displays real network data

const API_URL = 'https://ultradag-node-1.fly.dev';

// State
let currentRound = 0;
let currentPage = 1;
const ROUNDS_PER_PAGE = 10;
let statsHistory = [];
let autoRefreshEnabled = true;
let lastUpdateTime = 0;

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

function copyToClipboard(text) {
  navigator.clipboard.writeText(text).then(() => {
    showNotification('Copied to clipboard!');
  }).catch(err => {
    console.error('Failed to copy:', err);
  });
}

function showNotification(message) {
  const notification = document.createElement('div');
  notification.textContent = message;
  notification.style.cssText = 'position:fixed;top:80px;right:20px;background:var(--success);color:var(--white);padding:12px 20px;border-radius:4px;font-family:DM Mono,monospace;font-size:12px;z-index:1000;animation:slideIn 0.3s ease';
  document.body.appendChild(notification);
  setTimeout(() => {
    notification.style.animation = 'slideOut 0.3s ease';
    setTimeout(() => notification.remove(), 300);
  }, 2000);
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
    lastUpdateTime = Date.now();
    
    // Track stats for history
    trackStats(data);
    
    // Update stats with change indicators
    updateStatWithChange('latest-round', data.dag_round);
    updateStatWithChange('total-vertices', data.dag_vertices);
    document.getElementById('total-supply').textContent = formatUdag(data.total_supply);
    document.getElementById('account-count').textContent = data.account_count.toLocaleString();
    
    // Update network health indicator
    updateNetworkHealth(data);
    
    return data;
  } catch (error) {
    console.error('Failed to fetch status:', error);
    updateNetworkHealth(null);
  }
}

// Update stat with change indicator
function updateStatWithChange(elementId, newValue) {
  const el = document.getElementById(elementId);
  if (!el) return;
  
  const oldValue = parseInt(el.textContent.replace(/,/g, '')) || 0;
  el.textContent = newValue.toLocaleString();
  
  if (newValue > oldValue) {
    el.style.color = 'var(--success)';
    setTimeout(() => { el.style.color = 'var(--white)'; }, 1000);
  }
}

// Update network health indicator
function updateNetworkHealth(data) {
  const healthEl = document.getElementById('network-health');
  if (!healthEl) return;
  
  if (!data) {
    healthEl.innerHTML = '<span class="badge badge-warning">⚠️ Offline</span>';
    return;
  }
  
  // Check if network is healthy
  const isHealthy = data.peer_count >= 3 && data.dag_round > 0;
  const healthBadge = isHealthy 
    ? '<span class="badge badge-success">🟢 Healthy</span>'
    : '<span class="badge badge-warning">⚠️ Degraded</span>';
  
  healthEl.innerHTML = healthBadge;
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
      <tr onclick="viewRound(${round})" style="cursor:pointer">
        <td><strong style="color:var(--accent)">${round}</strong></td>
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
        <button onclick="closeDetail()" style="background:var(--bg3);border:1px solid var(--border);color:var(--subtle);padding:8px 16px;border-radius:2px;cursor:pointer;font-family:'DM Mono',monospace;font-size:11px;transition:all .2s" onmouseover="this.style.borderColor='var(--accent)';this.style.color='var(--accent)'" onmouseout="this.style.borderColor='var(--border)';this.style.color='var(--subtle)'">Close</button>
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
        <div class="detail-value">${formatUdag(totalRewards)} UDAG (${totalRewards.toLocaleString()} sats)</div>
        
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
                <td class="hash">
                  <span style="cursor:pointer" onclick="copyToClipboard('${v.hash}')" title="Click to copy full hash">${shortHash(v.hash)}</span>
                </td>
                <td class="hash">
                  <a href="#address-${v.validator}" onclick="event.preventDefault();viewAddress('${v.validator}')" title="View address details">${shortAddress(v.validator)}</a>
                  <span style="cursor:pointer;margin-left:8px;opacity:0.5;font-size:11px" onclick="copyToClipboard('${v.validator}')" title="Copy address">📋</span>
                </td>
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
    detailView.innerHTML = `
      <div class="detail-card">
        <div class="detail-header">
          <div class="detail-title">Address Not Found</div>
          <button onclick="closeDetail()" style="background:var(--bg3);border:1px solid var(--border);color:var(--subtle);padding:8px 16px;border-radius:2px;cursor:pointer;font-family:'DM Mono',monospace;font-size:11px;transition:all .2s" onmouseover="this.style.borderColor='var(--accent)';this.style.color='var(--accent)'" onmouseout="this.style.borderColor='var(--border)';this.style.color='var(--subtle)'">Close</button>
        </div>
        <div class="error">Address not found or has zero balance. Only addresses with balance are indexed.</div>
      </div>
    `;
    return;
  }
  
  let html = `
    <div class="detail-card">
      <div class="detail-header">
        <div class="detail-title">Address Details</div>
        <button onclick="closeDetail()" style="background:var(--bg3);border:1px solid var(--border);color:var(--subtle);padding:8px 16px;border-radius:2px;cursor:pointer;font-family:'DM Mono',monospace;font-size:11px;transition:all .2s" onmouseover="this.style.borderColor='var(--accent)';this.style.color='var(--accent)'" onmouseout="this.style.borderColor='var(--border)';this.style.color='var(--subtle)'">Close</button>
      </div>
      
      <div class="detail-grid">
        <div class="detail-label">Address</div>
        <div class="detail-value" style="display:flex;align-items:center;gap:12px">
          <span>${addressData.address}</span>
          <button onclick="copyToClipboard('${addressData.address}')" style="background:var(--bg3);border:1px solid var(--border);color:var(--subtle);padding:4px 12px;border-radius:2px;cursor:pointer;font-family:'DM Mono',monospace;font-size:10px;transition:all .2s" onmouseover="this.style.borderColor='var(--accent)';this.style.color='var(--accent)'" onmouseout="this.style.borderColor='var(--border)';this.style.color='var(--subtle)'">Copy</button>
        </div>
        
        <div class="detail-label">Balance</div>
        <div class="detail-value">${addressData.balance_udag.toFixed(8)} UDAG <span style="color:var(--muted);margin-left:8px">(${addressData.balance.toLocaleString()} sats)</span></div>
        
        <div class="detail-label">Nonce</div>
        <div class="detail-value">${addressData.nonce} <span style="color:var(--muted);margin-left:8px">(transactions sent)</span></div>
      </div>
    </div>
  `;
  
  detailView.innerHTML = html;
};

// Close detail view
window.closeDetail = function() {
  document.getElementById('detail-view').style.display = 'none';
};

// Show keyboard shortcuts modal
window.showShortcuts = function() {
  const detailView = document.getElementById('detail-view');
  detailView.style.display = 'block';
  detailView.innerHTML = `
    <div class="detail-card">
      <div class="detail-header">
        <div class="detail-title">⌨️ Keyboard Shortcuts</div>
        <button onclick="closeDetail()" style="background:var(--bg3);border:1px solid var(--border);color:var(--subtle);padding:8px 16px;border-radius:2px;cursor:pointer;font-family:'DM Mono',monospace;font-size:11px;transition:all .2s" onmouseover="this.style.borderColor='var(--accent)';this.style.color='var(--accent)'" onmouseout="this.style.borderColor='var(--border)';this.style.color='var(--subtle)'">Close</button>
      </div>
      
      <div style="display:grid;grid-template-columns:200px 1fr;gap:16px;margin-top:24px">
        <div style="font-family:'DM Mono',monospace;font-size:13px;color:var(--muted)">
          <kbd style="background:var(--bg3);padding:4px 8px;border-radius:2px;border:1px solid var(--border);display:inline-block">Ctrl/Cmd + K</kbd>
        </div>
        <div style="color:var(--body)">Focus search bar</div>
        
        <div style="font-family:'DM Mono',monospace;font-size:13px;color:var(--muted)">
          <kbd style="background:var(--bg3);padding:4px 8px;border-radius:2px;border:1px solid var(--border);display:inline-block">Escape</kbd>
        </div>
        <div style="color:var(--body)">Close detail view or modal</div>
        
        <div style="font-family:'DM Mono',monospace;font-size:13px;color:var(--muted)">
          <kbd style="background:var(--bg3);padding:4px 8px;border-radius:2px;border:1px solid var(--border);display:inline-block">←</kbd>
          <kbd style="background:var(--bg3);padding:4px 8px;border-radius:2px;border:1px solid var(--border);display:inline-block">→</kbd>
        </div>
        <div style="color:var(--body)">Navigate pagination (previous/next page)</div>
        
        <div style="font-family:'DM Mono',monospace;font-size:13px;color:var(--muted)">
          <kbd style="background:var(--bg3);padding:4px 8px;border-radius:2px;border:1px solid var(--border);display:inline-block">R</kbd>
        </div>
        <div style="color:var(--body)">Refresh data manually</div>
        
        <div style="font-family:'DM Mono',monospace;font-size:13px;color:var(--muted)">
          <kbd style="background:var(--bg3);padding:4px 8px;border-radius:2px;border:1px solid var(--border);display:inline-block">Enter</kbd>
        </div>
        <div style="color:var(--body)">Submit search (when in search box)</div>
      </div>
      
      <div style="margin-top:32px;padding-top:24px;border-top:1px solid var(--border)">
        <h4 style="font-family:'DM Mono',monospace;font-size:11px;letter-spacing:.1em;text-transform:uppercase;color:var(--accent);margin-bottom:16px">Tips</h4>
        <ul style="list-style:none;display:flex;flex-direction:column;gap:12px;color:var(--subtle);font-size:14px">
          <li>• Click any hash or address to copy it to clipboard</li>
          <li>• Click validator addresses to view their details</li>
          <li>• Click round numbers to see full round details</li>
          <li>• Auto-refresh updates data every 5 seconds</li>
          <li>• Stats flash green when values increase</li>
        </ul>
      </div>
    </div>
  `;
  detailView.scrollIntoView({ behavior: 'smooth' });
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
  } else if (tabName === 'validators') {
    loadValidators();
  } else if (tabName === 'transactions') {
    loadTransactions();
  }
}

// Load validators leaderboard
async function loadValidators() {
  const tbody = document.getElementById('validators-tbody');
  tbody.innerHTML = '<tr><td colspan="6" class="loading">Loading validators</td></tr>';
  
  if (currentRound === 0) {
    await fetchStatus();
  }
  
  // Aggregate validator stats from all rounds
  const validatorStats = new Map();
  
  for (let round = 1; round <= currentRound; round++) {
    const roundData = await fetchRound(round);
    if (!roundData) continue;
    
    roundData.forEach(vertex => {
      const addr = vertex.validator;
      if (!validatorStats.has(addr)) {
        validatorStats.set(addr, {
          address: addr,
          vertices: 0,
          rewards: 0,
          rounds: new Set()
        });
      }
      
      const stats = validatorStats.get(addr);
      stats.vertices++;
      stats.rewards += vertex.reward;
      stats.rounds.add(round);
    });
  }
  
  // Convert to array and sort by vertices produced
  const validators = Array.from(validatorStats.values())
    .sort((a, b) => b.vertices - a.vertices);
  
  if (validators.length === 0) {
    tbody.innerHTML = '<tr><td colspan="6" class="empty">No validators found</td></tr>';
    return;
  }
  
  tbody.innerHTML = validators.map((v, index) => {
    const participation = ((v.rounds.size / currentRound) * 100).toFixed(1);
    const isActive = v.rounds.has(currentRound);
    
    return `
      <tr>
        <td><strong style="color:var(--accent)">#${index + 1}</strong></td>
        <td class="hash">
          <a href="#address-${v.address}" onclick="event.preventDefault();viewAddress('${v.address}')" title="View validator details">${shortAddress(v.address)}</a>
          <span style="cursor:pointer;margin-left:8px;opacity:0.5;font-size:11px" onclick="copyToClipboard('${v.address}')" title="Copy address">📋</span>
        </td>
        <td><strong>${v.vertices.toLocaleString()}</strong></td>
        <td>${formatUdag(v.rewards)} UDAG</td>
        <td>
          <div style="display:flex;align-items:center;gap:8px">
            <div style="flex:1;background:var(--bg3);height:6px;border-radius:3px;overflow:hidden">
              <div style="width:${participation}%;height:100%;background:var(--accent);transition:width 0.3s"></div>
            </div>
            <span style="font-family:'DM Mono',monospace;font-size:12px">${participation}%</span>
          </div>
        </td>
        <td><span class="badge ${isActive ? 'badge-success' : 'badge-warning'}">${isActive ? '✓ Active' : '○ Inactive'}</span></td>
      </tr>
    `;
  }).join('');
}

// Load transactions (placeholder - would need tx history API)
async function loadTransactions() {
  const tbody = document.getElementById('transactions-tbody');
  tbody.innerHTML = '<tr><td colspan="6" class="empty">Transaction history requires additional API endpoint</td></tr>';
}

// Keyboard shortcuts
document.addEventListener('keydown', (e) => {
  // Ctrl/Cmd + K: Focus search
  if ((e.ctrlKey || e.metaKey) && e.key === 'k') {
    e.preventDefault();
    document.getElementById('search-input').focus();
  }
  
  // Escape: Close detail view
  if (e.key === 'Escape') {
    const detailView = document.getElementById('detail-view');
    if (detailView.style.display === 'block') {
      closeDetail();
    }
  }
  
  // Arrow keys for pagination (when not in input)
  if (document.activeElement.tagName !== 'INPUT') {
    if (e.key === 'ArrowLeft' && currentPage > 1) {
      changePage(currentPage - 1);
    } else if (e.key === 'ArrowRight') {
      const maxPages = Math.ceil(currentRound / ROUNDS_PER_PAGE);
      if (currentPage < maxPages) {
        changePage(currentPage + 1);
      }
    }
  }
  
  // R: Refresh data
  if (e.key === 'r' && !e.ctrlKey && !e.metaKey && document.activeElement.tagName !== 'INPUT') {
    fetchStatus();
    loadRounds();
    showNotification('Data refreshed');
  }
});

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

// Auto-refresh toggle
window.toggleAutoRefresh = function() {
  autoRefreshEnabled = !autoRefreshEnabled;
  const btn = document.getElementById('auto-refresh-btn');
  if (btn) {
    btn.textContent = autoRefreshEnabled ? '🔄 Auto-refresh ON' : '⏸️ Auto-refresh OFF';
    btn.style.background = autoRefreshEnabled ? 'var(--success)' : 'var(--muted)';
  }
  if (autoRefreshEnabled) {
    showNotification('Auto-refresh enabled');
  } else {
    showNotification('Auto-refresh paused');
  }
};

// Update last refresh time display
function updateRefreshTime() {
  const timeEl = document.getElementById('last-update');
  if (timeEl && lastUpdateTime) {
    const secondsAgo = Math.floor((Date.now() - lastUpdateTime) / 1000);
    timeEl.textContent = secondsAgo === 0 ? 'just now' : `${secondsAgo}s ago`;
  }
}

// Track stats history for mini charts
function trackStats(data) {
  statsHistory.push({
    timestamp: Date.now(),
    round: data.dag_round,
    vertices: data.dag_vertices,
    supply: data.total_supply,
    accounts: data.account_count
  });
  
  // Keep only last 50 data points
  if (statsHistory.length > 50) {
    statsHistory.shift();
  }
}

// Initialize
(async function init() {
  await fetchStatus();
  await loadRounds();
  
  // Refresh status every 5 seconds
  setInterval(async () => {
    if (autoRefreshEnabled) {
      await fetchStatus();
      // Auto-reload rounds if on first page
      if (currentPage === 1) {
        await loadRounds();
      }
    }
  }, 5000);
  
  // Update "last updated" time every second
  setInterval(updateRefreshTime, 1000);
})();
