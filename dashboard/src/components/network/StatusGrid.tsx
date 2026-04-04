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

const gridStyle: React.CSSProperties = {
  display: 'grid',
  gridTemplateColumns: 'repeat(auto-fill, minmax(150px, 1fr))',
  gap: 12,
};

const cardStyle: React.CSSProperties = {
  background: 'var(--dag-card)',
  border: '1px solid var(--dag-border)',
  borderRadius: 8,
  padding: 12,
};

export function StatusGrid({ status }: StatusGridProps) {
  if (!status) {
    return <div style={{ color: 'var(--dag-text-faint)', fontSize: 12, padding: '16px 0' }}>Loading status...</div>;
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
      <div style={{ display: 'flex', alignItems: 'center', gap: 12, marginBottom: 16 }}>
        <h2 style={{ fontSize: 18, fontWeight: 600, color: 'var(--dag-text-secondary)' }}>Node Status</h2>
        <FinalityBadge lag={lag} />
      </div>
      <div style={gridStyle}>
        {cards.map((card) => (
          <div key={card.label} style={cardStyle}>
            <p style={{ fontSize: 10, color: 'var(--dag-text-faint)', marginBottom: 4 }}>{card.label}</p>
            <p style={{ fontSize: 18, fontWeight: 600, color: 'var(--dag-text-secondary)', fontFamily: 'monospace' }}>{card.value}</p>
            {card.sub && <p style={{ fontSize: 10, color: 'var(--dag-text-faint)', marginTop: 2 }}>{card.sub}</p>}
          </div>
        ))}
      </div>
    </div>
  );
}
