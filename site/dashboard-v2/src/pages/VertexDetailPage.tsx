import { useEffect, useState } from 'react';
import { useParams, Link } from 'react-router-dom';
import { ChevronLeft } from 'lucide-react';
import { getVertex, connectToNode, isConnected, shortHash, shortAddr, formatUdag } from '../lib/api.ts';
import { CopyButton } from '../components/shared/CopyButton.tsx';
import { Badge } from '../components/shared/Badge.tsx';

export function VertexDetailPage() {
  const { hash } = useParams<{ hash: string }>();
  const [vertex, setVertex] = useState<Record<string, unknown> | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState('');

  useEffect(() => {
    if (!hash) return;

    const fetchVertex = async () => {
      setLoading(true);
      try {
        if (!isConnected()) await connectToNode();
        const data = await getVertex(hash);
        setVertex(data);
        setError('');
      } catch (e) {
        setError(e instanceof Error ? e.message : 'Vertex not found');
      } finally {
        setLoading(false);
      }
    };

    fetchVertex();
  }, [hash]);

  if (loading) return <div className="text-slate-500 py-8 text-center">Loading vertex...</div>;
  if (error) {
    return (
      <div className="space-y-4">
        <Link to="/explorer" className="inline-flex items-center gap-1 text-slate-400 hover:text-slate-200 text-sm">
          <ChevronLeft className="w-4 h-4" /> Back to Explorer
        </Link>
        <div className="bg-red-500/10 border border-red-500/30 rounded-lg p-3 text-sm text-red-400">{error}</div>
      </div>
    );
  }
  if (!vertex) return null;

  const round = Number(vertex.round ?? 0);
  const validator = String(vertex.validator ?? '');
  const coinbase = vertex.coinbase as Record<string, unknown> | undefined;
  const parents = (vertex.parents ?? vertex.parent_hashes ?? []) as string[];
  const transactions = (vertex.transactions ?? []) as Array<Record<string, unknown>>;

  return (
    <div className="space-y-6">
      <div className="flex items-center gap-4">
        <Link to={`/round/${round}`} className="text-slate-400 hover:text-slate-200">
          <ChevronLeft className="w-5 h-5" />
        </Link>
        <h1 className="text-xl font-bold text-white">Vertex Detail</h1>
      </div>

      {/* Vertex info */}
      <div className="bg-slate-800/50 border border-slate-700 rounded-lg p-4 space-y-3">
        <InfoRow label="Hash" value={hash ?? ''} mono copy />
        <InfoRow label="Round">
          <Link to={`/round/${round}`} className="font-mono text-blue-400 hover:text-blue-300">
            #{round.toLocaleString()}
          </Link>
        </InfoRow>
        <InfoRow label="Validator">
          <div className="flex items-center gap-1">
            <Link to={`/address/${validator}`} className="font-mono text-blue-400 hover:text-blue-300 text-sm">
              {shortAddr(validator)}
            </Link>
            <CopyButton text={validator} />
          </div>
        </InfoRow>
        {coinbase && (
          <>
            <InfoRow label="Coinbase Reward" value={
              coinbase.reward_udag != null
                ? `${coinbase.reward_udag} UDAG`
                : coinbase.reward != null
                  ? `${formatUdag(Number(coinbase.reward))} UDAG`
                  : '--'
            } mono />
            <InfoRow label="Coinbase Height" value={String(coinbase.height ?? '--')} mono />
          </>
        )}
        <InfoRow label="Parent Count" value={String(parents.length)} mono />
        <InfoRow label="Transaction Count" value={String(transactions.length)} mono />
      </div>

      {/* Parents */}
      {parents.length > 0 && (
        <div className="bg-slate-800/50 border border-slate-700 rounded-lg p-4">
          <h2 className="text-sm font-semibold text-slate-200 mb-3">Parent Vertices ({parents.length})</h2>
          <div className="space-y-1">
            {parents.map((parentHash) => (
              <div key={parentHash} className="flex items-center gap-2">
                <Link to={`/vertex/${parentHash}`} className="font-mono text-sm text-blue-400 hover:text-blue-300">
                  {shortHash(parentHash)}
                </Link>
                <CopyButton text={parentHash} />
              </div>
            ))}
          </div>
        </div>
      )}

      {/* Transactions */}
      {transactions.length > 0 && (
        <div className="bg-slate-800/50 border border-slate-700 rounded-lg p-4">
          <h2 className="text-sm font-semibold text-slate-200 mb-3">Transactions ({transactions.length})</h2>
          <div className="overflow-x-auto">
            <table className="w-full text-sm">
              <thead>
                <tr className="text-left text-slate-400 border-b border-slate-700">
                  <th className="py-2 px-3 font-medium">Hash</th>
                  <th className="py-2 px-3 font-medium">Type</th>
                  <th className="py-2 px-3 font-medium">From</th>
                  <th className="py-2 px-3 font-medium">Fee</th>
                </tr>
              </thead>
              <tbody>
                {transactions.map((tx, i) => (
                  <tr key={String(tx.hash ?? i)} className="border-b border-slate-800">
                    <td className="py-2 px-3">
                      {tx.hash ? (
                        <div className="flex items-center gap-1">
                          <Link to={`/tx/${tx.hash}`} className="font-mono text-blue-400 hover:text-blue-300 text-xs">
                            {shortHash(String(tx.hash))}
                          </Link>
                          <CopyButton text={String(tx.hash)} />
                        </div>
                      ) : (
                        <span className="text-slate-500 text-xs">--</span>
                      )}
                    </td>
                    <td className="py-2 px-3">
                      <Badge label={String(tx.tx_type ?? tx.type ?? 'unknown')} variant="blue" />
                    </td>
                    <td className="py-2 px-3 font-mono text-xs text-slate-300">
                      {tx.from ? shortAddr(String(tx.from)) : '--'}
                    </td>
                    <td className="py-2 px-3 font-mono text-xs text-slate-300">
                      {tx.fee != null ? formatUdag(Number(tx.fee)) : '--'}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </div>
      )}
    </div>
  );
}

function InfoRow({
  label,
  value,
  mono,
  copy,
  children,
}: {
  label: string;
  value?: string;
  mono?: boolean;
  copy?: boolean;
  children?: React.ReactNode;
}) {
  return (
    <div className="flex items-start gap-3">
      <span className="text-slate-500 text-sm w-36 shrink-0">{label}</span>
      {children ?? (
        <div className="flex items-center gap-1 min-w-0">
          <span className={`text-sm text-slate-200 break-all ${mono ? 'font-mono' : ''}`}>{value}</span>
          {copy && value && <CopyButton text={value} />}
        </div>
      )}
    </div>
  );
}
