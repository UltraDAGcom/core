import { shortAddr, fullAddr, formatUdag } from '../../lib/api';
import { avatarGradient, roundToDate, type Badge } from '../../lib/badges';
import { CopyButton } from '../shared/CopyButton';

interface UltraIdCardProps {
  address: string;
  name: string | null;
  badges: Badge[];
  balance: number;
  staked: number;
  delegatorCount?: number;
  createdAtRound?: number | null;
  currentRound?: number | null;
  bio?: string | null;
  size?: 'sm' | 'lg';
}

export function UltraIdCard({
  address, name, badges, balance, staked, delegatorCount = 0,
  createdAtRound, currentRound, bio, size = 'lg',
}: UltraIdCardProps) {
  const earnedBadges = badges.filter(b => b.earned);
  const isLg = size === 'lg';
  const avatarSize = isLg ? 64 : 40;
  const bech = fullAddr(address);

  return (
    <div style={{
      background: 'linear-gradient(135deg, rgba(0,224,196,0.06) 0%, rgba(0,102,255,0.04) 50%, rgba(168,85,247,0.04) 100%)',
      border: '1px solid var(--dag-border)',
      borderRadius: isLg ? 16 : 12,
      padding: isLg ? '24px 28px' : '14px 16px',
      position: 'relative',
      overflow: 'hidden',
    }}>
      {/* Holographic accent line */}
      <div style={{
        position: 'absolute', top: 0, left: 0, right: 0, height: 2,
        background: 'linear-gradient(90deg, #00E0C4, #0066FF, #A855F7, #FFB800, #00E0C4)',
        backgroundSize: '200% 100%',
        animation: 'gradientShift 6s linear infinite',
      }} />

      {/* Header: ULTRA ID label */}
      <div style={{
        display: 'flex', justifyContent: 'space-between', alignItems: 'center',
        marginBottom: isLg ? 16 : 10,
      }}>
        <span style={{ fontSize: isLg ? 10 : 8, fontWeight: 700, letterSpacing: 2.5, color: 'var(--dag-text-faint)', textTransform: 'uppercase' }}>
          ULTRA ID
        </span>
        <div style={{ display: 'flex', gap: 3 }}>
          {[0, 1, 2].map(i => (
            <div key={i} style={{ width: isLg ? 6 : 4, height: isLg ? 6 : 4, borderRadius: '50%', background: 'var(--dag-text-faint)', opacity: 0.4 }} />
          ))}
        </div>
      </div>

      {/* Avatar + Name + Address */}
      <div style={{ display: 'flex', gap: isLg ? 16 : 10, alignItems: 'center', marginBottom: isLg ? 16 : 10 }}>
        {/* Generated avatar */}
        <div style={{
          width: avatarSize, height: avatarSize, borderRadius: isLg ? 14 : 10, flexShrink: 0,
          background: avatarGradient(address),
          border: '2px solid rgba(255,255,255,0.1)',
          display: 'flex', alignItems: 'center', justifyContent: 'center',
          fontSize: isLg ? 24 : 16, fontWeight: 800, color: 'rgba(255,255,255,0.7)',
        }}>
          {(name || address)[0]?.toUpperCase()}
        </div>

        <div style={{ minWidth: 0 }}>
          {name ? (
            <div style={{ fontSize: isLg ? 22 : 14, fontWeight: 700, color: '#00E0C4' }}>
              @{name}
            </div>
          ) : (
            <div style={{ fontSize: isLg ? 16 : 12, fontWeight: 600, color: 'var(--dag-text)' }}>
              {shortAddr(address)}
            </div>
          )}
          <div style={{ display: 'flex', alignItems: 'center', gap: 4, marginTop: 2 }}>
            <span style={{ fontSize: isLg ? 11 : 9, fontFamily: "'DM Mono',monospace", color: 'var(--dag-text-muted)', overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>
              {isLg ? bech : shortAddr(address)}
            </span>
            <CopyButton text={bech} />
          </div>
          {bio && isLg && (
            <p style={{ fontSize: 11.5, color: 'var(--dag-text-secondary)', marginTop: 6, lineHeight: 1.5 }}>{bio}</p>
          )}
        </div>
      </div>

      {/* Badges */}
      {earnedBadges.length > 0 && (
        <div style={{ display: 'flex', flexWrap: 'wrap', gap: isLg ? 6 : 4, marginBottom: isLg ? 16 : 10 }}>
          {earnedBadges.map(b => (
            <span key={b.id} title={b.description} style={{
              display: 'inline-flex', alignItems: 'center', gap: 4,
              padding: isLg ? '3px 10px' : '2px 7px', borderRadius: 6,
              background: `${b.color}12`, border: `1px solid ${b.color}25`,
              fontSize: isLg ? 10.5 : 8.5, fontWeight: 600, color: b.color,
              cursor: 'default',
            }}>
              <span>{b.icon}</span> {b.label}
            </span>
          ))}
        </div>
      )}

      {/* Stats row */}
      <div style={{
        display: 'grid', gridTemplateColumns: isLg ? 'repeat(3, 1fr)' : 'repeat(2, 1fr)',
        gap: isLg ? 10 : 6,
        paddingTop: isLg ? 14 : 8,
        borderTop: '1px solid var(--dag-border)',
      }}>
        <div>
          <div style={{ fontSize: isLg ? 9 : 7.5, color: 'var(--dag-text-faint)', letterSpacing: 1, marginBottom: 2 }}>BALANCE</div>
          <div style={{ fontSize: isLg ? 14 : 11, fontWeight: 700, color: 'var(--dag-text)', fontFamily: "'DM Mono',monospace" }}>
            {formatUdag(balance)} <span style={{ fontSize: isLg ? 10 : 8, color: 'var(--dag-text-muted)', fontWeight: 400 }}>UDAG</span>
          </div>
        </div>
        <div>
          <div style={{ fontSize: isLg ? 9 : 7.5, color: 'var(--dag-text-faint)', letterSpacing: 1, marginBottom: 2 }}>STAKED</div>
          <div style={{ fontSize: isLg ? 14 : 11, fontWeight: 700, color: staked > 0 ? '#00E0C4' : 'var(--dag-text-muted)', fontFamily: "'DM Mono',monospace" }}>
            {staked > 0 ? formatUdag(staked) : '—'}
          </div>
        </div>
        {isLg && (
          <div>
            <div style={{ fontSize: 9, color: 'var(--dag-text-faint)', letterSpacing: 1, marginBottom: 2 }}>
              {delegatorCount > 0 ? 'DELEGATORS' : 'MEMBER SINCE'}
            </div>
            <div style={{ fontSize: 14, fontWeight: 700, color: 'var(--dag-text)', fontFamily: "'DM Mono',monospace" }}>
              {delegatorCount > 0
                ? delegatorCount
                : createdAtRound != null && currentRound != null
                  ? roundToDate(createdAtRound, currentRound)
                  : '—'
              }
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
