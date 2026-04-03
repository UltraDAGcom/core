import { NavLink } from 'react-router-dom';
import { fonts } from '../../lib/theme';
import { useIsMobile } from '../../hooks/useIsMobile';
import type { NetworkType } from '../../lib/api';
import type { Theme } from '../../hooks/useTheme';

interface NavItem { to: string; icon: string; label: string }

const sections: { label?: string; items: NavItem[] }[] = [
  { items: [{ to: '/', icon: '\u25C8', label: 'Dashboard' }] },
  { label: 'WALLET', items: [
    { to: '/wallet', icon: '\u25C7', label: 'Wallets' },
    { to: '/wallet/send', icon: '\u21C4', label: 'Send & Receive' },
    { to: '/streams', icon: '\u224B', label: 'Streams' },
    { to: '/smart-account', icon: '\u25CE', label: 'SmartAccount' },
  ]},
  { label: 'NETWORK', items: [
    { to: '/staking', icon: '\u2B21', label: 'Staking' },
    { to: '/governance', icon: '\u2699', label: 'Governance' },
    { to: '/bounties', icon: '\u26A1', label: 'Bounties' },
    { to: '/council', icon: '\u265B', label: 'Council' },
    { to: '/explorer', icon: '\u25C9', label: 'Explorer' },
  ]},
  { label: 'ADVANCED', items: [
    { to: '/bridge', icon: '\u27F7', label: 'Bridge' },
    { to: '/network', icon: '\u26A1', label: 'Node Status' },
  ]},
];

interface SidebarProps {
  open: boolean;
  onClose: () => void;
  network?: NetworkType;
  onSwitchNetwork?: (net: NetworkType) => void;
  onToggleLock?: () => void;
  sessionSecondsLeft?: number;
  sessionTotalSeconds?: number;
  theme?: Theme;
  onToggleTheme?: () => void;
}

