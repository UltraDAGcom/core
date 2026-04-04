import { formatUdag } from '../lib/api.ts';
import { PageHeader } from '../components/shared/PageHeader.tsx';
import { Card } from '../components/shared/Card.tsx';
import { useIsMobile } from '../hooks/useIsMobile';
import type { Wallet } from '../lib/keystore.ts';
import type { WalletBalance } from '../hooks/useWalletBalances.ts';

interface PortfolioPageProps {
  unlocked: boolean;
  wallets: Wallet[];
  balances: Map<string, WalletBalance>;
  totalBalance: number;
  totalStaked: number;
  totalDelegated: number;
}

function pct(part: number, total: number): string {
  if (total === 0) return '0%';
  return `${Math.round((part / total) * 100)}%`;
}

function StatCard({
  label,
  value,
  accentColor,
  percentage,
}: {
  label: string;
  value: string;
  accentColor: string;
  percentage: string;
}) {
  return (
    <div style={{ background: 'var(--dag-card)', border: '1px solid var(--dag-border)', borderRadius: 12, overflow: 'hidden' }}>
      <div style={{ height: 2, background: accentColor }} />
      <div style={{ padding: 20 }}>
        <p style={{ fontSize: 10, color: 'var(--dag-text-muted)', textTransform: 'uppercase', letterSpacing: '0.05em' }}>{label}</p>
        <p style={{ fontSize: 18, fontWeight: 700, color: 'var(--dag-text)', marginTop: 4 }}>{value}</p>
        <p style={{ fontSize: 10, color: 'var(--dag-text-muted)', marginTop: 4 }}>{percentage} of total</p>
      </div>
    </div>
  );
}

export function PortfolioPage({
  unlocked,
  wallets,
  balances,
  totalBalance,
  totalStaked,
  totalDelegated,
}: PortfolioPageProps) {
  const m = useIsMobile();

  if (!unlocked) {
    return (
      <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'center', height: 256 }}>
        <p style={{ color: 'var(--dag-text-muted)' }}>Unlock your wallet to view portfolio.</p>
      </div>
    );
  }

  const totalValue = totalBalance + totalStaked + totalDelegated;

  return (
    <div style={{ padding: m ? '12px 14px' : '18px 26px', fontFamily: "'DM Sans',sans-serif" }}>
      <PageHeader title="Portfolio" subtitle="Your wallet breakdown" />

      <div style={{ display: 'grid', gridTemplateColumns: m ? '1fr' : 'repeat(4, 1fr)', gap: 16 }}>
        <StatCard
          label="Total Value"
          value={`${formatUdag(totalValue)} UDAG`}
          accentColor="#00E0C4"
          percentage="100%"
        />
        <StatCard
          label="Available"
          value={`${formatUdag(totalBalance)} UDAG`}
          accentColor="#00E0C4"
          percentage={pct(totalBalance, totalValue)}
        />
        <StatCard
          label="Staked"
          value={`${formatUdag(totalStaked)} UDAG`}
          accentColor="#A855F7"
          percentage={pct(totalStaked, totalValue)}
        />
        <StatCard
          label="Delegated"
          value={`${formatUdag(totalDelegated)} UDAG`}
          accentColor="#00E0C4"
          percentage={pct(totalDelegated, totalValue)}
        />
      </div>

      <Card title={`Wallets (${wallets.length})`}>
        {wallets.length === 0 ? (
          <p style={{ color: 'var(--dag-text-muted)', fontSize: 12 }}>No wallets. Create one in the wallet page.</p>
        ) : (
          <div style={{ overflowX: 'auto' }}>
            <table style={{ width: '100%', fontSize: 12, borderCollapse: 'collapse' }}>
              <thead>
                <tr style={{ textAlign: 'left', color: 'var(--dag-text-muted)', borderBottom: '1px solid var(--dag-border)' }}>
                  <th style={{ padding: '8px 12px', fontWeight: 500 }}>Name</th>
                  <th style={{ padding: '8px 12px', fontWeight: 500 }}>Balance</th>
                  <th style={{ padding: '8px 12px', fontWeight: 500 }}>Staked</th>
                  <th style={{ padding: '8px 12px', fontWeight: 500 }}>Delegated</th>
                  <th style={{ padding: '8px 12px', fontWeight: 500, width: 128 }}>Share</th>
                </tr>
              </thead>
              <tbody>
                {wallets.map(w => {
                  const b = balances.get(w.address);
                  const walletTotal = b ? b.balance + b.staked + b.delegated : 0;
                  const share = totalValue > 0 ? (walletTotal / totalValue) * 100 : 0;
                  return (
                    <tr key={w.address} style={{ borderBottom: '1px solid var(--dag-border)' }}>
                      <td style={{ padding: '10px 12px', color: 'var(--dag-text)' }}>{w.name}</td>
                      <td style={{ padding: '10px 12px', fontFamily: "'DM Mono',monospace", color: 'var(--dag-text-secondary)' }}>{b ? formatUdag(b.balance) : '--'}</td>
                      <td style={{ padding: '10px 12px', fontFamily: "'DM Mono',monospace", color: 'var(--dag-text-secondary)' }}>{b ? formatUdag(b.staked) : '--'}</td>
                      <td style={{ padding: '10px 12px', fontFamily: "'DM Mono',monospace", color: 'var(--dag-text-secondary)' }}>{b ? formatUdag(b.delegated) : '--'}</td>
                      <td style={{ padding: '10px 12px' }}>
                        <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
                          <div style={{ flex: 1, height: 6, borderRadius: 9999, background: 'var(--dag-surface)', overflow: 'hidden' }}>
                            <div
                              style={{ height: '100%', borderRadius: 9999, background: '#00E0C4', width: `${Math.min(share, 100)}%`, transition: 'width 0.5s' }}
                            />
                          </div>
                          <span style={{ fontSize: 10, color: 'var(--dag-text-muted)', fontFamily: "'DM Mono',monospace", width: 40, textAlign: 'right' }}>{share.toFixed(0)}%</span>
                        </div>
                      </td>
                    </tr>
                  );
                })}
                {/* Total row */}
                <tr style={{ borderTop: '2px solid var(--dag-border)' }}>
                  <td style={{ padding: '10px 12px', fontWeight: 600, color: 'var(--dag-text)' }}>Total</td>
                  <td style={{ padding: '10px 12px', fontFamily: "'DM Mono',monospace", fontWeight: 600, color: 'var(--dag-text)' }}>{formatUdag(totalBalance)}</td>
                  <td style={{ padding: '10px 12px', fontFamily: "'DM Mono',monospace", fontWeight: 600, color: 'var(--dag-text)' }}>{formatUdag(totalStaked)}</td>
                  <td style={{ padding: '10px 12px', fontFamily: "'DM Mono',monospace", fontWeight: 600, color: 'var(--dag-text)' }}>{formatUdag(totalDelegated)}</td>
                  <td style={{ padding: '10px 12px' }}>
                    <span style={{ fontSize: 10, fontWeight: 600, color: 'var(--dag-text)', fontFamily: "'DM Mono',monospace" }}>100%</span>
                  </td>
                </tr>
              </tbody>
            </table>
          </div>
        )}
      </Card>
    </div>
  );
}
