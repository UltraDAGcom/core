// Global reactive state store

const listeners = new Map();
const state = {
  connected: false,
  nodeUrl: 'http://127.0.0.1:10333',
  status: null,        // latest /status response
  activeWallet: null,  // index into wallets array
  wallets: [],         // [{name, address, secret_key}]
  balance: null,       // balance of active wallet
};

export function get(key) {
  return state[key];
}

export function set(key, value) {
  state[key] = value;
  const fns = listeners.get(key);
  if (fns) fns.forEach(fn => fn(value));
  // Also notify wildcard listeners
  const all = listeners.get('*');
  if (all) all.forEach(fn => fn(key, value));
}

export function on(key, fn) {
  if (!listeners.has(key)) listeners.set(key, new Set());
  listeners.get(key).add(fn);
  return () => listeners.get(key).delete(fn);
}

// Wallet persistence
const WALLETS_KEY = 'ultradag_wallets';
const ACTIVE_KEY = 'ultradag_active_wallet';

export function loadWallets() {
  try {
    const raw = localStorage.getItem(WALLETS_KEY);
    if (raw) {
      state.wallets = JSON.parse(raw);
    }
    const active = localStorage.getItem(ACTIVE_KEY);
    if (active !== null && state.wallets.length > 0) {
      const idx = parseInt(active);
      state.activeWallet = idx < state.wallets.length ? idx : 0;
    }
  } catch (e) {
    console.error('Failed to load wallets:', e);
  }
}

export function saveWallets() {
  localStorage.setItem(WALLETS_KEY, JSON.stringify(state.wallets));
  if (state.activeWallet !== null) {
    localStorage.setItem(ACTIVE_KEY, String(state.activeWallet));
  }
}

export function addWallet(wallet) {
  state.wallets.push(wallet);
  if (state.activeWallet === null) state.activeWallet = 0;
  saveWallets();
  set('wallets', state.wallets);
  set('activeWallet', state.activeWallet);
}

export function removeWallet(index) {
  state.wallets.splice(index, 1);
  if (state.wallets.length === 0) {
    state.activeWallet = null;
  } else if (state.activeWallet >= state.wallets.length) {
    state.activeWallet = state.wallets.length - 1;
  }
  saveWallets();
  set('wallets', state.wallets);
  set('activeWallet', state.activeWallet);
}

export function setActiveWallet(index) {
  state.activeWallet = index;
  saveWallets();
  set('activeWallet', index);
  set('balance', null);
}

export function getActiveWallet() {
  if (state.activeWallet === null || !state.wallets.length) return null;
  return state.wallets[state.activeWallet];
}
