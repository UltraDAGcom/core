import { useState } from 'react';
import { Plus, Download, KeyRound, Wallet as WalletIcon, ShieldAlert, X } from 'lucide-react';
import { WalletCard, WalletDetail } from '../components/wallet/WalletCard';
import { CreateKeystoreModal } from '../components/wallet/CreateKeystoreModal';
import { AddWalletModal } from '../components/wallet/AddWalletModal';
import { Pagination } from '../components/shared/Pagination';
import { changePassword } from '../lib/keystore';
import type { Wallet } from '../lib/keystore';
import type { WalletBalance } from '../hooks/useWalletBalances';

function ChangePasswordModal({ open, onClose }: { open: boolean; onClose: () => void }) {
  const [currentPw, setCurrentPw] = useState('');
  const [newPw, setNewPw] = useState('');
  const [confirmPw, setConfirmPw] = useState('');
  const [error, setError] = useState('');
  const [success, setSuccess] = useState(false);
  const [loading, setLoading] = useState(false);

  if (!open) return null;

  const handleClose = () => {
    setCurrentPw('');
    setNewPw('');
    setConfirmPw('');
    setError('');
    setSuccess(false);
    onClose();
  };

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setError('');
    setSuccess(false);

    if (newPw.length < 8) {
      setError('New password must be at least 8 characters.');
      return;
    }
    if (newPw !== confirmPw) {
      setError('New passwords do not match.');
      return;
    }
    if (currentPw === newPw) {
      setError('New password must be different from current password.');
      return;
    }

    setLoading(true);
    try {
      const ok = await changePassword(currentPw, newPw);
      if (ok) {
        setSuccess(true);
        setCurrentPw('');
        setNewPw('');
        setConfirmPw('');
      } else {
        setError('Current password is incorrect.');
      }
    } catch {
      setError('Failed to change password.');
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-sm">
      <div className="bg-dag-card border border-dag-border rounded-xl shadow-2xl w-full max-w-md p-6">
        <div className="flex items-center justify-between mb-5">
          <h2 className="text-lg font-semibold text-white">Change Password</h2>
          <button onClick={handleClose} className="text-dag-muted hover:text-white transition-colors">
            <X className="w-5 h-5" />
          </button>
        </div>
        <form onSubmit={handleSubmit} className="space-y-4">
          <div>
            <label className="block text-xs text-dag-muted mb-1">Current Password</label>
            <input
              type="password"
              value={currentPw}
              onChange={(e) => setCurrentPw(e.target.value)}
              className="w-full px-3 py-2 rounded-lg bg-dag-bg border border-dag-border text-white text-sm focus:outline-none focus:border-dag-accent"
              autoFocus
              required
            />
          </div>
          <div>
            <label className="block text-xs text-dag-muted mb-1">New Password (min 8 characters)</label>
            <input
              type="password"
              value={newPw}
              onChange={(e) => setNewPw(e.target.value)}
              className="w-full px-3 py-2 rounded-lg bg-dag-bg border border-dag-border text-white text-sm focus:outline-none focus:border-dag-accent"
              required
              minLength={8}
            />
          </div>
          <div>
            <label className="block text-xs text-dag-muted mb-1">Confirm New Password</label>
            <input
              type="password"
              value={confirmPw}
              onChange={(e) => setConfirmPw(e.target.value)}
              className="w-full px-3 py-2 rounded-lg bg-dag-bg border border-dag-border text-white text-sm focus:outline-none focus:border-dag-accent"
              required
              minLength={8}
            />
          </div>
          {error && (
            <div className="text-red-400 text-xs bg-red-500/10 border border-red-500/20 rounded-lg px-3 py-2">
              {error}
            </div>
          )}
          {success && (
            <div className="text-green-400 text-xs bg-green-500/10 border border-green-500/20 rounded-lg px-3 py-2">
              Password changed successfully.
            </div>
          )}
          <div className="flex justify-end gap-3 pt-2">
            <button
              type="button"
              onClick={handleClose}
              className="px-4 py-2 rounded-lg bg-slate-700 text-slate-200 text-sm font-medium hover:bg-slate-600 transition-colors"
            >
              Cancel
            </button>
            <button
              type="submit"
              disabled={loading}
              className="px-4 py-2 rounded-lg bg-dag-accent text-white text-sm font-medium hover:bg-dag-accent/80 transition-colors disabled:opacity-50"
            >
              {loading ? 'Changing...' : 'Change Password'}
            </button>
          </div>
        </form>
      </div>
    </div>
  );
}

