import { Menu, Wifi, WifiOff, LogOut, Wallet } from 'lucide-react';
import type { NetworkType } from '../../lib/api';

interface TopBarProps {
  connected: boolean;
  nodeUrl: string;
  keystoreUnlocked: boolean;
  network: NetworkType;
  walletAddress?: string;
  walletBalance?: number;
  onToggleSidebar: () => void;
  onToggleLock: () => void;
  onSwitchNetwork: (network: NetworkType) => void;
}

export function TopBar({
  connected,
  nodeUrl,
  keystoreUnlocked,
  network,
  walletAddress,
  walletBalance,
  onToggleSidebar,
  onToggleLock,
  onSwitchNetwork,
}: TopBarProps) {
  const isMainnet = network === 'mainnet';
  const shortAddr = walletAddress
    ? walletAddress.slice(0, 6) + '...' + walletAddress.slice(-4)
    : '';

  const formatBalance = (sats: number) => {
    const udag = sats / 100_000_000;
    return udag < 0.01 && udag > 0 ? '<0.01' : udag.toLocaleString(undefined, { maximumFractionDigits: 2 });
  };

  return (
    <header className="h-14 bg-dag-sidebar/80 backdrop-blur border-b border-dag-border flex items-center justify-between px-4 lg:px-6 sticky top-0 z-30">
      {/* Left: hamburger + connection */}
      <div className="flex items-center gap-3">
        <button
          onClick={onToggleSidebar}
          className="lg:hidden p-1.5 rounded-lg text-slate-400 hover:text-white hover:bg-slate-800 transition-colors"
        >
          <Menu className="w-5 h-5" />
        </button>

        <div className="flex items-center gap-2">
          {connected ? (
            <Wifi className="w-3.5 h-3.5 text-dag-green" />
          ) : (
            <WifiOff className="w-3.5 h-3.5 text-dag-red" />
          )}
          <span className="text-[11px] text-dag-muted font-mono hidden sm:inline">
            {connected ? nodeUrl.replace('https://', '').replace('.fly.dev', '') : 'Disconnected'}
          </span>
        </div>
      </div>

      {/* Center: network switcher */}
      <div className="flex items-center bg-dag-surface border border-dag-border rounded-lg p-0.5">
        <button
          onClick={() => onSwitchNetwork('mainnet')}
          className={`px-3 py-1 rounded-md text-xs font-medium transition-all ${
            isMainnet
              ? 'bg-dag-green/20 text-dag-green border border-dag-green/30'
              : 'text-dag-muted hover:text-white'
          }`}
        >
          Mainnet
        </button>
        <button
          onClick={() => onSwitchNetwork('testnet')}
          className={`px-3 py-1 rounded-md text-xs font-medium transition-all ${
            !isMainnet
              ? 'bg-dag-yellow/20 text-dag-yellow border border-dag-yellow/30'
              : 'text-dag-muted hover:text-white'
          }`}
        >
          Testnet
        </button>
      </div>

      {/* Right: wallet info + sign out */}
      {keystoreUnlocked ? (
        <div className="flex items-center gap-2">
          {walletAddress && (
            <div className="hidden sm:flex items-center gap-2 px-3 py-1.5 rounded-lg bg-dag-surface border border-dag-border">
              <Wallet className="w-3.5 h-3.5 text-dag-accent" />
              <span className="text-xs font-mono text-slate-300">{shortAddr}</span>
              {walletBalance !== undefined && (
                <>
                  <span className="text-slate-600">|</span>
                  <span className="text-xs font-semibold text-dag-green">{formatBalance(walletBalance)} UDAG</span>
                </>
              )}
            </div>
          )}
          <button
            onClick={onToggleLock}
            className="flex items-center gap-1.5 px-2.5 py-1.5 rounded-lg text-xs text-slate-400 hover:text-white hover:bg-slate-800 transition-colors"
            title="Sign out"
          >
            <LogOut className="w-3.5 h-3.5" />
            <span className="hidden sm:inline">Sign Out</span>
          </button>
        </div>
      ) : (
        <div className="w-20" /> // Spacer when not logged in
      )}
    </header>
  );
}
