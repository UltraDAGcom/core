import { Link } from 'react-router-dom';
import { shortHash } from '../../lib/api.ts';
import { CopyButton } from '../shared/CopyButton.tsx';
import { Badge } from '../shared/Badge.tsx';

interface Vertex {
  hash: string;
  validator: string;
  reward_udag?: number;
  reward?: number;
  tx_count: number;
  parent_count: number;
}

interface RoundTableProps {
  rounds: Array<{
    round: number;
    vertices: Vertex[];
    finalized: boolean;
  }>;
}

export function RoundTable({ rounds }: RoundTableProps) {
  if (rounds.length === 0) {
    return <p className="text-slate-500 text-sm py-8 text-center">No rounds to display</p>;
  }

  return (
    <div className="overflow-x-auto">
      <table className="w-full text-sm">
        <thead>
          <tr className="text-left text-slate-400 border-b border-slate-700">
            <th className="py-2 px-3 font-medium">Round</th>
            <th className="py-2 px-3 font-medium">Vertices</th>
            <th className="py-2 px-3 font-medium">Validators</th>
            <th className="py-2 px-3 font-medium">Status</th>
          </tr>
        </thead>
        <tbody>
          {rounds.map((r) => (
            <tr key={r.round} className="border-b border-slate-800 hover:bg-slate-800/50 transition-colors">
              <td className="py-2.5 px-3">
                <Link to={`/round/${r.round}`} className="text-blue-400 hover:text-blue-300 font-mono">
                  #{r.round.toLocaleString()}
                </Link>
              </td>
              <td className="py-2.5 px-3 font-mono">{r.vertices.length}</td>
              <td className="py-2.5 px-3">
                <div className="flex flex-wrap gap-1">
                  {r.vertices.slice(0, 3).map((v) => (
                    <span key={v.hash} className="inline-flex items-center gap-1 text-xs">
                      <span className="font-mono text-slate-300">{shortHash(v.validator)}</span>
                      <CopyButton text={v.validator} />
                    </span>
                  ))}
                  {r.vertices.length > 3 && (
                    <span className="text-xs text-slate-500">+{r.vertices.length - 3} more</span>
                  )}
                </div>
              </td>
              <td className="py-2.5 px-3">
                <Badge label={r.finalized ? 'Finalized' : 'Pending'} variant={r.finalized ? 'green' : 'yellow'} />
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}
