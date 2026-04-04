import { Menu, WifiOff, Lock, Wallet } from 'lucide-react';
import { SessionBadge } from './SessionTimer';
import { useName } from '../../contexts/NameCacheContext';
import { shortAddr } from '../../lib/api';
import type { NetworkType } from '../../lib/api';

interface TopBarProps {
  connected: boolean;
  nodeUrl?: string;
  keystoreUnlocked: boolean;
  network: NetworkType;
  walletAddress?: string;
  walletName?: string;
  walletBalance?: number;
  sessionSecondsLeft?: number;
  onToggleSidebar: () => void;
  onToggleLock: () => void;
  onSwitchNetwork: (network: NetworkType) => void;
}

const headerStyle: React.CSSProperties = {
  height: 56,
  background: 'var(--dag-sidebar)',
  backdropFilter: 'blur(12px)',
  borderBottom: '1px solid var(--dag-border)',
  display: 'flex',
  alignItems: 'center',
  justifyContent: 'space-between',
  padding: '0 24px',
  position: 'sticky',
  top: 0,
  zIndex: 30,
};

const hamburgerStyle: React.CSSProperties = {
  padding: 6,
  borderRadius: 8,
  color: 'var(--dag-text-muted)',
  background: 'none',
  border: 'none',
  cursor: 'pointer',
  transition: 'all 0.15s',
  display: 'none', // hidden on desktop; shown via media query or JS
};

const switcherWrapStyle: React.CSSProperties = {
  display: 'flex',
  alignItems: 'center',
  background: 'var(--dag-input-bg)',
  border: '1px solid var(--dag-border)',
  borderRadius: 8,
  padding: 2,
};

const switchBtnBase: React.CSSProperties = {
  padding: '4px 12px',
  borderRadius: 6,
  fontSize: 10,
  fontWeight: 500,
  transition: 'all 0.15s',
  border: 'none',
  cursor: 'pointer',
  background: 'none',
};

const walletPillStyle: React.CSSProperties = {
  display: 'flex',
  alignItems: 'center',
  gap: 8,
  padding: '6px 12px',
  borderRadius: 8,
  background: 'var(--dag-input-bg)',
  border: '1px solid var(--dag-border)',
};

const lockBtnStyle: React.CSSProperties = {
  display: 'flex',
  alignItems: 'center',
  gap: 6,
  padding: '6px 10px',
  borderRadius: 8,
  fontSize: 10,
  color: 'var(--dag-text-muted)',
  background: 'none',
  border: 'none',
  cursor: 'pointer',
  transition: 'all 0.15s',
};

export function TopBar({
  connected,
  keystoreUnlocked,
  network,
  walletAddress,
  walletName,
  walletBalance,
  sessionSecondsLeft,
  onToggleSidebar,
  onToggleLock,
  onSwitchNetwork,
}: TopBarProps) {
  const isMainnet = network === 'mainnet';
  const { name: ultraId } = useName(walletAddress);
  const displayName = ultraId ? `@${ultraId}` : walletName || (walletAddress ? shortAddr(walletAddress) : '');

  const formatBalance = (sats: number) => {
    const udag = sats / 100_000_000;
    return udag < 0.01 && udag > 0 ? '<0.01' : udag.toLocaleString(undefined, { maximumFractionDigits: 2 });
  };

  const mainnetBtnStyle: React.CSSProperties = {
    ...switchBtnBase,
    ...(isMainnet
      ? { background: 'rgba(0,224,196,0.15)', color: '#00E0C4', border: '1px solid rgba(0,224,196,0.25)' }
      : { color: 'var(--dag-text-muted)' }),
  };

  const testnetBtnStyle: React.CSSProperties = {
    ...switchBtnBase,
    ...(!isMainnet
      ? { background: 'rgba(255,184,0,0.15)', color: '#FFB800', border: '1px solid rgba(255,184,0,0.25)' }
      : { color: 'var(--dag-text-muted)' }),
  };

  return (
    <header style={headerStyle}>
      {/* Left: hamburger + connection */}
      <div style={{ display: 'flex', alignItems: 'center', gap: 12 }}>
        <button onClick={onToggleSidebar} style={hamburgerStyle}>
          <Menu size={20} />
        </button>

        <div style={{ display: 'flex', alignItems: 'center', gap: 6 }}>
          {connected ? (
            <>
              <span style={{
                width: 8, height: 8, borderRadius: '50%', background: '#00E0C4',
                animation: 'pulse 2s ease-in-out infinite', display: 'inline-block',
              }} />
              <span style={{ fontSize: 11, color: 'var(--dag-text-muted)' }}>Connected</span>
            </>
          ) : (
            <>
              <WifiOff size={14} style={{ color: '#EF4444' }} />
              <span style={{ fontSize: 11, color: '#EF4444' }}>Disconnected</span>
            </>
          )}
        </div>
      </div>

      {/* Center: network switcher */}
      <div style={switcherWrapStyle}>
        <button onClick={() => onSwitchNetwork('mainnet')} style={mainnetBtnStyle}>
          Mainnet
        </button>
        <button onClick={() => onSwitchNetwork('testnet')} style={testnetBtnStyle}>
          Testnet
        </button>
      </div>

      {/* Right: wallet info + session timer + lock */}
      {keystoreUnlocked ? (
        <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
          {walletAddress && (
            <div style={walletPillStyle}>
              <Wallet size={14} style={{ color: 'var(--dag-subheading)' }} />
              <span style={{
                fontSize: 10,
                ...(ultraId
                  ? { fontWeight: 600, color: 'var(--dag-subheading)' }
                  : { fontFamily: 'monospace', color: 'var(--dag-text-secondary)' }),
              }}>
                {displayName}
              </span>
              {walletBalance !== undefined && (
                <>
                  <span style={{ color: 'var(--dag-text-faint)' }}>|</span>
                  <span style={{ fontSize: 10, fontWeight: 600, color: '#00E0C4' }}>{formatBalance(walletBalance)} UDAG</span>
                </>
              )}
            </div>
          )}
          {sessionSecondsLeft !== undefined && (
            <SessionBadge secondsLeft={sessionSecondsLeft} />
          )}
          <button onClick={onToggleLock} style={lockBtnStyle} title="Lock wallet">
            <Lock size={14} />
            <span>Lock</span>
          </button>
        </div>
      ) : (
        <div style={{ width: 80 }} />
      )}
    </header>
  );
}
