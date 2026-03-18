import { useState } from 'react';
import { Plus, Download, Wallet as WalletIcon, ShieldAlert } from 'lucide-react';
import { WalletCard, WalletDetail } from '../components/wallet/WalletCard';
import { CreateKeystoreModal } from '../components/wallet/CreateKeystoreModal';
import { AddWalletModal } from '../components/wallet/AddWalletModal';
import { Pagination } from '../components/shared/Pagination';
import type { Wallet } from '../lib/keystore';
import type { WalletBalance } from '../hooks/useWalletBalances';

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
    </div>
  );
}
