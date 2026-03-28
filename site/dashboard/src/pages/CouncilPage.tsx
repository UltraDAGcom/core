import { useState, useEffect, useCallback } from 'react';
import { getCouncil, getGovernanceConfig, shortAddr } from '../lib/api';
import { CopyButton } from '../components/shared/CopyButton';
import { Pagination } from '../components/shared/Pagination';
import { CouncilSeatGrid } from '../components/governance/CouncilSeatGrid';
import { useIsMobile } from '../hooks/useIsMobile';

interface CouncilMember { address: string; category: string }
interface SeatInfo { available: number; filled: number; max: number }
interface CouncilData {
  members: CouncilMember[]; total_seats: number; filled_seats: number;
  member_count: number; max_members: number; emission_percent: number;
  per_member_reward_sats: number; per_member_reward_udag: number;
  seats: Record<string, SeatInfo>;
}

const S = {
  card: { background: 'var(--dag-card)', border: '1px solid var(--dag-border)', borderRadius: 14, padding: '18px 20px' } as React.CSSProperties,
  mono: { fontFamily: "'DM Mono',monospace" },
};

const catColor: Record<string, string> = {
  Engineering: '#00E0C4', Growth: '#0066FF', Legal: '#A855F7',
  Research: '#FFB800', Community: '#22D3EE', Operations: '#F472B6', Security: '#EF4444',
};

