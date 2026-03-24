import { NavLink, useLocation } from 'react-router-dom';
import {
  LayoutDashboard,
  Wallet,
  Coins,
  Vote,
  Users,
  Search,
  Activity,
  X,
  Send as SendIcon,
  ArrowRightLeft,
} from 'lucide-react';

interface NavItem {
  to: string;
  icon: React.ComponentType<{ className?: string }>;
  label: string;
  children?: { to: string; icon: React.ComponentType<{ className?: string }>; label: string }[];
}

const navItems: NavItem[] = [
  { to: '/', icon: LayoutDashboard, label: 'Dashboard' },
  {
    to: '/wallet',
    icon: Wallet,
    label: 'Wallet',
    children: [
      { to: '/wallet/send', icon: SendIcon, label: 'Send & Receive' },
    ],
  },
  { to: '/staking', icon: Coins, label: 'Staking' },
  {
    to: '/governance',
    icon: Vote,
    label: 'Governance',
    children: [
      { to: '/council', icon: Users, label: 'Council' },
    ],
  },
  { to: '/explorer', icon: Search, label: 'Explorer' },
  { to: '/bridge', icon: ArrowRightLeft, label: 'Bridge' },
  { to: '/network', icon: Activity, label: 'Network' },
];

interface SidebarProps {
  open: boolean;
  onClose: () => void;
  network?: 'mainnet' | 'testnet';
}

export function Sidebar({ open, onClose, network = 'testnet' }: SidebarProps) {
  const location = useLocation();

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
            <div className="w-8 h-8 rounded-lg bg-gradient-to-br from-dag-accent to-purple-500 flex items-center justify-center shadow-lg shadow-dag-accent/10">
              <span className="font-bold text-sm text-white">U</span>
            </div>
            <div>
              <span className="text-white font-semibold text-sm tracking-wide">UltraDAG</span>
            </div>
          </div>
          <button onClick={onClose} className="lg:hidden p-1 text-slate-400 hover:text-white">
            <X className="w-5 h-5" />
          </button>
        </div>

        {/* Nav */}
        <nav className="flex-1 py-4 px-3 space-y-0.5 overflow-y-auto">
          {navItems.map(({ to, icon: Icon, label, children }) => {
            const isParentActive = location.pathname === to || (children && location.pathname.startsWith(to + '/'));
            return (
              <div key={to}>
                <NavLink
                  to={to}
                  end={to === '/' || !!children}
                  onClick={onClose}
                  className={({ isActive }) =>
                    `flex items-center gap-3 px-3 py-2 rounded-lg text-sm font-medium transition-all ${
                      isActive || isParentActive
                        ? 'bg-dag-accent/15 text-white'
                        : 'text-slate-400 hover:text-white hover:bg-white/5'
                    }`
                  }
                >
                  <Icon className={`w-[18px] h-[18px] ${isParentActive ? 'text-dag-accent' : ''}`} />
                  {label}
                </NavLink>
                {children && isParentActive && (
                  <div className="ml-7 mt-0.5 space-y-0.5 border-l border-dag-border/50 pl-3">
                    {children.map(({ to: childTo, icon: ChildIcon, label: childLabel }) => (
                      <NavLink
                        key={childTo}
                        to={childTo}
                        onClick={onClose}
                        className={({ isActive }) =>
                          `flex items-center gap-2 px-2 py-1.5 rounded-md text-xs font-medium transition-colors ${
                            isActive
                              ? 'text-dag-accent'
                              : 'text-slate-500 hover:text-slate-300'
                          }`
                        }
                      >
                        <ChildIcon className="w-3.5 h-3.5" />
                        {childLabel}
                      </NavLink>
                    ))}
                  </div>
                )}
              </div>
            );
          })}
        </nav>

        {/* Footer */}
        <div className="px-4 py-3 border-t border-dag-border">
          <p className={`text-[11px] ${network === 'mainnet' ? 'text-dag-green' : 'text-slate-500'}`}>
            {network === 'mainnet' ? 'Mainnet' : 'Testnet'} v0.1
          </p>
        </div>
      </aside>
    </>
  );
}
