import { useEffect, useState, useCallback, useRef } from 'react';
import { getStatus, getRound, connectToNode, isConnected } from '../lib/api.ts';
import { SearchBar } from '../components/explorer/SearchBar.tsx';
import { RoundTable } from '../components/explorer/RoundTable.tsx';
import { Pagination } from '../components/shared/Pagination.tsx';
import { Badge, FinalityBadge } from '../components/shared/Badge.tsx';

const PAGE_SIZE = 10;
const AUTO_REFRESH_INTERVAL_MS = 10_000;

interface RoundData {
  round: number;
  vertices: Array<{
    hash: string;
    validator: string;
    reward_udag?: number;
    reward?: number;
    tx_count: number;
    parent_count: number;
  }>;
  finalized: boolean;
}

interface NetworkStats {
  dagRound: number;
  lastFinalized: number;
  finalityLag: number;
  validatorCount: number;
  totalSupply: number;
}

export function ExplorerPage() {
  const [rounds, setRounds] = useState<RoundData[]>([]);
  const [stats, setStats] = useState<NetworkStats>({ dagRound: 0, lastFinalized: 0, finalityLag: 0, validatorCount: 0, totalSupply: 0 });
  const [page, setPage] = useState(1);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState('');
  const autoRefreshRef = useRef<ReturnType<typeof setInterval> | null>(null);

  const fetchRounds = useCallback(async () => {
    try {
      if (!isConnected()) await connectToNode();
      const statusData = await getStatus();
      const dagRound = Number(statusData.dag_round ?? 0);
      const finalized = Number(statusData.last_finalized_round ?? 0);
      const validatorCount = Number(statusData.validator_count ?? statusData.active_stakers ?? 0);
      const totalSupply = Number(statusData.total_supply ?? 0);

      setStats({
        dagRound,
        lastFinalized: finalized,
        finalityLag: dagRound - finalized,
        validatorCount,
        totalSupply,
      });

      if (finalized <= 0) {
        setRounds([]);
        setLoading(false);
        return;
      }

      // Fetch PAGE_SIZE rounds starting from the most recent finalized
      const startRound = finalized - (page - 1) * PAGE_SIZE;
      const endRound = Math.max(1, startRound - PAGE_SIZE + 1);

      const promises: Promise<RoundData | null>[] = [];
      for (let r = startRound; r >= endRound; r--) {
        promises.push(
          getRound(r)
            .then((data) => ({
              round: r,
              vertices: Array.isArray(data) ? data : data?.vertices ?? [],
              finalized: r <= finalized,
            }))
            .catch(() => null)
        );
      }

      const results = await Promise.all(promises);
      setRounds(results.filter((r): r is RoundData => r !== null));
      setError('');
    } catch (e) {
      setError(e instanceof Error ? e.message : 'Failed to fetch rounds');
    } finally {
      setLoading(false);
    }
  }, [page]);

  useEffect(() => {
    setLoading(true);
    fetchRounds();
  }, [fetchRounds]);

  // Auto-refresh every 10s when on page 1
  useEffect(() => {
    if (autoRefreshRef.current) {
      clearInterval(autoRefreshRef.current);
      autoRefreshRef.current = null;
    }

    if (page === 1) {
      autoRefreshRef.current = setInterval(() => {
        fetchRounds();
      }, AUTO_REFRESH_INTERVAL_MS);
    }

    return () => {
      if (autoRefreshRef.current) {
        clearInterval(autoRefreshRef.current);
        autoRefreshRef.current = null;
      }
    };
  }, [page, fetchRounds]);

  const totalPages = Math.max(1, Math.ceil(stats.lastFinalized / PAGE_SIZE));

  return (
    <div className="space-y-6">
      <div className="flex items-start justify-between">
        <div>
          <h1 className="text-2xl font-bold text-white mb-1">Explorer</h1>
          <p className="text-sm text-slate-400">Search and browse the UltraDAG</p>
        </div>
        {stats.dagRound > 0 && (
          <div className="flex items-center gap-2">
            <FinalityBadge lag={stats.finalityLag} />
            <Badge label={`${stats.validatorCount} validators`} variant="purple" />
          </div>
        )}
      </div>

      {/* Network stats */}
      {stats.dagRound > 0 && (
        <div className="grid grid-cols-1 sm:grid-cols-2 md:grid-cols-4 gap-3">
          <StatCard label="DAG Round" value={stats.dagRound.toLocaleString()} />
          <StatCard label="Finalized Round" value={stats.lastFinalized.toLocaleString()} highlight />
          <StatCard label="Finality Lag" value={String(stats.finalityLag)} badge={stats.finalityLag <= 3 ? 'green' : stats.finalityLag <= 10 ? 'yellow' : 'red'} />
          <StatCard label="Validators" value={String(stats.validatorCount)} />
        </div>
      )}

      <SearchBar />

      {error && <div className="bg-red-500/10 border border-red-500/30 rounded-lg p-3 text-sm text-red-400">{error}</div>}

      <div className="bg-slate-800/50 border border-slate-700 rounded-lg p-4">
        <div className="flex items-center justify-between mb-3">
          <div className="flex items-center gap-3">
            <h2 className="text-sm font-semibold text-slate-200">Recent Finalized Rounds</h2>
            {page === 1 && (
              <span className="text-[10px] text-slate-500 flex items-center gap-1">
                <span className="w-1.5 h-1.5 rounded-full bg-dag-green animate-pulse inline-block" />
                Auto-refreshing every 10s
              </span>
            )}
          </div>
          {stats.lastFinalized > 0 && (
            <span className="text-xs text-slate-500">
              Showing rounds {Math.max(1, stats.lastFinalized - (page - 1) * PAGE_SIZE - PAGE_SIZE + 1).toLocaleString()} &ndash; {(stats.lastFinalized - (page - 1) * PAGE_SIZE).toLocaleString()} of {stats.lastFinalized.toLocaleString()}
            </span>
          )}
        </div>

        {loading ? (
          <div className="text-slate-500 text-sm py-8 text-center">Loading rounds...</div>
        ) : (
          <>
            <RoundTable rounds={rounds} />
            <Pagination currentPage={page} totalPages={totalPages} onPageChange={setPage} />
          </>
        )}
      </div>
    </div>
  );
}

function StatCard({ label, value, highlight, badge }: { label: string; value: string; highlight?: boolean; badge?: string }) {
  const borderColor = badge === 'green' ? 'border-green-500/30' : badge === 'yellow' ? 'border-yellow-500/30' : badge === 'red' ? 'border-red-500/30' : 'border-slate-700';
  return (
    <div className={`bg-slate-800/50 border ${borderColor} rounded-lg p-3`}>
      <p className="text-xs text-slate-500 mb-1">{label}</p>
      <p className={`text-lg font-bold font-mono ${highlight ? 'text-blue-400' : 'text-white'}`}>{value}</p>
    </div>
  );
}
