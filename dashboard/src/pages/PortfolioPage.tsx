import { formatUdag } from '../lib/api.ts';
import { Card } from '../components/shared/Card.tsx';
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
  gradient,
  percentage,
}: {
  label: string;
  value: string;
  gradient: string;
  percentage: string;
}) {
  return (
    <div className="bg-dag-card border border-dag-border rounded-xl overflow-hidden">
      <div className={`h-0.5 bg-gradient-to-r ${gradient}`} />
      <div className="p-5">
        <p className="text-xs text-dag-muted uppercase tracking-wider">{label}</p>
        <p className="text-xl font-bold text-white mt-1">{value}</p>
        <p className="text-xs text-dag-muted mt-1">{percentage} of total</p>
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
  if (!unlocked) {
    return (
      <div className="flex items-center justify-center h-64">
        <p className="text-dag-muted">Unlock your wallet to view portfolio.</p>
      </div>
    );
  }

  const totalValue = totalBalance + totalStaked + totalDelegated;

  return (
    <div className="space-y-6">
      <h1 className="text-2xl font-bold text-white">Portfolio</h1>

      <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4 gap-4">
        <StatCard
          label="Total Value"
          value={`${formatUdag(totalValue)} UDAG`}
          gradient="from-dag-blue to-dag-purple"
          percentage="100%"
        />
        <StatCard
          label="Available"
          value={`${formatUdag(totalBalance)} UDAG`}
          gradient="from-dag-green to-emerald-400"
          percentage={pct(totalBalance, totalValue)}
        />
        <StatCard
          label="Staked"
          value={`${formatUdag(totalStaked)} UDAG`}
          gradient="from-dag-purple to-fuchsia-400"
          percentage={pct(totalStaked, totalValue)}
        />
        <StatCard
          label="Delegated"
          value={`${formatUdag(totalDelegated)} UDAG`}
          gradient="from-dag-blue to-cyan-400"
          percentage={pct(totalDelegated, totalValue)}
        />
      </div>

      <Card title={`Wallets (${wallets.length})`}>
        {wallets.length === 0 ? (
          <p className="text-dag-muted text-sm">No wallets. Create one in the wallet page.</p>
        ) : (
          <div className="overflow-x-auto">
            <table className="w-full text-sm">
              <thead>
                <tr className="text-left text-dag-muted border-b border-dag-border">
                  <th className="py-2 px-3 font-medium">Name</th>
                  <th className="py-2 px-3 font-medium">Balance</th>
                  <th className="py-2 px-3 font-medium">Staked</th>
                  <th className="py-2 px-3 font-medium">Delegated</th>
                  <th className="py-2 px-3 font-medium w-32">Share</th>
                </tr>
              </thead>
              <tbody>
                {wallets.map(w => {
                  const b = balances.get(w.address);
                  const walletTotal = b ? b.balance + b.staked + b.delegated : 0;
                  const share = totalValue > 0 ? (walletTotal / totalValue) * 100 : 0;
                  return (
                    <tr key={w.address} className="border-b border-dag-border/50">
                      <td className="py-2.5 px-3 text-white">{w.name}</td>
                      <td className="py-2.5 px-3 font-mono text-slate-300">{b ? formatUdag(b.balance) : '--'}</td>
                      <td className="py-2.5 px-3 font-mono text-slate-300">{b ? formatUdag(b.staked) : '--'}</td>
                      <td className="py-2.5 px-3 font-mono text-slate-300">{b ? formatUdag(b.delegated) : '--'}</td>
                      <td className="py-2.5 px-3">
                        <div className="flex items-center gap-2">
                          <div className="flex-1 h-1.5 rounded-full bg-dag-surface overflow-hidden">
                            <div
                              className="h-full rounded-full bg-gradient-to-r from-dag-blue to-dag-purple transition-all duration-500"
                              style={{ width: `${Math.min(share, 100)}%` }}
                            />
                          </div>
                          <span className="text-xs text-dag-muted font-mono w-10 text-right">{share.toFixed(0)}%</span>
                        </div>
                      </td>
                    </tr>
                  );
                })}
                {/* Total row */}
                <tr className="border-t-2 border-dag-border">
                  <td className="py-2.5 px-3 font-semibold text-white">Total</td>
                  <td className="py-2.5 px-3 font-mono font-semibold text-white">{formatUdag(totalBalance)}</td>
                  <td className="py-2.5 px-3 font-mono font-semibold text-white">{formatUdag(totalStaked)}</td>
                  <td className="py-2.5 px-3 font-mono font-semibold text-white">{formatUdag(totalDelegated)}</td>
                  <td className="py-2.5 px-3">
                    <span className="text-xs font-semibold text-white font-mono">100%</span>
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
