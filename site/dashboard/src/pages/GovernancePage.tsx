import { useState, useEffect, useCallback } from 'react';
import { getProposals, getProposal, shortAddr, formatProposalType } from '../lib/api';
import { useKeystore } from '../hooks/useKeystore';
import { VoteButton } from '../components/governance/VoteButton';
import { CreateProposalModal } from '../components/governance/CreateProposalModal';
import { Pagination } from '../components/shared/Pagination';
import { useIsMobile } from '../hooks/useIsMobile';

const S = {
  card: { background: 'var(--dag-card)', border: '1px solid var(--dag-border)', borderRadius: 14, padding: '18px 20px' } as React.CSSProperties,
  mono: { fontFamily: "'DM Mono',monospace" },
};

function normalizeStatus(raw: unknown): { label: string; execute_at_round?: number } {
  if (typeof raw === 'string') return { label: raw };
  if (raw && typeof raw === 'object') { const k = Object.keys(raw)[0]; const v = (raw as Record<string, Record<string, unknown>>)[k]; if (k === 'PassedPending') return { label: 'PassedPending', execute_at_round: v?.execute_at_round as number }; return { label: k }; }
  return { label: String(raw) };
}

interface Proposal {
  id: number; title: string; description: string; status: string; proposal_type: unknown;
  proposer: string; votes_for: number; votes_against: number;
  snapshot_council_size?: number; snapshot_total_stake?: number;
  voting_ends: number; execute_at_round?: number | null;
  voters?: Array<{ address: string; vote: string; vote_weight: number; category?: string }>;
}

const statusColor = (s: string) => s === 'Active' ? '#00E0C4' : s === 'Executed' ? '#0066FF' : s === 'PassedPending' ? '#FFB800' : s === 'Rejected' || s === 'Failed' ? '#EF4444' : 'var(--dag-text-muted)';

