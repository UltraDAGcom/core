import { StatusBadge } from '../shared/StatusBadge';
import { formatUdag } from '../../lib/api';

interface ProposalCardProps {
  id: number;
  title: string;
  status: string;
  proposal_type: string;
  votes_for: number;
  votes_against: number;
  snapshot_total_stake: number;
  onClick: () => void;
}

export function ProposalCard({
  id,
  title,
  status,
  proposal_type,
  votes_for,
  votes_against,
  snapshot_total_stake,
  onClick,
}: ProposalCardProps) {
  const totalVotes = votes_for + votes_against;
  const approvalPct = totalVotes > 0 ? Math.round((votes_for / totalVotes) * 100) : 0;
  const quorumPct = snapshot_total_stake > 0 ? Math.round((totalVotes / snapshot_total_stake) * 100) : 0;

  return (
    <button
      onClick={onClick}
      className="w-full text-left rounded-lg bg-dag-surface border border-dag-border p-4 hover:border-dag-blue/50 transition-colors"
    >
      <div className="flex items-start justify-between gap-3">
        <div className="min-w-0 flex-1">
          <div className="flex items-center gap-2 mb-1">
            <span className="text-xs text-dag-muted">#{id}</span>
            <StatusBadge status={status} />
            <span className="text-xs text-dag-muted px-1.5 py-0.5 rounded bg-dag-card border border-dag-border">
              {proposal_type}
            </span>
          </div>
          <h4 className="text-white font-medium truncate">{title}</h4>
        </div>
      </div>

      <div className="mt-3 grid grid-cols-3 gap-3 text-sm">
        <div>
          <span className="text-dag-muted block text-xs">For</span>
          <span className="text-dag-green">{formatUdag(votes_for)} ({approvalPct}%)</span>
        </div>
        <div>
          <span className="text-dag-muted block text-xs">Against</span>
          <span className="text-dag-red">{formatUdag(votes_against)}</span>
        </div>
        <div>
          <span className="text-dag-muted block text-xs">Quorum</span>
          <span className="text-white">{quorumPct}%</span>
        </div>
      </div>
    </button>
  );
}
