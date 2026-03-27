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
import { StreamsPage } from './pages/StreamsPage';
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

  // Passkey wallet: if it exists but is locked, show biometric unlock
  if (pk.hasWallet && !pk.unlocked && !showOnboarding) {
    return (
      <ToastProvider>
        <div style={{ minHeight: '100vh', background: '#080C14', fontFamily: "'DM Sans',sans-serif", display: 'flex', flexDirection: 'column' }}>
          <style>{`@keyframes slideUp{from{opacity:0;transform:translateY(12px)}to{opacity:1;transform:translateY(0)}} @keyframes glow{0%,100%{box-shadow:0 0 20px rgba(0,224,196,0.15)}50%{box-shadow:0 0 40px rgba(0,224,196,0.3)}} @keyframes pulse{0%,100%{opacity:1}50%{opacity:.5}}`}</style>
          <div style={{ flex: 1, display: 'flex', alignItems: 'center', justifyContent: 'center' }}>
            <div style={{ maxWidth: 380, width: '100%', textAlign: 'center', padding: '0 20px', animation: 'slideUp 0.5s ease' }}>
              {/* Logo */}
              <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'center', gap: 10, marginBottom: 40, opacity: 0.6 }}>
                <div style={{ width: 28, height: 28, borderRadius: 7, background: 'linear-gradient(135deg,#00E0C4,#0066FF)', display: 'flex', alignItems: 'center', justifyContent: 'center', fontSize: 12, fontWeight: 800, color: '#fff' }}>U</div>
                <span style={{ fontSize: 13, fontWeight: 700, letterSpacing: 1.5, color: '#fff' }}>ULTRADAG</span>
              </div>

              {/* Biometric icon */}
              <div style={{
                width: 88, height: 88, borderRadius: 22, margin: '0 auto 24px',
                background: 'linear-gradient(135deg, rgba(0,224,196,0.08), rgba(0,102,255,0.08))',
                border: '1px solid rgba(0,224,196,0.15)',
                display: 'flex', alignItems: 'center', justifyContent: 'center',
                animation: 'glow 3s ease-in-out infinite',
              }}>
                <span style={{ fontSize: 38 }}>◎</span>
              </div>

              <h1 style={{ fontSize: 22, fontWeight: 700, color: '#fff', marginBottom: 6 }}>
                Welcome back{pk.wallet?.name ? `, ${pk.wallet.name}` : ''}
              </h1>
              <p style={{ fontSize: 12, color: 'rgba(255,255,255,0.25)', marginBottom: 28 }}>
                Verify your identity to unlock your wallet
              </p>

              <button onClick={async () => { await pk.unlock(); }} style={{
                width: '100%', padding: '14px 0', borderRadius: 12,
                background: 'linear-gradient(135deg, #00E0C4, #0066FF)',
                color: '#fff', fontSize: 14, fontWeight: 700, cursor: 'pointer', border: 'none',
                boxShadow: '0 4px 20px rgba(0,224,196,0.2)',
                transition: 'opacity 0.2s',
                display: 'flex', alignItems: 'center', justifyContent: 'center', gap: 8,
              }}>
                ◎ Unlock with Biometrics
              </button>

              <button onClick={() => pk.destroy()} style={{
                background: 'none', border: 'none', color: 'rgba(255,255,255,0.15)',
                fontSize: 11, cursor: 'pointer', marginTop: 20, transition: 'color 0.2s',
              }}>
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
        <div style={{ minHeight: '100vh', background: '#080C14', fontFamily: "'DM Sans',sans-serif" }}>
          <style>{`@keyframes slideUp{from{opacity:0;transform:translateY(12px)}to{opacity:1;transform:translateY(0)}} @keyframes glow{0%,100%{box-shadow:0 0 12px rgba(0,224,196,0.15)}50%{box-shadow:0 0 20px rgba(0,224,196,0.3)}}`}</style>
          {/* Minimal top bar */}
          <header style={{
            height: 52, display: 'flex', alignItems: 'center', padding: '0 20px',
            borderBottom: '1px solid rgba(255,255,255,0.03)',
          }}>
            <div style={{ display: 'flex', alignItems: 'center', gap: 9 }}>
              <div style={{
                width: 28, height: 28, borderRadius: 7,
                background: 'linear-gradient(135deg,#00E0C4,#0066FF)',
                display: 'flex', alignItems: 'center', justifyContent: 'center',
                fontSize: 12, fontWeight: 800, color: '#fff',
                animation: 'glow 4s ease-in-out infinite',
              }}>U</div>
              <span style={{ fontSize: 13, fontWeight: 700, letterSpacing: 1.2, color: '#fff' }}>ULTRADAG</span>
            </div>
          </header>
          <WelcomeScreen
            hasExisting={ks.hasStore}
            isPostCreate={showOnboarding}
            network={network}
            onSwitchNetwork={handleSwitchNetwork}
            onCreateWallet={async (pw, name, secretKey, address) => {
              setShowOnboarding(true);
              if (ks.hasStore) {
                // Unlock existing keystore — preserves all previous wallets
                const ok = await ks.unlock(pw);
                if (!ok) return false;
              } else {
                await ks.create(pw);
              }
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
          <Route path="streams" element={<StreamsPage wallets={allWallets} network={network} />} />
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
          if (ks.hasStore) {
            const ok = await ks.unlock(pw);
            if (!ok) return false;
          } else {
            await ks.create(pw);
          }
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
