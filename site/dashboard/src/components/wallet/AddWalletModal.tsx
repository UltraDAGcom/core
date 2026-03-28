import { useState } from 'react';
import { X, Plus, Eye, EyeOff, Copy, Check, AlertTriangle } from 'lucide-react';

interface AddWalletModalProps {
  open: boolean;
  onClose: () => void;
  onGenerate: () => Promise<{ secret_key: string; address: string } | null>;
  onAdd: (name: string, secretKey: string, address: string) => Promise<void>;
}

export function AddWalletModal({ open, onClose, onGenerate, onAdd }: AddWalletModalProps) {
  const [tab, setTab] = useState<'generate' | 'import'>('generate');
  const [name, setName] = useState('');
  const [secretKey, setSecretKey] = useState('');
  const [address, setAddress] = useState('');
  const [error, setError] = useState('');
  const [loading, setLoading] = useState(false);
  const [generated, setGenerated] = useState(false);

  if (!open) return null;

  const handleGenerate = async () => {
    setError('');
    setLoading(true);
    try {
      const result = await onGenerate();
      if (result) {
        setSecretKey(result.secret_key);
        setAddress(result.address);
        setGenerated(true);
      } else {
        setError('Failed to generate keypair. Check node connection.');
      }
    } catch (err) {
      setError(String(err));
    } finally {
      setLoading(false);
    }
  };

  const handleSave = async () => {
    setError('');
    if (!name.trim()) {
      setError('Name is required');
      return;
    }
    if (!secretKey.trim() || !address.trim()) {
      setError('Secret key and address are required');
      return;
    }
    if (!/^[0-9a-fA-F]{64}$/.test(secretKey.trim())) {
      setError('Secret key must be 64 hex characters');
      return;
    }
    if (!/^[0-9a-fA-F]{40}$/.test(address.trim())) {
      setError('Address must be 40 hex characters');
      return;
    }
    setLoading(true);
    try {
      await onAdd(name.trim(), secretKey.trim().toLowerCase(), address.trim().toLowerCase());
      onClose();
      resetState();
    } catch (err) {
      setError(String(err));
    } finally {
      setLoading(false);
    }
  };

  const resetState = () => {
    setName('');
    setSecretKey('');
    setAddress('');
    setError('');
    setGenerated(false);
  };

  const handleClose = () => {
    onClose();
    resetState();
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center p-4 modal-backdrop bg-black/70">
      <div className="modal-content bg-dag-card border border-dag-border rounded-2xl shadow-2xl w-full max-w-md">
        {/* Header */}
        <div className="flex items-center justify-between p-5 border-b border-dag-border">
          <div className="flex items-center gap-2">
            <Plus className="w-5 h-5 text-dag-accent" />
            <h2 className="text-lg font-semibold text-white">Add Wallet</h2>
          </div>
          <button onClick={handleClose} className="p-1.5 rounded-lg text-slate-400 hover:text-white hover:bg-slate-700">
            <X className="w-4 h-4" />
          </button>
        </div>

        {/* Tabs */}
        <div className="flex border-b border-dag-border">
          <TabBtn active={tab === 'generate'} onClick={() => setTab('generate')} label="Generate" />
          <TabBtn active={tab === 'import'} onClick={() => setTab('import')} label="Import Key" />
        </div>

        {/* Body */}
        <div className="p-5 space-y-4">
          <div>
            <label className="text-xs text-dag-muted uppercase tracking-wider">Wallet Name</label>
            <input
              type="text"
              value={name}
              onChange={(e) => setName(e.target.value)}
              placeholder="My Wallet"
              className="mt-1 w-full px-3 py-2.5 bg-slate-800 border border-dag-border rounded-lg text-sm text-slate-200 placeholder-slate-500 focus:outline-none focus:border-dag-accent"
              autoFocus
            />
          </div>

          {tab === 'generate' && (
            <GenerateTab
              generated={generated}
              loading={loading}
              address={address}
              secretKey={secretKey}
              onGenerate={handleGenerate}
            />
          )}

          {tab === 'import' && (
            <>
              <div>
                <label className="text-xs text-dag-muted uppercase tracking-wider">Secret Key (64 hex)</label>
                <input
                  type="password"
                  value={secretKey}
                  onChange={(e) => setSecretKey(e.target.value)}
                  placeholder="Enter 64-character hex secret key"
                  className="mt-1 w-full px-3 py-2.5 bg-slate-800 border border-dag-border rounded-lg text-xs font-mono text-slate-200 placeholder-slate-500 focus:outline-none focus:border-dag-accent"
                />
              </div>
              <div>
                <label className="text-xs text-dag-muted uppercase tracking-wider">Address (64 hex)</label>
                <input
                  type="text"
                  value={address}
                  onChange={(e) => setAddress(e.target.value)}
                  placeholder="Enter 64-character hex address"
                  className="mt-1 w-full px-3 py-2.5 bg-slate-800 border border-dag-border rounded-lg text-xs font-mono text-slate-200 placeholder-slate-500 focus:outline-none focus:border-dag-accent"
                />
              </div>
              <p className="text-[10px] text-slate-500">Your key is stored encrypted on this device. It never leaves your browser.</p>
            </>
          )}

          {error && <p className="text-sm text-red-400">{error}</p>}

          <button
            onClick={handleSave}
            disabled={loading || !name.trim() || !secretKey.trim() || !address.trim()}
            className="w-full py-2.5 rounded-lg bg-dag-accent text-white font-medium text-sm hover:bg-dag-accent/80 disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
          >
            {loading ? 'Saving...' : 'Save Wallet'}
          </button>
        </div>
      </div>
    </div>
  );
}

