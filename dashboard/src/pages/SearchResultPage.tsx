import { useEffect, useState } from 'react';
import { useParams, useNavigate, Link } from 'react-router-dom';
import { ChevronLeft, Loader } from 'lucide-react';
import { getTx, getVertex, getBalance, connectToNode, isConnected } from '../lib/api.ts';

export function SearchResultPage() {
  const { query } = useParams<{ query: string }>();
  const navigate = useNavigate();
  const [status, setStatus] = useState('Searching...');
  const [error, setError] = useState('');

  useEffect(() => {
    if (!query) return;

    const search = async () => {
      try {
        if (!isConnected()) await connectToNode();

        // If it looks like a round number, navigate directly
        if (/^\d+$/.test(query)) {
          navigate(`/round/${query}`, { replace: true });
          return;
        }

        // Try tx hash first (64-hex queries)
        setStatus('Checking transaction...');
        try {
          await getTx(query);
          navigate(`/tx/${query}`, { replace: true });
          return;
        } catch {
          // not a tx
        }

        // Try vertex hash
        setStatus('Checking vertex...');
        try {
          await getVertex(query);
          navigate(`/vertex/${query}`, { replace: true });
          return;
        } catch {
          // not a vertex
        }

        // Try address
        setStatus('Checking address...');
        try {
          await getBalance(query);
          navigate(`/address/${query}`, { replace: true });
          return;
        } catch {
          // not an address
        }

        setError(`No results found for "${query}". The query does not match any known transaction, vertex, address, or round number.`);
      } catch (e) {
        setError(e instanceof Error ? e.message : 'Search failed');
      }
    };

    search();
  }, [query, navigate]);

  return (
    <div className="space-y-6">
      <div className="flex items-center gap-4">
        <Link to="/explorer" className="text-slate-400 hover:text-slate-200">
          <ChevronLeft className="w-5 h-5" />
        </Link>
        <h1 className="text-xl font-bold text-white">Search</h1>
      </div>

      {!error && (
        <div className="flex items-center gap-3 justify-center py-12">
          <Loader className="w-5 h-5 text-blue-400 animate-spin" />
          <span className="text-slate-400">{status}</span>
        </div>
      )}

      {error && (
        <div className="bg-slate-800/50 border border-slate-700 rounded-lg p-6 text-center">
          <p className="text-slate-400 mb-3">{error}</p>
          <Link to="/explorer" className="text-sm text-blue-400 hover:text-blue-300">Back to Explorer</Link>
        </div>
      )}
    </div>
  );
}
