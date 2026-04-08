// UltraDAG Dashboard — main entry point

import * as api from './api.js';
import * as store from './store.js';
import * as router from './router.js';
import { toast } from './utils.js';
import { mount as mountSidebar } from './components/sidebar.js';
import { mount as mountDashboard } from './components/dashboard.js';
import { mount as mountWallet } from './components/wallet-view.js';
import { mount as mountExplorer } from './components/explorer.js';
import { mount as mountMempool } from './components/mempool.js';
import { mount as mountNetwork } from './components/network.js';

let pollTimer = null;
let currentCleanup = null;

// Register routes
router.register('dashboard', (c) => { currentCleanup?.(); const v = mountDashboard(c); currentCleanup = v?.cleanup; return v; });
router.register('wallet',    (c) => { currentCleanup?.(); const v = mountWallet(c);    currentCleanup = v?.cleanup; return v; });
router.register('explorer',  (c) => { currentCleanup?.(); const v = mountExplorer(c);  currentCleanup = v?.cleanup; return v; });
router.register('mempool',   (c) => { currentCleanup?.(); const v = mountMempool(c);   currentCleanup = v?.cleanup; return v; });
router.register('network',   (c) => { currentCleanup?.(); const v = mountNetwork(c);   currentCleanup = v?.cleanup; return v; });

async function connect(url) {
  api.setNodeUrl(url);
  store.set('nodeUrl', url);

  try {
    const data = await api.getStatus();
    store.set('connected', true);
    store.set('status', data);
    toast('Connected to node', 'ok');
    startPolling();
  } catch (e) {
    store.set('connected', false);
    store.set('status', null);
    toast('Connection failed: ' + e.message, 'err');
  }
}

async function poll() {
  if (!store.get('connected')) return;
  try {
    const data = await api.getStatus();
    store.set('status', data);
  } catch (e) {
    store.set('connected', false);
    store.set('status', null);
    stopPolling();
  }
}

function startPolling() {
  stopPolling();
  pollTimer = setInterval(poll, 5000);
}

function stopPolling() {
  if (pollTimer) { clearInterval(pollTimer); pollTimer = null; }
}

// Listen for connect events from network page
window.addEventListener('ultradag-connect', (e) => connect(e.detail));

// Mobile sidebar toggle
document.querySelector('.mobile-toggle')?.addEventListener('click', () => {
  document.querySelector('.sidebar')?.classList.toggle('open');
});

// Init
store.loadWallets();
mountSidebar(document.querySelector('.sidebar'));
router.start(document.querySelector('.main'));

// Auto-connect
connect(store.get('nodeUrl'));
