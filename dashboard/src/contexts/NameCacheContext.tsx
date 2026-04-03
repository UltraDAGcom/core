import { createContext, useContext, useState, useCallback, useRef, useEffect } from 'react';
import { getNodeUrl } from '../lib/api';

interface NameEntry {
  name: string | null;
  fetchedAt: number;
}

interface NameCacheContextValue {
  /** Get the cached ULTRA ID for a hex address. Returns null if unknown or not yet fetched. */
  getName: (hexAddr: string) => string | null;
  /** Whether a name is currently being fetched for this address */
  isLoading: (hexAddr: string) => boolean;
  /** Request name resolution for an address (batched, deduped) */
  resolve: (hexAddr: string) => void;
  /** Bulk-seed the cache (e.g. from useWalletBalances) */
  seed: (entries: Array<{ address: string; name: string | null }>) => void;
}

const TTL_MS = 5 * 60 * 1000; // 5 minutes

const NameCacheCtx = createContext<NameCacheContextValue | null>(null);

export function NameCacheProvider({ children }: { children: React.ReactNode }) {
  const cache = useRef<Map<string, NameEntry>>(new Map());
  const inflight = useRef<Set<string>>(new Set());
  const pending = useRef<Set<string>>(new Set());
  const batchTimer = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);
  const [, bump] = useState(0);

  const rerender = useCallback(() => bump(n => n + 1), []);

  const isStale = useCallback((key: string) => {
    const entry = cache.current.get(key);
    if (!entry) return true;
    return Date.now() - entry.fetchedAt > TTL_MS;
  }, []);

  const fetchOne = useCallback(async (hexAddr: string) => {
    if (inflight.current.has(hexAddr)) return;
    inflight.current.add(hexAddr);
    try {
      const res = await fetch(`${getNodeUrl()}/balance/${hexAddr}`, { signal: AbortSignal.timeout(4000) });
      if (res.ok) {
        const data = await res.json();
        cache.current.set(hexAddr.toLowerCase(), { name: data.name ?? null, fetchedAt: Date.now() });
      } else {
        cache.current.set(hexAddr.toLowerCase(), { name: null, fetchedAt: Date.now() });
      }
    } catch {
      // network error — don't cache, allow retry
    } finally {
      inflight.current.delete(hexAddr);
      rerender();
    }
  }, [rerender]);

  const flushPending = useCallback(() => {
    const batch = Array.from(pending.current);
    pending.current.clear();
    // Cap concurrent requests
    const MAX_CONCURRENT = 5;
    const toFetch = batch.filter(a => !inflight.current.has(a)).slice(0, MAX_CONCURRENT);
    toFetch.forEach(fetchOne);
  }, [fetchOne]);

  const resolve = useCallback((hexAddr: string) => {
    const key = hexAddr.toLowerCase();
    if (!isStale(key) || inflight.current.has(key)) return;
    pending.current.add(key);
    clearTimeout(batchTimer.current);
    batchTimer.current = setTimeout(flushPending, 50);
  }, [isStale, flushPending]);

  const getName = useCallback((hexAddr: string): string | null => {
    const key = hexAddr.toLowerCase();
    const entry = cache.current.get(key);
    if (!entry || isStale(key)) {
      resolve(key);
      return entry?.name ?? null;
    }
    return entry.name;
  }, [isStale, resolve]);

  const isLoading = useCallback((hexAddr: string): boolean => {
    const key = hexAddr.toLowerCase();
    return inflight.current.has(key) || pending.current.has(key);
  }, []);

  const seed = useCallback((entries: Array<{ address: string; name: string | null }>) => {
    let changed = false;
    for (const { address, name } of entries) {
      const key = address.toLowerCase();
      const existing = cache.current.get(key);
      if (!existing || isStale(key) || existing.name !== name) {
        cache.current.set(key, { name, fetchedAt: Date.now() });
        changed = true;
      }
    }
    if (changed) rerender();
  }, [isStale, rerender]);

  // Clear cache on network switch
  useEffect(() => {
    const handler = () => {
      cache.current.clear();
      inflight.current.clear();
      pending.current.clear();
      rerender();
    };
    window.addEventListener('ultradag-network-switch', handler);
    return () => window.removeEventListener('ultradag-network-switch', handler);
  }, [rerender]);

  return (
    <NameCacheCtx.Provider value={{ getName, isLoading, resolve, seed }}>
      {children}
    </NameCacheCtx.Provider>
  );
}

/** Get the ULTRA ID (registered name) for a hex address. Auto-resolves on first call. */
export function useName(hexAddr: string | undefined): { name: string | null; loading: boolean } {
  const ctx = useContext(NameCacheCtx);
  if (!ctx || !hexAddr) return { name: null, loading: false };
  const name = ctx.getName(hexAddr);
  const loading = ctx.isLoading(hexAddr);
  return { name, loading };
}

/** Bulk name resolution for lists of addresses. */
export function useNames(hexAddrs: string[]): Map<string, string | null> {
  const ctx = useContext(NameCacheCtx);
  const result = new Map<string, string | null>();
  if (!ctx) return result;
  for (const addr of hexAddrs) {
    result.set(addr, ctx.getName(addr));
  }
  return result;
}

/** Direct access to the name cache context (for seeding from useWalletBalances). */
export function useNameCache(): NameCacheContextValue | null {
  return useContext(NameCacheCtx);
}
