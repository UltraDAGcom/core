import * as store from '../store.js';
import * as api from '../api.js';
import { formatUdag, toast } from '../utils.js';

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
        <!-- Faucet -->
        <div class="card" style="margin-bottom: 1rem; border-color: var(--accent); border-width: 1px;">
          <div class="card-header">
            <span class="card-title" style="color: var(--accent)">Testnet Faucet</span>
            <span style="font-size: .75rem; color: var(--fg3)">Get 100 UDAG to start using UltraDAG</span>
          </div>
          <div class="card-body">
            <div style="display: flex; gap: .5rem; align-items: flex-end;">
              <div style="flex: 1;">
                <label style="font-size: .7rem; text-transform: uppercase; letter-spacing: .08em; color: var(--fg3); display: block; margin-bottom: .25rem;">Your Address (64-char hex)</label>
                <input type="text" id="faucet-address" placeholder="Paste your UltraDAG address here..." style="width: 100%; font-family: 'DM Mono', monospace; font-size: .85rem;">
              </div>
              <button class="btn btn-accent" id="btn-faucet" style="white-space: nowrap; padding: .5rem 1.25rem;">
                Get 100 UDAG
              </button>
            </div>
            <div id="faucet-result" style="margin-top: .5rem; font-size: .8rem; font-family: 'DM Mono', monospace; display: none;"></div>
          </div>
        </div>

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

    bindFaucet();
  }

  function bindFaucet() {
    const btn = el.querySelector('#btn-faucet');
    const input = el.querySelector('#faucet-address');
    const result = el.querySelector('#faucet-result');
    if (!btn) return;

    btn.addEventListener('click', async () => {
      const address = input.value.trim().toLowerCase();

      if (!address || address.length !== 64 || !/^[0-9a-f]+$/.test(address)) {
        result.style.display = 'block';
        result.style.color = 'var(--red, #f87171)';
        result.textContent = 'Enter a valid 64-character hex address.';
        return;
      }

      btn.disabled = true;
      btn.textContent = 'Sending...';
      result.style.display = 'none';

      try {
        const amount = 100 * 100_000_000; // 100 UDAG in sats
        const data = await api.faucet(address, amount);
        result.style.display = 'block';
        result.style.color = 'var(--green, #4ade80)';
        result.textContent = '100 UDAG sent! TX: ' + data.tx_hash.slice(0, 24) + '...';
        toast('100 UDAG sent to ' + address.slice(0, 8) + '...', 'ok');
      } catch (e) {
        result.style.display = 'block';
        result.style.color = 'var(--red, #f87171)';
        result.textContent = e.message || 'Faucet request failed';
      } finally {
        btn.disabled = false;
        btn.textContent = 'Get 100 UDAG';
      }
    });

    // Allow Enter key
    input.addEventListener('keydown', (e) => {
      if (e.key === 'Enter') btn.click();
    });
  }

  render();
  const unsub = store.on('status', render);
  const unsub2 = store.on('connected', render);

  return {
    cleanup() { unsub(); unsub2(); }
  };
}
