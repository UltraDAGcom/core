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
        <p className="text-dag-muted">Unlock your keystore to view portfolio.</p>
      </div>
    );
  }

  const totalValue = totalBalance + totalStaked + totalDelegated;

  return (
    <div className="space-y-6">
      <h1 className="text-2xl font-bold text-white">Portfolio</h1>

      <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4 gap-4">
        <Card>
          <p className="text-xs text-dag-muted uppercase tracking-wider">Total Value</p>
          <p className="text-xl font-bold text-white mt-1">{formatUdag(totalValue)} UDAG</p>
        </Card>
        <Card>
          <p className="text-xs text-dag-muted uppercase tracking-wider">Available</p>
          <p className="text-xl font-bold text-white mt-1">{formatUdag(totalBalance)} UDAG</p>
        </Card>
        <Card>
          <p className="text-xs text-dag-muted uppercase tracking-wider">Staked</p>
          <p className="text-xl font-bold text-dag-green mt-1">{formatUdag(totalStaked)} UDAG</p>
        </Card>
        <Card>
          <p className="text-xs text-dag-muted uppercase tracking-wider">Delegated</p>
          <p className="text-xl font-bold text-dag-blue mt-1">{formatUdag(totalDelegated)} UDAG</p>
        </Card>
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
                </tr>
              </thead>
              <tbody>
                {wallets.map(w => {
                  const b = balances.get(w.address);
                  return (
                    <tr key={w.address} className="border-b border-dag-border/50">
                      <td className="py-2.5 px-3 text-white">{w.name}</td>
                      <td className="py-2.5 px-3 font-mono text-slate-300">{b ? formatUdag(b.balance) : '--'}</td>
                      <td className="py-2.5 px-3 font-mono text-slate-300">{b ? formatUdag(b.staked) : '--'}</td>
                      <td className="py-2.5 px-3 font-mono text-slate-300">{b ? formatUdag(b.delegated) : '--'}</td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
          </div>
        )}
      </Card>
    </div>
  );
}
