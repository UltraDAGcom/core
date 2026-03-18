import { useState } from 'react';
import { Outlet } from 'react-router-dom';
import { Sidebar } from './Sidebar';
import { TopBar } from './TopBar';

interface LayoutProps {
  connected: boolean;
  nodeUrl: string;
  keystoreUnlocked: boolean;
  onToggleLock: () => void;
}

export function Layout({ connected, nodeUrl, keystoreUnlocked, onToggleLock }: LayoutProps) {
  const [sidebarOpen, setSidebarOpen] = useState(false);

  return (
    <div className="flex h-screen overflow-hidden">
      <Sidebar open={sidebarOpen} onClose={() => setSidebarOpen(false)} />
      <div className="flex-1 flex flex-col min-w-0 overflow-hidden">
        <TopBar
          connected={connected}
          nodeUrl={nodeUrl}
          keystoreUnlocked={keystoreUnlocked}
          onToggleSidebar={() => setSidebarOpen((o) => !o)}
          onToggleLock={onToggleLock}
        />
        <main className="flex-1 overflow-y-auto p-4 lg:p-6">
          <Outlet />
        </main>
      </div>
    </div>
  );
}
