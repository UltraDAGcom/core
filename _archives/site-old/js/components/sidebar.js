import * as store from '../store.js';
import * as router from '../router.js';
import * as icons from '../icons.js';

const NAV_ITEMS = [
  { id: 'dashboard', label: 'Dashboard', icon: icons.dashboard },
  { id: 'wallet',    label: 'Wallet',    icon: icons.wallet },
  { id: 'explorer',  label: 'Explorer',  icon: icons.explorer },
  { id: 'mempool',   label: 'Mempool',   icon: icons.mempool },
  { id: 'network',   label: 'Network',   icon: icons.network },
];

export function mount(el) {
  function render() {
    const route = router.currentRoute();
    const connected = store.get('connected');
    const nodeUrl = store.get('nodeUrl');

    el.innerHTML = `
      <div class="sidebar-logo">Ultra<span>DAG</span></div>
      <div class="sidebar-connection">
        <div class="conn-row">
          <div class="conn-dot ${connected ? 'ok' : ''}"></div>
          <span class="conn-url">${connected ? nodeUrl.replace('http://', '') : 'Disconnected'}</span>
        </div>
      </div>
      <nav class="sidebar-nav">
        ${NAV_ITEMS.map(item => `
          <a class="sidebar-link ${route === item.id ? 'active' : ''}" data-route="${item.id}">
            ${item.icon}
            ${item.label}
          </a>
        `).join('')}
      </nav>
      <div class="sidebar-footer">
        <a href="index.html" style="color: var(--fg3)">ultradag.com</a>
      </div>
    `;

    el.querySelectorAll('.sidebar-link').forEach(link => {
      link.addEventListener('click', (e) => {
        e.preventDefault();
        router.navigate(link.dataset.route);
        // Close mobile sidebar
        el.classList.remove('open');
      });
    });
  }

  render();
  const unsub1 = store.on('connected', render);
  const unsub2 = store.on('nodeUrl', render);
  window.addEventListener('hashchange', render);

  return {
    cleanup() {
      unsub1();
      unsub2();
      window.removeEventListener('hashchange', render);
    }
  };
}
