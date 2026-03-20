import { useEffect, useState, useCallback } from 'react';
import { Wifi, Database as DbIcon, BarChart3, HeartPulse, Radio } from 'lucide-react';
import { getStatus, getPeers, getMempool, getMetrics, getHealthDetailed, connectToNode, isConnected, getNodeUrl } from '../lib/api.ts';
import { StatusGrid } from '../components/network/StatusGrid.tsx';
import { MempoolTable } from '../components/network/MempoolTable.tsx';

interface PeersData {
  connected: number;
  peers: string[];
  bootstrap_nodes: Array<{ addr: string; connected: boolean }>;
}

interface HealthComponent {
  status: string;
  [key: string]: unknown;
}

interface HealthDetailed {
  status: string;
  components: Record<string, HealthComponent>;
  [key: string]: unknown;
}

interface MetricsData {
  checkpoint_production?: { total: number; last_duration_ms: number; last_size_bytes: number; errors: number };
  checkpoint_cosigning?: { total: number; last_signatures: number; quorum_reached: number; validation_failures: number };
  fast_sync?: { attempts: number; successes: number; failures: number };
  health?: { last_checkpoint_age_seconds: number; last_checkpoint_round: number; pending_checkpoints: number };
  storage?: { persist_success: number; persist_failures: number };
  [key: string]: unknown;
}

