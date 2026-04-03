import { useState, useEffect, useCallback } from 'react';
import { fetchBountyIssues, parseBounty, type ParsedBounty } from '../lib/github';

const POLL_INTERVAL = 120_000; // 2 minutes

export function useGitHubBounties() {
  const [bounties, setBounties] = useState<ParsedBounty[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [rateLimitRemaining, setRateLimitRemaining] = useState<number | null>(null);

  const refresh = useCallback(async () => {
    try {
      const { issues, rateLimitRemaining: rl } = await fetchBountyIssues();
      setRateLimitRemaining(rl);
      const parsed = issues.map(parseBounty);
      // Sort: open first, then by reward descending, then by date
      parsed.sort((a, b) => {
        const aOpen = a.status !== 'paid' && a.status !== 'cancelled' ? 0 : 1;
        const bOpen = b.status !== 'paid' && b.status !== 'cancelled' ? 0 : 1;
        if (aOpen !== bOpen) return aOpen - bOpen;
        if (b.reward !== a.reward) return b.reward - a.reward;
        return new Date(b.issue.updated_at).getTime() - new Date(a.issue.updated_at).getTime();
      });
      setBounties(parsed);
      setError(null);
    } catch (e) {
      setError(e instanceof Error ? e.message : 'Failed to load bounties');
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    refresh();
    const iv = setInterval(refresh, POLL_INTERVAL);
    return () => clearInterval(iv);
  }, [refresh]);

  return { bounties, loading, error, refresh, rateLimitRemaining };
}
