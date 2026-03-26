import { useState } from 'react';
import { Camera } from 'lucide-react';
import { postTx, postFaucet, formatUdag, shortAddr, fullAddr, prettyAddr, isValidAddress, normalizeAddress } from '../lib/api.ts';
import { Card } from '../components/shared/Card.tsx';
import { WalletSelector } from '../components/shared/WalletSelector.tsx';
import { CopyButton } from '../components/shared/CopyButton.tsx';
import { QrCode } from '../components/shared/QrCode.tsx';
import { QrScanner } from '../components/shared/QrScanner.tsx';
import { useToast } from '../hooks/useToast.tsx';
import type { Wallet } from '../lib/keystore.ts';
import type { WalletBalance } from '../hooks/useWalletBalances.ts';

interface SendPageProps {
  wallets: Wallet[];
  balances: Map<string, WalletBalance>;
  unlocked: boolean;
  network?: string;
}

export function SendPage({ wallets, balances, unlocked, network }: SendPageProps) {
  const { toast } = useToast();

  // Send form state
  const [selectedIdx, setSelectedIdx] = useState(0);
  const [to, setTo] = useState('');
  const [amount, setAmount] = useState('');
  const [fee, setFee] = useState('0.0001');
  const [memo, setMemo] = useState('');
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState('');
  const [success, setSuccess] = useState('');

  // Faucet state
  const [faucetIdx, setFaucetIdx] = useState(0);
  const [faucetLoading, setFaucetLoading] = useState(false);
  const [faucetError, setFaucetError] = useState('');
  const [faucetSuccess, setFaucetSuccess] = useState('');

  // Receive state
  const [receiveIdx, setReceiveIdx] = useState(0);

  // QR scanner state
  const [showScanner, setShowScanner] = useState(false);

  if (!unlocked) {
    return (
      <div className="flex items-center justify-center h-64">
        <p className="text-dag-muted">Unlock your wallet to send transactions.</p>
      </div>
    );
  }

  const wallet = wallets[selectedIdx];
  const balance = wallet ? balances.get(wallet.address) : undefined;
  const faucetWallet = wallets[faucetIdx];
  const receiveWallet = wallets[receiveIdx];

  const memoBytes = new TextEncoder().encode(memo).length;

  const handleSend = async () => {
    if (!wallet) return;
    setError('');
    setSuccess('');
    const sats = Math.floor(parseFloat(amount) * 100_000_000);
    const feeSats = Math.round(parseFloat(fee) * 100_000_000);
    if (isNaN(sats) || sats <= 0) { setError('Amount must be positive'); return; }
    if (isNaN(feeSats) || feeSats < 10000) { setError('Minimum fee is 0.0001 UDAG'); return; }
    if (!isValidAddress(to.trim())) { setError('Invalid recipient address (hex or bech32m)'); return; }
    if (memoBytes > 256) { setError('Memo exceeds 256 bytes'); return; }

    setLoading(true);
    try {
      const body: Record<string, unknown> = {
        secret_key: wallet.secret_key,
        to: normalizeAddress(to.trim()),
        amount: sats,
        fee: feeSats,
      };
      if (memo.trim()) {
        body.memo = memo.trim();
      }
      await postTx(body);
      const msg = `Sent ${formatUdag(sats)} UDAG to ${shortAddr(to)}`;
      setSuccess(msg);
      toast(msg, 'success');
      setTo('');
      setAmount('');
      setMemo('');
    } catch (e: unknown) {
      const msg = e instanceof Error ? e.message : 'Transaction failed';
      setError(msg);
      toast(msg, 'error');
    } finally {
      setLoading(false);
    }
  };

  const handleFaucet = async () => {
    if (!faucetWallet) return;
    setFaucetError('');
    setFaucetSuccess('');
    setFaucetLoading(true);
    try {
      await postFaucet({ address: faucetWallet.address, amount: 10_000_000_000 });
      setFaucetSuccess(`Requested 100 UDAG for ${shortAddr(faucetWallet.address)}`);
      toast('100 UDAG requested', 'success');
    } catch (e: unknown) {
      const msg = e instanceof Error ? e.message : 'Faucet request failed';
      setFaucetError(msg);
      toast(msg, 'error');
    } finally {
      setFaucetLoading(false);
    }
  };

  return (
    <div className="space-y-6">
      <h1 className="text-2xl font-bold text-white">Send & Receive</h1>

      <div className="grid grid-cols-1 xl:grid-cols-2 gap-6">
        {/* Left column: Send + Faucet */}
        <div className="space-y-6">
          {/* Send Card */}
          <Card>
            <div className="space-y-4">
              <h2 className="text-lg font-semibold text-white">Send UDAG</h2>

              <WalletSelector wallets={wallets} selectedIdx={selectedIdx} onChange={setSelectedIdx} />

              {balance && (
                <div className="text-sm text-dag-muted">
                  Available: <span className="text-white font-mono">{formatUdag(balance.balance)} UDAG</span>
                </div>
              )}

              <div className="block">
                <span className="text-sm text-dag-muted">Recipient Address (hex or bech32m)</span>
                <div className="mt-1 flex gap-2">
                  <input
                    type="text"
                    value={to}
                    onChange={e => setTo(e.target.value)}
                    placeholder="Enter recipient address"
                    className="block w-full rounded bg-dag-surface border border-dag-border px-3 py-2 text-sm text-white font-mono"
                  />
                  <button
                    type="button"
                    onClick={() => setShowScanner(true)}
                    className="flex-shrink-0 p-2 rounded bg-dag-surface border border-dag-border text-dag-muted hover:text-white hover:border-dag-accent transition-colors"
                    title="Scan QR code"
                  >
                    <Camera className="w-5 h-5" />
                  </button>
                </div>
              </div>

              <label className="block">
                <span className="text-sm text-dag-muted">Amount (UDAG)</span>
                <input
                  type="number"
                  min="0"
                  step="0.01"
                  value={amount}
                  onChange={e => setAmount(e.target.value)}
                  placeholder="0.00"
                  className="mt-1 block w-full rounded bg-dag-surface border border-dag-border px-3 py-2 text-sm text-white"
                />
              </label>

              <label className="block">
                <span className="text-sm text-dag-muted">Fee (UDAG)</span>
                <input
                  type="number"
                  min="0.0001"
                  step="0.0001"
                  value={fee}
                  onChange={e => setFee(e.target.value)}
                  className="mt-1 block w-full rounded bg-dag-surface border border-dag-border px-3 py-2 text-sm text-white"
                />
              </label>

              <label className="block">
                <div className="flex items-center justify-between">
                  <span className="text-sm text-dag-muted">Memo (optional, max 256 bytes)</span>
                  <span className={`text-xs ${memoBytes > 256 ? 'text-dag-red' : 'text-dag-muted'}`}>
                    {memoBytes}/256
                  </span>
                </div>
                <textarea
                  value={memo}
                  onChange={e => setMemo(e.target.value)}
                  placeholder="Optional message"
                  rows={2}
                  className="mt-1 block w-full rounded bg-dag-surface border border-dag-border px-3 py-2 text-sm text-white resize-none max-h-24"
                />
              </label>

              {error && <p className="text-sm text-dag-red">{error}</p>}
              {success && <p className="text-sm text-dag-green">{success}</p>}

              <button
                onClick={handleSend}
                disabled={loading}
                className="w-full py-2.5 rounded bg-dag-accent text-white font-medium text-sm hover:bg-dag-accent/80 disabled:opacity-50 transition-colors"
              >
                {loading ? 'Sending...' : 'Send'}
              </button>
            </div>
          </Card>

          {/* Faucet Card */}
          {network === 'mainnet' ? (
          <Card>
            <div className="space-y-2">
              <h2 className="text-lg font-semibold text-white">Fund Your Wallet</h2>
              <p className="text-sm text-dag-muted">
                The faucet is only available on testnet. To get UDAG on mainnet, receive it from another wallet or use the bridge.
              </p>
              <p className="text-xs text-dag-muted">
                Switch to testnet in Settings to access the faucet with free test tokens.
              </p>
            </div>
          </Card>
          ) : (
          <Card>
            <div className="space-y-4">
              <div className="flex items-center gap-2">
                <h2 className="text-lg font-semibold text-white">Testnet Faucet</h2>
                <span className="px-2 py-0.5 rounded text-xs font-semibold bg-amber-400/20 text-amber-400 border border-amber-400/30">
                  TESTNET
                </span>
              </div>

              <p className="text-sm text-dag-muted">
                Request free UDAG for testing. Max 100 UDAG per request, 1 request per 10 minutes.
              </p>

              <WalletSelector wallets={wallets} selectedIdx={faucetIdx} onChange={setFaucetIdx} />

              {faucetError && <p className="text-sm text-dag-red">{faucetError}</p>}
              {faucetSuccess && <p className="text-sm text-dag-green">{faucetSuccess}</p>}

              <button
                onClick={handleFaucet}
                disabled={faucetLoading}
                className="w-full py-2.5 rounded bg-dag-green text-white font-medium text-sm hover:bg-dag-green/80 disabled:opacity-50 transition-colors"
              >
                {faucetLoading ? 'Requesting...' : 'Request 100 UDAG'}
              </button>
            </div>
          </Card>
          )}
        </div>

        {/* Right column: Receive */}
        <div>
          <Card>
            <div className="space-y-4">
              <h2 className="text-lg font-semibold text-white">Receive UDAG</h2>

              <p className="text-sm text-dag-muted">
                Share your address to receive UDAG from others.
              </p>

              <WalletSelector wallets={wallets} selectedIdx={receiveIdx} onChange={setReceiveIdx} />

              {receiveWallet && (
                <div className="space-y-4">
                  <QrCode value={fullAddr(receiveWallet.address)} size={256} />

                  <span className="text-sm text-dag-muted">Your Address</span>
                  <div className="bg-dag-surface border border-dag-border rounded-lg p-4">
                    <div className="flex items-start justify-between gap-2 mb-3">
                      <code className="text-xs sm:text-base text-white font-mono leading-relaxed tracking-wide break-all">
                        {prettyAddr(receiveWallet.address)}
                      </code>
                      <CopyButton text={fullAddr(receiveWallet.address)} />
                    </div>
                    <div className="flex items-center gap-2 pt-2 border-t border-dag-border/50">
                      <span className="text-[10px] text-dag-muted uppercase tracking-wider">Hex</span>
                      <code className="text-[11px] font-mono text-dag-muted break-all">{receiveWallet.address}</code>
                      <CopyButton text={receiveWallet.address} />
                    </div>
                  </div>
                  <p className="text-xs text-dag-muted">Scan the QR code or share the address above to receive UDAG. Both formats (bech32m and hex) are accepted.</p>
                </div>
              )}
            </div>
          </Card>
        </div>
      </div>

      <QrScanner
        open={showScanner}
        onClose={() => setShowScanner(false)}
        onScan={(data) => {
          // Strip any URI prefix (e.g. "ultradag:" or "udag:")
          const cleaned = data.replace(/^(ultradag|udag):\/?\/?/i, '').split('?')[0];
          setTo(cleaned);
        }}
      />
    </div>
  );
}
