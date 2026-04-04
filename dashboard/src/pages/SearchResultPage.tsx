import { useEffect, useState } from 'react';
import { useParams, useNavigate, Link } from 'react-router-dom';
import { Loader } from 'lucide-react';
import { getTx, getVertex, getBalance, connectToNode, isConnected } from '../lib/api.ts';
import { PageHeader } from '../components/shared/PageHeader.tsx';
import { useIsMobile } from '../hooks/useIsMobile';

export function SearchResultPage() {
  const { query } = useParams<{ query: string }>();
  const navigate = useNavigate();
  const m = useIsMobile();
  const [status, setStatus] = useState('Searching...');
  const [error, setError] = useState('');
  const [switchCount, setSwitchCount] = useState(0);

  useEffect(() => {
    const handler = () => setSwitchCount(n => n + 1);
    window.addEventListener('ultradag-network-switch', handler);
    return () => window.removeEventListener('ultradag-network-switch', handler);
  }, []);

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
  }, [query, navigate, switchCount]);

  return (
    <div style={{ padding: m ? '12px 14px' : '18px 26px', fontFamily: "'DM Sans',sans-serif" }}>
      <PageHeader title="Search" subtitle={query ?? undefined} />

      {!error && (
        <div style={{ display: 'flex', alignItems: 'center', gap: 12, justifyContent: 'center', padding: '48px 0' }}>
          <Loader style={{ width: 20, height: 20, color: '#00E0C4', animation: 'spin 1s linear infinite' }} />
          <span style={{ color: 'var(--dag-text-muted)' }}>{status}</span>
        </div>
      )}

      {error && (
        <div style={{ background: 'var(--dag-card)', border: '1px solid var(--dag-border)', borderRadius: 10, padding: 24, textAlign: 'center' }}>
          <p style={{ color: 'var(--dag-text-muted)', marginBottom: 12 }}>{error}</p>
          <Link to="/explorer" style={{ fontSize: 12, color: '#00E0C4', textDecoration: 'none' }}>Back to Explorer</Link>
        </div>
      )}
    </div>
  );
}
