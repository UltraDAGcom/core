import { useState } from 'react';
import { postStake, postUnstake, postSetCommission } from '../../lib/api';
import { WalletSelector } from '../shared/WalletSelector';
import { useWalletSelector } from '../../hooks/useKeystore';

interface StakeFormProps {
  onSuccess: () => void;
}

export function StakeForm({ onSuccess }: StakeFormProps) {
  const { wallets, unlocked, selected, selectedIdx, setSelectedIdx } = useWalletSelector();
  const [amount, setAmount] = useState('');
  const [commission, setCommission] = useState('');
  const [mode, setMode] = useState<'stake' | 'unstake' | 'commission'>('stake');
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState('');
  const [success, setSuccess] = useState('');

  if (!unlocked) {
    return <p className="text-dag-muted text-sm">Unlock your keystore to stake.</p>;
  }

  const handleStake = async () => {
    if (!selected) return;
    const sats = Math.floor(parseFloat(amount) * 100_000_000);
    if (isNaN(sats) || sats < 10_000 * 100_000_000) {
      setError('Minimum stake is 10,000 UDAG');
      return;
    }
    setLoading(true);
    setError('');
    setSuccess('');
    try {
      await postStake({ secret_key: selected.secret_key, amount: sats });
      setSuccess('Stake submitted successfully');
      setAmount('');
      onSuccess();
    } catch (e: unknown) {
      setError(e instanceof Error ? e.message : 'Stake failed');
    } finally {
      setLoading(false);
    }
  };

  const handleUnstake = async () => {
    if (!selected) return;
    setLoading(true);
    setError('');
    setSuccess('');
    try {
      await postUnstake({ secret_key: selected.secret_key });
      setSuccess('Unstake submitted. Cooldown: ~2.8 hours.');
      onSuccess();
    } catch (e: unknown) {
      setError(e instanceof Error ? e.message : 'Unstake failed');
    } finally {
      setLoading(false);
    }
  };

  const handleSetCommission = async () => {
    if (!selected) return;
    const pct = parseInt(commission, 10);
    if (isNaN(pct) || pct < 0 || pct > 100) {
      setError('Commission must be 0-100');
      return;
    }
    setLoading(true);
    setError('');
    setSuccess('');
    try {
      await postSetCommission({ secret_key: selected.secret_key, commission_percent: pct });
      setSuccess(`Commission set to ${pct}%`);
      setCommission('');
      onSuccess();
    } catch (e: unknown) {
      setError(e instanceof Error ? e.message : 'Set commission failed');
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="space-y-4">
      <WalletSelector wallets={wallets} selectedIdx={selectedIdx} onChange={setSelectedIdx} />

      <div className="flex gap-2">
        {(['stake', 'unstake', 'commission'] as const).map(m => (
          <button
            key={m}
            onClick={() => { setMode(m); setError(''); setSuccess(''); }}
            className={`text-sm px-3 py-1.5 rounded border transition-colors ${
              mode === m
                ? 'bg-dag-blue text-white border-dag-blue'
                : 'bg-dag-surface border-dag-border text-dag-muted hover:text-white'
            }`}
          >
            {m === 'commission' ? 'Set Commission' : m.charAt(0).toUpperCase() + m.slice(1)}
          </button>
        ))}
      </div>

      {mode === 'stake' && (
        <div className="space-y-3">
          <label className="block">
            <span className="text-sm text-dag-muted">Amount (UDAG, min 10,000)</span>
            <input
              type="number"
              min="10000"
              step="1"
              value={amount}
              onChange={e => setAmount(e.target.value)}
              placeholder="10000"
              className="mt-1 block w-full rounded bg-dag-surface border border-dag-border px-3 py-2 text-sm text-white"
            />
          </label>
          <button
            onClick={handleStake}
            disabled={loading}
            className="w-full py-2 rounded bg-dag-green text-white font-medium text-sm hover:bg-dag-green/90 disabled:opacity-50"
          >
            {loading ? 'Submitting...' : 'Stake'}
          </button>
        </div>
      )}

      {mode === 'unstake' && (
        <div className="space-y-3">
          <p className="text-sm text-dag-muted">
            Unstaking begins a ~2.8 hour cooldown. All staked UDAG will be returned after the cooldown period.
          </p>
          <button
            onClick={handleUnstake}
            disabled={loading}
            className="w-full py-2 rounded bg-dag-red text-white font-medium text-sm hover:bg-dag-red/90 disabled:opacity-50"
          >
            {loading ? 'Submitting...' : 'Unstake All'}
          </button>
        </div>
      )}

      {mode === 'commission' && (
        <div className="space-y-3">
          <label className="block">
            <span className="text-sm text-dag-muted">Commission Rate (0-100%)</span>
            <input
              type="number"
              min="0"
              max="100"
              value={commission}
              onChange={e => setCommission(e.target.value)}
              placeholder="10"
              className="mt-1 block w-full rounded bg-dag-surface border border-dag-border px-3 py-2 text-sm text-white"
            />
          </label>
          <button
            onClick={handleSetCommission}
            disabled={loading}
            className="w-full py-2 rounded bg-dag-purple text-white font-medium text-sm hover:bg-dag-purple/90 disabled:opacity-50"
          >
            {loading ? 'Submitting...' : 'Set Commission'}
          </button>
        </div>
      )}

      {error && <p className="text-sm text-dag-red">{error}</p>}
      {success && <p className="text-sm text-dag-green">{success}</p>}
    </div>
  );
}
