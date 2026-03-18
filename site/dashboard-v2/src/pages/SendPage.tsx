import { useState } from 'react';
import { postTx, formatUdag, shortAddr } from '../lib/api.ts';
import { Card } from '../components/shared/Card.tsx';
import { WalletSelector } from '../components/shared/WalletSelector.tsx';
import type { Wallet } from '../lib/keystore.ts';
import type { WalletBalance } from '../hooks/useWalletBalances.ts';

interface SendPageProps {
  wallets: Wallet[];
  balances: Map<string, WalletBalance>;
  unlocked: boolean;
}

export function SendPage({ wallets, balances, unlocked }: SendPageProps) {
  const [selectedIdx, setSelectedIdx] = useState(0);
  const [to, setTo] = useState('');
  const [amount, setAmount] = useState('');
  const [fee, setFee] = useState('10000');
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState('');
  const [success, setSuccess] = useState('');

  if (!unlocked) {
    return (
      <div className="flex items-center justify-center h-64">
        <p className="text-dag-muted">Unlock your keystore to send transactions.</p>
      </div>
    );
  }

  const wallet = wallets[selectedIdx];
  const balance = wallet ? balances.get(wallet.address) : undefined;

  const handleSend = async () => {
    if (!wallet) return;
    setError('');
    setSuccess('');
    const sats = Math.floor(parseFloat(amount) * 100_000_000);
    const feeSats = parseInt(fee, 10);
    if (isNaN(sats) || sats <= 0) { setError('Amount must be positive'); return; }
    if (isNaN(feeSats) || feeSats < 10000) { setError('Minimum fee is 10,000 sats'); return; }
    if (!/^[0-9a-fA-F]{64}$/.test(to.trim())) { setError('Invalid recipient address (64 hex chars)'); return; }

    setLoading(true);
    try {
      await postTx({
        from_secret: wallet.secret_key,
        to: to.trim().toLowerCase(),
        amount: sats,
        fee: feeSats,
      });
      setSuccess(`Sent ${formatUdag(sats)} UDAG to ${shortAddr(to)}`);
      setTo('');
      setAmount('');
    } catch (e: unknown) {
      setError(e instanceof Error ? e.message : 'Transaction failed');
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="space-y-6">
      <h1 className="text-2xl font-bold text-white">Send UDAG</h1>

      <div className="max-w-lg">
        <Card>
          <div className="space-y-4">
            <WalletSelector wallets={wallets} selectedIdx={selectedIdx} onChange={setSelectedIdx} />

            {balance && (
              <div className="text-sm text-dag-muted">
                Available: <span className="text-white font-mono">{formatUdag(balance.balance)} UDAG</span>
              </div>
            )}

            <label className="block">
              <span className="text-sm text-dag-muted">Recipient Address (64 hex)</span>
              <input
                type="text"
                value={to}
                onChange={e => setTo(e.target.value)}
                placeholder="Enter recipient address"
                className="mt-1 block w-full rounded bg-dag-surface border border-dag-border px-3 py-2 text-sm text-white font-mono"
              />
            </label>

            <label className="block">
              <span className="text-sm text-dag-muted">Amount (UDAG)</span>
              <input
                type="number"
                min="0"
                step="0.01"
                value={amount}
                onChange={e => setAmount(e.target.value)}
                placeholder="0.00"
                className="mt-1 block w-full rounded bg-dag-surface border border-dag-border px-3 py-2 text-sm text-white"
              />
            </label>

            <label className="block">
              <span className="text-sm text-dag-muted">Fee (sats, min 10,000)</span>
              <input
                type="number"
                min="10000"
                value={fee}
                onChange={e => setFee(e.target.value)}
                className="mt-1 block w-full rounded bg-dag-surface border border-dag-border px-3 py-2 text-sm text-white"
              />
            </label>

            {error && <p className="text-sm text-dag-red">{error}</p>}
            {success && <p className="text-sm text-dag-green">{success}</p>}

            <button
              onClick={handleSend}
              disabled={loading}
              className="w-full py-2.5 rounded bg-dag-accent text-white font-medium text-sm hover:bg-dag-accent/80 disabled:opacity-50 transition-colors"
            >
              {loading ? 'Sending...' : 'Send'}
            </button>
          </div>
        </Card>
      </div>
    </div>
  );
}
