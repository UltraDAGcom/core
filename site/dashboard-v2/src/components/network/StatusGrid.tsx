import { FinalityBadge } from '../shared/Badge.tsx';
import { formatUdag } from '../../lib/api.ts';

interface StatusGridProps {
  status: {
    dag_round?: number;
    last_finalized_round?: number;
    total_supply?: number;
    total_supply_udag?: number;
    peers?: number;
    mempool_size?: number;
    total_vertices?: number;
    active_stakers?: number;
    total_staked?: number;
    total_staked_udag?: number;
    [key: string]: unknown;
  } | null;
}

export function StatusGrid({ status }: StatusGridProps) {
  if (!status) {
    return <div className="text-slate-500 text-sm py-4">Loading status...</div>;
  }

  const lag = (status.dag_round ?? 0) - (status.last_finalized_round ?? 0);

  const cards: Array<{ label: string; value: string | number; sub?: string }> = [
    { label: 'DAG Round', value: (status.dag_round ?? 0).toLocaleString() },
    { label: 'Finalized Round', value: (status.last_finalized_round ?? 0).toLocaleString() },
    {
      label: 'Total Supply',
      value: status.total_supply_udag != null
        ? `${Number(status.total_supply_udag).toLocaleString()} UDAG`
        : status.total_supply != null
          ? `${formatUdag(status.total_supply)} UDAG`
          : '--',
      sub: `${((Number(status.total_supply_udag ?? 0) / 21_000_000) * 100).toFixed(3)}% of max`,
    },
    { label: 'Connected Peers', value: status.peers ?? 0 },
    { label: 'Mempool', value: `${status.mempool_size ?? 0} txs` },
    { label: 'Total Vertices', value: (status.total_vertices ?? 0).toLocaleString() },
    { label: 'Active Stakers', value: status.active_stakers ?? 0 },
    {
      label: 'Total Staked',
      value: status.total_staked_udag != null
        ? `${Number(status.total_staked_udag).toLocaleString()} UDAG`
        : status.total_staked != null
          ? `${formatUdag(status.total_staked)} UDAG`
          : '--',
    },
  ];

  return (
    <div>
      <div className="flex items-center gap-3 mb-4">
        <h2 className="text-lg font-semibold text-slate-200">Node Status</h2>
        <FinalityBadge lag={lag} />
      </div>
      <div className="grid grid-cols-2 md:grid-cols-4 gap-3">
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
