import * as store from '../store.js';
import * as api from '../api.js';
import { shortAddr, formatUdag, formatSats } from '../utils.js';

export function mount(container) {
  const el = document.createElement('div');
  container.appendChild(el);
  let refreshTimer = null;

  async function loadAndRender() {
    const connected = store.get('connected');
    if (!connected) {
      el.innerHTML = `
        <div class="page-header">
          <div class="page-title">Mempool</div>
          <div class="page-sub">Pending transactions</div>
        </div>
        <div class="page-content">
          <div class="empty-state">Connect to a node to see the mempool</div>
        </div>
      `;
      return;
    }

    let txs = [];
    try {
      txs = await api.getMempool();
    } catch (e) { /* empty */ }

    el.innerHTML = `
      <div class="page-header">
        <div class="page-title">Mempool</div>
        <div class="page-sub">${txs.length} pending transaction${txs.length !== 1 ? 's' : ''}</div>
      </div>
      <div class="page-content">
        <div class="card">
          <div class="card-header">
            <span class="card-title">Pending Transactions</span>
            <button class="btn btn-sm" id="btn-refresh-mp">Refresh</button>
          </div>
          <div class="card-body" style="padding:${txs.length ? '.5rem' : '1.15rem'}">
            ${txs.length === 0
              ? '<div class="empty-state">Mempool is empty</div>'
              : txs.map(renderTx).join('')
            }
          </div>
        </div>
      </div>
    `;

    el.querySelector('#btn-refresh-mp')?.addEventListener('click', loadAndRender);
  }

  function renderTx(tx) {
    return `
      <div style="padding:.65rem .5rem;border-bottom:1px solid rgba(255,255,255,.03)">
        <div style="display:flex;justify-content:space-between;align-items:center;margin-bottom:.2rem">
          <span style="font-family:var(--mono);font-size:.68rem;color:var(--fg3)">${tx.hash.slice(0, 20)}...</span>
          <span class="badge">${formatSats(tx.fee)} fee</span>
        </div>
        <div style="display:flex;justify-content:space-between;align-items:center">
          <span style="font-size:.75rem;color:var(--fg2)">
            ${shortAddr(tx.from)} &rarr; ${shortAddr(tx.to)}
          </span>
          <span style="font-family:var(--mono);font-size:.82rem;font-weight:600">
            ${formatUdag(tx.amount)} UDAG
          </span>
        </div>
      </div>
    `;
  }

  loadAndRender();
  // Auto-refresh every 5 seconds
  refreshTimer = setInterval(loadAndRender, 5000);
  const unsub = store.on('connected', loadAndRender);

  return {
    cleanup() {
      unsub();
      clearInterval(refreshTimer);
    }
  };
}
