import { useState, useEffect, useCallback, useRef } from 'react';
import { connectToNode, getStatus, getNodeUrl, isConnected } from '../lib/api';

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
}

export function useNode() {
  const [connected, setConnected] = useState(isConnected());
  const [nodeUrl, setNodeUrl] = useState(getNodeUrl());
  const [status, setStatus] = useState<NodeStatus | null>(null);
  const [loading, setLoading] = useState(true);
  const intervalRef = useRef<ReturnType<typeof setInterval> | null>(null);

  const fetchStatus = useCallback(async () => {
    try {
      const s = await getStatus();
      setStatus({
        dag_round: s.dag_round ?? 0,
        last_finalized_round: s.last_finalized_round ?? 0,
        finality_lag: (s.dag_round ?? 0) - (s.last_finalized_round ?? 0),
        total_supply: s.total_supply ?? 0,
        total_staked: s.total_staked ?? 0,
        active_stakers: s.active_stakers ?? 0,
        mempool_size: s.mempool_size ?? 0,
        peer_count: s.peer_count ?? 0,
        active_accounts: s.active_accounts ?? 0,
        total_vertices: s.total_vertices ?? 0,
        validators: s.validators ?? 0,
      });
      setConnected(true);
      setNodeUrl(getNodeUrl());
    } catch {
      setConnected(false);
      setStatus(null);
    } finally {
      setLoading(false);
    }
  }, []);

  const connect = useCallback(async () => {
    setLoading(true);
    const ok = await connectToNode();
    setConnected(ok);
    setNodeUrl(getNodeUrl());
    if (ok) await fetchStatus();
    else setLoading(false);
  }, [fetchStatus]);

  useEffect(() => {
    connect();
    intervalRef.current = setInterval(fetchStatus, 30_000);
    return () => {
      if (intervalRef.current) clearInterval(intervalRef.current);
    };
  }, [connect, fetchStatus]);

  return { connected, nodeUrl, status, loading, reconnect: connect };
}