function GenerateTab({ generated, loading, address, secretKey, onGenerate }: {
  generated: boolean; loading: boolean; address: string; secretKey: string; onGenerate: () => void;
}) {
  const [showKey, setShowKey] = useState(false);
  const [copied, setCopied] = useState(false);

  const truncAddr = address ? `${address.slice(0, 8)}...${address.slice(-6)}` : '';

  const handleCopyKey = async () => {
    try {
      await navigator.clipboard.writeText(secretKey);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    } catch { /* clipboard unavailable */ }
  };

  if (!generated) {
    return (
      <button onClick={onGenerate} disabled={loading}
        className="w-full py-2.5 rounded-lg bg-slate-700 text-slate-200 font-medium text-sm hover:bg-slate-600 disabled:opacity-50 transition-colors">
        {loading ? 'Generating...' : 'Generate Keypair'}
      </button>
    );
  }

  return (
    <>
      <div>
        <label className="text-xs text-dag-muted uppercase tracking-wider">Your Address</label>
        <p className="mt-1 text-xs font-mono text-slate-300 bg-slate-800 px-3 py-2 rounded-lg">
          {truncAddr}
        </p>
      </div>
      <div className="rounded-lg border border-amber-500/20 bg-amber-500/5 p-3 space-y-2">
        <div className="flex items-start gap-2">
          <AlertTriangle className="w-3.5 h-3.5 text-amber-400 mt-0.5 flex-shrink-0" />
          <p className="text-[11px] text-amber-400 font-medium">Save your private key. It cannot be recovered if lost.</p>
        </div>
        <div className="flex items-center gap-2">
          <button onClick={() => setShowKey(!showKey)}
            className="flex items-center gap-1.5 px-2.5 py-1.5 rounded text-[10px] font-medium bg-slate-700/60 text-slate-300 hover:bg-slate-700 hover:text-white transition-all">
            {showKey ? <EyeOff className="w-3 h-3" /> : <Eye className="w-3 h-3" />}
            {showKey ? 'Hide Key' : 'Show Private Key'}
          </button>
          <button onClick={handleCopyKey}
            className={`flex items-center gap-1.5 px-2.5 py-1.5 rounded text-[10px] font-medium transition-all ${
              copied ? 'bg-green-500/15 text-green-400' : 'bg-slate-700/60 text-slate-300 hover:bg-slate-700 hover:text-white'
            }`}>
            {copied ? <Check className="w-3 h-3" /> : <Copy className="w-3 h-3" />}
            {copied ? 'Copied!' : 'Copy Key'}
          </button>
        </div>
        {showKey && (
          <p className="text-xs font-mono text-amber-300 bg-slate-800/80 px-3 py-2 rounded break-all border border-amber-500/10">
            {secretKey}
          </p>
        )}
      </div>
    </>
  );
}

function TabBtn({ active, onClick, label }: { active: boolean; onClick: () => void; label: string }) {
  return (
    <button
      onClick={onClick}
      className={`flex-1 py-3 text-sm font-medium transition-colors ${
        active
          ? 'text-dag-accent border-b-2 border-dag-accent'
          : 'text-slate-400 hover:text-slate-200'
      }`}
    >
      {label}
    </button>
  );
}
