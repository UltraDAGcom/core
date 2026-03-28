import { useState } from 'react';
import { Outlet } from 'react-router-dom';
import { Sidebar } from './Sidebar';
import { colors, fonts, globalStyles } from '../../lib/theme';
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

export function Layout({ connected, nodeUrl, keystoreUnlocked, network, walletAddress, walletBalance, sessionSecondsLeft, sessionTotalSeconds, onToggleLock, onSwitchNetwork, theme, onToggleTheme }: LayoutProps) {
  const [sidebarOpen, setSidebarOpen] = useState(false);

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
        <main style={{ flex: 1, overflowY: 'auto', maxHeight: '100vh' }}>
          <Outlet />
        </main>
      </div>
    </div>
  );
}
