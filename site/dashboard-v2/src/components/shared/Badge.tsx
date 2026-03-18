const variants: Record<string, string> = {
  green: 'bg-green-500/20 text-green-400 border-green-500/30',
  yellow: 'bg-yellow-500/20 text-yellow-400 border-yellow-500/30',
  red: 'bg-red-500/20 text-red-400 border-red-500/30',
  blue: 'bg-blue-500/20 text-blue-400 border-blue-500/30',
  purple: 'bg-purple-500/20 text-purple-400 border-purple-500/30',
  gray: 'bg-slate-500/20 text-slate-400 border-slate-500/30',
};

export function Badge({ label, variant = 'gray' }: { label: string; variant?: string }) {
  const cls = variants[variant] ?? variants.gray;
  return (
    <span className={`inline-flex items-center px-2 py-0.5 text-xs font-medium rounded border ${cls}`}>
      {label}
    </span>
  );
}

export function StatusBadge({ status }: { status: string }) {
  const map: Record<string, { label: string; variant: string }> = {
    finalized: { label: 'Finalized', variant: 'green' },
    pending: { label: 'Pending', variant: 'yellow' },
    active: { label: 'Active', variant: 'blue' },
    executed: { label: 'Executed', variant: 'green' },
    rejected: { label: 'Rejected', variant: 'red' },
    passed_pending: { label: 'Passed', variant: 'purple' },
  };
  const entry = map[status.toLowerCase()] ?? { label: status, variant: 'gray' };
  return <Badge label={entry.label} variant={entry.variant} />;
}

export function FinalityBadge({ lag }: { lag: number }) {
  const variant = lag <= 3 ? 'green' : lag <= 10 ? 'yellow' : 'red';
  return <Badge label={`Lag ${lag}`} variant={variant} />;
}
