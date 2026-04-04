import { useState } from 'react';
import { useGitHubBounties } from '../hooks/useGitHubBounties';
import { useKeystore } from '../hooks/useKeystore';
import { useIsMobile } from '../hooks/useIsMobile';
import { BountyCard } from '../components/bounty/BountyCard';
import { BountyDetail } from '../components/bounty/BountyDetail';
import { BountyFilters } from '../components/bounty/BountyFilters';
import { PayBountyModal } from '../components/bounty/PayBountyModal';
import { Pagination } from '../components/shared/Pagination';
import { PageHeader } from '../components/shared/PageHeader';
import type { ParsedBounty } from '../lib/github';

const PAGE_SIZE = 10;

const S = {
  card: { background: 'var(--dag-card)', border: '1px solid var(--dag-border)', borderRadius: 14, padding: '18px 20px' } as React.CSSProperties,
  mono: { fontFamily: "'DM Mono',monospace" } as React.CSSProperties,
};

export function BountyPage() {
  const { wallets, unlocked } = useKeystore();
  const m = useIsMobile();
  const { bounties, loading, error, rateLimitRemaining } = useGitHubBounties();

  const [selected, setSelected] = useState<ParsedBounty | null>(null);
  const [filterCategory, setFilterCategory] = useState('all');
  const [filterStatus, setFilterStatus] = useState('open');
  const [page, setPage] = useState(1);
  const [showPayModal, setShowPayModal] = useState(false);

  // Filter bounties
  const filtered = bounties.filter(b => {
    if (filterCategory !== 'all') {
      if (filterCategory === 'security' && !b.category.startsWith('security')) return false;
      if (filterCategory === 'bug' && b.category !== 'bug') return false;
      if (filterCategory === 'feature' && b.category !== 'feature') return false;
    }
    if (filterStatus === 'open' && (b.status === 'paid' || b.status === 'cancelled')) return false;
    return true;
  });

  const totalPages = Math.ceil(filtered.length / PAGE_SIZE);
  const paged = filtered.slice((page - 1) * PAGE_SIZE, page * PAGE_SIZE);

  const totalReward = bounties.filter(b => b.status !== 'paid' && b.status !== 'cancelled').reduce((s, b) => s + b.reward, 0);

  return (
    <div style={{ padding: m ? '12px 14px' : '18px 26px', fontFamily: "'DM Sans',sans-serif" }}>
      {/* Header */}
      <PageHeader
        title="Bug Bounties"
        subtitle="Earn UDAG by finding bugs, reporting issues, and building features"
        right={<>
          {totalReward > 0 && (
            <span style={{ fontSize: 12, color: '#00E0C4', fontWeight: 600, ...S.mono }}>
              {totalReward.toLocaleString()} UDAG available
            </span>
          )}
          <a
            href="https://github.com/UltraDAGcom/core/issues/new?labels=bounty&title=[BOUNTY]%20&body=%0A---%0Areward%3A%20100%0Acreator_address%3A%20%0A---%0A%0A%23%23%20Description%0A%0A%23%23%20Acceptance%20Criteria%0A"
            target="_blank"
            rel="noopener noreferrer"
            style={{
              padding: '8px 16px', borderRadius: 8, border: 'none',
              background: 'linear-gradient(135deg, #00E0C4, #0066FF)',
              color: '#fff', fontSize: 12, fontWeight: 700, textDecoration: 'none',
              cursor: 'pointer', boxShadow: '0 2px 12px rgba(0,224,196,0.15)',
            }}
          >
            + Post Bounty
          </a>
        </>}
      />

      {/* Stats bar */}
      <div style={{ display: 'grid', gridTemplateColumns: 'repeat(4, 1fr)', gap: 10, marginBottom: 16 }}>
        {[
          { l: 'TOTAL BOUNTIES', v: String(bounties.length), c: 'var(--dag-text)' },
          { l: 'OPEN', v: String(bounties.filter(b => b.status === 'open').length), c: '#00E0C4' },
          { l: 'IN PROGRESS', v: String(bounties.filter(b => b.status === 'claimed' || b.status === 'in-review').length), c: '#FFB800' },
          { l: 'PAID OUT', v: String(bounties.filter(b => b.status === 'paid').length), c: '#34d399' },
        ].map((stat, i) => (
          <div key={i} style={{ background: 'var(--dag-card)', border: '1px solid var(--dag-border)', borderRadius: 10, padding: '10px 12px' }}>
            <div style={{ fontSize: 9, color: 'var(--dag-text-faint)', letterSpacing: 1, marginBottom: 3 }}>{stat.l}</div>
            <div style={{ fontSize: 18, fontWeight: 700, color: stat.c, ...S.mono }}>{stat.v}</div>
          </div>
        ))}
      </div>

      {/* Error/loading */}
      {error && (
        <div style={{ marginBottom: 12, fontSize: 11, color: '#FFB800', background: 'rgba(255,184,0,0.06)', border: '1px solid rgba(255,184,0,0.15)', borderRadius: 8, padding: '8px 12px' }}>
          {rateLimitRemaining === 0 ? 'GitHub rate limit reached — showing cached data' : error}
        </div>
      )}

      {loading && bounties.length === 0 && (
        <div style={{ textAlign: 'center', padding: '40px 0' }}>
          <div style={{ width: 32, height: 32, border: '2px solid rgba(0,224,196,0.2)', borderTop: '2px solid #00E0C4', borderRadius: '50%', margin: '0 auto 12px', animation: 'spin 0.8s linear infinite' }} />
          <style>{`@keyframes spin{to{transform:rotate(360deg)}}`}</style>
          <p style={{ fontSize: 12, color: 'var(--dag-text-faint)' }}>Loading bounties from GitHub...</p>
        </div>
      )}

      {/* Main content */}
      {!loading || bounties.length > 0 ? (
        <div style={{ display: 'grid', gridTemplateColumns: m ? '1fr' : '2fr 1.2fr', gap: 16 }}>
          {/* Left: List */}
          <div style={S.card}>
            <BountyFilters
              category={filterCategory}
              status={filterStatus}
              onCategoryChange={c => { setFilterCategory(c); setPage(1); }}
              onStatusChange={s => { setFilterStatus(s); setPage(1); }}
              bounties={bounties}
            />

            {paged.length === 0 ? (
              <div style={{ textAlign: 'center', padding: '30px 0' }}>
                <div style={{ fontSize: 28, color: 'var(--dag-text-faint)', marginBottom: 8, opacity: 0.3 }}>⚡</div>
                <p style={{ fontSize: 12, color: 'var(--dag-text-faint)' }}>
                  {bounties.length === 0 ? 'No bounties yet — be the first to post one!' : 'No bounties match your filters'}
                </p>
              </div>
            ) : (
              <>
                {paged.map(b => (
                  <BountyCard
                    key={b.issue.id}
                    bounty={b}
                    active={selected?.issue.id === b.issue.id}
                    onClick={() => setSelected(b)}
                  />
                ))}
                <Pagination page={page} totalPages={totalPages} onPageChange={setPage} totalItems={filtered.length} pageSize={PAGE_SIZE} />
              </>
            )}
          </div>

          {/* Right: Detail */}
          <div style={S.card}>
            <BountyDetail
              bounty={selected}
              unlocked={unlocked}
              onPayClick={() => setShowPayModal(true)}
            />
          </div>
        </div>
      ) : null}

      {/* Rate limit footer */}
      {rateLimitRemaining !== null && (
        <div style={{ textAlign: 'center', marginTop: 12, fontSize: 9.5, color: 'var(--dag-text-faint)' }}>
          GitHub API: {rateLimitRemaining}/60 requests remaining
        </div>
      )}

      {/* Pay modal */}
      {showPayModal && selected && unlocked && wallets.length > 0 && (
        <PayBountyModal
          bounty={selected}
          wallets={wallets}
          onClose={() => setShowPayModal(false)}
          onSuccess={() => setShowPayModal(false)}
        />
      )}
    </div>
  );
}