export function CouncilPage() {
  const m = useIsMobile();
  const [council, setCouncil] = useState<CouncilData | null>(null);
  const [govConfig, setGovConfig] = useState<Record<string, unknown> | null>(null);
  const [loading, setLoading] = useState(true);
  const [memberPage, setMemberPage] = useState(1);
  const COUNCIL_PAGE_SIZE = 10;

  const refresh = useCallback(async () => {
    try {
      const [c, g] = await Promise.all([getCouncil().catch(() => null), getGovernanceConfig().catch(() => null)]);
      if (c) setCouncil(c); if (g) setGovConfig(g);
    } catch {} setLoading(false);
  }, []);

  useEffect(() => { refresh(); const iv = setInterval(refresh, 30000); return () => clearInterval(iv); }, [refresh]);

  useEffect(() => {
    const handler = () => { setCouncil(null); setGovConfig(null); setLoading(true); setMemberPage(1); refresh(); };
    window.addEventListener('ultradag-network-switch', handler);
    return () => window.removeEventListener('ultradag-network-switch', handler);
  }, [refresh]);

  if (loading) return <div style={{ padding: '18px 26px', color: 'var(--dag-text-faint)', fontSize: 12, fontFamily: "'DM Sans',sans-serif" }}>Loading council...</div>;

  const members = council?.members ?? [];
  const memberCount = council?.member_count ?? members.length;
  const maxMembers = council?.max_members ?? 21;
  const openSeats = maxMembers - memberCount;
  const perMemberReward = council?.per_member_reward_udag ?? 0;
  const emissionPercent = council?.emission_percent ?? 10;

  return (
    <div style={{ padding: m ? '12px 14px' : '18px 26px', fontFamily: "'DM Sans',sans-serif" }}>
      <style>{`@keyframes slideUp{from{opacity:0;transform:translateY(10px)}to{opacity:1;transform:translateY(0)}}`}</style>

      <div style={{ marginBottom: m ? 16 : 22, animation: 'slideUp 0.3s ease' }}>
        <h1 style={{ fontSize: m ? 18 : 21, fontWeight: 700, color: 'var(--dag-text)' }}>Council of 21</h1>
        <p style={{ fontSize: 11.5, color: 'var(--dag-subheading)', marginTop: 2 }}>The elected governance body that guides UltraDAG</p>
      </div>

      {/* Stats */}
      <div style={{ display: 'grid', gridTemplateColumns: m ? 'repeat(2,1fr)' : 'repeat(4,1fr)', gap: m ? 10 : 12, marginBottom: 18, animation: 'slideUp 0.4s ease' }}>
        {[
          { l: 'MEMBERS', v: `${memberCount}/${maxMembers}`, c: '#A855F7', i: '♛' },
          { l: 'OPEN SEATS', v: String(openSeats), c: openSeats > 0 ? '#00E0C4' : 'var(--dag-text-muted)', i: '◇' },
          { l: 'PER-MEMBER', v: `${perMemberReward} UDAG`, c: '#FFB800', i: '⬡' },
          { l: 'EMISSION', v: `${emissionPercent}%`, c: '#0066FF', i: '◎' },
        ].map((s, i) => (
          <div key={i} style={S.card}>
            <div style={{ display: 'flex', alignItems: 'center', gap: 6, marginBottom: 6 }}>
              <span style={{ color: s.c, fontSize: 13 }}>{s.i}</span>
              <span style={{ fontSize: 9, color: 'var(--dag-subheading)', letterSpacing: 1.2 }}>{s.l}</span>
            </div>
            <div style={{ fontSize: 22, fontWeight: 700, color: s.c, ...S.mono }}>{s.v}</div>
          </div>
        ))}
      </div>

      <div style={{ display: 'grid', gridTemplateColumns: m ? '1fr' : '1fr 1fr', gap: m ? 14 : 16, animation: 'slideUp 0.5s ease' }}>
        {/* Seat Categories */}
        <div style={S.card}>
          <div style={{ display: 'flex', alignItems: 'center', gap: 7, marginBottom: 14 }}>
            <span style={{ color: '#A855F7', fontSize: 14 }}>♛</span>
            <span style={{ fontSize: 13, fontWeight: 600, color: 'var(--dag-text-secondary)' }}>Seat Categories</span>
          </div>
          <CouncilSeatGrid members={members} seats={council?.seats} />
        </div>

        {/* Governance Parameters */}
        {govConfig && (
          <div style={S.card}>
            <div style={{ display: 'flex', alignItems: 'center', gap: 7, marginBottom: 14 }}>
              <span style={{ color: '#0066FF', fontSize: 14 }}>⚙</span>
              <span style={{ fontSize: 13, fontWeight: 600, color: 'var(--dag-text-secondary)' }}>Governance Parameters</span>
            </div>

            {/* Percentage params */}
            {(() => {
              const pcts = Object.entries(govConfig).filter(([k]) => k.includes('percent'));
              return pcts.length > 0 ? (
                <div style={{ display: 'grid', gridTemplateColumns: `repeat(${Math.min(pcts.length, 3)},1fr)`, gap: 8, marginBottom: 12 }}>
                  {pcts.map(([k, v]) => (
                    <div key={k} style={{ background: 'var(--dag-card)', borderRadius: 8, padding: '10px', textAlign: 'center' }}>
                      <div style={{ fontSize: 9, color: 'var(--dag-text-faint)', letterSpacing: 0.8, marginBottom: 3 }}>{k.replace(/_/g, ' ')}</div>
                      <div style={{ fontSize: 17, fontWeight: 700, color: 'var(--dag-text)' }}>{String(v)}%</div>
                    </div>
                  ))}
                </div>
              ) : null;
            })()}

            {/* Governable params tags */}
            {Array.isArray(govConfig.governable_params) && (
              <div style={{ marginBottom: 12 }}>
                <div style={{ fontSize: 9, color: 'var(--dag-text-faint)', letterSpacing: 1, marginBottom: 6 }}>GOVERNABLE</div>
                <div style={{ display: 'flex', flexWrap: 'wrap', gap: 4 }}>
                  {(govConfig.governable_params as string[]).map(p => (
                    <span key={p} style={{ fontSize: 9.5, padding: '2px 8px', borderRadius: 4, background: 'rgba(0,224,196,0.08)', color: '#00E0C4', fontWeight: 500 }}>{p.replace(/_/g, ' ')}</span>
                  ))}
                </div>
              </div>
            )}

            {/* Other params */}
            <div style={{ display: 'flex', flexDirection: 'column', gap: 2 }}>
              {Object.entries(govConfig)
                .filter(([k]) => !k.includes('percent') && k !== 'governable_params')
                .map(([k, v]) => (
                  <div key={k} style={{ display: 'flex', justifyContent: 'space-between', padding: '5px 0', borderBottom: '1px solid var(--dag-row-border)' }}>
                    <span style={{ fontSize: 10.5, color: 'var(--dag-text-muted)' }}>{k.replace(/_/g, ' ')}</span>
                    <span style={{ fontSize: 10.5, fontWeight: 600, color: 'var(--dag-text-secondary)', ...S.mono }}>{typeof v === 'number' ? v.toLocaleString() : String(v)}</span>
                  </div>
                ))}
            </div>
          </div>
        )}
      </div>

      {/* Members Table */}
      <div style={{ ...S.card, marginTop: 16, animation: 'slideUp 0.6s ease' }}>
        <div style={{ display: 'flex', alignItems: 'center', gap: 7, marginBottom: 14 }}>
          <span style={{ color: '#A855F7', fontSize: 14 }}>◉</span>
          <span style={{ fontSize: 13, fontWeight: 600, color: 'var(--dag-text-secondary)' }}>Members ({members.length})</span>
        </div>
        {members.length === 0 ? (
          <p style={{ fontSize: 12, color: 'var(--dag-text-faint)', textAlign: 'center', padding: '20px 0' }}>No council members.</p>
        ) : (
          <>
            <div style={{ display: 'grid', gridTemplateColumns: 'auto 2fr 1fr', gap: '0 16px' }}>
              {['#', 'ADDRESS', 'CATEGORY'].map((h, i) => (
                <div key={i} style={{ fontSize: 8.5, fontWeight: 600, color: 'var(--dag-text-faint)', letterSpacing: 1.5, paddingBottom: 8, borderBottom: '1px solid var(--dag-table-border)' }}>{h}</div>
              ))}
              {members.slice((memberPage - 1) * COUNCIL_PAGE_SIZE, memberPage * COUNCIL_PAGE_SIZE).map((mb, pi) => {
                const idx = (memberPage - 1) * COUNCIL_PAGE_SIZE + pi;
                return [
                  <div key={`n${idx}`} style={{ fontSize: 11, color: 'var(--dag-text-faint)', padding: '7px 0', borderBottom: '1px solid var(--dag-row-border)' }}>{idx + 1}</div>,
                  <div key={`a${idx}`} style={{ display: 'flex', alignItems: 'center', gap: 6, padding: '7px 0', borderBottom: '1px solid var(--dag-row-border)' }}>
                    <span style={{ fontSize: 11, color: 'var(--dag-text)', ...S.mono }}>{shortAddr(mb.address)}</span>
                    <CopyButton text={mb.address} />
                  </div>,
                  <div key={`c${idx}`} style={{ padding: '7px 0', borderBottom: '1px solid var(--dag-row-border)' }}>
                    <span style={{ fontSize: 9.5, padding: '2px 8px', borderRadius: 4, background: (catColor[mb.category] || '#888') + '12', color: catColor[mb.category] || '#888', fontWeight: 600 }}>{mb.category}</span>
                  </div>,
                ];
              }).flat()}
            </div>
            <Pagination page={memberPage} totalPages={Math.ceil(members.length / COUNCIL_PAGE_SIZE)} onPageChange={setMemberPage} totalItems={members.length} pageSize={COUNCIL_PAGE_SIZE} />
          </>
        )}
      </div>
    </div>
  );
}
