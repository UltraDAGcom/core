import { useState, useEffect, useCallback, useRef } from 'react';
import { connectToNode, getStatus, getNodeUrl, isConnected, getSwitchGeneration } from '../lib/api';

export interface NodeStatus {
  dag_round: number;
  last_finalized_round: number;
  finality_lag: number;
  total_supply: number;
  total_staked: number;
  active_stakers: number;
  mempool_size: number;
  peer_count: number;
  active_accounts: number;
  total_vertices: number;
  validators: number;
  dag_tips: number;
  finalized_count: number;
  treasury_balance: number;
  memory_usage_bytes: number;
  uptime_seconds: number;
}

export function useNode() {
  const [connected, setConnected] = useState(isConnected());
  const [nodeUrl, setNodeUrl] = useState(getNodeUrl());
  const [status, setStatus] = useState<NodeStatus | null>(null);
  const [loading, setLoading] = useState(true);
  const intervalRef = useRef<ReturnType<typeof setInterval> | null>(null);
  const genRef = useRef(getSwitchGeneration());

  const fetchStatus = useCallback(async () => {
    const gen = getSwitchGeneration();
    try {
      const s = await getStatus();
      // Discard if network switched since we started fetching
      if (gen !== getSwitchGeneration()) return;
      setStatus({
        dag_round: s.dag_round ?? 0,
        last_finalized_round: s.last_finalized_round ?? 0,
        finality_lag: (s.dag_round ?? 0) - (s.last_finalized_round ?? 0),
        total_supply: s.total_supply ?? 0,
        total_staked: s.total_staked ?? 0,
        active_stakers: s.active_stakers ?? 0,
        mempool_size: s.mempool_size ?? 0,
        peer_count: s.peer_count ?? 0,
        active_accounts: s.account_count ?? s.active_accounts ?? 0,
        total_vertices: s.dag_vertices ?? s.total_vertices ?? 0,
        validators: s.validator_count ?? s.validators ?? 0,
        dag_tips: s.dag_tips ?? 0,
        finalized_count: s.finalized_count ?? 0,
        treasury_balance: s.treasury_balance ?? 0,
        memory_usage_bytes: s.memory_usage_bytes ?? 0,
        uptime_seconds: s.uptime_seconds ?? 0,
      });
      setConnected(true);
      setNodeUrl(getNodeUrl());
    } catch {
      // Only set disconnected if this is still the current generation
      if (gen === getSwitchGeneration()) {
        setConnected(false);
        setStatus(null);
      }
    } finally {
      setLoading(false);
    }
  }, []);

  const connect = useCallback(async () => {
    setLoading(true);
    setStatus(null);
    // Stop the old polling interval before reconnecting
    if (intervalRef.current) { clearInterval(intervalRef.current); intervalRef.current = null; }
    const gen = getSwitchGeneration();
    genRef.current = gen;
    const ok = await connectToNode();
    // Discard if network switched while connecting
    if (gen !== getSwitchGeneration()) return;
    setConnected(ok);
    setNodeUrl(getNodeUrl());
    if (ok) await fetchStatus();
    else setLoading(false);
    // Restart polling with fresh interval tied to current network
    if (intervalRef.current) clearInterval(intervalRef.current);
    intervalRef.current = setInterval(fetchStatus, 5_000);
  }, [fetchStatus]);

  // Initial connect only (not on every reconnect — connect handles its own interval)
  useEffect(() => {
    connect();
    return () => {
      if (intervalRef.current) clearInterval(intervalRef.current);
    };
  }, []); // eslint-disable-line react-hooks/exhaustive-deps

  // Listen for network switch and reconnect
  useEffect(() => {
    const handler = () => { connect(); };
    window.addEventListener('ultradag-network-switch', handler);
    return () => window.removeEventListener('ultradag-network-switch', handler);
  }, [connect]);

  return { connected, nodeUrl, status, loading, reconnect: connect };
}
