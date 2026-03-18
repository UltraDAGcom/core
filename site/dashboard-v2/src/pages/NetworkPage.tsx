import { useEffect, useState, useCallback } from 'react';
import { Wifi, Database as DbIcon, BarChart3 } from 'lucide-react';
import { getStatus, getPeers, getMempool, getMetrics, connectToNode, isConnected, getNodeUrl, shortAddr } from '../lib/api.ts';
import { StatusGrid } from '../components/network/StatusGrid.tsx';
import { MempoolTable } from '../components/network/MempoolTable.tsx';

export function NetworkPage() {
  const [status, setStatus] = useState<Record<string, unknown> | null>(null);
  const [peers, setPeers] = useState<Array<Record<string, unknown>>>([]);
  const [mempool, setMempool] = useState<Array<Record<string, unknown>>>([]);
  const [metrics, setMetrics] = useState<Record<string, unknown> | null>(null);
  const [error, setError] = useState('');

  const fetchAll = useCallback(async () => {
    try {
      if (!isConnected()) await connectToNode();

      const results = await Promise.allSettled([
        getStatus(),
        getPeers(),
        getMempool(),
        getMetrics(),
      ]);

      if (results[0].status === 'fulfilled') setStatus(results[0].value);
      if (results[1].status === 'fulfilled') {
        const peersData = results[1].value;
        setPeers(Array.isArray(peersData) ? peersData : peersData?.peers ?? []);
      }
      if (results[2].status === 'fulfilled') {
        const mempoolData = results[2].value;
        setMempool(Array.isArray(mempoolData) ? mempoolData : mempoolData?.transactions ?? []);
      }
      if (results[3].status === 'fulfilled') setMetrics(results[3].value);

      setError('');
    } catch (e) {
      setError(e instanceof Error ? e.message : 'Failed to fetch network data');
    }
  }, []);

  useEffect(() => {
    fetchAll();
    const interval = setInterval(fetchAll, 5000);
    return () => clearInterval(interval);
  }, [fetchAll]);

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-bold text-white mb-1">Network</h1>
          <p className="text-sm text-slate-400">
            Connected to <span className="font-mono text-slate-300">{getNodeUrl()}</span>
          </p>
        </div>
        <div className="flex items-center gap-2 text-xs text-slate-500">
          <span className="w-1.5 h-1.5 rounded-full bg-green-500 animate-pulse" />
          Auto-refreshing every 5s
        </div>
      </div>

      {error && <div className="bg-red-500/10 border border-red-500/30 rounded-lg p-3 text-sm text-red-400">{error}</div>}

      {/* Status grid */}
      <StatusGrid status={status as Parameters<typeof StatusGrid>[0]['status']} />

      {/* Peers */}
      <div className="bg-slate-800/50 border border-slate-700 rounded-lg p-4">
        <div className="flex items-center gap-2 mb-3">
          <Wifi className="w-4 h-4 text-green-400" />
          <h2 className="text-sm font-semibold text-slate-200">Connected Peers ({peers.length})</h2>
        </div>
        {peers.length === 0 ? (
          <p className="text-slate-500 text-sm">No peers connected</p>
        ) : (
          <div className="overflow-x-auto">
            <table className="w-full text-sm">
              <thead>
                <tr className="text-left text-slate-400 border-b border-slate-700">
                  <th className="py-2 px-3 font-medium">Address</th>
                  <th className="py-2 px-3 font-medium">Listen Address</th>
                  <th className="py-2 px-3 font-medium">Round</th>
                </tr>
              </thead>
              <tbody>
                {peers.map((peer, i) => (
                  <tr key={String(peer.address ?? peer.addr ?? i)} className="border-b border-slate-800">
                    <td className="py-2 px-3 font-mono text-xs text-slate-300">
                      {String(peer.address ?? peer.addr ?? '--')}
                    </td>
                    <td className="py-2 px-3 font-mono text-xs text-slate-300">
                      {String(peer.listen_addr ?? peer.listen_address ?? '--')}
                    </td>
                    <td className="py-2 px-3 font-mono text-xs text-slate-300">
                      {peer.round != null ? Number(peer.round).toLocaleString() : peer.validator ? shortAddr(String(peer.validator)) : '--'}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}
      </div>

      {/* Mempool */}
      <div className="bg-slate-800/50 border border-slate-700 rounded-lg p-4">
        <div className="flex items-center gap-2 mb-3">
          <DbIcon className="w-4 h-4 text-yellow-400" />
          <h2 className="text-sm font-semibold text-slate-200">Mempool ({mempool.length} transactions)</h2>
        </div>
        <MempoolTable transactions={mempool as unknown as Parameters<typeof MempoolTable>[0]['transactions']} />
      </div>

      {/* Metrics */}
      {metrics && (
        <div className="bg-slate-800/50 border border-slate-700 rounded-lg p-4">
          <div className="flex items-center gap-2 mb-3">
            <BarChart3 className="w-4 h-4 text-blue-400" />
            <h2 className="text-sm font-semibold text-slate-200">Metrics</h2>
          </div>
          <MetricsGrid metrics={metrics} />
        </div>
      )}
    </div>
  );
}

function MetricsGrid({ metrics }: { metrics: Record<string, unknown> }) {
  // Flatten metrics object into displayable key-value pairs
  const entries: Array<{ key: string; value: string }> = [];

  function flatten(obj: Record<string, unknown>, prefix: string) {
    for (const [key, val] of Object.entries(obj)) {
      const fullKey = prefix ? `${prefix}.${key}` : key;
      if (val != null && typeof val === 'object' && !Array.isArray(val)) {
        flatten(val as Record<string, unknown>, fullKey);
      } else {
        entries.push({
          key: fullKey.replace(/_/g, ' '),
          value: typeof val === 'number' ? val.toLocaleString() : String(val ?? '--'),
        });
      }
    }
  }

  flatten(metrics, '');

  if (entries.length === 0) {
    return <p className="text-slate-500 text-sm">No metrics available</p>;
  }

  return (
    <div className="grid grid-cols-2 md:grid-cols-3 lg:grid-cols-4 gap-2">
      {entries.slice(0, 24).map((entry) => (
        <div key={entry.key} className="bg-slate-800 border border-slate-700 rounded p-2">
          <p className="text-xs text-slate-500 truncate" title={entry.key}>{entry.key}</p>
          <p className="text-sm font-mono text-slate-200 truncate">{entry.value}</p>
        </div>
      ))}
    </div>
  );
}
