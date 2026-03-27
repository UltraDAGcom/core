import { useState, useCallback, useMemo } from 'react';
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
import { SmartAccountPage } from './pages/SmartAccountPage';
import { useKeystore } from './hooks/useKeystore';
import { usePasskeyWallet } from './hooks/usePasskeyWallet';
import { useNode } from './hooks/useNode';
import { useWalletBalances } from './hooks/useWalletBalances';
import { useNotifications } from './hooks/useNotifications';
import { getNodeUrl, getNetwork, switchNetwork, type NetworkType } from './lib/api';
import { ToastProvider } from './hooks/useToast';

function App() {
  const pk = usePasskeyWallet();
  const ks = useKeystore();
  const node = useNode();

  // Unified wallet list: passkey wallet (if exists) + keystore wallets
  // Memoized to prevent infinite re-render loops in useWalletBalances
  const allWallets = useMemo(() => {
    return pk.wallet
      ? [{ name: pk.wallet.name || 'Passkey Wallet', address: pk.wallet.address, secret_key: '' }, ...ks.wallets]
      : ks.wallets;
  }, [pk.wallet?.address, pk.wallet?.name, ks.wallets]);
  const isUnlocked = pk.unlocked || ks.unlocked;
  const primaryAddress = pk.wallet?.address || ks.wallets[0]?.address;

  const wb = useWalletBalances(allWallets, node.connected);
  const notifications = useNotifications({
    addresses: allWallets.map(w => w.address),
    balances: wb.balances,
    unlocked: isUnlocked,
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
    if (pk.unlocked) {
      pk.lock();
    } else if (ks.unlocked) {
      ks.lock();
    } else {
      setShowLockModal(true);
    }
  }, [pk, ks]);

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

  // Passkey wallet: if it exists but is locked, show biometric unlock (not WelcomeScreen)
  if (pk.hasWallet && !pk.unlocked && !showOnboarding) {
    return (
      <ToastProvider>
        <div className="min-h-screen bg-dag-bg">
          <header className="h-14 bg-dag-sidebar/80 backdrop-blur border-b border-dag-border flex items-center px-4 lg:px-6">
            <div className="flex items-center gap-2">
              <div className="w-7 h-7 rounded-lg bg-gradient-to-br from-dag-accent to-purple-500 flex items-center justify-center">
                <span className="text-white font-bold text-xs">U</span>
              </div>
              <span className="font-semibold text-white text-sm">UltraDAG</span>
            </div>
          </header>
          <div className="min-h-[calc(100vh-3.5rem)] flex items-center justify-center p-6">
            <div className="max-w-md w-full space-y-6 text-center">
              <div className="w-20 h-20 rounded-2xl bg-gradient-to-br from-dag-accent to-purple-500 flex items-center justify-center mx-auto shadow-lg shadow-dag-accent/20">
                <svg className="w-10 h-10 text-white" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}><path strokeLinecap="round" strokeLinejoin="round" d="M12 11c0-1.1.9-2 2-2s2 .9 2 2-.9 2-2 2-2-.9-2-2zm-6 0c0-1.1.9-2 2-2s2 .9 2 2-.9 2-2 2-2-.9-2-2zm0 0V8a4 4 0 118 0v3" /></svg>
              </div>
              <h1 className="text-2xl font-bold text-white">Welcome Back{pk.wallet?.name ? `, ${pk.wallet.name}` : ''}</h1>
              <p className="text-dag-muted text-sm">Verify your identity to unlock</p>

              <button
                onClick={async () => {
                  const ok = await pk.unlock();
                  if (!ok) {
                    // Could show error, but for now just let user retry
                  }
                }}
                className="w-full py-4 rounded-xl bg-gradient-to-r from-dag-accent to-purple-500 text-white font-semibold text-lg hover:opacity-90 transition-all flex items-center justify-center gap-2"
              >
                <svg className="w-6 h-6" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}><path strokeLinecap="round" strokeLinejoin="round" d="M12 11c0-1.1.9-2 2-2s2 .9 2 2-.9 2-2 2-2-.9-2-2zm-6 0c0-1.1.9-2 2-2s2 .9 2 2-.9 2-2 2-2-.9-2-2zm0 0V8a4 4 0 118 0v3" /></svg>
                Unlock with Biometrics
              </button>

              <button onClick={() => pk.destroy()} className="text-xs text-slate-500 hover:text-red-400 transition-colors">
                Start Fresh
              </button>
            </div>
          </div>
        </div>
      </ToastProvider>
    );
  }

  // Show welcome/onboarding when no wallet exists or keystore is locked
  if ((!pk.hasWallet && !ks.unlocked) || showOnboarding) {
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
              keystoreUnlocked={isUnlocked}
              network={network}
              walletAddress={primaryAddress}
              walletBalance={wb.totalBalance}
              sessionSecondsLeft={pk.unlocked ? 9999 : ks.sessionSecondsLeft}
              sessionTotalSeconds={pk.unlocked ? 9999 : ks.sessionTotalSeconds}
              onToggleLock={handleToggleLock}
              onSwitchNetwork={handleSwitchNetwork}
            />
          }
        >
          <Route
            index
            element={<DashboardPage status={node.status} loading={node.loading} network={network} wallets={allWallets} totalBalance={wb.totalBalance} totalStaked={wb.totalStaked} totalDelegated={wb.totalDelegated} />}
          />
          <Route
            path="wallet"
            element={
              <WalletPage
                unlocked={isUnlocked}
                hasStore={ks.hasStore}
                wallets={allWallets}
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
                unlocked={isUnlocked}
                wallets={allWallets}
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
                wallets={allWallets}
                balances={wb.balances}
                unlocked={isUnlocked}
                network={network}
              />
            }
          />
          <Route path="bridge" element={<BridgePage />} />
          <Route path="smart-account" element={<SmartAccountPage walletAddress={primaryAddress} nodeUrl={getNodeUrl()} />} />
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
