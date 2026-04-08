import { useEffect, useRef, useCallback, useState } from 'react';
import {
  isNotificationSupported,
  isEnabled,
  setEnabled,
  requestPermission,
  hasPermission,
  notify,
  checkBalanceChange,
  formatUdagNotification,
} from '../lib/notifications.ts';
import type { WalletBalance } from './useWalletBalances.ts';

const POLL_INTERVAL_MS = 30_000; // 30 seconds

interface UseNotificationsOptions {
  /** Wallet addresses to monitor */
  addresses: string[];
  /** Current balance map from useWalletBalances */
  balances: Map<string, WalletBalance>;
  /** Whether the wallet is unlocked */
  unlocked: boolean;
}

export function useNotifications({ addresses, balances, unlocked }: UseNotificationsOptions) {
  const [enabled, setEnabledState] = useState(isEnabled());
  const [supported] = useState(isNotificationSupported());
  const [permission, setPermission] = useState(hasPermission());
  const intervalRef = useRef<ReturnType<typeof setInterval> | null>(null);

  const toggle = useCallback(async () => {
    if (enabled) {
      // Disable
      setEnabled(false);
      setEnabledState(false);
    } else {
      // Enable — request permission first if needed
      if (!hasPermission()) {
        const granted = await requestPermission();
        setPermission(granted);
        if (!granted) return;
      }
      setEnabled(true);
      setEnabledState(true);
    }
  }, [enabled]);

  // Poll for balance changes
  const checkChanges = useCallback(() => {
    if (!enabled || !unlocked || !hasPermission()) return;

    for (const address of addresses) {
      const wb = balances.get(address);
      if (!wb) continue;

      const increase = checkBalanceChange(address, wb.balance);
      if (increase > 0) {
        const shortAddr = address.slice(0, 8) + '...' + address.slice(-6);
        notify(
          'Payment Received',
          `+${formatUdagNotification(increase)} to ${shortAddr}`
        );
      }
    }
  }, [enabled, unlocked, addresses, balances]);

  useEffect(() => {
    if (!enabled || !unlocked) {
      if (intervalRef.current) {
        clearInterval(intervalRef.current);
        intervalRef.current = null;
      }
      return;
    }

    // Initial check
    checkChanges();

    // Set up polling
    intervalRef.current = setInterval(checkChanges, POLL_INTERVAL_MS);
    return () => {
      if (intervalRef.current) {
        clearInterval(intervalRef.current);
        intervalRef.current = null;
      }
    };
  }, [enabled, unlocked, checkChanges]);

  return {
    supported,
    enabled,
    permission,
    toggle,
  };
}
