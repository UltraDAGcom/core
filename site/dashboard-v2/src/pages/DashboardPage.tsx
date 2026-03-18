import { Activity, Box, Users, Layers, Clock, Coins } from 'lucide-react';
import { formatUdag } from '../lib/api';
import type { NodeStatus } from '../hooks/useNode';

interface DashboardPageProps {
  status: NodeStatus | null;
  loading: boolean;
}

export function DashboardPage({ status, loading }: DashboardPageProps) {
  if (loading) {
    return (
      <div className="flex items-center justify-center h-64">
        <div className="animate-pulse text-dag-muted">Connecting to node...</div>
      </div>
    );
  }

  if (!status) {
    return (
      <div className="flex items-center justify-center h-64">
        <p className="text-dag-muted">Unable to connect to any node.</p>
      </div>
    );
  }

  const cards = [
    {
      label: 'DAG Round',
      value: status.dag_round.toLocaleString(),
      icon: Layers,
      color: 'text-dag-blue',
    },
    {
      label: 'Finalized Round',
      value: status.last_finalized_round.toLocaleString(),
      icon: Clock,
      color: status.finality_lag <= 3 ? 'text-dag-green' : status.finality_lag <= 10 ? 'text-dag-yellow' : 'text-dag-red',
      sub: `Lag: ${status.finality_lag}`,
    },
    {
      label: 'Validators',
      value: String(status.validators),
      icon: Users,
      color: 'text-dag-purple',
      sub: `${status.active_stakers} stakers`,
    },
    {
      label: 'Total Supply',
      value: `${formatUdag(status.total_supply)} UDAG`,
      icon: Coins,
      color: 'text-dag-accent',
      sub: `${((status.total_supply / 2_100_000_000_000_000) * 100).toFixed(4)}% of 21M`,
    },
    {
      label: 'Total Staked',
      value: `${formatUdag(status.total_staked)} UDAG`,
      icon: Activity,
      color: 'text-dag-green',
    },
    {
      label: 'Mempool',
      value: String(status.mempool_size),
      icon: Box,
      color: 'text-dag-yellow',
      sub: `${status.peer_count} peers`,
    },
  ];

  return (
    <div className="space-y-6">
      <div>
        <h1 className="text-2xl font-bold text-white">Dashboard</h1>
        <p className="text-sm text-dag-muted mt-1">UltraDAG testnet overview</p>
      </div>

      <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-4">
        {cards.map(({ label, value, icon: Icon, color, sub }) => (
          <div
            key={label}
            className="bg-dag-card border border-dag-border rounded-xl p-5 hover:border-slate-500 transition-colors"
          >
            <div className="flex items-center justify-between mb-3">
              <span className="text-xs text-dag-muted uppercase tracking-wider">{label}</span>
              <Icon className={`w-5 h-5 ${color}`} />
            </div>
            <p className="text-xl font-bold text-white">{value}</p>
            {sub && <p className="text-xs text-dag-muted mt-1">{sub}</p>}
          </div>
        ))}
      </div>
    </div>
  );
}
