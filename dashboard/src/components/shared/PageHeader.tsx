import { useIsMobile } from '../../hooks/useIsMobile';
import { useAppStatus } from '../../contexts/AppStatusContext';

interface PageHeaderProps {
  title: string;
  subtitle?: string;
  /** Additional right-side content (buttons, etc.) — shown BEFORE the global status */
  right?: React.ReactNode;
  /** When provided, renders a refresh button next to the title */
  onRefresh?: () => void;
}

const SATS = 100_000_000;

export function PageHeader({ title, subtitle, right, onRefresh }: PageHeaderProps) {
  const m = useIsMobile();
  const status = useAppStatus();

  return (
    <div style={{
      display: 'flex', justifyContent: 'space-between',
      alignItems: m ? 'flex-start' : 'center',
      marginBottom: m ? 16 : 22,
      flexDirection: m ? 'column' : 'row',
      gap: m ? 10 : 0,
    }}>
      <div>
        <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
          <h1 style={{ fontSize: m ? 18 : 21, fontWeight: 700, letterSpacing: -0.3, color: 'var(--dag-text)' }}>
            {title}
          </h1>
          {onRefresh && (
            <button
              onClick={onRefresh}
              title="Refresh"
              style={{
                background: 'none', border: 'none', cursor: 'pointer', padding: 4,
                color: 'var(--dag-text-faint)', fontSize: 15, lineHeight: 1,
                borderRadius: 6, transition: 'color 0.2s',
              }}
              onMouseEnter={e => (e.currentTarget.style.color = '#00E0C4')}
              onMouseLeave={e => (e.currentTarget.style.color = 'var(--dag-text-faint)')}
            >
              ↻
            </button>
          )}
        </div>
        {subtitle && (
          <p style={{ fontSize: 11.5, color: 'var(--dag-subheading)', marginTop: 2 }}>{subtitle}</p>
        )}
      </div>
      <div style={{ display: 'flex', alignItems: 'center', gap: m ? 8 : 10, flexWrap: 'wrap' }}>
        {/* Page-specific actions */}
        {right}

        {/* Global status bar — always visible */}
        {status && !m && (
          <>
            {/* Health indicator */}
            <div style={{ display: 'flex', alignItems: 'center', gap: 5 }}>
              <div style={{
                width: 7, height: 7, borderRadius: '50%',
                background: status.connected ? '#00E0C4' : '#EF4444',
                boxShadow: status.connected ? '0 0 6px #00E0C4' : '0 0 6px #EF4444',
              }} />
              <span style={{ fontSize: 11, fontWeight: 600, color: status.connected ? '#00E0C4' : '#EF4444' }}>
                {status.connected ? 'HEALTHY' : 'OFFLINE'}
              </span>
            </div>

            {/* Network badge */}
            <div style={{
              padding: '3px 10px', borderRadius: 12,
              background: status.network === 'mainnet' ? 'rgba(0,224,196,0.06)' : 'rgba(255,184,0,0.06)',
              border: `1px solid ${status.network === 'mainnet' ? 'rgba(0,224,196,0.12)' : 'rgba(255,184,0,0.12)'}`,
              fontSize: 10, fontWeight: 600, letterSpacing: 0.8, textTransform: 'uppercase' as const,
              color: status.network === 'mainnet' ? '#00E0C4' : '#FFB800',
            }}>
              {status.network}
            </div>

            {/* User + Balance pill */}
            <div style={{
              display: 'flex', alignItems: 'center', gap: 6,
              padding: '3px 12px', borderRadius: 12,
              background: 'var(--dag-card)', border: '1px solid var(--dag-border)',
            }}>
              <div style={{
                width: 16, height: 16, borderRadius: 4,
                background: '#00E0C4',
                display: 'flex', alignItems: 'center', justifyContent: 'center',
                fontSize: 7, fontWeight: 800, color: '#fff',
              }}>
                {status.userName[0]?.toUpperCase()}
              </div>
              <span style={{ fontSize: 11, fontWeight: 600, color: 'var(--dag-text)' }}>{status.userName}</span>
              <span style={{ color: 'var(--dag-text-faint)', fontSize: 10 }}>|</span>
              <span style={{ fontSize: 11, color: '#00E0C4', fontWeight: 600, fontFamily: "'DM Mono',monospace" }}>
                {(status.totalBalance / SATS).toFixed(2)} UDAG
              </span>
            </div>
          </>
        )}
      </div>
    </div>
  );
}