export function GovernancePage() {
  const { wallets, unlocked } = useKeystore();
  const m = useIsMobile();
  const [proposals, setProposals] = useState<Proposal[]>([]);
  const [selected, setSelected] = useState<Proposal | null>(null);
  const [showCreate, setShowCreate] = useState(false);
  const [loading, setLoading] = useState(true);
  const [proposalPage, setProposalPage] = useState(1);
  const [voterPage, setVoterPage] = useState(1);
  const GOV_PAGE_SIZE = 10;

  const refresh = useCallback(async () => {
    try {
      const data = await getProposals();
      const rawList = Array.isArray(data) ? data : (data?.proposals ?? []);
      setProposals(rawList.map((p: Record<string, unknown>) => {
        const s = normalizeStatus(p.status);
        return { ...p, status: s.label, execute_at_round: s.execute_at_round ?? p.execute_at_round } as Proposal;
      }).sort((a: Proposal, b: Proposal) => b.id - a.id));
    } catch {}
    setLoading(false);
  }, []);

  useEffect(() => { refresh(); const iv = setInterval(refresh, 30000); return () => clearInterval(iv); }, [refresh]);

  useEffect(() => {
    const handler = () => { setProposals([]); setSelected(null); setLoading(true); setProposalPage(1); setVoterPage(1); refresh(); };
    window.addEventListener('ultradag-network-switch', handler);
    return () => window.removeEventListener('ultradag-network-switch', handler);
  }, [refresh]);

  const selectProposal = async (id: number) => {
    setVoterPage(1);
    try { const d = await getProposal(id); const s = normalizeStatus(d.status); setSelected({ ...d, status: s.label, execute_at_round: s.execute_at_round ?? d.execute_at_round }); }
    catch { setSelected(proposals.find(p => p.id === id) || null); }
  };

  return (
    <div style={{ padding: m ? '12px 14px' : '18px 26px', fontFamily: "'DM Sans',sans-serif" }}>
      <style>{`@keyframes slideUp{from{opacity:0;transform:translateY(10px)}to{opacity:1;transform:translateY(0)}}`}</style>

      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: m ? 'flex-start' : 'center', marginBottom: m ? 16 : 22, animation: 'slideUp 0.3s ease', flexDirection: m ? 'column' : 'row', gap: m ? 10 : 0 }}>
        <div>
          <h1 style={{ fontSize: m ? 18 : 21, fontWeight: 700, color: 'var(--dag-text)' }}>Governance</h1>
          <p style={{ fontSize: 11.5, color: 'var(--dag-subheading)', marginTop: 2 }}>Vote on proposals that shape the network</p>
        </div>
        {unlocked && wallets.length > 0 && (
          <button onClick={() => setShowCreate(true)} style={{ padding: '8px 16px', borderRadius: 10, background: '#0066FF', color: 'var(--dag-text)', fontSize: 12, fontWeight: 700, cursor: 'pointer', border: 'none' }}>+ New Proposal</button>
        )}
      </div>

      <div style={{ display: 'grid', gridTemplateColumns: m ? '1fr' : '2fr 1.2fr', gap: m ? 14 : 16, animation: 'slideUp 0.4s ease' }}>
        {/* Proposal List */}
        <div style={S.card}>
          <div style={{ display: 'flex', alignItems: 'center', gap: 7, marginBottom: 14 }}>
            <span style={{ color: '#0066FF', fontSize: 14 }}>⚙</span>
            <span style={{ fontSize: 13, fontWeight: 600, color: 'var(--dag-text-secondary)' }}>Proposals ({proposals.length})</span>
          </div>
          {loading ? (
            <p style={{ fontSize: 12, color: 'var(--dag-text-faint)', textAlign: 'center', padding: '30px 0' }}>Loading proposals...</p>
          ) : proposals.length === 0 ? (
            <div style={{ textAlign: 'center', padding: '40px 0' }}>
              <div style={{ fontSize: 30, opacity: 0.1, marginBottom: 10 }}>⚙</div>
              <p style={{ fontSize: 13, color: 'var(--dag-text-muted)' }}>No proposals yet</p>
              <p style={{ fontSize: 10.5, color: 'var(--dag-text-faint)', marginTop: 4 }}>Create the first governance proposal to shape the network.</p>
            </div>
          ) : (
            <div style={{ display: 'flex', flexDirection: 'column', gap: 6 }}>
              {proposals.slice((proposalPage - 1) * GOV_PAGE_SIZE, proposalPage * GOV_PAGE_SIZE).map(p => {
                const sc = statusColor(p.status);
                const active = selected?.id === p.id;
                return (
                  <div key={p.id} onClick={() => selectProposal(p.id)} style={{
                    padding: '12px 14px', borderRadius: 10, cursor: 'pointer', transition: 'all 0.2s',
                    background: active ? 'rgba(0,102,255,0.04)' : 'var(--dag-card)',
                    border: active ? '1px solid rgba(0,102,255,0.2)' : '1px solid var(--dag-table-border)',
                  }}>
                    <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'flex-start' }}>
                      <div style={{ flex: 1, minWidth: 0 }}>
                        <div style={{ display: 'flex', alignItems: 'center', gap: 6, marginBottom: 4 }}>
                          <span style={{ fontSize: 10, ...S.mono, color: 'var(--dag-text-faint)' }}>#{p.id}</span>
                          <span style={{ fontSize: 8.5, padding: '1px 6px', borderRadius: 4, background: sc + '12', color: sc, fontWeight: 600 }}>{p.status}</span>
                          <span style={{ fontSize: 9, color: 'var(--dag-text-faint)', padding: '1px 5px', borderRadius: 3, background: 'var(--dag-input-bg)' }}>{formatProposalType(p.proposal_type)}</span>
                        </div>
                        <div style={{ fontSize: 12.5, fontWeight: 600, color: 'var(--dag-text)', overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>{p.title}</div>
                      </div>
                      <div style={{ textAlign: 'right', flexShrink: 0, marginLeft: 12 }}>
                        <span style={{ fontSize: 11, color: '#00E0C4', fontWeight: 600 }}>{p.votes_for}</span>
                        <span style={{ fontSize: 10, color: 'var(--dag-text-faint)' }}> / </span>
                        <span style={{ fontSize: 11, color: '#EF4444', fontWeight: 600 }}>{p.votes_against}</span>
                      </div>
                    </div>
                  </div>
                );
              })}
              <Pagination page={proposalPage} totalPages={Math.ceil(proposals.length / GOV_PAGE_SIZE)} onPageChange={setProposalPage} totalItems={proposals.length} pageSize={GOV_PAGE_SIZE} />
            </div>
          )}
        </div>

        {/* Detail Panel */}
        <div style={S.card}>
          {selected ? (
            <div>
              <div style={{ display: 'flex', alignItems: 'center', gap: 6, marginBottom: 12 }}>
                <span style={{ fontSize: 8.5, padding: '2px 7px', borderRadius: 4, background: statusColor(selected.status) + '12', color: statusColor(selected.status), fontWeight: 600 }}>{selected.status}</span>
                <span style={{ fontSize: 9, color: 'var(--dag-text-faint)' }}>{formatProposalType(selected.proposal_type)}</span>
              </div>
              <h3 style={{ fontSize: 15, fontWeight: 700, color: 'var(--dag-text)', marginBottom: 8 }}>{selected.title}</h3>
              <p style={{ fontSize: 11.5, color: 'var(--dag-text-muted)', lineHeight: 1.6, marginBottom: 14, whiteSpace: 'pre-wrap' }}>{selected.description}</p>

              <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 8, marginBottom: 14 }}>
                {[
                  { l: 'PROPOSER', v: shortAddr(selected.proposer) },
                  { l: 'VOTING ENDS', v: `Round ${selected.voting_ends?.toLocaleString()}` },
                  { l: 'VOTES FOR', v: String(selected.votes_for), c: '#00E0C4' },
                  { l: 'VOTES AGAINST', v: String(selected.votes_against), c: '#EF4444' },
                  ...(selected.execute_at_round != null ? [{ l: 'EXECUTES AT', v: `Round ${selected.execute_at_round.toLocaleString()}` }] : []),
                ].map((x, i) => (
                  <div key={i} style={{ background: 'var(--dag-card)', borderRadius: 8, padding: '8px 10px' }}>
                    <div style={{ fontSize: 9, color: 'var(--dag-text-faint)', letterSpacing: 1, marginBottom: 2 }}>{x.l}</div>
                    <div style={{ fontSize: 13, fontWeight: 600, color: x.c || '#fff', ...S.mono }}>{x.v}</div>
                  </div>
                ))}
              </div>

              {/* Vote buttons */}
              {unlocked && wallets.length > 0 && selected.status === 'Active' && (
                <div style={{ display: 'flex', gap: 8, paddingTop: 12, borderTop: '1px solid var(--dag-table-border)' }}>
                  <VoteButton proposalId={selected.id} secretKey={wallets[0].secret_key} approve={true} fee={10000} onSuccess={refresh} />
                  <VoteButton proposalId={selected.id} secretKey={wallets[0].secret_key} approve={false} fee={10000} onSuccess={refresh} />
                </div>
              )}

              {/* Voter list */}
              {selected.voters && selected.voters.length > 0 && (
                <div style={{ marginTop: 14, paddingTop: 12, borderTop: '1px solid var(--dag-table-border)' }}>
                  <div style={{ fontSize: 10, color: 'var(--dag-subheading)', letterSpacing: 1, marginBottom: 8 }}>VOTERS ({selected.voters.length})</div>
                  {selected.voters.slice((voterPage - 1) * GOV_PAGE_SIZE, voterPage * GOV_PAGE_SIZE).map(v => (
                    <div key={v.address} style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', padding: '4px 0', borderBottom: '1px solid var(--dag-row-border)' }}>
                      <span style={{ fontSize: 10.5, color: 'var(--dag-text-muted)', ...S.mono }}>{shortAddr(v.address)}</span>
                      <div style={{ display: 'flex', alignItems: 'center', gap: 6 }}>
                        {v.category && <span style={{ fontSize: 9, color: 'var(--dag-text-faint)' }}>{v.category}</span>}
                        <span style={{ fontSize: 10, fontWeight: 600, color: v.vote === 'yes' ? '#00E0C4' : '#EF4444' }}>{v.vote === 'yes' ? 'YES' : 'NO'}</span>
                      </div>
                    </div>
                  ))}
                  <Pagination page={voterPage} totalPages={Math.ceil(selected.voters.length / GOV_PAGE_SIZE)} onPageChange={setVoterPage} totalItems={selected.voters.length} pageSize={GOV_PAGE_SIZE} />
                </div>
              )}
            </div>
          ) : (
            <div style={{ display: 'flex', flexDirection: 'column', alignItems: 'center', justifyContent: 'center', minHeight: 200, gap: 8 }}>
              <span style={{ fontSize: 28, opacity: 0.1 }}>⚙</span>
              <p style={{ fontSize: 12, color: 'var(--dag-text-faint)' }}>Select a proposal to view details</p>
            </div>
          )}
        </div>
      </div>

      {showCreate && <CreateProposalModal wallets={wallets} onClose={() => setShowCreate(false)} onSuccess={refresh} />}
    </div>
  );
}
