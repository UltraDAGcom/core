import * as store from '../store.js';
import * as api from '../api.js';
import * as icons from '../icons.js';
import { formatUdag, formatSats, shortAddr, copyText, toast } from '../utils.js';

export function mount(container) {
  const el = document.createElement('div');
  container.appendChild(el);

  function render() {
    const wallets = store.get('wallets');
    const activeIdx = store.get('activeWallet');
    const active = store.getActiveWallet();
    const balance = store.get('balance');
    const connected = store.get('connected');

    el.innerHTML = `
      <div class="page-header">
        <div class="page-title">Wallet</div>
        <div class="page-sub">Manage keys and send UDAG</div>
      </div>
      <div class="page-content">
        <div class="grid-2">
          <!-- Wallet list -->
          <div class="stack">
            <div class="card">
              <div class="card-header">
                <span class="card-title">Wallets</span>
                <div style="display:flex;gap:.35rem">
                  <button class="btn btn-sm btn-accent" id="btn-new-wallet">${icons.plus} New</button>
                  <button class="btn btn-sm" id="btn-import-wallet">${icons.upload} Import</button>
                </div>
              </div>
              <div class="card-body" style="padding:.5rem">
                ${wallets.length === 0
                  ? '<div class="empty-state">No wallets yet. Create or import one.</div>'
                  : wallets.map((w, i) => `
                    <div class="wallet-item ${i === activeIdx ? 'active' : ''}" data-idx="${i}">
                      <div class="wallet-icon">${w.name.charAt(0).toUpperCase()}</div>
                      <div>
                        <div class="wallet-name">${esc(w.name)}</div>
                        <div class="wallet-addr">${shortAddr(w.address)}</div>
                      </div>
                    </div>
                  `).join('')
                }
              </div>
            </div>

            <!-- Import dialog (hidden by default) -->
            <div class="card" id="import-dialog" style="display:none">
              <div class="card-header"><span class="card-title">Import Secret Key</span></div>
              <div class="card-body">
                <div class="form-group">
                  <label>Name</label>
                  <input type="text" id="import-name" placeholder="My wallet">
                </div>
                <div class="form-group">
                  <label>Secret Key (64 hex chars)</label>
                  <input type="text" id="import-sk" placeholder="Paste hex secret key...">
                </div>
                <div style="display:flex;gap:.35rem">
                  <button class="btn btn-sm btn-accent" id="btn-do-import">Import</button>
                  <button class="btn btn-sm" id="btn-cancel-import">Cancel</button>
                </div>
              </div>
            </div>
          </div>

          <!-- Active wallet detail -->
          <div class="stack">
            ${active ? renderActiveWallet(active, balance, connected) : '<div class="card"><div class="card-body"><div class="empty-state">Select or create a wallet</div></div></div>'}

            ${active ? renderSendForm(connected) : ''}
          </div>
        </div>
      </div>
    `;

    bindEvents();
  }

  function renderActiveWallet(w, balance, connected) {
    return `
      <div class="card">
        <div class="card-header">
          <span class="card-title">${esc(w.name)}</span>
          <div style="display:flex;gap:.35rem">
            <button class="btn btn-sm" id="btn-refresh-bal" ${!connected ? 'disabled' : ''}>Refresh</button>
            <button class="btn btn-sm" id="btn-export-key">${icons.download}</button>
            <button class="btn btn-sm" id="btn-delete-wallet" style="color:var(--red)">${icons.trash}</button>
          </div>
        </div>
        <div class="card-body">
          <div class="stat-label">Balance</div>
          <div class="stat-value" style="margin:.25rem 0">${balance ? formatUdag(balance.balance) + ' UDAG' : (connected ? 'Loading...' : '...')}</div>
          ${balance ? `<div class="stat-sub">${formatSats(balance.balance)} sats | nonce: ${balance.nonce}</div>` : ''}

          <div style="margin-top:1rem">
            <div class="stat-label">Address</div>
            <div class="mono-box" style="margin-top:.25rem">
              <span id="full-addr">${w.address}</span>
              <button class="copy-btn" id="btn-copy-addr">copy</button>
            </div>
          </div>
        </div>
      </div>
    `;
  }

  function renderSendForm(connected) {
    return `
      <div class="card">
        <div class="card-header"><span class="card-title">Send UDAG</span></div>
        <div class="card-body">
          <div class="form-group">
            <label>Recipient Address</label>
            <input type="text" id="send-to" placeholder="64-char hex address">
          </div>
          <div class="form-row">
            <div class="form-group">
              <label>Amount (UDAG)</label>
              <input type="number" id="send-amount" placeholder="0.00" step="0.00000001" min="0">
            </div>
            <div class="form-group">
              <label>Fee (sats)</label>
              <input type="number" id="send-fee" value="100000" min="0">
              <div class="form-help">1 UDAG = 100,000,000 sats</div>
            </div>
          </div>
          <button class="btn btn-accent btn-full" id="btn-send" ${!connected ? 'disabled' : ''} style="margin-top:.5rem">
            ${icons.send} Send Transaction
          </button>
        </div>
      </div>
    `;
  }

  function bindEvents() {
    // Select wallet
    el.querySelectorAll('.wallet-item').forEach(item => {
      item.addEventListener('click', () => {
        store.setActiveWallet(parseInt(item.dataset.idx));
        render();
        refreshActiveBalance();
      });
    });

    // New wallet
    el.querySelector('#btn-new-wallet')?.addEventListener('click', async () => {
      if (!store.get('connected')) {
        toast('Connect to a node first', 'info');
        return;
      }
      try {
        const data = await api.keygen();
        const name = 'Wallet ' + (store.get('wallets').length + 1);
        store.addWallet({ name, address: data.address, secret_key: data.secret_key });
        render();
        refreshActiveBalance();
        toast('New wallet created', 'ok');
      } catch (e) {
        toast('Keygen failed: ' + e.message, 'err');
      }
    });

    // Import wallet toggle
    el.querySelector('#btn-import-wallet')?.addEventListener('click', () => {
      const dlg = el.querySelector('#import-dialog');
      dlg.style.display = dlg.style.display === 'none' ? 'block' : 'none';
    });

    el.querySelector('#btn-cancel-import')?.addEventListener('click', () => {
      el.querySelector('#import-dialog').style.display = 'none';
    });

    el.querySelector('#btn-do-import')?.addEventListener('click', () => {
      const name = el.querySelector('#import-name').value.trim() || 'Imported';
      const sk = el.querySelector('#import-sk').value.trim().toLowerCase();
      if (sk.length !== 64 || !/^[0-9a-f]+$/.test(sk)) {
        toast('Secret key must be exactly 64 hex characters', 'err');
        return;
      }
      // We don't know the address without Ed25519 in browser.
      // We'll mark it as unknown and discover it on first tx.
      // Actually, the keygen endpoint doesn't help here. We need to derive it.
      // For now, store with placeholder address. The /tx endpoint returns the address.
      store.addWallet({ name, address: '(send a tx to discover address)', secret_key: sk });
      el.querySelector('#import-dialog').style.display = 'none';
      render();
      toast('Key imported. Send a transaction to discover your address.', 'info');
    });

    // Active wallet buttons
    el.querySelector('#btn-refresh-bal')?.addEventListener('click', () => refreshActiveBalance());

    el.querySelector('#btn-copy-addr')?.addEventListener('click', () => {
      const addr = store.getActiveWallet()?.address;
      if (addr) copyText(addr);
    });

    el.querySelector('#btn-export-key')?.addEventListener('click', () => {
      const w = store.getActiveWallet();
      if (!w) return;
      const blob = new Blob([w.secret_key], { type: 'text/plain' });
      const a = document.createElement('a');
      a.href = URL.createObjectURL(blob);
      a.download = 'ultradag-' + w.name.replace(/\s+/g, '-') + '.key';
      a.click();
      URL.revokeObjectURL(a.href);
      toast('Secret key exported', 'ok');
    });

    el.querySelector('#btn-delete-wallet')?.addEventListener('click', () => {
      const w = store.getActiveWallet();
      if (!w) return;
      if (!confirm(`Delete wallet "${w.name}"? Make sure you have backed up your secret key.`)) return;
      store.removeWallet(store.get('activeWallet'));
      render();
      toast('Wallet deleted', 'ok');
    });

    // Send
    el.querySelector('#btn-send')?.addEventListener('click', handleSend);
  }

  async function handleSend() {
    const w = store.getActiveWallet();
    if (!w || !store.get('connected')) return;

    const to = el.querySelector('#send-to').value.trim();
    const amountUdag = parseFloat(el.querySelector('#send-amount').value);
    const fee = parseInt(el.querySelector('#send-fee').value) || 100000;

    if (!to || to.length !== 64) { toast('Recipient must be 64-char hex', 'err'); return; }
    if (isNaN(amountUdag) || amountUdag <= 0) { toast('Enter a valid amount', 'err'); return; }

    const amountSats = Math.round(amountUdag * 100_000_000);
    const btn = el.querySelector('#btn-send');
    btn.disabled = true;
    btn.textContent = 'Sending...';

    try {
      const data = await api.sendTx({
        from_secret: w.secret_key,
        to,
        amount: amountSats,
        fee,
      });

      // Update address if we didn't have it
      if (data.from && (!w.address || w.address.startsWith('('))) {
        w.address = data.from;
        store.saveWallets();
        store.set('wallets', store.get('wallets'));
      }

      toast('TX sent! Hash: ' + data.hash.slice(0, 16) + '...', 'ok');
      el.querySelector('#send-to').value = '';
      el.querySelector('#send-amount').value = '';
      refreshActiveBalance();
    } catch (e) {
      toast('Send failed: ' + e.message, 'err');
    } finally {
      btn.disabled = false;
      btn.innerHTML = icons.send + ' Send Transaction';
    }
  }

  async function refreshActiveBalance() {
    const w = store.getActiveWallet();
    if (!w || !store.get('connected') || !w.address || w.address.startsWith('(')) return;
    try {
      const bal = await api.getBalance(w.address);
      store.set('balance', bal);
      render();
    } catch (e) {
      // Address might not exist on chain yet, that's fine
      store.set('balance', { balance: 0, nonce: 0 });
      render();
    }
  }

  render();
  if (store.get('connected') && store.getActiveWallet()) {
    refreshActiveBalance();
  }
  const unsub1 = store.on('wallets', render);
  const unsub2 = store.on('activeWallet', () => { render(); refreshActiveBalance(); });
  const unsub3 = store.on('connected', render);

  return {
    cleanup() { unsub1(); unsub2(); unsub3(); }
  };
}

function esc(s) {
  const div = document.createElement('div');
  div.textContent = s;
  return div.innerHTML;
}
