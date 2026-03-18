import { shortAddr, formatUdag } from '../../lib/api';

interface ValidatorCardProps {
  address: string;
  effective_stake: number;
  delegator_count: number;
  commission_percent: number;
  is_active: boolean;
  onDelegate?: () => void;
}

export function ValidatorCard({
  address,
  effective_stake,
  delegator_count,
  commission_percent,
  is_active,
  onDelegate,
}: ValidatorCardProps) {
  return (
    <div className="rounded-lg bg-dag-surface border border-dag-border p-4 flex flex-col gap-2">
      <div className="flex items-center justify-between">
        <span className="font-mono text-sm text-white" title={address}>
          {shortAddr(address)}
        </span>
        {is_active && (
          <span className="text-xs px-2 py-0.5 rounded bg-dag-green/20 text-dag-green border border-dag-green/40">
            Active
          </span>
        )}
      </div>
      <div className="grid grid-cols-3 gap-3 text-sm">
        <div>
          <span className="text-dag-muted block text-xs">Effective Stake</span>
          <span className="text-white">{formatUdag(effective_stake)} UDAG</span>
        </div>
        <div>
          <span className="text-dag-muted block text-xs">Delegators</span>
          <span className="text-white">{delegator_count}</span>
        </div>
        <div>
          <span className="text-dag-muted block text-xs">Commission</span>
          <span className="text-white">{commission_percent}%</span>
        </div>
      </div>
      {onDelegate && (
        <button
          onClick={onDelegate}
          className="mt-1 text-sm px-3 py-1.5 rounded bg-dag-blue/20 text-dag-blue border border-dag-blue/40 hover:bg-dag-blue/30 transition-colors"
        >
          Delegate
        </button>
      )}
    </div>
  );
}
