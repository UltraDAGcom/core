import { NavLink } from 'react-router-dom';
import {
  LayoutDashboard,
  Wallet,
  Send as SendIcon,
  Coins,
  Vote,
  Users,
  Search,
  Activity,
  ArrowRightLeft,
  X,
} from 'lucide-react';

interface NavItem {
  to: string;
  icon: React.ComponentType<{ className?: string }>;
  label: string;
}

function SessionIndicator({ secondsLeft, totalSeconds }: { secondsLeft: number; totalSeconds: number }) {
  const fraction = Math.max(0, Math.min(1, secondsLeft / totalSeconds));
  const mins = Math.floor(secondsLeft / 60);
  const secs = secondsLeft % 60;
  const timeStr = `${mins}:${secs.toString().padStart(2, '0')}`;

  const urgent = secondsLeft <= 120;
  const critical = secondsLeft <= 30;

  const trackColor = 'bg-slate-800';
  const barColor = critical
    ? 'bg-gradient-to-r from-red-500 to-rose-400'
    : urgent
      ? 'bg-gradient-to-r from-amber-500 to-yellow-400'
      : 'bg-gradient-to-r from-dag-accent to-indigo-400';

  const textColor = critical
    ? 'text-red-400'
    : urgent
      ? 'text-amber-400'
      : 'text-slate-500';

  const dotColor = critical
    ? 'bg-red-400'
    : urgent
      ? 'bg-amber-400'
      : 'bg-dag-accent/60';

  return (
    <div className="space-y-1.5">
      <div className="flex items-center justify-between px-1">
        <div className="flex items-center gap-1.5">
          <span className={`w-1.5 h-1.5 rounded-full ${dotColor} ${critical ? 'animate-pulse' : ''}`} />
          <span className={`text-[10px] font-medium ${textColor}`}>Session</span>
        </div>
        <span className={`text-[10px] font-mono tabular-nums ${textColor}`}>{timeStr}</span>
      </div>
      <div className={`h-1 w-full ${trackColor} rounded-full overflow-hidden`}>
        <div
          className={`h-full ${barColor} rounded-full transition-all duration-1000 ease-linear ${critical ? 'animate-pulse' : ''}`}
          style={{ width: `${fraction * 100}%` }}
        />
      </div>
    </div>
  );
}

// Flat list, grouped by section dividers
const sections: { label?: string; items: NavItem[] }[] = [
  {
    items: [
      { to: '/', icon: LayoutDashboard, label: 'Dashboard' },
    ],
  },
  {
    label: 'Wallet',
    items: [
      { to: '/wallet', icon: Wallet, label: 'Wallets' },
      { to: '/wallet/send', icon: SendIcon, label: 'Send & Receive' },
    ],
  },
  {
    label: 'Network',
    items: [
      { to: '/staking', icon: Coins, label: 'Staking' },
      { to: '/governance', icon: Vote, label: 'Governance' },
      { to: '/council', icon: Users, label: 'Council' },
      { to: '/explorer', icon: Search, label: 'Explorer' },
    ],
  },
  {
    label: 'Advanced',
    items: [
      { to: '/bridge', icon: ArrowRightLeft, label: 'Bridge' },
      { to: '/network', icon: Activity, label: 'Node Status' },
    ],
  },
];

interface SidebarProps {
  open: boolean;
  onClose: () => void;
  network?: 'mainnet' | 'testnet';
  sessionSecondsLeft?: number;
  sessionTotalSeconds?: number;
}

export function Sidebar({ open, onClose, network = 'testnet', sessionSecondsLeft, sessionTotalSeconds }: SidebarProps) {
  return (
    <>
      {/* Mobile overlay */}
      {open && (
        <div
          className="fixed inset-0 bg-black/60 z-40 lg:hidden"
          onClick={onClose}
        />
      )}

      <aside
        className={`
          fixed top-0 left-0 z-50 h-full w-60 bg-dag-sidebar border-r border-dag-border
          flex flex-col transition-transform duration-200
          lg:translate-x-0 lg:static lg:z-auto
          ${open ? 'translate-x-0' : '-translate-x-full'}
        `}
      >
        {/* Logo */}
        <div className="flex items-center justify-between h-14 px-4 border-b border-dag-border">
          <div className="flex items-center gap-2.5">
            <div className="w-7 h-7 rounded-lg bg-gradient-to-br from-dag-accent to-purple-500 flex items-center justify-center">
              <span className="font-bold text-xs text-white">U</span>
            </div>
            <span className="text-white font-semibold text-sm">UltraDAG</span>
          </div>
          <button onClick={onClose} className="lg:hidden p-1 text-slate-400 hover:text-white">
            <X className="w-5 h-5" />
          </button>
        </div>

        {/* Nav */}
        <nav className="flex-1 py-3 px-3 overflow-y-auto">
          {sections.map((section, si) => (
            <div key={si} className={si > 0 ? 'mt-4' : ''}>
              {section.label && (
                <p className="px-3 mb-1.5 text-[10px] font-semibold text-slate-500 uppercase tracking-wider">
                  {section.label}
                </p>
              )}
              <div className="space-y-0.5">
                {section.items.map(({ to, icon: Icon, label }) => (
                  <NavLink
                    key={to}
                    to={to}
                    end={to === '/' || to === '/wallet'}
                    onClick={onClose}
                    className={({ isActive }) =>
                      `flex items-center gap-3 px-3 py-2 rounded-lg text-[13px] font-medium transition-all ${
                        isActive
                          ? 'bg-dag-accent/15 text-white'
                          : 'text-slate-400 hover:text-white hover:bg-white/5'
                      }`
                    }
                  >
                    {({ isActive }) => (
                      <>
                        <Icon className={`w-4 h-4 ${isActive ? 'text-dag-accent' : ''}`} />
                        {label}
                      </>
                    )}
                  </NavLink>
                ))}
              </div>
            </div>
          ))}
        </nav>

        {/* Footer: session timer + version */}
        <div className="px-3 py-3 border-t border-dag-border space-y-2.5">
          {sessionSecondsLeft !== undefined && sessionTotalSeconds !== undefined && sessionTotalSeconds > 0 && (
            <SessionIndicator secondsLeft={sessionSecondsLeft} totalSeconds={sessionTotalSeconds} />
          )}
          <p className={`text-[10px] px-1 ${network === 'mainnet' ? 'text-dag-green' : 'text-slate-500'}`}>
            {network === 'mainnet' ? 'Mainnet' : 'Testnet'} v0.1
          </p>
        </div>
      </aside>
    </>
  );
}
