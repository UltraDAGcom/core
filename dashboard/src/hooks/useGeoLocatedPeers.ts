import { useState, useEffect, useRef, useCallback } from 'react';
import { getPeers, getValidators } from '../lib/api';

export interface GeoLocatedPeer {
  ip: string;
  lat: number;
  lng: number;
  city: string;
  country: string;
  validator?: {
    address: string;
    effective_stake: number;
    delegator_count: number;
    commission_percent: number;
    is_active: boolean;
  };
}

interface GeoCache {
  lat: number;
  lng: number;
  city: string;
  country: string;
}

const CACHE_KEY = 'ultradag_geo_cache';
const POLL_INTERVAL = 30_000;

function loadCache(): Map<string, GeoCache> {
  try {
    const raw = sessionStorage.getItem(CACHE_KEY);
    if (raw) return new Map(JSON.parse(raw));
  } catch { /* ignore */ }
  return new Map();
}

function saveCache(cache: Map<string, GeoCache>) {
  try {
    sessionStorage.setItem(CACHE_KEY, JSON.stringify(Array.from(cache.entries())));
  } catch { /* ignore */ }
}

async function resolveGeoIP(ip: string): Promise<GeoCache | null> {
  try {
    const res = await fetch(`http://ip-api.com/json/${ip}?fields=status,lat,lon,city,country`, {
      signal: AbortSignal.timeout(4000),
    });
    if (!res.ok) return null;
    const data = await res.json();
    if (data.status !== 'success') return null;
    return { lat: data.lat, lng: data.lon, city: data.city || '', country: data.country || '' };
  } catch {
    return null;
  }
}

export function useGeoLocatedPeers() {
  const [peers, setPeers] = useState<GeoLocatedPeer[]>([]);
  const [loading, setLoading] = useState(true);
  const cacheRef = useRef(loadCache());
  const intervalRef = useRef<ReturnType<typeof setInterval> | null>(null);

  const fetchAndResolve = useCallback(async () => {
    try {
      const [peersRes, validatorsRes] = await Promise.allSettled([getPeers(), getValidators()]);
      const peerIps: string[] = peersRes.status === 'fulfilled' ? (peersRes.value.peers ?? []) : [];
      const validators: Array<Record<string, unknown>> = validatorsRes.status === 'fulfilled'
        ? (Array.isArray(validatorsRes.value) ? validatorsRes.value : validatorsRes.value?.validators ?? [])
        : [];

      // Extract just the IP (strip port)
      const ips = peerIps.map(p => p.split(':')[0]).filter(Boolean);
      const uniqueIps = [...new Set(ips)];

      // Resolve uncached IPs (rate-limited: 1 per 1.5s)
      const uncached = uniqueIps.filter(ip => !cacheRef.current.has(ip));
      for (let i = 0; i < Math.min(uncached.length, 10); i++) {
        const geo = await resolveGeoIP(uncached[i]);
        if (geo) {
          cacheRef.current.set(uncached[i], geo);
        }
        // Rate limit: ip-api.com allows 45/min
        if (i < uncached.length - 1) {
          await new Promise(r => setTimeout(r, 1500));
        }
      }
      saveCache(cacheRef.current);

      // Build result
      const result: GeoLocatedPeer[] = [];
      for (const ip of uniqueIps) {
        const geo = cacheRef.current.get(ip);
        if (!geo) continue;

        // Try to match with validator by checking if any validator is on this peer
        // (heuristic: validators array doesn't have IP, so we can't perfectly match,
        // but we can distribute validators across located peers for visual representation)
        const peer: GeoLocatedPeer = {
          ip,
          lat: geo.lat,
          lng: geo.lng,
          city: geo.city,
          country: geo.country,
        };
        result.push(peer);
      }

      // Distribute validator info across peers (best-effort mapping)
      for (let i = 0; i < Math.min(validators.length, result.length); i++) {
        const v = validators[i];
        result[i].validator = {
          address: String(v.address ?? ''),
          effective_stake: Number(v.effective_stake ?? 0),
          delegator_count: Number(v.delegator_count ?? 0),
          commission_percent: Number(v.commission_percent ?? 10),
          is_active: Boolean(v.is_active),
        };
      }

      setPeers(result);
    } catch {
      // Silent — keep existing data
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    fetchAndResolve();
    intervalRef.current = setInterval(fetchAndResolve, POLL_INTERVAL);
    return () => { if (intervalRef.current) clearInterval(intervalRef.current); };
  }, [fetchAndResolve]);

  // Clear cache on network switch
  useEffect(() => {
    const handler = () => {
      cacheRef.current.clear();
      sessionStorage.removeItem(CACHE_KEY);
      setPeers([]);
      setLoading(true);
      fetchAndResolve();
    };
    window.addEventListener('ultradag-network-switch', handler);
    return () => window.removeEventListener('ultradag-network-switch', handler);
  }, [fetchAndResolve]);

  return { peers, loading, peerCount: peers.length, countryCount: new Set(peers.map(p => p.country)).size };
}
