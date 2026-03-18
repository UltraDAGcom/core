const STATUS_COLORS: Record<string, string> = {
  Active: 'bg-dag-blue/20 text-dag-blue border-dag-blue/40',
  PassedPending: 'bg-dag-yellow/20 text-dag-yellow border-dag-yellow/40',
  Executed: 'bg-dag-green/20 text-dag-green border-dag-green/40',
  Rejected: 'bg-dag-red/20 text-dag-red border-dag-red/40',
  Expired: 'bg-dag-muted/20 text-dag-muted border-dag-muted/40',
};

interface StatusBadgeProps {
  status: string;
}

export function StatusBadge({ status }: StatusBadgeProps) {
  const colors = STATUS_COLORS[status] ?? STATUS_COLORS.Active;
  return (
    <span className={`inline-block text-xs px-2 py-0.5 rounded border ${colors}`}>
      {status}
    </span>
  );
}
