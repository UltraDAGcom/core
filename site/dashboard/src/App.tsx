import { useState, useCallback } from 'react';
import { Routes, Route } from 'react-router-dom';
import { Layout } from './components/layout/Layout';
import { WelcomeScreen } from './components/wallet/WelcomeScreen';
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
import { useNotifications } from './hooks/useNotifications';
import { getNodeUrl, getNetwork, switchNetwork, type NetworkType } from './lib/api';
import { ToastProvider } from './hooks/useToast';

function App() {
  const ks = useKeystore();
  const node = useNode();
  const wb = useWalletBalances(ks.wallets, node.connected);
  const notifications = useNotifications({
    addresses: ks.wallets.map(w => w.address),
    balances: wb.balances,
    unlocked: ks.unlocked,
  });
  const [showLockModal, setShowLockModal] = useState(false);
  const [network, setNetwork] = useState<NetworkType>(getNetwork());
  const [showOnboarding, setShowOnboarding] = useState(false);

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

  // Show welcome/onboarding when wallet is not unlocked, or during post-create onboarding
  if (!ks.unlocked || showOnboarding) {
    return (
      <ToastProvider>
        <div className="min-h-screen bg-dag-bg">
          {/* Minimal top bar — network is chosen during onboarding */}
          <header className="h-14 bg-dag-sidebar/80 backdrop-blur border-b border-dag-border flex items-center px-4 lg:px-6">
            <div className="flex items-center gap-2">
              <div className="w-7 h-7 rounded-lg bg-gradient-to-br from-dag-accent to-purple-500 flex items-center justify-center">
                <span className="text-white font-bold text-xs">U</span>
              </div>
              <span className="font-semibold text-white text-sm">UltraDAG</span>
            </div>
          </header>
          <WelcomeScreen
            hasExisting={ks.hasStore}
            isPostCreate={showOnboarding}
            network={network}
            onSwitchNetwork={handleSwitchNetwork}
            onCreateWallet={async (pw, name, secretKey, address) => {
              setShowOnboarding(true);
              await ks.create(pw);
              await ks.addWallet(name, secretKey, address);
              return true;
            }}
            onUnlock={ks.unlock}
            onUnlockWithWebAuthn={ks.unlockWithWebAuthn}
            onEnrollWebAuthn={ks.enrollWebAuthn}
            onExportBlob={ks.exportBlob}
            onResetWallet={ks.destroy}
            webauthnAvailable={ks.webauthnAvailable}
            webauthnEnrolled={ks.webauthnEnrolled}
            onImportBlob={ks.importBlob}
            onFinishOnboarding={() => setShowOnboarding(false)}
          />
        </div>
      </ToastProvider>
    );
  }

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
              walletAddress={ks.wallets[0]?.address}
              walletBalance={wb.totalBalance}
              sessionSecondsLeft={ks.sessionSecondsLeft}
              sessionTotalSeconds={ks.sessionTotalSeconds}
              onToggleLock={handleToggleLock}
              onSwitchNetwork={handleSwitchNetwork}
            />
          }
        >
          <Route
            index
            element={<DashboardPage status={node.status} loading={node.loading} network={network} wallets={ks.wallets} totalBalance={wb.totalBalance} totalStaked={wb.totalStaked} totalDelegated={wb.totalDelegated} />}
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
                webauthnAvailable={ks.webauthnAvailable}
                webauthnEnrolled={ks.webauthnEnrolled}
                onEnrollWebAuthn={ks.enrollWebAuthn}
                onRemoveWebAuthn={ks.removeWebAuthn}
                notificationsSupported={notifications.supported}
                notificationsEnabled={notifications.enabled}
                onToggleNotifications={notifications.toggle}
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
                network={network}
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
