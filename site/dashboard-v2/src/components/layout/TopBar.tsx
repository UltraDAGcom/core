import { Menu, Lock, Unlock, Wifi, WifiOff } from 'lucide-react';
import type { NetworkType } from '../../lib/api';

interface TopBarProps {
  connected: boolean;
  nodeUrl: string;
  keystoreUnlocked: boolean;
  network: NetworkType;
  onToggleSidebar: () => void;
  onToggleLock: () => void;
  onSwitchNetwork: (network: NetworkType) => void;
}

export function TopBar({
  connected,
  nodeUrl,
  keystoreUnlocked,
  network,
  onToggleSidebar,
  onToggleLock,
  onSwitchNetwork,
}: TopBarProps) {
  const isMainnet = network === 'mainnet';

  return (
    <header className="h-14 bg-dag-sidebar/80 backdrop-blur border-b border-dag-border flex items-center justify-between px-4 lg:px-6 sticky top-0 z-30">
      {/* Left: hamburger + node status */}
      <div className="flex items-center gap-3">
        <button
          onClick={onToggleSidebar}
          className="lg:hidden p-1.5 rounded-lg text-slate-400 hover:text-white hover:bg-slate-800 transition-colors"
        >
          <Menu className="w-5 h-5" />
        </button>

        <div className="flex items-center gap-2">
          {connected ? (
            <Wifi className="w-4 h-4 text-dag-green" />
          ) : (
            <WifiOff className="w-4 h-4 text-dag-red" />
          )}
          <span className="text-xs text-dag-muted font-mono hidden sm:inline">
            {connected ? nodeUrl.replace('https://', '') : 'Disconnected'}
          </span>
          <span
            className={`w-2 h-2 rounded-full ${
              connected ? 'bg-dag-green animate-pulse' : 'bg-dag-red'
            }`}
          />
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

      {/* Right: lock button */}
      <button
        onClick={onToggleLock}
        className={`flex items-center gap-2 px-3 py-1.5 rounded-lg text-xs font-medium transition-colors ${
          keystoreUnlocked
            ? 'bg-green-500/15 text-green-400 hover:bg-green-500/25'
            : 'bg-slate-700/50 text-slate-400 hover:bg-slate-700'
        }`}
        title={keystoreUnlocked ? 'Lock keystore' : 'Unlock keystore'}
      >
        {keystoreUnlocked ? (
          <>
            <Unlock className="w-3.5 h-3.5" />
            <span className="hidden sm:inline">Unlocked</span>
          </>
        ) : (
          <>
            <Lock className="w-3.5 h-3.5" />
            <span className="hidden sm:inline">Locked</span>
          </>
        )}
      </button>
    </header>
  );
}
