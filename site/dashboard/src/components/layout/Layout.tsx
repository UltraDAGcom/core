import { useState } from 'react';
import { Outlet } from 'react-router-dom';
import { Sidebar } from './Sidebar';
import { TopBar } from './TopBar';
import type { NetworkType } from '../../lib/api';

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
}

export function Layout({ connected, nodeUrl, keystoreUnlocked, network, walletAddress, walletBalance, sessionSecondsLeft, sessionTotalSeconds, onToggleLock, onSwitchNetwork }: LayoutProps) {
  const [sidebarOpen, setSidebarOpen] = useState(false);

  return (
    <div className="flex h-screen overflow-hidden">
      <Sidebar
        open={sidebarOpen}
        onClose={() => setSidebarOpen(false)}
        network={network}
        sessionSecondsLeft={keystoreUnlocked ? sessionSecondsLeft : undefined}
        sessionTotalSeconds={keystoreUnlocked ? sessionTotalSeconds : undefined}
      />
      <div className="flex-1 flex flex-col min-w-0 overflow-hidden">
        <TopBar
          connected={connected}
          nodeUrl={nodeUrl}
          keystoreUnlocked={keystoreUnlocked}
          network={network}
          walletAddress={walletAddress}
          walletBalance={walletBalance}
          sessionSecondsLeft={keystoreUnlocked ? sessionSecondsLeft : undefined}
          onToggleSidebar={() => setSidebarOpen((o) => !o)}
          onToggleLock={onToggleLock}
          onSwitchNetwork={onSwitchNetwork}
        />
        <main className="flex-1 overflow-y-auto p-4 lg:p-6">
          <div className="max-w-7xl mx-auto">
            <Outlet />
          </div>
        </main>
      </div>
    </div>
  );
}
