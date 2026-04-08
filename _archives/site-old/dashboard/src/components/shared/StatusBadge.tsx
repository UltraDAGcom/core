const STATUS_COLORS: Record<string, string> = {
  Active: 'bg-dag-blue/20 text-dag-blue border-dag-blue/40',
  PassedPending: 'bg-dag-purple/20 text-dag-purple border-dag-purple/40',
  Executed: 'bg-dag-green/20 text-dag-green border-dag-green/40',
  Rejected: 'bg-dag-red/20 text-dag-red border-dag-red/40',
  Failed: 'bg-dag-red/20 text-dag-red border-dag-red/40',
  Cancelled: 'bg-dag-muted/20 text-dag-muted border-dag-muted/40',
};

const STATUS_LABELS: Record<string, string> = {
  PassedPending: 'Passed',
};

interface StatusBadgeProps {
  status: string;
}

export function StatusBadge({ status }: StatusBadgeProps) {
  const colors = STATUS_COLORS[status] ?? STATUS_COLORS.Active;
  const label = STATUS_LABELS[status] ?? status;
  return (
    <span className={`inline-block text-xs px-2 py-0.5 rounded border ${colors}`}>
      {label}
    </span>
  );
}
