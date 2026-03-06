import * as store from '../store.js';
import { formatUdag } from '../utils.js';

export function mount(container) {
  const el = document.createElement('div');
  container.appendChild(el);

  function render() {
    const s = store.get('status');
    const connected = store.get('connected');

    if (!connected || !s) {
      el.innerHTML = `
        <div class="page-header">
          <div class="page-title">Dashboard</div>
          <div class="page-sub">Network overview</div>
        </div>
        <div class="page-content">
          <div class="empty-state">Connect to a node to see network stats</div>
        </div>
      `;
      return;
    }

    el.innerHTML = `
      <div class="page-header">
        <div class="page-title">Dashboard</div>
        <div class="page-sub">UltraDAG network overview</div>
      </div>
      <div class="page-content">
        <div class="grid-4" style="margin-bottom: 1rem">
          <div class="stat-card">
            <div class="stat-label">DAG Round</div>
            <div class="stat-value">${s.dag_round}</div>
          </div>
          <div class="stat-card">
            <div class="stat-label">Finalized</div>
            <div class="stat-value">${s.finalized_count}</div>
            <div class="stat-sub">${s.last_finalized_round !== null ? 'round ' + s.last_finalized_round : 'none yet'}</div>
          </div>
          <div class="stat-card">
            <div class="stat-label">Total Supply</div>
            <div class="stat-value">${formatUdag(s.total_supply)}</div>
            <div class="stat-sub">UDAG</div>
          </div>
          <div class="stat-card">
            <div class="stat-label">Validators</div>
            <div class="stat-value">${s.validator_count}</div>
          </div>
        </div>

        <div class="grid-2">
          <div class="card">
            <div class="card-header"><span class="card-title">DAG Status</span></div>
            <div class="card-body">
              <div class="kv-row">
                <span class="kv-label">Vertices</span>
                <span class="kv-value">${s.dag_vertices}</span>
              </div>
              <div class="kv-row">
                <span class="kv-label">Current Round</span>
                <span class="kv-value">${s.dag_round}</span>
              </div>
              <div class="kv-row">
                <span class="kv-label">Tips</span>
                <span class="kv-value">${s.dag_tips}</span>
              </div>
              <div class="kv-row">
                <span class="kv-label">Finalized Vertices</span>
                <span class="kv-value">${s.finalized_count}</span>
              </div>
              <div class="kv-row">
                <span class="kv-label">Last Finalized Round</span>
                <span class="kv-value">${s.last_finalized_round ?? 'N/A'}</span>
              </div>
            </div>
          </div>

          <div class="card">
            <div class="card-header"><span class="card-title">Network</span></div>
            <div class="card-body">
              <div class="kv-row">
                <span class="kv-label">Peers</span>
                <span class="kv-value">${s.peer_count}</span>
              </div>
              <div class="kv-row">
                <span class="kv-label">Validators</span>
                <span class="kv-value">${s.validator_count}</span>
              </div>
              <div class="kv-row">
                <span class="kv-label">Mempool</span>
                <span class="kv-value">${s.mempool_size} txs</span>
              </div>
              <div class="kv-row">
                <span class="kv-label">Accounts</span>
                <span class="kv-value">${s.account_count}</span>
              </div>
              <div class="kv-row">
                <span class="kv-label">Supply</span>
                <span class="kv-value">${formatUdag(s.total_supply)} UDAG</span>
              </div>
            </div>
          </div>
        </div>
      </div>
    `;
  }

  render();
  const unsub = store.on('status', render);
  const unsub2 = store.on('connected', render);

  return {
    cleanup() { unsub(); unsub2(); }
  };
}
