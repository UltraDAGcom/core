import { Menu, Lock, Unlock, Wifi, WifiOff } from 'lucide-react';

interface TopBarProps {
  connected: boolean;
  nodeUrl: string;
  keystoreUnlocked: boolean;
  onToggleSidebar: () => void;
  onToggleLock: () => void;
}

export function TopBar({
  connected,
  nodeUrl,
  keystoreUnlocked,
  onToggleSidebar,
  onToggleLock,
}: TopBarProps) {
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
          <span className="text-[10px] font-bold px-1.5 py-0.5 rounded bg-dag-yellow/20 text-dag-yellow border border-dag-yellow/40">TESTNET</span>
          <span
            className={`w-2 h-2 rounded-full ${
              connected ? 'bg-dag-green animate-pulse' : 'bg-dag-red'
            }`}
          />
        </div>
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