export function NetworkPage() {
  const [status, setStatus] = useState<Record<string, unknown> | null>(null);
  const [peersData, setPeersData] = useState<PeersData | null>(null);
  const [mempool, setMempool] = useState<Array<Record<string, unknown>>>([]);
  const [metrics, setMetrics] = useState<MetricsData | null>(null);
  const [health, setHealth] = useState<HealthDetailed | null>(null);
  const [error, setError] = useState('');

  const fetchAll = useCallback(async () => {
    try {
      if (!isConnected()) await connectToNode();

      const results = await Promise.allSettled([
        getStatus(),
        getPeers(),
        getMempool(),
        getMetrics(),
        getHealthDetailed(),
      ]);

      if (results[0].status === 'fulfilled') setStatus(results[0].value);
      if (results[1].status === 'fulfilled') {
        const raw = results[1].value;
        setPeersData({
          connected: raw.connected ?? 0,
          peers: Array.isArray(raw.peers) ? raw.peers : [],
          bootstrap_nodes: Array.isArray(raw.bootstrap_nodes) ? raw.bootstrap_nodes : [],
        });
      }
      if (results[2].status === 'fulfilled') {
        const mempoolData = results[2].value;
        setMempool(Array.isArray(mempoolData) ? mempoolData : mempoolData?.transactions ?? []);
      }
      if (results[3].status === 'fulfilled') setMetrics(results[3].value);
      if (results[4].status === 'fulfilled') setHealth(results[4].value);

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

  const peerList = peersData?.peers ?? [];
  const bootstrapNodes = peersData?.bootstrap_nodes ?? [];

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

      {/* Health status */}
      {health && <HealthCard health={health} />}

      {/* Status grid */}
      <StatusGrid status={status as Parameters<typeof StatusGrid>[0]['status']} />

      {/* Peers */}
      <div className="bg-slate-800/50 border border-slate-700 rounded-lg p-4">
        <div className="flex items-center gap-2 mb-3">
          <Wifi className="w-4 h-4 text-green-400" />
          <h2 className="text-sm font-semibold text-slate-200">Connected Peers ({peersData?.connected ?? peerList.length})</h2>
        </div>
        {peerList.length === 0 ? (
          <p className="text-slate-500 text-sm">No peers connected</p>
        ) : (
          <div className="overflow-x-auto">
            <table className="w-full text-sm">
              <thead>
                <tr className="text-left text-slate-400 border-b border-slate-700">
                  <th className="py-2 px-3 font-medium">#</th>
                  <th className="py-2 px-3 font-medium">Peer Address</th>
                </tr>
              </thead>
              <tbody>
                {peerList.map((peer, i) => (
                  <tr key={peer} className="border-b border-slate-800">
                    <td className="py-2 px-3 text-xs text-slate-500">{i + 1}</td>
                    <td className="py-2 px-3 font-mono text-xs text-slate-300">{peer}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}
      </div>

      {/* Bootstrap Nodes — only show if any are connected */}
      {bootstrapNodes.length > 0 && bootstrapNodes.some((n: any) => n.connected) && (
        <div className="bg-slate-800/50 border border-slate-700 rounded-lg p-4">
          <div className="flex items-center gap-2 mb-3">
            <Radio className="w-4 h-4 text-purple-400" />
            <h2 className="text-sm font-semibold text-slate-200">Bootstrap Nodes ({bootstrapNodes.length})</h2>
          </div>
          <div className="overflow-x-auto">
            <table className="w-full text-sm">
              <thead>
                <tr className="text-left text-slate-400 border-b border-slate-700">
                  <th className="py-2 px-3 font-medium">Address</th>
                  <th className="py-2 px-3 font-medium">Status</th>
                </tr>
              </thead>
              <tbody>
                {bootstrapNodes.map((node) => (
                  <tr key={node.addr} className="border-b border-slate-800">
                    <td className="py-2 px-3 font-mono text-xs text-slate-300">{node.addr}</td>
                    <td className="py-2 px-3">
                      <span className={`inline-flex items-center gap-1.5 text-xs ${node.connected ? 'text-green-400' : 'text-slate-500'}`}>
                        <span className={`w-1.5 h-1.5 rounded-full ${node.connected ? 'bg-green-500' : 'bg-slate-600'}`} />
                        {node.connected ? 'Connected' : 'Disconnected'}
                      </span>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </div>
      )}

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

/* ---------- Health Card ---------- */

const healthTextColors: Record<string, string> = {
  healthy: 'text-green-400',
  warning: 'text-yellow-400',
  unhealthy: 'text-red-400',
  degraded: 'text-orange-400',
};

function HealthCard({ health }: { health: HealthDetailed }) {
  const overallColor = healthTextColors[health.status] ?? 'text-slate-400';
  const components = health.components ?? {};
  const componentNames = Object.keys(components);

  return (
    <div className="bg-slate-800/50 border border-slate-700 rounded-lg p-4">
      <div className="flex items-center gap-2 mb-3">
        <HeartPulse className="w-4 h-4 text-pink-400" />
        <h2 className="text-sm font-semibold text-slate-200">Health</h2>
        <span className={`text-xs font-medium capitalize ${overallColor}`}>{health.status}</span>
      </div>
      {componentNames.length > 0 && (
        <div className="flex flex-wrap gap-3">
          {componentNames.map((name) => {
            const comp = components[name];
            const dotColor = comp.available === true ? 'bg-green-500' : comp.available === false ? 'bg-red-500' : 'bg-slate-600';
            return (
              <div key={name} className="flex items-center gap-1.5 bg-slate-800 border border-slate-700 rounded px-2.5 py-1.5">
                <span className={`w-2 h-2 rounded-full ${dotColor}`} />
                <span className="text-xs text-slate-300 capitalize">{name}</span>
              </div>
            );
          })}
        </div>
      )}
    </div>
  );
}

/* ---------- Metrics Grid ---------- */

interface MetricEntry {
  label: string;
  value: string;
}

interface MetricSection {
  title: string;
  entries: MetricEntry[];
}

function MetricsGrid({ metrics }: { metrics: MetricsData }) {
  const sections: MetricSection[] = [];

  if (metrics.checkpoint_production) {
    const cp = metrics.checkpoint_production;
    sections.push({
      title: 'Checkpoint Production',
      entries: [
        { label: 'Total Produced', value: cp.total.toLocaleString() },
        { label: 'Last Duration', value: `${cp.last_duration_ms} ms` },
        { label: 'Last Size', value: `${cp.last_size_bytes.toLocaleString()} B` },
        { label: 'Errors', value: cp.errors.toLocaleString() },
      ],
    });
  }

  if (metrics.checkpoint_cosigning) {
    const cs = metrics.checkpoint_cosigning;
    sections.push({
      title: 'Checkpoint Co-signing',
      entries: [
        { label: 'Total Received', value: cs.total.toLocaleString() },
        { label: 'Last Signatures', value: cs.last_signatures.toLocaleString() },
        { label: 'Quorum Reached', value: cs.quorum_reached.toLocaleString() },
        { label: 'Validation Failures', value: cs.validation_failures.toLocaleString() },
      ],
    });
  }

  if (metrics.fast_sync) {
    const fs = metrics.fast_sync;
    sections.push({
      title: 'Fast Sync',
      entries: [
        { label: 'Attempts', value: fs.attempts.toLocaleString() },
        { label: 'Successes', value: fs.successes.toLocaleString() },
        { label: 'Failures', value: fs.failures.toLocaleString() },
      ],
    });
  }

  if (metrics.health) {
    const h = metrics.health;
    sections.push({
      title: 'Health',
      entries: [
        { label: 'Last Checkpoint Age', value: `${h.last_checkpoint_age_seconds.toLocaleString()}s` },
        { label: 'Last Checkpoint Round', value: h.last_checkpoint_round.toLocaleString() },
        { label: 'Pending Checkpoints', value: h.pending_checkpoints.toLocaleString() },
      ],
    });
  }

  if (metrics.storage) {
    const s = metrics.storage;
    sections.push({
      title: 'Storage',
      entries: [
        { label: 'Persist Success', value: s.persist_success.toLocaleString() },
        { label: 'Persist Failures', value: s.persist_failures.toLocaleString() },
      ],
    });
  }

  if (sections.length === 0) {
    return <p className="text-slate-500 text-sm">No metrics available</p>;
  }

  return (
    <div className="space-y-4">
      {sections.map((section) => (
        <div key={section.title}>
          <h3 className="text-xs font-medium text-slate-400 uppercase tracking-wider mb-2">{section.title}</h3>
          <div className="grid grid-cols-2 md:grid-cols-3 lg:grid-cols-4 gap-2">
            {section.entries.map((entry) => (
              <div key={entry.label} className="bg-slate-800 border border-slate-700 rounded p-2">
                <p className="text-xs text-slate-500 truncate" title={entry.label}>{entry.label}</p>
                <p className="text-sm font-mono text-slate-200 truncate">{entry.value}</p>
              </div>
            ))}
          </div>
        </div>
      ))}
    </div>
  );
}