const WALLETS_PER_PAGE = 8;

interface WalletPageProps {
  unlocked: boolean;
  hasStore: boolean;
  wallets: Wallet[];
  balances: Map<string, WalletBalance>;
  onCreate: (password: string) => Promise<void>;
  onUnlock: (password: string) => Promise<boolean>;
  onImportBlob: (json: string) => boolean;
  onAddWallet: (name: string, secretKey: string, address: string) => Promise<void>;
  onRemoveWallet: (index: number) => Promise<void>;
  onExportBlob: () => string | null;
  onGenerateKeypair: () => Promise<{ secret_key: string; address: string } | null>;
}

export function WalletPage({
  unlocked,
  hasStore,
  wallets,
  balances,
  onCreate,
  onUnlock,
  onImportBlob,
  onAddWallet,
  onRemoveWallet,
  onExportBlob,
  onGenerateKeypair,
}: WalletPageProps) {
  const [showKeystoreModal, setShowKeystoreModal] = useState(false);
  const [showAddModal, setShowAddModal] = useState(false);
  const [showChangePwModal, setShowChangePwModal] = useState(false);
  const [selectedWallet, setSelectedWallet] = useState<number | null>(null);
  const [page, setPage] = useState(1);

  // If not unlocked, show locked state
  if (!unlocked) {
    return (
      <div className="space-y-6">
        <div>
          <h1 className="text-2xl font-bold text-white">Wallet</h1>
          <p className="text-sm text-dag-muted mt-1">Manage your UltraDAG wallets</p>
        </div>

        <div className="flex flex-col items-center justify-center py-20 space-y-6">
          <div className="w-20 h-20 rounded-2xl bg-dag-card border border-dag-border flex items-center justify-center">
            <ShieldAlert className="w-10 h-10 text-dag-muted" />
          </div>
          <div className="text-center">
            <h2 className="text-lg font-semibold text-white">Keystore Locked</h2>
            <p className="text-sm text-dag-muted mt-1 max-w-sm">
              {hasStore
                ? 'Unlock your keystore to access your wallets.'
                : 'Create a new keystore to get started, or import an existing one.'}
            </p>
          </div>
          <div className="flex gap-3">
            {hasStore ? (
              <button
                onClick={() => setShowKeystoreModal(true)}
                className="px-5 py-2.5 rounded-lg bg-dag-accent text-white font-medium text-sm hover:bg-dag-accent/80 transition-colors"
              >
                Unlock Keystore
              </button>
            ) : (
              <>
                <button
                  onClick={() => setShowKeystoreModal(true)}
                  className="px-5 py-2.5 rounded-lg bg-dag-accent text-white font-medium text-sm hover:bg-dag-accent/80 transition-colors"
                >
                  Create Keystore
                </button>
                <button
                  onClick={() => setShowKeystoreModal(true)}
                  className="px-5 py-2.5 rounded-lg bg-slate-700 text-slate-200 font-medium text-sm hover:bg-slate-600 transition-colors"
                >
                  Import Keystore
                </button>
              </>
            )}
          </div>
        </div>

        <CreateKeystoreModal
          open={showKeystoreModal}
          onClose={() => setShowKeystoreModal(false)}
          onCreateOrUnlock={async (pw) => {
            if (hasStore) {
              return onUnlock(pw);
            } else {
              await onCreate(pw);
              return true;
            }
          }}
          onImport={onImportBlob}
          hasExisting={hasStore}
        />
      </div>
    );
  }

  // Unlocked state
  const totalPages = Math.ceil(wallets.length / WALLETS_PER_PAGE);
  const pagedWallets = wallets.slice((page - 1) * WALLETS_PER_PAGE, page * WALLETS_PER_PAGE);
  const selected = selectedWallet !== null ? wallets[selectedWallet] : null;

  const handleExport = () => {
    const json = onExportBlob();
    if (json) {
      const blob = new Blob([json], { type: 'application/json' });
      const url = URL.createObjectURL(blob);
      const a = document.createElement('a');
      a.href = url;
      a.download = 'ultradag-keystore.json';
      a.click();
      URL.revokeObjectURL(url);
    }
  };

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-bold text-white">Wallet</h1>
          <p className="text-sm text-dag-muted mt-1">{wallets.length} wallet{wallets.length !== 1 ? 's' : ''}</p>
        </div>
        <div className="flex gap-2">
          <button
            onClick={() => setShowChangePwModal(true)}
            className="flex items-center gap-2 px-3 py-2 rounded-lg bg-slate-700 text-slate-200 text-xs font-medium hover:bg-slate-600 transition-colors"
          >
            <KeyRound className="w-3.5 h-3.5" />
            Change Password
          </button>
          <button
            onClick={handleExport}
            className="flex items-center gap-2 px-3 py-2 rounded-lg bg-slate-700 text-slate-200 text-xs font-medium hover:bg-slate-600 transition-colors"
          >
            <Download className="w-3.5 h-3.5" />
            Export
          </button>
          <button
            onClick={() => setShowAddModal(true)}
            className="flex items-center gap-2 px-4 py-2 rounded-lg bg-dag-accent text-white text-sm font-medium hover:bg-dag-accent/80 transition-colors"
          >
            <Plus className="w-4 h-4" />
            Add Wallet
          </button>
        </div>
      </div>

      {wallets.length === 0 ? (
        <div className="flex flex-col items-center justify-center py-16 space-y-4">
          <WalletIcon className="w-12 h-12 text-slate-600" />
          <p className="text-dag-muted text-sm">No wallets yet. Add one to get started.</p>
          <button
            onClick={() => setShowAddModal(true)}
            className="px-5 py-2.5 rounded-lg bg-dag-accent text-white font-medium text-sm hover:bg-dag-accent/80 transition-colors"
          >
            Add Wallet
          </button>
        </div>
      ) : (
        <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
          {/* Wallet list */}
          <div className="space-y-3">
            {pagedWallets.map((w, i) => {
              const globalIndex = (page - 1) * WALLETS_PER_PAGE + i;
              return (
                <WalletCard
                  key={w.address}
                  name={w.name}
                  address={w.address}
                  balance={balances.get(w.address)}
                  selected={selectedWallet === globalIndex}
                  onClick={() => setSelectedWallet(selectedWallet === globalIndex ? null : globalIndex)}
                />
              );
            })}
            <Pagination currentPage={page} totalPages={totalPages} onPageChange={setPage} />
          </div>

          {/* Wallet detail */}
          <div>
            {selected ? (
              <WalletDetail
                name={selected.name}
                address={selected.address}
                secretKey={selected.secret_key}
                balance={balances.get(selected.address)}
                onRemove={() => {
                  if (selectedWallet !== null) {
                    onRemoveWallet(selectedWallet);
                    setSelectedWallet(null);
                  }
                }}
              />
            ) : (
              <div className="bg-dag-card border border-dag-border rounded-xl p-8 flex flex-col items-center justify-center text-center h-full min-h-[200px]">
                <WalletIcon className="w-8 h-8 text-slate-600 mb-3" />
                <p className="text-sm text-dag-muted">Select a wallet to view details</p>
              </div>
            )}
          </div>
        </div>
      )}

      <AddWalletModal
        open={showAddModal}
        onClose={() => setShowAddModal(false)}
        onGenerate={onGenerateKeypair}
        onAdd={onAddWallet}
      />

      <ChangePasswordModal
        open={showChangePwModal}
        onClose={() => setShowChangePwModal(false)}
      />
    </div>
  );
}
