import { Link } from 'react-router-dom';
import { Badge } from '../shared/Badge.tsx';
import { DisplayIdentity } from '../shared/DisplayIdentity.tsx';

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
            <th className="py-2 px-3 font-medium w-28">Round</th>
            <th className="py-2 px-3 font-medium w-20 text-center">Vertices</th>
            <th className="py-2 px-3 font-medium w-16 text-center">Txns</th>
            <th className="py-2 px-3 font-medium">Validators</th>
            <th className="py-2 px-3 font-medium w-24">Status</th>
          </tr>
        </thead>
        <tbody>
          {rounds.map((r) => {
            const totalTxs = r.vertices.reduce((sum, v) => sum + v.tx_count, 0);
            return (
              <tr key={r.round} className="border-b border-slate-800 hover:bg-slate-800/50 transition-colors">
                <td className="py-2.5 px-3">
                  <Link to={`/round/${r.round}`} className="text-blue-400 hover:text-blue-300 font-mono font-bold text-base">
                    #{r.round.toLocaleString()}
                  </Link>
                </td>
                <td className="py-2.5 px-3 font-mono text-center text-slate-300">{r.vertices.length}</td>
                <td className="py-2.5 px-3 font-mono text-center text-slate-300">{totalTxs > 0 ? totalTxs : <span className="text-slate-600">0</span>}</td>
                <td className="py-2.5 px-3">
                  <div className="flex flex-wrap gap-1">
                    {r.vertices.slice(0, 3).map((v) => (
                      <span key={v.hash} className="inline-flex items-center gap-1 text-xs">
                        <DisplayIdentity address={v.validator} link size="xs" />
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
            );
          })}
        </tbody>
      </table>
    </div>
  );
}
