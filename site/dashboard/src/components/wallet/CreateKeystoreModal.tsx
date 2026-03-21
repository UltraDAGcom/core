import { useState } from 'react';
import { X, Shield } from 'lucide-react';

interface CreateKeystoreModalProps {
  open: boolean;
  onClose: () => void;
  onCreateOrUnlock: (password: string) => Promise<boolean>;
  onImport: (json: string) => boolean;
  hasExisting: boolean;
}

export function CreateKeystoreModal({
  open,
  onClose,
  onCreateOrUnlock,
  onImport,
  hasExisting,
}: CreateKeystoreModalProps) {
  const [tab, setTab] = useState<'unlock' | 'create' | 'import'>(hasExisting ? 'unlock' : 'create');
  const [password, setPassword] = useState('');
  const [confirmPassword, setConfirmPassword] = useState('');
  const [importJson, setImportJson] = useState('');
  const [error, setError] = useState('');
  const [loading, setLoading] = useState(false);

  if (!open) return null;

  const handleSubmit = async () => {
    setError('');
    setLoading(true);
    try {
      if (tab === 'create') {
        if (password.length < 8) {
          setError('Password must be at least 8 characters');
          return;
        }
        if (password !== confirmPassword) {
          setError('Passwords do not match');
          return;
        }
        const ok = await onCreateOrUnlock(password);
        if (ok !== false) onClose();
      } else if (tab === 'unlock') {
        const ok = await onCreateOrUnlock(password);
        if (!ok) setError('Incorrect password');
        else onClose();
      } else if (tab === 'import') {
        const ok = onImport(importJson);
        if (!ok) setError('Invalid keystore JSON');
        else {
          setTab('unlock');
          setError('');
        }
      }
    } catch (err) {
      setError(String(err));
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center p-4 modal-backdrop bg-black/70">
      <div className="modal-content bg-dag-card border border-dag-border rounded-2xl shadow-2xl w-full max-w-md">
        {/* Header */}
        <div className="flex items-center justify-between p-5 border-b border-dag-border">
          <div className="flex items-center gap-2">
            <Shield className="w-5 h-5 text-dag-accent" />
            <h2 className="text-lg font-semibold text-white">Keystore</h2>
          </div>
          <button onClick={onClose} className="p-1.5 rounded-lg text-slate-400 hover:text-white hover:bg-slate-700">
            <X className="w-4 h-4" />
          </button>
        </div>

        {/* Tabs */}
        <div className="flex border-b border-dag-border">
          {hasExisting && (
            <TabBtn active={tab === 'unlock'} onClick={() => setTab('unlock')} label="Unlock" />
          )}
          <TabBtn active={tab === 'create'} onClick={() => setTab('create')} label="Create New" />
          <TabBtn active={tab === 'import'} onClick={() => setTab('import')} label="Import" />
        </div>

        {/* Body */}
        <div className="p-5 space-y-4">
          {tab === 'unlock' && (
            <>
              <p className="text-sm text-dag-muted">Enter your password to unlock the keystore.</p>
              <input
                type="password"
                value={password}
                onChange={(e) => setPassword(e.target.value)}
                placeholder="Password"
                className="w-full px-3 py-2.5 bg-slate-800 border border-dag-border rounded-lg text-sm text-slate-200 placeholder-slate-500 focus:outline-none focus:border-dag-accent"
                onKeyDown={(e) => e.key === 'Enter' && handleSubmit()}
                autoFocus
              />
            </>
          )}

          {tab === 'create' && (
            <>
              <p className="text-sm text-dag-muted">
                Create a new encrypted keystore. Your wallets will be protected with AES-256-GCM encryption.
              </p>
              <input
                type="password"
                value={password}
                onChange={(e) => setPassword(e.target.value)}
                placeholder="Password (min 8 characters)"
                className="w-full px-3 py-2.5 bg-slate-800 border border-dag-border rounded-lg text-sm text-slate-200 placeholder-slate-500 focus:outline-none focus:border-dag-accent"
                autoFocus
              />
              <input
                type="password"
                value={confirmPassword}
                onChange={(e) => setConfirmPassword(e.target.value)}
                placeholder="Confirm password"
                className="w-full px-3 py-2.5 bg-slate-800 border border-dag-border rounded-lg text-sm text-slate-200 placeholder-slate-500 focus:outline-none focus:border-dag-accent"
                onKeyDown={(e) => e.key === 'Enter' && handleSubmit()}
              />
            </>
          )}

          {tab === 'import' && (
            <>
              <p className="text-sm text-dag-muted">
                Paste a previously exported keystore JSON blob.
              </p>
              <textarea
                value={importJson}
                onChange={(e) => setImportJson(e.target.value)}
                placeholder='{"version":1,"kdf":"pbkdf2-sha256",...}'
                rows={5}
                className="w-full px-3 py-2.5 bg-slate-800 border border-dag-border rounded-lg text-xs font-mono text-slate-200 placeholder-slate-500 focus:outline-none focus:border-dag-accent resize-none"
                autoFocus
              />
            </>
          )}

          {error && <p className="text-sm text-red-400">{error}</p>}

          <button
            onClick={handleSubmit}
            disabled={loading}
            className="w-full py-2.5 rounded-lg bg-dag-accent text-white font-medium text-sm hover:bg-dag-accent/80 disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
          >
            {loading ? 'Processing...' : tab === 'create' ? 'Create Keystore' : tab === 'unlock' ? 'Unlock' : 'Import'}
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
