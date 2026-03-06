import * as store from '../store.js';
import * as api from '../api.js';

export function mount(container) {
  const el = document.createElement('div');
  container.appendChild(el);

  function render() {
    const connected = store.get('connected');
    const nodeUrl = store.get('nodeUrl');
    const status = store.get('status');

    el.innerHTML = `
      <div class="page-header">
        <div class="page-title">Network</div>
        <div class="page-sub">Node connection and network status</div>
      </div>
      <div class="page-content">
        <div class="grid-2">
          <div class="card">
            <div class="card-header"><span class="card-title">Node Connection</span></div>
            <div class="card-body">
              <div class="form-group">
                <label>Node RPC URL</label>
                <div style="display:flex;gap:.5rem">
                  <input type="text" id="node-url" value="${nodeUrl}" placeholder="http://host:port">
                  <button class="btn btn-accent" id="btn-connect">${connected ? 'Reconnect' : 'Connect'}</button>
                </div>
              </div>
              <div style="margin-top:.75rem;display:flex;align-items:center;gap:.5rem">
                <div class="conn-dot ${connected ? 'ok' : ''}"></div>
                <span style="font-size:.78rem;color:${connected ? 'var(--green)' : 'var(--fg3)'}">
                  ${connected ? 'Connected' : 'Disconnected'}
                </span>
              </div>
            </div>
          </div>

          <div class="card">
            <div class="card-header"><span class="card-title">Network Info</span></div>
            <div class="card-body">
              ${status ? `
                <div class="kv-row">
                  <span class="kv-label">Peers</span>
                  <span class="kv-value">${status.peer_count}</span>
                </div>
                <div class="kv-row">
                  <span class="kv-label">Validators</span>
                  <span class="kv-value">${status.validator_count}</span>
                </div>
                <div class="kv-row">
                  <span class="kv-label">DAG Round</span>
                  <span class="kv-value">${status.dag_round}</span>
                </div>
                <div class="kv-row">
                  <span class="kv-label">Mempool Size</span>
                  <span class="kv-value">${status.mempool_size}</span>
                </div>
                <div class="kv-row">
                  <span class="kv-label">Accounts</span>
                  <span class="kv-value">${status.account_count}</span>
                </div>
              ` : '<div class="empty-state">Connect to see network info</div>'}
            </div>
          </div>
        </div>
      </div>
    `;

    el.querySelector('#btn-connect')?.addEventListener('click', () => {
      const url = el.querySelector('#node-url').value.trim();
      if (url) {
        store.set('nodeUrl', url);
        // Trigger reconnect via main app
        window.dispatchEvent(new CustomEvent('ultradag-connect', { detail: url }));
      }
    });

    el.querySelector('#node-url')?.addEventListener('keydown', (e) => {
      if (e.key === 'Enter') el.querySelector('#btn-connect').click();
    });
  }

  render();
  const unsub1 = store.on('connected', render);
  const unsub2 = store.on('status', render);

  return {
    cleanup() { unsub1(); unsub2(); }
  };
}
