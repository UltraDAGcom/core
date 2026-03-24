import { useState } from 'react';
import { X, Shield, Key } from 'lucide-react';
import { deriveAddress } from '../../lib/keygen';

interface CreateKeystoreModalProps {
  open: boolean;
  onClose: () => void;
  onCreateOrUnlock: (password: string) => Promise<boolean>;
  onCreateWithKey: (password: string, name: string, secretKey: string, address: string) => Promise<boolean>;
  onImport: (json: string) => boolean;
  hasExisting: boolean;
}

export function CreateKeystoreModal({
  open,
  onClose,
  onCreateOrUnlock,
  onCreateWithKey,
  onImport,
  hasExisting,
}: CreateKeystoreModalProps) {
  const [tab, setTab] = useState<'unlock' | 'create' | 'importkey' | 'import'>(hasExisting ? 'unlock' : 'create');
  const [password, setPassword] = useState('');
  const [confirmPassword, setConfirmPassword] = useState('');
  const [importJson, setImportJson] = useState('');
  const [importKeyHex, setImportKeyHex] = useState('');
  const [importKeyName, setImportKeyName] = useState('');
  const [derivedAddress, setDerivedAddress] = useState('');
  const [error, setError] = useState('');
  const [loading, setLoading] = useState(false);

  if (!open) return null;

  const handleKeyChange = async (hex: string) => {
    setImportKeyHex(hex);
    setDerivedAddress('');
    const clean = hex.replace(/\s/g, '').toLowerCase();
    if (/^[0-9a-f]{64}$/.test(clean)) {
      try {
        const addr = await deriveAddress(clean);
        setDerivedAddress(addr);
      } catch {
        // invalid key
      }
    }
  };

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
      } else if (tab === 'importkey') {
        if (password.length < 8) {
          setError('Password must be at least 8 characters');
          return;
        }
        if (password !== confirmPassword) {
          setError('Passwords do not match');
          return;
        }
        const clean = importKeyHex.replace(/\s/g, '').toLowerCase();
        if (!/^[0-9a-f]{64}$/.test(clean)) {
          setError('Private key must be exactly 64 hex characters');
          return;
        }
        if (!importKeyName.trim()) {
          setError('Please enter a wallet name');
          return;
        }
        const addr = await deriveAddress(clean);
        const ok = await onCreateWithKey(password, importKeyName.trim(), clean, addr);
        if (ok !== false) onClose();
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
          <TabBtn active={tab === 'importkey'} onClick={() => setTab('importkey')} label="Import Key" />
          <TabBtn active={tab === 'import'} onClick={() => setTab('import')} label="Import JSON" />
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

          {tab === 'importkey' && (
            <>
              <div className="flex items-center gap-2 mb-1">
                <Key className="w-4 h-4 text-dag-accent" />
                <p className="text-sm text-dag-muted">Import a private key and create an encrypted keystore.</p>
              </div>
              <input
                type="text"
                value={importKeyName}
                onChange={(e) => setImportKeyName(e.target.value)}
                placeholder="Wallet name"
                className="w-full px-3 py-2.5 bg-slate-800 border border-dag-border rounded-lg text-sm text-slate-200 placeholder-slate-500 focus:outline-none focus:border-dag-accent"
                autoFocus
              />
              <input
                type="password"
                value={importKeyHex}
                onChange={(e) => handleKeyChange(e.target.value)}
                placeholder="Private key (64 hex characters)"
                className="w-full px-3 py-2.5 bg-slate-800 border border-dag-border rounded-lg text-xs font-mono text-slate-200 placeholder-slate-500 focus:outline-none focus:border-dag-accent"
              />
              {derivedAddress && (
                <div className="bg-slate-800/50 rounded-lg px-3 py-2 border border-dag-border/50">
                  <p className="text-[10px] text-dag-muted uppercase tracking-wider">Derived Address</p>
                  <p className="text-xs font-mono text-dag-green break-all">{derivedAddress}</p>
                </div>
              )}
              <input
                type="password"
                value={password}
                onChange={(e) => setPassword(e.target.value)}
                placeholder="Keystore password (min 8 characters)"
                className="w-full px-3 py-2.5 bg-slate-800 border border-dag-border rounded-lg text-sm text-slate-200 placeholder-slate-500 focus:outline-none focus:border-dag-accent"
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
            {loading
              ? 'Processing...'
              : tab === 'create'
                ? 'Create Keystore'
                : tab === 'unlock'
                  ? 'Unlock'
                  : tab === 'importkey'
                    ? 'Import & Create Keystore'
                    : 'Import'}
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
