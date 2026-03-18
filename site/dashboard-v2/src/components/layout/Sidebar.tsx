import { NavLink } from 'react-router-dom';
import {
  LayoutDashboard,
  Wallet,
  Coins,
  Vote,
  Users,
  Search,
  Network,
  X,
} from 'lucide-react';

const navItems = [
  { to: '/', icon: LayoutDashboard, label: 'Overview' },
  { to: '/wallet', icon: Wallet, label: 'Wallet' },
  { to: '/staking', icon: Coins, label: 'Staking' },
  { to: '/governance', icon: Vote, label: 'Governance' },
  { to: '/council', icon: Users, label: 'Council' },
  { to: '/explorer', icon: Search, label: 'Explorer' },
  { to: '/network', icon: Network, label: 'Network' },
];

interface SidebarProps {
  open: boolean;
  onClose: () => void;
}

export function Sidebar({ open, onClose }: SidebarProps) {
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
          fixed top-0 left-0 z-50 h-full w-64 bg-dag-sidebar border-r border-dag-border
          flex flex-col transition-transform duration-200
          lg:translate-x-0 lg:static lg:z-auto
          ${open ? 'translate-x-0' : '-translate-x-full'}
        `}
      >
        {/* Logo */}
        <div className="flex items-center justify-between h-16 px-5 border-b border-dag-border">
          <div className="flex items-center gap-2.5">
            <div className="w-8 h-8 rounded-lg bg-dag-accent/20 flex items-center justify-center">
              <span className="text-dag-accent font-bold text-sm">U</span>
            </div>
            <div>
              <span className="text-white font-semibold text-sm tracking-wide">UltraDAG</span>
              <span className="block text-[10px] text-dag-muted leading-none">Dashboard</span>
            </div>
          </div>
          <button onClick={onClose} className="lg:hidden p-1 text-slate-400 hover:text-white">
            <X className="w-5 h-5" />
          </button>
        </div>

        {/* Nav */}
        <nav className="flex-1 py-4 px-3 space-y-1 overflow-y-auto">
          {navItems.map(({ to, icon: Icon, label }) => (
            <NavLink
              key={to}
              to={to}
              end={to === '/'}
              onClick={onClose}
              className={({ isActive }) =>
                `flex items-center gap-3 px-3 py-2.5 rounded-lg text-sm font-medium transition-colors ${
                  isActive
                    ? 'bg-dag-accent/15 text-dag-accent'
                    : 'text-slate-400 hover:text-slate-200 hover:bg-slate-800'
                }`
              }
            >
              <Icon className="w-[18px] h-[18px]" />
              {label}
            </NavLink>
          ))}
        </nav>

        {/* Footer */}
        <div className="px-4 py-3 border-t border-dag-border">
          <p className="text-[11px] text-slate-500">Testnet v0.1</p>
        </div>
      </aside>
    </>
  );
}
