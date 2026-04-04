import { useState } from 'react';
import { formatUdag } from '../../lib/api';
import { DisplayIdentity } from '../shared/DisplayIdentity';
import type { ProfileData } from '../../hooks/useProfile';

type Tab = 'overview' | 'staking' | 'governance' | 'streams';

interface ProfileActivityProps {
  profile: ProfileData;
}

const S = {
  statBox: { background: 'var(--dag-card)', border: '1px solid var(--dag-border)', borderRadius: 10, padding: '12px 14px' } as React.CSSProperties,
  statLabel: { fontSize: 9, color: 'var(--dag-text-faint)', letterSpacing: 1, marginBottom: 3 } as React.CSSProperties,
  statValue: { fontSize: 16, fontWeight: 700, color: 'var(--dag-text)', fontFamily: "'DM Mono',monospace" } as React.CSSProperties,
};

const tabStyle = (active: boolean): React.CSSProperties => ({
  padding: '6px 14px', borderRadius: 8, fontSize: 11, fontWeight: 600, cursor: 'pointer',
  border: 'none', transition: 'all 0.15s',
  background: active ? 'rgba(0,224,196,0.1)' : 'transparent',
  color: active ? '#00E0C4' : 'var(--dag-text-muted)',
});

export function ProfileActivity({ profile }: ProfileActivityProps) {
  const [tab, setTab] = useState<Tab>('overview');

  return (
    <div>
      {/* Tab bar */}
      <div style={{ display: 'flex', gap: 4, marginBottom: 16 }}>
        {([['overview', 'Overview'], ['staking', 'Staking'], ['governance', 'Governance'], ['streams', 'Streams']] as const).map(([key, label]) => (
          <button key={key} onClick={() => setTab(key)} style={tabStyle(tab === key)}>{label}</button>
        ))}
      </div>

      {/* Overview */}
      {tab === 'overview' && (
        <div style={{ display: 'grid', gridTemplateColumns: 'repeat(2, 1fr)', gap: 10 }}>
          <div style={S.statBox}>
            <div style={S.statLabel}>BALANCE</div>
            <div style={S.statValue}>{formatUdag(profile.balance)}</div>
          </div>
          <div style={S.statBox}>
            <div style={S.statLabel}>STAKED</div>
            <div style={{ ...S.statValue, color: profile.staked > 0 ? '#00E0C4' : 'var(--dag-text-muted)' }}>
              {profile.staked > 0 ? formatUdag(profile.staked) : '—'}
            </div>
          </div>
          <div style={S.statBox}>
            <div style={S.statLabel}>PROPOSALS VOTED</div>
            <div style={S.statValue}>{profile.votedOnProposals}</div>
          </div>
          <div style={S.statBox}>
            <div style={S.statLabel}>STREAMS</div>
            <div style={S.statValue}>{profile.streamsSentCount + profile.streamsReceivedCount}</div>
          </div>
          <div style={S.statBox}>
            <div style={S.statLabel}>SECURITY KEYS</div>
            <div style={S.statValue}>{profile.keyCount}</div>
          </div>
          <div style={S.statBox}>
            <div style={S.statLabel}>SMART ACCOUNT</div>
            <div style={{ ...S.statValue, fontSize: 13 }}>{profile.isSmartAccount ? 'Yes' : 'No'}</div>
          </div>
        </div>
      )}

      {/* Staking */}
      {tab === 'staking' && (
        <div style={{ display: 'flex', flexDirection: 'column', gap: 10 }}>
          {profile.isValidator ? (
            <>
              <div style={S.statBox}>
                <div style={S.statLabel}>ROLE</div>
                <div style={{ ...S.statValue, color: '#00E0C4' }}>Active Validator</div>
              </div>
              <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 10 }}>
                <div style={S.statBox}>
                  <div style={S.statLabel}>EFFECTIVE STAKE</div>
                  <div style={S.statValue}>{formatUdag(profile.effectiveStake)}</div>
                </div>
                <div style={S.statBox}>
                  <div style={S.statLabel}>COMMISSION</div>
                  <div style={S.statValue}>{profile.commissionPercent ?? 10}%</div>
                </div>
                <div style={S.statBox}>
                  <div style={S.statLabel}>DELEGATORS</div>
                  <div style={S.statValue}>{profile.delegatorCount}</div>
                </div>
                <div style={S.statBox}>
                  <div style={S.statLabel}>PERSONAL STAKE</div>
                  <div style={S.statValue}>{formatUdag(profile.staked)}</div>
                </div>
              </div>
            </>
          ) : profile.staked > 0 ? (
            <div style={S.statBox}>
              <div style={S.statLabel}>STAKED</div>
              <div style={S.statValue}>{formatUdag(profile.staked)} UDAG</div>
            </div>
          ) : profile.delegated > 0 ? (
            <div style={S.statBox}>
              <div style={S.statLabel}>DELEGATED</div>
              <div style={S.statValue}>{formatUdag(profile.delegated)} UDAG</div>
              {profile.delegatedTo && (
                <div style={{ marginTop: 6 }}>
                  <span style={{ fontSize: 10, color: 'var(--dag-text-faint)' }}>to </span>
                  <DisplayIdentity address={profile.delegatedTo} link size="xs" />
                </div>
              )}
            </div>
          ) : (
            <div style={{ textAlign: 'center', padding: '24px 0', color: 'var(--dag-text-faint)', fontSize: 12 }}>
              Not staking or delegating
            </div>
          )}
        </div>
      )}

      {/* Governance */}
      {tab === 'governance' && (
        <div style={{ display: 'flex', flexDirection: 'column', gap: 10 }}>
          <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 10 }}>
            <div style={S.statBox}>
              <div style={S.statLabel}>COUNCIL MEMBER</div>
              <div style={{ ...S.statValue, color: profile.isCouncil ? '#A855F7' : 'var(--dag-text-muted)' }}>
                {profile.isCouncil ? 'Yes' : 'No'}
              </div>
            </div>
            <div style={S.statBox}>
              <div style={S.statLabel}>PROPOSALS VOTED</div>
              <div style={S.statValue}>{profile.votedOnProposals}</div>
            </div>
          </div>
          {profile.votedOnProposals === 0 && (
            <div style={{ textAlign: 'center', padding: '16px 0', color: 'var(--dag-text-faint)', fontSize: 12 }}>
              No governance activity yet
            </div>
          )}
        </div>
      )}

      {/* Streams */}
      {tab === 'streams' && (
        <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 10 }}>
          <div style={S.statBox}>
            <div style={S.statLabel}>STREAMS SENT</div>
            <div style={S.statValue}>{profile.streamsSentCount}</div>
          </div>
          <div style={S.statBox}>
            <div style={S.statLabel}>STREAMS RECEIVED</div>
            <div style={S.statValue}>{profile.streamsReceivedCount}</div>
          </div>
          {profile.streamsSentCount + profile.streamsReceivedCount === 0 && (
            <div style={{ gridColumn: '1 / -1', textAlign: 'center', padding: '16px 0', color: 'var(--dag-text-faint)', fontSize: 12 }}>
              No stream activity yet
            </div>
          )}
        </div>
      )}
    </div>
  );
}
