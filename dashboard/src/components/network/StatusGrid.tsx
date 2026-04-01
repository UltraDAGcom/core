import { FinalityBadge } from '../shared/Badge.tsx';
import { formatUdag } from '../../lib/api.ts';

interface StatusGridProps {
  status: {
    dag_round?: number;
    last_finalized_round?: number;
    total_supply?: number;
    peer_count?: number;
    mempool_size?: number;
    dag_vertices?: number;
    dag_tips?: number;
    finalized_count?: number;
    validator_count?: number;
    active_stakers?: number;
    total_staked?: number;
    treasury_balance?: number;
    account_count?: number;
    memory_usage_bytes?: number;
    uptime_seconds?: number;
    [key: string]: unknown;
  } | null;
}

function formatUptime(seconds: number): string {
  const d = Math.floor(seconds / 86400);
  const h = Math.floor((seconds % 86400) / 3600);
  const m = Math.floor((seconds % 3600) / 60);
  if (d > 0) return `${d}d ${h}h ${m}m`;
  if (h > 0) return `${h}h ${m}m`;
  return `${m}m`;
}

function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1048576) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / 1048576).toFixed(1)} MB`;
}

export function StatusGrid({ status }: StatusGridProps) {
  if (!status) {
    return <div className="text-slate-500 text-sm py-4">Loading status...</div>;
  }

  const lag = (status.dag_round ?? 0) - (status.last_finalized_round ?? 0);
  const supplyPct = status.total_supply != null
    ? ((status.total_supply / 100_000_000 / 21_000_000) * 100).toFixed(3)
    : '0';

  const cards: Array<{ label: string; value: string | number; sub?: string }> = [
    { label: 'DAG Round', value: (status.dag_round ?? 0).toLocaleString() },
    { label: 'Finalized Round', value: (status.last_finalized_round ?? 0).toLocaleString() },
    {
      label: 'Total Supply',
      value: status.total_supply != null
        ? `${formatUdag(status.total_supply)} UDAG`
        : '--',
      sub: `${supplyPct}% of 21M max`,
    },
    { label: 'Connected Peers', value: status.peer_count ?? 0 },
    { label: 'Mempool', value: `${status.mempool_size ?? 0} txs` },
    { label: 'DAG Vertices', value: (status.dag_vertices ?? 0).toLocaleString(), sub: `${status.dag_tips ?? 0} tips` },
    { label: 'Validators', value: status.validator_count ?? 0, sub: `${status.active_stakers ?? 0} stakers` },
    {
      label: 'Total Staked',
      value: status.total_staked != null && status.total_staked > 0
        ? `${formatUdag(status.total_staked)} UDAG`
        : '0 UDAG',
    },
    {
      label: 'Treasury',
      value: status.treasury_balance != null
        ? `${formatUdag(status.treasury_balance)} UDAG`
        : '--',
    },
    { label: 'Accounts', value: (status.account_count ?? 0).toLocaleString() },
    { label: 'Finalized', value: (status.finalized_count ?? 0).toLocaleString() },
    {
      label: 'Node',
      value: status.memory_usage_bytes != null ? formatBytes(status.memory_usage_bytes) : '--',
      sub: status.uptime_seconds != null ? `Up ${formatUptime(status.uptime_seconds)}` : undefined,
    },
  ];

  return (
    <div>
      <div className="flex items-center gap-3 mb-4">
        <h2 className="text-lg font-semibold text-slate-200">Node Status</h2>
        <FinalityBadge lag={lag} />
      </div>
      <div className="grid grid-cols-2 md:grid-cols-4 lg:grid-cols-6 gap-3">
        {cards.map((card) => (
          <div key={card.label} className="bg-slate-800 border border-slate-700 rounded-lg p-3">
            <p className="text-xs text-slate-500 mb-1">{card.label}</p>
            <p className="text-lg font-semibold text-slate-200 font-mono">{card.value}</p>
            {card.sub && <p className="text-xs text-slate-500 mt-0.5">{card.sub}</p>}
          </div>
        ))}
      </div>
    </div>
  );
}
