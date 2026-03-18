import { useEffect, useState, useCallback } from 'react';
import { getStatus, getRound, connectToNode, isConnected } from '../lib/api.ts';
import { SearchBar } from '../components/explorer/SearchBar.tsx';
import { RoundTable } from '../components/explorer/RoundTable.tsx';
import { Pagination } from '../components/shared/Pagination.tsx';

const PAGE_SIZE = 10;

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

export function ExplorerPage() {
  const [rounds, setRounds] = useState<RoundData[]>([]);
  const [lastFinalized, setLastFinalized] = useState(0);
  const [page, setPage] = useState(1);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState('');

  const fetchRounds = useCallback(async () => {
    try {
      if (!isConnected()) await connectToNode();
      const statusData = await getStatus();
      const finalized = Number(statusData.last_finalized_round ?? 0);
      setLastFinalized(finalized);

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

  const totalPages = Math.max(1, Math.ceil(lastFinalized / PAGE_SIZE));

  return (
    <div className="space-y-6">
      <div>
        <h1 className="text-2xl font-bold text-white mb-1">Explorer</h1>
        <p className="text-sm text-slate-400">Search and browse the UltraDAG</p>
      </div>

      <SearchBar />

      {error && <div className="bg-red-500/10 border border-red-500/30 rounded-lg p-3 text-sm text-red-400">{error}</div>}

      <div className="bg-slate-800/50 border border-slate-700 rounded-lg p-4">
        <div className="flex items-center justify-between mb-3">
          <h2 className="text-sm font-semibold text-slate-200">Recent Finalized Rounds</h2>
          {lastFinalized > 0 && (
            <span className="text-xs text-slate-500">
              Showing rounds {Math.max(1, lastFinalized - (page - 1) * PAGE_SIZE - PAGE_SIZE + 1).toLocaleString()} &ndash; {(lastFinalized - (page - 1) * PAGE_SIZE).toLocaleString()} of {lastFinalized.toLocaleString()}
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
