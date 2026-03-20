import { useState } from 'react';
import { X, Plus } from 'lucide-react';

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
            <>
              {!generated ? (
                <button
                  onClick={handleGenerate}
                  disabled={loading}
                  className="w-full py-2.5 rounded-lg bg-slate-700 text-slate-200 font-medium text-sm hover:bg-slate-600 disabled:opacity-50 transition-colors"
                >
                  {loading ? 'Generating...' : 'Generate Keypair'}
                </button>
              ) : (
                <>
                  <div>
                    <label className="text-xs text-dag-muted uppercase tracking-wider">Address</label>
                    <p className="mt-1 text-xs font-mono text-slate-300 bg-slate-800 px-3 py-2 rounded-lg break-all">
                      {address}
                    </p>
                  </div>
                  <div>
                    <label className="text-xs text-dag-muted uppercase tracking-wider">Secret Key</label>
                    <p className="mt-1 text-xs font-mono text-yellow-400 bg-slate-800 px-3 py-2 rounded-lg break-all">
                      {secretKey}
                    </p>
                    <p className="mt-1 text-[11px] text-yellow-500">
                      Save this key securely. It cannot be recovered if lost.
                    </p>
                  </div>
                </>
              )}
            </>
          )}

          {tab === 'import' && (
            <>
              <div>
                <label className="text-xs text-dag-muted uppercase tracking-wider">Secret Key (64 hex)</label>
                <input
                  type="text"
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
