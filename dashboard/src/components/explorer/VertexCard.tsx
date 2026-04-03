import { Link } from 'react-router-dom';
import { shortHash, formatUdag } from '../../lib/api.ts';
import { CopyButton } from '../shared/CopyButton.tsx';
import { DisplayIdentity } from '../shared/DisplayIdentity.tsx';

interface VertexCardProps {
  hash: string;
  validator: string;
  reward?: number;
  reward_udag?: number;
  tx_count: number;
  parent_count: number;
  showLink?: boolean;
}

export function VertexCard({ hash, validator, reward, reward_udag, tx_count, parent_count, showLink = true }: VertexCardProps) {
  const rewardDisplay = reward_udag != null ? `${reward_udag} UDAG` : reward != null ? formatUdag(reward) + ' UDAG' : '--';

  return (
    <div className="bg-slate-800 border border-slate-700 rounded-lg p-4 hover:border-slate-600 transition-colors">
      <div className="flex items-start justify-between mb-3">
        <div className="flex items-center gap-2">
          <span className="text-xs text-slate-500 uppercase tracking-wide">Vertex</span>
          {showLink ? (
            <Link to={`/vertex/${hash}`} className="font-mono text-sm text-blue-400 hover:text-blue-300">
              {shortHash(hash)}
            </Link>
          ) : (
            <span className="font-mono text-sm text-slate-200">{shortHash(hash)}</span>
          )}
          <CopyButton text={hash} />
        </div>
      </div>
      <div className="grid grid-cols-2 gap-3 text-sm">
        <div>
          <span className="text-slate-500 text-xs">Validator</span>
          <DisplayIdentity address={validator} link size="xs" />
        </div>
        <div>
          <span className="text-slate-500 text-xs">Reward</span>
          <p className="text-slate-300 font-mono text-xs">{rewardDisplay}</p>
        </div>
        <div>
          <span className="text-slate-500 text-xs">Transactions</span>
          <p className="text-slate-300 font-mono">{tx_count}</p>
        </div>
        <div>
          <span className="text-slate-500 text-xs">Parents</span>
          <p className="text-slate-300 font-mono">{parent_count}</p>
        </div>
      </div>
    </div>
  );
}
