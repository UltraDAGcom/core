import { useState } from 'react';
import { Wallet as WalletIcon, Eye, Trash2, Shield } from 'lucide-react';
import { CopyButton } from '../shared/CopyButton.tsx';
import { formatUdag, shortAddr } from '../../lib/api.ts';
import type { WalletBalance } from '../../hooks/useWalletBalances.ts';

interface WalletCardProps {
  name: string;
  address: string;
  balance?: WalletBalance;
  selected: boolean;
  onClick: () => void;
}

export function WalletCard({ name, address, balance, selected, onClick }: WalletCardProps) {
  return (
    <button
      onClick={onClick}
      className={`w-full text-left p-4 rounded-xl border transition-all ${
        selected
          ? 'bg-dag-accent/10 border-dag-accent/40 ring-1 ring-dag-accent/20'
          : 'bg-dag-card border-dag-border hover:border-slate-500 hover:bg-dag-card-hover'
      }`}
    >
      <div className="flex items-start justify-between">
        <div className="flex items-center gap-3">
          <div
            className={`w-10 h-10 rounded-lg flex items-center justify-center ${
              selected ? 'bg-dag-accent/20' : 'bg-slate-700'
            }`}
          >
            <WalletIcon className={`w-5 h-5 ${selected ? 'text-dag-accent' : 'text-slate-400'}`} />
          </div>
          <div>
            <p className="text-sm font-medium text-slate-200">{name}</p>
            <div className="flex items-center gap-1 mt-0.5">
              <p className="text-xs font-mono text-dag-muted">{shortAddr(address)}</p>
              <CopyButton text={address} />
            </div>
          </div>
        </div>
        {balance?.is_active_validator && (
          <span title="Active validator"><Shield className="w-4 h-4 text-dag-green" /></span>
        )}
      </div>
      <div className="mt-3 grid grid-cols-3 gap-2">
        <div>
          <p className="text-[10px] uppercase text-dag-muted tracking-wider">Balance</p>
          <p className="text-sm font-medium text-slate-200">
            {balance ? formatUdag(balance.balance) : '--'} <span className="text-[10px] text-dag-muted">UDAG</span>
          </p>
        </div>
        <div>
          <p className="text-[10px] uppercase text-dag-muted tracking-wider">Staked</p>
          <p className="text-sm font-medium text-slate-200">
            {balance ? formatUdag(balance.staked) : '--'}
          </p>
        </div>
        <div>
          <p className="text-[10px] uppercase text-dag-muted tracking-wider">Delegated</p>
          <p className="text-sm font-medium text-slate-200">
            {balance ? formatUdag(balance.delegated) : '--'}
          </p>
        </div>
      </div>
    </button>
  );
}

interface WalletDetailProps {
  name: string;
  address: string;
  secretKey: string;
  balance?: WalletBalance;
  onRemove: () => void;
}

export function WalletDetail({ name, address, secretKey, balance, onRemove }: WalletDetailProps) {
  const [showKey, setShowKey] = useState(false);
  return (
    <div className="bg-dag-card border border-dag-border rounded-xl p-5 space-y-4">
      <div className="flex items-center justify-between">
        <h3 className="text-lg font-semibold text-white">{name}</h3>
        <button
          onClick={onRemove}
          className="p-2 rounded-lg text-red-400 hover:bg-red-500/15 transition-colors"
          title="Remove wallet"
        >
          <Trash2 className="w-4 h-4" />
        </button>
      </div>

      {/* Address */}
      <div>
        <label className="text-xs text-dag-muted uppercase tracking-wider">Address</label>
        <div className="flex items-center gap-2 mt-1">
          <code className="text-xs font-mono text-slate-300 bg-slate-800 px-3 py-2 rounded-lg break-all flex-1">
            {address}
          </code>
          <CopyButton text={address} />
        </div>
      </div>

      {/* Secret Key */}
      <div>
        <label className="text-xs text-dag-muted uppercase tracking-wider">Secret Key</label>
        <div className="flex items-center gap-2 mt-1">
          <code className="text-xs font-mono text-slate-300 bg-slate-800 px-3 py-2 rounded-lg break-all flex-1">
            {showKey ? secretKey : '************************************************************'}
          </code>
          <button
            onClick={() => setShowKey(!showKey)}
            className="p-2 rounded-lg text-slate-400 hover:text-slate-200 hover:bg-slate-700 transition-colors"
            title={showKey ? 'Hide key' : 'Reveal key'}
          >
            <Eye className="w-4 h-4" />
          </button>
          {showKey && <CopyButton text={secretKey} />}
        </div>
      </div>

      {/* Balance detail */}
      {balance && (
        <div className="grid grid-cols-2 sm:grid-cols-4 gap-3 pt-2 border-t border-dag-border">
          <Stat label="Balance" value={`${formatUdag(balance.balance)} UDAG`} />
          <Stat label="Staked" value={`${formatUdag(balance.staked)} UDAG`} />
          <Stat label="Delegated" value={`${formatUdag(balance.delegated)} UDAG`} />
          <Stat label="Nonce" value={String(balance.nonce)} />
        </div>
      )}
    </div>
  );
}

function Stat({ label, value }: { label: string; value: string }) {
  return (
    <div>
      <p className="text-[10px] uppercase text-dag-muted tracking-wider">{label}</p>
      <p className="text-sm font-medium text-slate-200 mt-0.5">{value}</p>
    </div>
  );
}

