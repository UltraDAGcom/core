import { useState, useEffect, useRef, useCallback } from 'react';
import { getBalance } from '../lib/api';
import type { Wallet } from '../lib/keystore';

export interface WalletBalance {
  address: string;
  balance: number;
  nonce: number;
  staked: number;
  delegated: number;
  is_active_validator: boolean;
  commission_percent: number;
  error?: string;
}

export function useWalletBalances(wallets: Wallet[], connected: boolean) {
  const [balances, setBalances] = useState<Map<string, WalletBalance>>(new Map());
  const [loading, setLoading] = useState(false);
  const intervalRef = useRef<ReturnType<typeof setInterval> | null>(null);

  const fetchAll = useCallback(async () => {
    if (!connected || wallets.length === 0) return;
    setLoading(true);
    const results = new Map<string, WalletBalance>();

    // Only fetch balance for first 10 wallets to avoid request spam
    const walletsToFetch = wallets.slice(0, 10);

    // Fetch sequentially (not parallel) to avoid overwhelming the node
    for (const w of walletsToFetch) {
      try {
        const bal = await getBalance(w.address).catch(() => null);
        results.set(w.address, {
          address: w.address,
          balance: bal?.balance ?? 0,
          nonce: bal?.nonce ?? 0,
          staked: bal?.staked ?? 0,
          delegated: bal?.delegated ?? 0,
          is_active_validator: bal?.is_active_validator ?? false,
          commission_percent: bal?.commission_percent ?? 10,
        });
      } catch {
        results.set(w.address, {
          address: w.address, balance: 0, nonce: 0, staked: 0, delegated: 0,
          is_active_validator: false, commission_percent: 10,
        });
      }
    }

    setBalances(results);
    setLoading(false);
  }, [wallets, connected]);

  useEffect(() => {
    fetchAll();
    // Poll every 30 seconds (was 10s — too aggressive with many wallets)
    intervalRef.current = setInterval(fetchAll, 30_000);
    return () => {
      if (intervalRef.current) clearInterval(intervalRef.current);
    };
  }, [fetchAll]);

  // Clear stale balances and refetch on network switch
  useEffect(() => {
    const handler = () => { setBalances(new Map()); fetchAll(); };
    window.addEventListener('ultradag-network-switch', handler);
    return () => window.removeEventListener('ultradag-network-switch', handler);
  }, [fetchAll]);

  const totalBalance = Array.from(balances.values()).reduce((s, b) => s + b.balance, 0);
  const totalStaked = Array.from(balances.values()).reduce((s, b) => s + b.staked, 0);
  const totalDelegated = Array.from(balances.values()).reduce((s, b) => s + b.delegated, 0);

  return { balances, loading, totalBalance, totalStaked, totalDelegated, refresh: fetchAll };
}
