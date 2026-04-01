// Browser notification utilities for UltraDAG wallet

const STORAGE_KEY_ENABLED = 'ultradag_notifications_enabled';
const STORAGE_KEY_BALANCES = 'ultradag_last_balances';

export function isNotificationSupported(): boolean {
  return 'Notification' in window;
}

export function isEnabled(): boolean {
  try {
    return localStorage.getItem(STORAGE_KEY_ENABLED) === 'true';
  } catch {
    return false;
  }
}

export function setEnabled(enabled: boolean): void {
  try {
    localStorage.setItem(STORAGE_KEY_ENABLED, enabled ? 'true' : 'false');
  } catch {
    // localStorage unavailable
  }
}

export async function requestPermission(): Promise<boolean> {
  if (!isNotificationSupported()) return false;
  const result = await Notification.requestPermission();
  return result === 'granted';
}

export function hasPermission(): boolean {
  if (!isNotificationSupported()) return false;
  return Notification.permission === 'granted';
}

export function notify(title: string, body: string): void {
  if (!isNotificationSupported() || !hasPermission() || !isEnabled()) return;
  try {
    new Notification(title, {
      body,
      icon: '/favicon.ico',
      badge: '/favicon.ico',
      tag: 'ultradag-' + Date.now(),
    });
  } catch {
    // Notification creation failed (e.g. service worker context)
  }
}

// Balance tracking for change detection

interface StoredBalances {
  [address: string]: number;
}

function getStoredBalances(): StoredBalances {
  try {
    const raw = localStorage.getItem(STORAGE_KEY_BALANCES);
    if (raw) return JSON.parse(raw);
  } catch {
    // ignore
  }
  return {};
}

function setStoredBalances(balances: StoredBalances): void {
  try {
    localStorage.setItem(STORAGE_KEY_BALANCES, JSON.stringify(balances));
  } catch {
    // ignore
  }
}

/**
 * Check if a wallet's balance increased since last check.
 * Returns the increase amount (in sats) or 0 if no increase.
 * Updates the stored balance for future comparisons.
 */
export function checkBalanceChange(address: string, currentBalance: number): number {
  const stored = getStoredBalances();
  const lastBalance = stored[address];
  let increase = 0;

  if (lastBalance !== undefined && currentBalance > lastBalance) {
    increase = currentBalance - lastBalance;
  }

  // Always update stored balance
  stored[address] = currentBalance;
  setStoredBalances(stored);

  return increase;
}

/**
 * Format sats as UDAG for display in notifications.
 */
export function formatUdagNotification(sats: number): string {
  const udag = sats / 100_000_000;
  if (udag >= 1) {
    return udag.toFixed(2) + ' UDAG';
  }
  return udag.toFixed(8).replace(/0+$/, '').replace(/\.$/, '.0') + ' UDAG';
}
