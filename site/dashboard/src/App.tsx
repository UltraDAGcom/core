import { useState, useCallback } from 'react';
import { Routes, Route } from 'react-router-dom';
import { Layout } from './components/layout/Layout';
import { CreateKeystoreModal } from './components/wallet/CreateKeystoreModal';
import { DashboardPage } from './pages/DashboardPage';
import { WalletPage } from './pages/WalletPage';
import { PortfolioPage } from './pages/PortfolioPage';
import { SendPage } from './pages/SendPage';
import { StakingPage } from './pages/StakingPage';
import { GovernancePage } from './pages/GovernancePage';
import { CouncilPage } from './pages/CouncilPage';
import { ExplorerPage } from './pages/ExplorerPage';
import { NetworkPage } from './pages/NetworkPage';
import { RoundDetailPage } from './pages/RoundDetailPage';
import { TxDetailPage } from './pages/TxDetailPage';
import { VertexDetailPage } from './pages/VertexDetailPage';
import { AddressPage } from './pages/AddressPage';
import { SearchResultPage } from './pages/SearchResultPage';
import { BridgePage } from './pages/BridgePage';
import { useKeystore } from './hooks/useKeystore';
import { useNode } from './hooks/useNode';
import { useWalletBalances } from './hooks/useWalletBalances';
import { getNodeUrl, getNetwork, switchNetwork, type NetworkType } from './lib/api';
import { ToastProvider } from './hooks/useToast';

function App() {
  const ks = useKeystore();
  const node = useNode();
  const wb = useWalletBalances(ks.wallets, node.connected);
  const [showLockModal, setShowLockModal] = useState(false);
  const [network, setNetwork] = useState<NetworkType>(getNetwork());

  const handleSwitchNetwork = useCallback((net: NetworkType) => {
    switchNetwork(net);
    setNetwork(net);
    // Reconnect to the new network's nodes
    node.reconnect();
  }, [node]);

  const handleToggleLock = useCallback(() => {
    if (ks.unlocked) {
      ks.lock();
    } else {
      setShowLockModal(true);
    }
  }, [ks]);

  const handleGenerateKeypair = useCallback(async () => {
    try {
      // Client-side key generation — keys never leave the browser
      const { generateKeypair } = await import('./lib/keygen');
      return generateKeypair();
    } catch {
      // Fallback to server keygen (testnet only)
      try {
        const res = await fetch(getNodeUrl() + '/keygen', {
          signal: AbortSignal.timeout(5000),
        });
        if (!res.ok) return null;
        return res.json();
      } catch {
        return null;
      }
    }
  }, []);

  return (
    <ToastProvider>
      <Routes>
        <Route
          element={
            <Layout
              connected={node.connected}
              nodeUrl={node.nodeUrl}
              keystoreUnlocked={ks.unlocked}
              network={network}
              onToggleLock={handleToggleLock}
              onSwitchNetwork={handleSwitchNetwork}
            />
          }
        >
          <Route
            index
            element={<DashboardPage status={node.status} loading={node.loading} network={network} />}
          />
          <Route
            path="wallet"
            element={
              <WalletPage
                unlocked={ks.unlocked}
                hasStore={ks.hasStore}
                wallets={ks.wallets}
                balances={wb.balances}
                onCreate={ks.create}
                onUnlock={ks.unlock}
                onImportBlob={ks.importBlob}
                onAddWallet={ks.addWallet}
                onRemoveWallet={ks.removeWallet}
                onExportBlob={ks.exportBlob}
                onGenerateKeypair={handleGenerateKeypair}
              />
            }
          />
          <Route
            path="wallet/portfolio"
            element={
              <PortfolioPage
                unlocked={ks.unlocked}
                wallets={ks.wallets}
                balances={wb.balances}
                totalBalance={wb.totalBalance}
                totalStaked={wb.totalStaked}
                totalDelegated={wb.totalDelegated}
              />
            }
          />
          <Route
            path="wallet/send"
            element={
              <SendPage
                wallets={ks.wallets}
                balances={wb.balances}
                unlocked={ks.unlocked}
              />
            }
          />
          <Route path="bridge" element={<BridgePage />} />
          <Route path="staking" element={<StakingPage />} />
          <Route path="governance" element={<GovernancePage />} />
          <Route path="council" element={<CouncilPage />} />
          <Route path="explorer" element={<ExplorerPage />} />
          <Route path="network" element={<NetworkPage />} />
          <Route path="round/:round" element={<RoundDetailPage />} />
          <Route path="tx/:hash" element={<TxDetailPage />} />
          <Route path="vertex/:hash" element={<VertexDetailPage />} />
          <Route path="address/:address" element={<AddressPage />} />
          <Route path="search/:query" element={<SearchResultPage />} />
          <Route path="*" element={
            <div className="flex flex-col items-center justify-center h-64 text-center">
              <h1 className="text-4xl font-bold text-white mb-2">404</h1>
              <p className="text-dag-muted">Page not found</p>
              <a href="/" className="text-dag-accent mt-4 hover:underline">Go to Dashboard</a>
            </div>
          } />
        </Route>
      </Routes>

      <CreateKeystoreModal
        open={showLockModal}
        onClose={() => setShowLockModal(false)}
        onCreateOrUnlock={async (pw) => {
          if (ks.hasStore) {
            return ks.unlock(pw);
          } else {
            await ks.create(pw);
            return true;
          }
        }}
        onCreateWithKey={async (pw, name, secretKey, address) => {
          await ks.create(pw);
          await ks.addWallet(name, secretKey, address);
          return true;
        }}
        onImport={ks.importBlob}
        hasExisting={ks.hasStore}
      />
    </ToastProvider>
  );
}

export default App;
