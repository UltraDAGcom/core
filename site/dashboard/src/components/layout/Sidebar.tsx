import { NavLink } from 'react-router-dom';
import { fonts } from '../../lib/theme';
import type { NetworkType } from '../../lib/api';

interface NavItem { to: string; icon: string; label: string }

const sections: { label?: string; items: NavItem[] }[] = [
  { items: [{ to: '/', icon: '◈', label: 'Dashboard' }] },
  { label: 'WALLET', items: [
    { to: '/wallet', icon: '◇', label: 'Wallets' },
    { to: '/wallet/send', icon: '⇄', label: 'Send & Receive' },
    { to: '/smart-account', icon: '◎', label: 'SmartAccount' },
  ]},
  { label: 'NETWORK', items: [
    { to: '/staking', icon: '⬡', label: 'Staking' },
    { to: '/governance', icon: '⚙', label: 'Governance' },
    { to: '/council', icon: '♛', label: 'Council' },
    { to: '/explorer', icon: '◉', label: 'Explorer' },
  ]},
  { label: 'ADVANCED', items: [
    { to: '/bridge', icon: '⟷', label: 'Bridge' },
    { to: '/network', icon: '⚡', label: 'Node Status' },
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
}

export function Sidebar({ open, onClose, network = 'testnet', onSwitchNetwork, onToggleLock, sessionSecondsLeft }: SidebarProps) {
  const mins = Math.floor((sessionSecondsLeft ?? 0) / 60);
  const secs = (sessionSecondsLeft ?? 0) % 60;
  const isMainnet = network === 'mainnet';

  return (
    <>
      {open && <div style={{ position: 'fixed', inset: 0, background: 'rgba(0,0,0,0.6)', zIndex: 40 }} onClick={onClose} />}

      <aside style={{
        width: 216, padding: '18px 10px',
        background: 'rgba(255,255,255,0.008)',
        borderRight: '1px solid rgba(255,255,255,0.04)',
        display: 'flex', flexDirection: 'column',
        position: 'sticky', top: 0, height: '100vh', overflowY: 'auto',
        fontFamily: fonts.sans,
        zIndex: open ? 50 : 'auto',
      }}>
        {/* Logo */}
        <div style={{ display: 'flex', alignItems: 'center', gap: 10, padding: '2px 8px', marginBottom: 20 }}>
          <div style={{
            width: 30, height: 30, borderRadius: 8, display: 'flex', alignItems: 'center', justifyContent: 'center',
            background: 'linear-gradient(135deg,#00E0C4,#0066FF)', fontSize: 14, fontWeight: 800, color: '#fff',
            boxShadow: '0 0 16px rgba(0,224,196,0.25)',
          }}>U</div>
          <div>
            <div style={{ fontSize: 13, fontWeight: 700, letterSpacing: 1.2, color: '#fff' }}>ULTRADAG</div>
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
              background: 'rgba(255,255,255,0.02)', border: '1px solid rgba(255,255,255,0.04)',
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
                    : 'rgba(255,255,255,0.18)',
                  borderBottom: network === net
                    ? `2px solid ${net === 'mainnet' ? '#00E0C4' : '#FFB800'}`
                    : '2px solid transparent',
                }}>
                  {net === 'mainnet' ? '◈ Main' : '◇ Test'}
                </button>
              ))}
            </div>
          </div>
        )}

        {/* Nav */}
        {sections.map((section, si) => (
          <div key={si}>
            {section.label && (
              <div style={{ fontSize: 9, fontWeight: 600, color: 'rgba(255,255,255,0.12)', letterSpacing: 2, padding: '12px 8px 5px', marginTop: 2 }}>
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
                  color: isActive ? '#00E0C4' : 'rgba(255,255,255,0.35)',
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

        {/* Lock button */}
        {onToggleLock && (
          <button onClick={onToggleLock} style={{
            display: 'flex', alignItems: 'center', justifyContent: 'center', gap: 6,
            margin: '0 6px 8px', padding: '7px 0', borderRadius: 8,
            background: 'rgba(255,255,255,0.02)', border: '1px solid rgba(255,255,255,0.04)',
            color: 'rgba(255,255,255,0.3)', fontSize: 11, fontWeight: 500, cursor: 'pointer',
            transition: 'all 0.2s',
          }}>
            🔒 Lock Wallet
          </button>
        )}

        {/* Footer */}
        <div style={{ padding: '8px 8px', borderTop: '1px solid rgba(255,255,255,0.03)' }}>
          <div style={{ display: 'flex', alignItems: 'center', gap: 5, marginBottom: 3 }}>
            <div style={{ width: 5, height: 5, borderRadius: '50%', background: '#00E0C4', boxShadow: '0 0 6px #00E0C4' }} />
            <span style={{ fontSize: 10, color: 'rgba(255,255,255,0.3)' }}>Connected</span>
          </div>
          {sessionSecondsLeft != null && sessionSecondsLeft < 9000 && (
            <div style={{ fontSize: 9, color: 'rgba(255,255,255,0.15)', fontFamily: fonts.mono }}>
              Session {mins}:{secs.toString().padStart(2, '0')}
            </div>
          )}
        </div>
      </aside>
    </>
  );
}
