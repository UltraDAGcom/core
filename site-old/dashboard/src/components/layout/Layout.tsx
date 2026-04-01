import { useState } from 'react';
import { Outlet } from 'react-router-dom';
import { Sidebar } from './Sidebar';
import { colors, fonts, globalStyles } from '../../lib/theme';
import { useIsMobile } from '../../hooks/useIsMobile';
import type { NetworkType } from '../../lib/api';
import type { Theme } from '../../hooks/useTheme';

interface LayoutProps {
  connected: boolean;
  nodeUrl: string;
  keystoreUnlocked: boolean;
  network: NetworkType;
  walletAddress?: string;
  walletBalance?: number;
  sessionSecondsLeft?: number;
  sessionTotalSeconds?: number;
  onToggleLock: () => void;
  onSwitchNetwork: (network: NetworkType) => void;
  theme?: Theme;
  onToggleTheme?: () => void;
}

export function Layout({ connected: _connected, nodeUrl: _nodeUrl, keystoreUnlocked, network, walletAddress: _walletAddress, walletBalance: _walletBalance, sessionSecondsLeft, sessionTotalSeconds, onToggleLock, onSwitchNetwork, theme, onToggleTheme }: LayoutProps) {
  const [sidebarOpen, setSidebarOpen] = useState(false);
  const m = useIsMobile();

  return (
    <div style={{ display: 'flex', minHeight: '100vh', background: colors.bg, fontFamily: fonts.sans, color: colors.textPrimary }}>
      <style>{globalStyles}</style>
      <Sidebar
        open={sidebarOpen}
        onClose={() => setSidebarOpen(false)}
        network={network}
        onSwitchNetwork={onSwitchNetwork}
        onToggleLock={onToggleLock}
        sessionSecondsLeft={keystoreUnlocked ? sessionSecondsLeft : undefined}
        sessionTotalSeconds={keystoreUnlocked ? sessionTotalSeconds : undefined}
        theme={theme}
        onToggleTheme={onToggleTheme}
      />
      <div style={{ flex: 1, display: 'flex', flexDirection: 'column', minWidth: 0, overflow: 'hidden' }}>
        {m && (
          <div style={{
            display: 'flex', alignItems: 'center', justifyContent: 'space-between',
            padding: '10px 14px', background: 'var(--dag-sidebar-bg)',
            borderBottom: '1px solid var(--dag-sidebar-border)',
            position: 'sticky', top: 0, zIndex: 30,
          }}>
            <button onClick={() => setSidebarOpen(true)} style={{
              background: 'none', border: 'none', color: 'var(--dag-text)', fontSize: 22,
              cursor: 'pointer', padding: '2px 6px', lineHeight: 1,
            }}>&#9776;</button>
            <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
              <div style={{
                width: 24, height: 24, borderRadius: 6, display: 'flex', alignItems: 'center', justifyContent: 'center',
                background: 'linear-gradient(135deg,#00E0C4,#0066FF)', fontSize: 10, fontWeight: 800, color: '#fff',
              }}>U</div>
              <span style={{ fontSize: 12, fontWeight: 700, letterSpacing: 1, color: 'var(--dag-text)' }}>ULTRADAG</span>
            </div>
            <div style={{
              padding: '3px 10px', borderRadius: 14, fontSize: 9, fontWeight: 600, letterSpacing: 0.8,
              textTransform: 'uppercase',
              background: network === 'mainnet' ? 'rgba(0,224,196,0.08)' : 'rgba(255,184,0,0.08)',
              color: network === 'mainnet' ? '#00E0C4' : '#FFB800',
            }}>{network}</div>
          </div>
        )}
        <main style={{ flex: 1, overflowY: 'auto', maxHeight: m ? undefined : '100vh' }}>
          <Outlet />
        </main>
      </div>
    </div>
  );
}