export function Sidebar({ open, onClose, network = 'testnet', onSwitchNetwork, onToggleLock, sessionSecondsLeft, theme, onToggleTheme }: SidebarProps) {
  const mins = Math.floor((sessionSecondsLeft ?? 0) / 60);
  const secs = (sessionSecondsLeft ?? 0) % 60;
  const isMainnet = network === 'mainnet';
  const m = useIsMobile();

  return (
    <>
      {open && <div style={{ position: 'fixed', inset: 0, background: 'var(--dag-overlay)', zIndex: 40 }} onClick={onClose} />}

      <aside style={{
        width: m ? 260 : 216, padding: '18px 10px',
        background: m ? 'var(--dag-bg)' : 'var(--dag-sidebar-bg)',
        borderRight: '1px solid var(--dag-sidebar-border)',
        display: m && !open ? 'none' : 'flex', flexDirection: 'column',
        position: m ? 'fixed' : 'sticky', top: 0, left: 0, height: '100vh', overflowY: 'auto',
        fontFamily: fonts.sans,
        zIndex: m ? 50 : (open ? 50 : 'auto'),
        transition: 'transform 0.2s ease',
      }}>
        {/* Logo */}
        <div style={{ display: 'flex', alignItems: 'center', gap: 10, padding: '2px 8px', marginBottom: 20 }}>
          <img src="/media/logo/logo_website.png" alt="UltraDAG" style={{ height: 30, width: 'auto' }} />
          <div>
            <div style={{ fontSize: 8.5, color: isMainnet ? '#00E0C4' : '#FFB800', letterSpacing: 2.5, fontWeight: 600 }}>
              {network.toUpperCase()} v0.1
            </div>
          </div>
        </div>

        {/* Network Switcher */}
        {onSwitchNetwork && (
          <div style={{ padding: '0 6px', marginBottom: 16 }}>
            <div style={{
              display: 'flex', borderRadius: 8, overflow: 'hidden',
              background: 'var(--dag-net-switch-bg)', border: '1px solid var(--dag-net-switch-border)',
            }}>
              {(['mainnet', 'testnet'] as const).map(net => (
                <button key={net} onClick={() => onSwitchNetwork(net)} style={{
                  flex: 1, padding: '6px 0', border: 'none', cursor: 'pointer',
                  fontSize: 10, fontWeight: 600, letterSpacing: 0.8, textTransform: 'uppercase',
                  transition: 'all 0.2s',
                  background: network === net
                    ? net === 'mainnet' ? 'rgba(0,224,196,0.12)' : 'rgba(255,184,0,0.12)'
                    : 'transparent',
                  color: network === net
                    ? net === 'mainnet' ? '#00E0C4' : '#FFB800'
                    : 'var(--dag-net-inactive)',
                  borderBottom: network === net
                    ? `2px solid ${net === 'mainnet' ? '#00E0C4' : '#FFB800'}`
                    : '2px solid transparent',
                }}>
                  {net === 'mainnet' ? '\u25C8 Main' : '\u25C7 Test'}
                </button>
              ))}
            </div>
          </div>
        )}

        {/* Nav */}
        {sections.map((section, si) => (
          <div key={si}>
            {section.label && (
              <div style={{ fontSize: 9, fontWeight: 600, color: 'var(--dag-sidebar-section)', letterSpacing: 2, padding: '12px 8px 5px', marginTop: 2 }}>
                {section.label}
              </div>
            )}
            {section.items.map(({ to, icon, label }) => (
              <NavLink key={to} to={to} end={to === '/' || to === '/wallet'} onClick={onClose}
                style={({ isActive }) => ({
                  display: 'flex', alignItems: 'center', gap: 9, padding: '7px 11px', borderRadius: 8,
                  cursor: 'pointer', marginBottom: 1, textDecoration: 'none',
                  background: isActive ? 'rgba(0,224,196,0.06)' : 'transparent',
                  border: isActive ? '1px solid rgba(0,224,196,0.1)' : '1px solid transparent',
                  color: isActive ? '#00E0C4' : 'var(--dag-sidebar-inactive)',
                  fontSize: 12.5, fontWeight: isActive ? 600 : 400,
                  transition: 'all 0.2s',
                })}>
                <span style={{ fontSize: 13, width: 18, textAlign: 'center' }}>{icon}</span>
                {label}
              </NavLink>
            ))}
          </div>
        ))}

        <div style={{ flex: 1 }} />

        {/* Theme toggle */}
        {onToggleTheme && (
          <button onClick={onToggleTheme} style={{
            display: 'flex', alignItems: 'center', justifyContent: 'center', gap: 6,
            margin: '0 6px 4px', padding: '6px 0', borderRadius: 8,
            background: 'var(--dag-sidebar-lock-bg)', border: '1px solid var(--dag-sidebar-lock-border)',
            color: 'var(--dag-text-muted)', fontSize: 11, fontWeight: 500, cursor: 'pointer',
            transition: 'all 0.2s',
          }}>
            {theme === 'dark' ? '\u2600 Light Mode' : '\u263E Dark Mode'}
          </button>
        )}

        {/* Lock button */}
        {onToggleLock && (
          <button onClick={onToggleLock} style={{
            display: 'flex', alignItems: 'center', justifyContent: 'center', gap: 6,
            margin: '0 6px 8px', padding: '7px 0', borderRadius: 8,
            background: 'var(--dag-sidebar-lock-bg)', border: '1px solid var(--dag-sidebar-lock-border)',
            color: 'var(--dag-sidebar-lock-text)', fontSize: 11, fontWeight: 500, cursor: 'pointer',
            transition: 'all 0.2s',
          }}>
            {'🔒'} Lock Wallet
          </button>
        )}

        {/* Footer */}
        <div style={{ padding: '8px 8px', borderTop: '1px solid var(--dag-sidebar-footer-border)' }}>
          <div style={{ display: 'flex', alignItems: 'center', gap: 5, marginBottom: 3 }}>
            <div style={{ width: 5, height: 5, borderRadius: '50%', background: '#00E0C4', boxShadow: '0 0 6px #00E0C4' }} />
            <span style={{ fontSize: 10, color: 'var(--dag-sidebar-footer-text)' }}>Connected</span>
          </div>
          {sessionSecondsLeft != null && sessionSecondsLeft < 9000 ? (
            <div style={{ fontSize: 9, color: 'var(--dag-sidebar-footer-muted)', fontFamily: fonts.mono }}>
              Session {mins}:{secs.toString().padStart(2, '0')}
            </div>
          ) : sessionSecondsLeft != null && sessionSecondsLeft >= 9000 ? (
            <div style={{ fontSize: 9, color: 'var(--dag-sidebar-footer-faint)' }}>
              \u25CE Passkey session
            </div>
          ) : null}
        </div>
      </aside>
    </>
  );
}
