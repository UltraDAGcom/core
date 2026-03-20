import { useState } from 'react';
import { ArrowRightLeft, ArrowRight, ExternalLink, Shield, Clock, AlertTriangle, Info, Wallet, Unplug, Loader2, CheckCircle } from 'lucide-react';
import { Card } from '../components/shared/Card.tsx';
import { useKeystore } from '../hooks/useKeystore.ts';
import { useEthWallet } from '../hooks/useEthWallet.ts';
import { useToast } from '../hooks/useToast.tsx';
import { fullAddr, normalizeAddress, isValidAddress } from '../lib/api.ts';
import { WalletSelector } from '../components/shared/WalletSelector.tsx';
import { CopyButton } from '../components/shared/CopyButton.tsx';
import { CONTRACTS_DEPLOYED } from '../lib/contracts.ts';
import { formatUnits } from 'ethers';

export function BridgePage() {
  const { wallets, unlocked } = useKeystore();
  const eth = useEthWallet();
  const { toast } = useToast();

  const [direction, setDirection] = useState<'to-native' | 'to-arbitrum'>('to-native');
  const [amount, setAmount] = useState('');
  const [arbAddress, setArbAddress] = useState('');
  const [nativeAddress, setNativeAddress] = useState('');
  const [selectedWalletIdx, setSelectedWalletIdx] = useState(0);
  const [bridging, setBridging] = useState(false);
  const [approving, setApproving] = useState(false);
  const [txHash, setTxHash] = useState('');

  const wallet = wallets[selectedWalletIdx];
  const bridgeActive = eth.contractsDeployed ? eth.bridgeActive : false;
  const bridgePaused = eth.contractsDeployed ? eth.bridgePaused : false;
  const canBridge = bridgeActive && !bridgePaused;

  // Parse amount to sats
  const amountSats = (() => {
    try { return eth.parseUdag(amount || '0'); } catch { return 0n; }
  })();
  const needsApproval = eth.connected && amountSats > 0n && eth.udagAllowance < amountSats;

  const handleApprove = async () => {
    setApproving(true);
    const ok = await eth.approve(amountSats);
    setApproving(false);
    if (ok) toast('Approval confirmed', 'success');
  };

  const handleBridgeToNative = async () => {
    if (!eth.connected) return;
    const recipient = wallet ? normalizeAddress(wallet.address) : normalizeAddress(nativeAddress);
    if (!recipient || (!wallet && !isValidAddress(nativeAddress))) {
      toast('Invalid UltraDAG recipient address', 'error');
      return;
    }
    if (amountSats <= 0n) { toast('Enter a valid amount', 'error'); return; }

    setBridging(true);
    setTxHash('');
    const hash = await eth.bridgeToNative(recipient, amountSats);
    setBridging(false);
    if (hash) {
      setTxHash(hash);
      setAmount('');
      toast('Bridge transfer submitted! Tokens escrowed.', 'success');
    } else if (eth.error) {
      toast(eth.error, 'error');
    }
  };


  // Format bridge stats from contract or defaults
  const dailyCap = eth.contractsDeployed && eth.dailyCap > 0n ? eth.dailyCap : 50000000000000n; // 500k UDAG
  const maxPerTx = eth.contractsDeployed && eth.maxPerTx > 0n ? eth.maxPerTx : 10000000000000n; // 100k UDAG
  const dailyVolume = eth.dailyVolume;

  return (
    <div className="space-y-6 animate-page-enter">
      <div>
        <h1 className="text-2xl font-bold text-white">Bridge</h1>
        <p className="text-sm text-dag-muted mt-1">Transfer UDAG between Arbitrum and UltraDAG native chain</p>
      </div>

      {/* Status Banner */}
      <div className="bg-dag-surface border border-dag-border rounded-xl p-4">
        <div className="flex flex-wrap items-center gap-3">
          <div className={`w-3 h-3 rounded-full ${canBridge ? 'bg-dag-green animate-pulse' : bridgePaused ? 'bg-dag-red' : bridgeActive ? 'bg-dag-yellow' : 'bg-dag-yellow'}`} />
          <div className="flex-1 min-w-[200px]">
            <span className={`text-sm font-medium ${canBridge ? 'text-dag-green' : 'text-dag-yellow'}`}>
              {canBridge ? 'Bridge Active' : bridgePaused ? 'Bridge Paused' : CONTRACTS_DEPLOYED ? 'Bridge Inactive' : 'Phase 1: Token Only'}
            </span>
            <p className="text-xs text-dag-muted mt-0.5">
              {canBridge
                ? 'Transfer UDAG between Arbitrum and the native chain.'
                : CONTRACTS_DEPLOYED
                  ? 'Bridge contracts deployed. Waiting for activation.'
                  : 'Contracts not yet deployed. Buy UDAG on Arbitrum via Uniswap.'}
            </p>
          </div>
          {eth.connected && (
            <div className="flex items-center gap-2">
              <div className="w-2 h-2 rounded-full bg-dag-green" />
              <span className="text-xs text-dag-muted font-mono">{eth.address.slice(0, 6)}...{eth.address.slice(-4)}</span>
            </div>
          )}
        </div>
      </div>

      <div className="grid grid-cols-1 lg:grid-cols-3 gap-6">
        {/* Left: Bridge Form */}
        <div className="lg:col-span-2 space-y-6">
          <Card>
            <div className="space-y-5">
              <div className="flex items-center gap-3">
                <ArrowRightLeft className="w-5 h-5 text-dag-accent" />
                <h2 className="text-lg font-semibold text-white">Bridge Transfer</h2>
              </div>

              {/* Direction toggle */}
              <div className="flex gap-2">
                <button
                  onClick={() => setDirection('to-native')}
                  className={`flex-1 py-3 px-4 rounded-lg text-sm font-medium transition-all ${
                    direction === 'to-native'
                      ? 'bg-dag-accent/15 text-dag-accent border border-dag-accent/40'
                      : 'bg-dag-surface border border-dag-border text-dag-muted hover:text-white'
                  }`}
                >
                  <div className="flex items-center justify-center gap-2">
                    <span className="text-xs px-1.5 py-0.5 rounded bg-blue-500/20 text-blue-400">Arbitrum</span>
                    <ArrowRight className="w-4 h-4" />
                    <span className="text-xs px-1.5 py-0.5 rounded bg-dag-accent/20 text-dag-accent">UltraDAG</span>
                  </div>
                </button>
                <button
                  onClick={() => setDirection('to-arbitrum')}
                  className={`flex-1 py-3 px-4 rounded-lg text-sm font-medium transition-all ${
                    direction === 'to-arbitrum'
                      ? 'bg-dag-accent/15 text-dag-accent border border-dag-accent/40'
                      : 'bg-dag-surface border border-dag-border text-dag-muted hover:text-white'
                  }`}
                >
                  <div className="flex items-center justify-center gap-2">
                    <span className="text-xs px-1.5 py-0.5 rounded bg-dag-accent/20 text-dag-accent">UltraDAG</span>
                    <ArrowRight className="w-4 h-4" />
                    <span className="text-xs px-1.5 py-0.5 rounded bg-blue-500/20 text-blue-400">Arbitrum</span>
                  </div>
                </button>
              </div>

              {/* ─── Arbitrum → Native ─── */}
              {direction === 'to-native' ? (
                <div className="space-y-4">
                  {/* Arbitrum wallet connection */}
                  <div className="bg-dag-surface border border-dag-border rounded-lg p-4">
                    <div className="flex items-center justify-between mb-3">
                      <span className="text-xs text-dag-muted uppercase tracking-wider">From (Arbitrum)</span>
                      <span className="text-xs px-1.5 py-0.5 rounded bg-blue-500/20 text-blue-400">ERC-20</span>
                    </div>
                    {eth.connected ? (
                      <div className="space-y-2">
                        <div className="flex items-center justify-between">
                          <div className="flex items-center gap-2">
                            <div className="w-2 h-2 rounded-full bg-dag-green" />
                            <span className="text-sm text-white font-mono">{eth.address.slice(0, 6)}...{eth.address.slice(-4)}</span>
                            <CopyButton text={eth.address} />
                          </div>
                          <button onClick={eth.disconnect} className="text-xs text-dag-muted hover:text-dag-red flex items-center gap-1">
                            <Unplug className="w-3 h-3" /> Disconnect
                          </button>
                        </div>
                        <div className="grid grid-cols-2 gap-3 mt-2">
                          <div className="bg-dag-bg rounded px-3 py-2">
                            <span className="text-[10px] text-dag-muted uppercase">UDAG Balance</span>
                            <p className="text-sm text-white font-mono">{Number(eth.udagBalance).toLocaleString(undefined, { maximumFractionDigits: 4 })} UDAG</p>
                          </div>
                          <div className="bg-dag-bg rounded px-3 py-2">
                            <span className="text-[10px] text-dag-muted uppercase">ETH Balance</span>
                            <p className="text-sm text-white font-mono">{Number(eth.balance).toFixed(6)} ETH</p>
                          </div>
                        </div>
                        {!eth.isCorrectChain && (
                          <button onClick={eth.switchToArbitrum} className="w-full py-2 mt-1 rounded bg-dag-yellow/20 text-dag-yellow border border-dag-yellow/30 text-xs font-medium">
                            Switch to Arbitrum
                          </button>
                        )}
                      </div>
                    ) : (
                      <p className="text-sm text-dag-muted">
                        {eth.hasMetaMask ? 'Connect your Arbitrum wallet below to bridge tokens.' : 'Install MetaMask or a compatible wallet to continue.'}
                      </p>
                    )}
                  </div>

                  {/* Amount */}
                  <label className="block">
                    <div className="flex items-center justify-between">
                      <span className="text-sm text-dag-muted">Amount (UDAG)</span>
                      {eth.connected && (
                        <button onClick={() => setAmount(eth.udagBalance)} className="text-xs text-dag-accent hover:text-dag-accent/80">
                          Max
                        </button>
                      )}
                    </div>
                    <input
                      type="number"
                      min="0"
                      step="0.01"
                      value={amount}
                      onChange={e => setAmount(e.target.value)}
                      placeholder="0.00"
                      className="mt-1 block w-full rounded-lg bg-dag-surface border border-dag-border px-3 py-2.5 text-sm text-white"
                    />
                    <span className="text-xs text-dag-muted mt-1">Max per transaction: {formatUnits(maxPerTx, 8)} UDAG</span>
                  </label>

                  {/* Recipient */}
                  <label className="block">
                    <span className="text-sm text-dag-muted">Recipient (UltraDAG address)</span>
                    {unlocked && wallets.length > 0 ? (
                      <div className="mt-1">
                        <WalletSelector wallets={wallets} selectedIdx={selectedWalletIdx} onChange={setSelectedWalletIdx} label="" />
                        {wallet && (
                          <div className="flex items-center gap-2 mt-2 px-3 py-2 bg-dag-bg rounded border border-dag-border/50">
                            <code className="text-xs text-dag-muted font-mono truncate">{fullAddr(wallet.address)}</code>
                            <CopyButton text={fullAddr(wallet.address)} />
                          </div>
                        )}
                      </div>
                    ) : (
                      <input
                        type="text"
                        value={nativeAddress}
                        onChange={e => setNativeAddress(e.target.value)}
                        placeholder="tudg1... or udag1... or 40-char hex"
                        className="mt-1 block w-full rounded-lg bg-dag-surface border border-dag-border px-3 py-2.5 text-sm text-white font-mono"
                      />
                    )}
                  </label>

                  {eth.error && (
                    <div className="flex items-start gap-2 p-3 rounded-lg bg-dag-red/10 border border-dag-red/20">
                      <AlertTriangle className="w-4 h-4 text-dag-red shrink-0 mt-0.5" />
                      <div className="flex-1 min-w-0">
                        <p className="text-sm text-dag-red break-words">{eth.error.length > 120 ? eth.error.slice(0, 120) + '...' : eth.error}</p>
                        <button onClick={eth.clearError} className="text-xs text-dag-muted hover:text-white mt-1">Dismiss</button>
                      </div>
                    </div>
                  )}

                  {txHash && (
                    <div className="flex items-center gap-2 p-3 rounded-lg bg-dag-green/10 border border-dag-green/20">
                      <CheckCircle className="w-4 h-4 text-dag-green shrink-0" />
                      <div>
                        <p className="text-sm text-dag-green">Bridge transfer submitted!</p>
                        <p className="text-xs text-dag-muted font-mono mt-0.5">{txHash}</p>
                      </div>
                    </div>
                  )}

                  {/* Action buttons */}
                  {!eth.connected ? (
                    <button
                      onClick={eth.connect}
                      disabled={!eth.hasMetaMask}
                      className="w-full py-3 rounded-lg bg-gradient-to-r from-blue-500 to-dag-accent text-white font-medium text-sm hover:opacity-90 transition-opacity disabled:opacity-40 flex items-center justify-center gap-2"
                    >
                      <Wallet className="w-4 h-4" /> Connect Wallet to Bridge
                    </button>
                  ) : needsApproval ? (
                    <button
                      onClick={handleApprove}
                      disabled={approving || !canBridge}
                      className="w-full py-3 rounded-lg bg-blue-500/80 text-white font-medium text-sm hover:bg-blue-500 transition-colors disabled:opacity-40 flex items-center justify-center gap-2"
                    >
                      {approving ? <Loader2 className="w-4 h-4 animate-spin" /> : null}
                      {approving ? 'Approving...' : `Approve ${amount} UDAG`}
                    </button>
                  ) : (
                    <button
                      onClick={handleBridgeToNative}
                      disabled={bridging || !canBridge || amountSats <= 0n}
                      className="w-full py-3 rounded-lg bg-gradient-to-r from-blue-500 to-dag-accent text-white font-medium text-sm disabled:opacity-40 hover:opacity-90 transition-opacity flex items-center justify-center gap-2"
                    >
                      {bridging ? <Loader2 className="w-4 h-4 animate-spin" /> : null}
                      {bridging ? 'Bridging...' : !canBridge ? 'Bridge Not Active' : 'Bridge to UltraDAG'}
                    </button>
                  )}
                </div>
              ) : (
                /* ─── Native → Arbitrum ─── */
                <div className="space-y-4">
                  <div className="bg-dag-surface border border-dag-border rounded-lg p-4">
                    <div className="flex items-center justify-between mb-2">
                      <span className="text-xs text-dag-muted uppercase tracking-wider">From (UltraDAG)</span>
                      <span className="text-xs px-1.5 py-0.5 rounded bg-dag-accent/20 text-dag-accent">Native</span>
                    </div>
                    {unlocked && wallets.length > 0 ? (
                      <WalletSelector wallets={wallets} selectedIdx={selectedWalletIdx} onChange={setSelectedWalletIdx} label="" />
                    ) : (
                      <p className="text-sm text-dag-muted">Unlock your keystore to bridge from UltraDAG.</p>
                    )}
                  </div>

                  <label className="block">
                    <span className="text-sm text-dag-muted">Amount (UDAG)</span>
                    <input type="number" min="0" step="0.01" value={amount} onChange={e => setAmount(e.target.value)} placeholder="0.00"
                      className="mt-1 block w-full rounded-lg bg-dag-surface border border-dag-border px-3 py-2.5 text-sm text-white" />
                  </label>

                  <label className="block">
                    <span className="text-sm text-dag-muted">Recipient (Arbitrum/Ethereum address)</span>
                    {eth.connected ? (
                      <div className="mt-1 flex items-center gap-2 px-3 py-2.5 bg-dag-surface border border-dag-border rounded-lg">
                        <div className="w-2 h-2 rounded-full bg-dag-green" />
                        <span className="text-sm text-white font-mono">{eth.address}</span>
                        <CopyButton text={eth.address} />
                      </div>
                    ) : (
                      <input type="text" value={arbAddress} onChange={e => setArbAddress(e.target.value)} placeholder="0x..."
                        className="mt-1 block w-full rounded-lg bg-dag-surface border border-dag-border px-3 py-2.5 text-sm text-white font-mono" />
                    )}
                  </label>

                  <div className="flex items-start gap-2 px-3 py-2.5 rounded-lg bg-dag-accent/5 border border-dag-accent/20">
                    <Info className="w-4 h-4 text-dag-accent shrink-0 mt-0.5" />
                    <p className="text-xs text-dag-muted">
                      Native → Arbitrum bridging requires submitting a BridgeLock transaction on the UltraDAG network. The relayers will then mint ERC-20 UDAG to your Arbitrum address. This feature will be available after mainnet launch.
                    </p>
                  </div>

                  <button disabled className="w-full py-3 rounded-lg bg-gradient-to-r from-dag-accent to-blue-500 text-white font-medium text-sm disabled:opacity-40 cursor-not-allowed">
                    Coming After Mainnet Launch
                  </button>
                </div>
              )}

              {!canBridge && !bridgePaused && (
                <div className="flex items-start gap-2 px-3 py-2.5 rounded-lg bg-dag-yellow/5 border border-dag-yellow/20">
                  <AlertTriangle className="w-4 h-4 text-dag-yellow shrink-0 mt-0.5" />
                  <p className="text-xs text-dag-yellow/80">
                    {CONTRACTS_DEPLOYED
                      ? 'Bridge is deployed but not yet activated. Activation happens after mainnet launch.'
                      : 'Bridge contracts are not yet deployed. You can buy UDAG on Arbitrum via Uniswap in the meantime.'}
                  </p>
                </div>
              )}
            </div>
          </Card>

          {/* How It Works */}
          <Card title="How the Bridge Works">
            <div className="grid grid-cols-1 sm:grid-cols-4 gap-4">
              {[
                { num: '1', color: 'blue-500', label: 'Approve', desc: 'Approve the bridge contract to spend your UDAG tokens.' },
                { num: '2', color: 'dag-accent', label: 'Escrow', desc: 'Tokens are held in the bridge contract (not burned immediately).' },
                { num: '3', color: 'dag-purple', label: 'Verify', desc: '3-of-5 relayers confirm delivery on the destination chain.' },
                { num: '4', color: 'dag-green', label: 'Complete', desc: direction === 'to-native' ? 'Relayers burn escrowed tokens. Native UDAG minted.' : 'ERC-20 UDAG minted to your Arbitrum address.' },
              ].map(step => (
                <div key={step.num} className="space-y-2">
                  <div className={`w-8 h-8 rounded-lg bg-${step.color}/15 flex items-center justify-center`}>
                    <span className={`text-${step.color} font-bold text-sm`}>{step.num}</span>
                  </div>
                  <h4 className="text-sm font-medium text-white">{step.label}</h4>
                  <p className="text-xs text-dag-muted">{step.desc}</p>
                </div>
              ))}
            </div>
            <div className="mt-4 pt-3 border-t border-dag-border">
              <p className="text-xs text-dag-muted">
                If relayers don't confirm within 7 days, you can reclaim your escrowed tokens via the refund mechanism.
              </p>
            </div>
          </Card>
        </div>

        {/* Right: Bridge Info */}
        <div className="space-y-6">
          {/* Ethereum Wallet */}
          {eth.connected && (
            <Card title="Arbitrum Wallet">
              <div className="space-y-3">
                <div className="flex items-center justify-between text-sm">
                  <span className="text-dag-muted">Address</span>
                  <div className="flex items-center gap-1">
                    <span className="text-white font-mono text-xs">{eth.address.slice(0, 8)}...{eth.address.slice(-6)}</span>
                    <CopyButton text={eth.address} />
                  </div>
                </div>
                <div className="flex items-center justify-between text-sm">
                  <span className="text-dag-muted">UDAG Balance</span>
                  <span className="text-white font-mono">{Number(eth.udagBalance).toLocaleString(undefined, { maximumFractionDigits: 4 })}</span>
                </div>
                <div className="flex items-center justify-between text-sm">
                  <span className="text-dag-muted">ETH Balance</span>
                  <span className="text-white font-mono">{Number(eth.balance).toFixed(6)}</span>
                </div>
                <div className="flex items-center justify-between text-sm">
                  <span className="text-dag-muted">Chain</span>
                  <span className={eth.isCorrectChain ? 'text-dag-green' : 'text-dag-yellow'}>
                    {eth.chainId === 42161 ? 'Arbitrum One' : eth.chainId === 421614 ? 'Arbitrum Sepolia' : `Chain ${eth.chainId}`}
                  </span>
                </div>
              </div>
            </Card>
          )}

          {/* Bridge Stats */}
          <Card title="Bridge Status">
            <div className="space-y-3">
              <div className="flex items-center justify-between text-sm">
                <span className="text-dag-muted">Status</span>
                <span className={canBridge ? 'text-dag-green' : bridgePaused ? 'text-dag-red' : 'text-dag-yellow'}>
                  {canBridge ? 'Active' : bridgePaused ? 'Paused' : 'Pre-Mainnet'}
                </span>
              </div>
              <div className="flex items-center justify-between text-sm">
                <span className="text-dag-muted">Daily Volume</span>
                <span className="text-white font-mono">{formatUnits(dailyVolume, 8)} UDAG</span>
              </div>
              <div className="flex items-center justify-between text-sm">
                <span className="text-dag-muted">Daily Cap</span>
                <span className="text-white font-mono">{formatUnits(dailyCap, 8)} UDAG</span>
              </div>
              <div className="flex items-center justify-between text-sm">
                <span className="text-dag-muted">Max per Tx</span>
                <span className="text-white font-mono">{formatUnits(maxPerTx, 8)} UDAG</span>
              </div>
              {eth.contractsDeployed && (
                <div className="flex items-center justify-between text-sm">
                  <span className="text-dag-muted">Bridge Nonce</span>
                  <span className="text-white font-mono">{eth.nonce.toString()}</span>
                </div>
              )}
            </div>
          </Card>

          {/* Security */}
          <Card title="Security">
            <div className="space-y-3">
              <div className="flex items-start gap-2">
                <Shield className="w-4 h-4 text-dag-green shrink-0 mt-0.5" />
                <div>
                  <p className="text-sm text-white">Escrow + Refund</p>
                  <p className="text-xs text-dag-muted">Tokens held in escrow, refundable after 7 days if relayers fail.</p>
                </div>
              </div>
              <div className="flex items-start gap-2">
                <Shield className="w-4 h-4 text-dag-green shrink-0 mt-0.5" />
                <div>
                  <p className="text-sm text-white">3-of-5 Multi-sig</p>
                  <p className="text-xs text-dag-muted">Independent relayers must sign every bridge transfer.</p>
                </div>
              </div>
              <div className="flex items-start gap-2">
                <Clock className="w-4 h-4 text-dag-blue shrink-0 mt-0.5" />
                <div>
                  <p className="text-sm text-white">Finality Confirmed</p>
                  <p className="text-xs text-dag-muted">Transfers wait for BFT finality (~10s) before processing.</p>
                </div>
              </div>
              <div className="flex items-start gap-2">
                <AlertTriangle className="w-4 h-4 text-dag-yellow shrink-0 mt-0.5" />
                <div>
                  <p className="text-sm text-white">Rate Limiting</p>
                  <p className="text-xs text-dag-muted">Daily volume caps and per-tx limits prevent abuse.</p>
                </div>
              </div>
            </div>
          </Card>

          {/* Quick Links */}
          <Card title="Quick Links">
            <div className="space-y-2">
              {[
                { label: 'Buy UDAG on Uniswap', href: 'https://app.uniswap.org' },
                { label: 'UDAG on Arbiscan', href: 'https://arbiscan.io' },
                { label: 'UltraDAG Website', href: 'https://ultradag.com' },
              ].map(link => (
                <a key={link.label} href={link.href} target="_blank" rel="noopener noreferrer"
                  className="flex items-center justify-between px-3 py-2.5 rounded-lg bg-dag-surface border border-dag-border hover:border-dag-accent/40 transition-colors">
                  <span className="text-sm text-white">{link.label}</span>
                  <ExternalLink className="w-4 h-4 text-dag-muted" />
                </a>
              ))}
            </div>
          </Card>
        </div>
      </div>
    </div>
  );
}
