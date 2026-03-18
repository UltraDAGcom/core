import { useState } from 'react';
import { Wallet as WalletIcon, Eye, EyeOff, Trash2, Shield, AlertTriangle } from 'lucide-react';
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
    <div className="bg-dag-card border border-dag-border rounded-xl overflow-hidden">
      {/* Gradient header */}
      <div className="h-1 bg-gradient-to-r from-dag-blue via-dag-purple to-dag-accent" />
      <div className="p-5 space-y-4">
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

        {/* Prominent balance */}
        {balance && (
          <div className="py-3">
            <p className="text-xs text-dag-muted uppercase tracking-wider mb-1">Total Balance</p>
            <p className="text-3xl font-bold text-white font-mono">
              {formatUdag(balance.balance + balance.staked + balance.delegated)}
              <span className="text-base text-dag-muted font-normal ml-2">UDAG</span>
            </p>
          </div>
        )}

        {/* Balance breakdown */}
        {balance && (
          <div className="grid grid-cols-2 sm:grid-cols-4 gap-3 pt-2 border-t border-dag-border">
            <div>
              <p className="text-[10px] uppercase text-dag-muted tracking-wider">Available</p>
              <p className="text-sm font-medium text-slate-200 mt-0.5">{formatUdag(balance.balance)} UDAG</p>
            </div>
            {balance.staked > 0 && (
              <div>
                <p className="text-[10px] uppercase text-dag-green tracking-wider">Staked</p>
                <p className="text-sm font-medium text-dag-green mt-0.5">{formatUdag(balance.staked)} UDAG</p>
              </div>
            )}
            {balance.delegated > 0 && (
              <div>
                <p className="text-[10px] uppercase text-dag-blue tracking-wider">Delegated</p>
                <p className="text-sm font-medium text-dag-blue mt-0.5">{formatUdag(balance.delegated)} UDAG</p>
              </div>
            )}
            <div>
              <p className="text-[10px] uppercase text-dag-muted tracking-wider">Nonce</p>
              <p className="text-sm font-mono text-dag-muted mt-0.5">{balance.nonce}</p>
            </div>
          </div>
        )}

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
          {showKey && (
            <div className="flex items-center gap-2 mt-1.5 mb-1.5 px-3 py-2 rounded-lg bg-red-950/40 border border-red-500/20">
              <AlertTriangle className="w-3.5 h-3.5 text-red-400 shrink-0" />
              <span className="text-[11px] text-red-300">Never share your secret key. Anyone with it controls your funds.</span>
            </div>
          )}
          <div className={`flex items-center gap-2 mt-1 ${showKey ? 'bg-red-950/20 border border-red-500/10 rounded-lg p-1' : ''}`}>
            <code className="text-xs font-mono text-slate-300 bg-slate-800 px-3 py-2 rounded-lg break-all flex-1">
              {showKey ? secretKey : '************************************************************'}
            </code>
            <button
              onClick={() => setShowKey(!showKey)}
              className="p-2 rounded-lg text-slate-400 hover:text-slate-200 hover:bg-slate-700 transition-colors"
              title={showKey ? 'Hide key' : 'Reveal key'}
            >
              {showKey ? <EyeOff className="w-4 h-4" /> : <Eye className="w-4 h-4" />}
            </button>
            {showKey && <CopyButton text={secretKey} />}
          </div>
        </div>
      </div>
    </div>
  );
}
