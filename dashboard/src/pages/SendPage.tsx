import { useState, useEffect, useRef } from 'react';
import { postTx, postFaucet, formatUdag, shortAddr, fullAddr, isValidAddress, normalizeAddress, getNodeUrl, getBalance } from '../lib/api';
import { resolvePocket, NameNotFoundError, PocketNotFoundError } from '../lib/names';
import { DisplayIdentity } from '../components/shared/DisplayIdentity';
import { getPasskeyInfo, signAndSubmitWithPasskey } from '../lib/webauthn-sign';
import { getPasskeyWallet } from '../lib/passkey-wallet';
import { QrCode } from '../components/shared/QrCode';
import { QrScanner } from '../components/shared/QrScanner';
import { useToast } from '../hooks/useToast';
import { useIsMobile } from '../hooks/useIsMobile';
import { PageHeader } from '../components/shared/PageHeader';
import { primaryButtonStyle, secondaryButtonStyle } from '../lib/theme';
import type { Wallet } from '../lib/keystore';
import type { WalletBalance } from '../hooks/useWalletBalances';

const SATS = 100_000_000;
const S = {
  card: { background: 'var(--dag-card)', border: '1px solid var(--dag-border)', borderRadius: 14, padding: '20px 22px' } as React.CSSProperties,
  label: { fontSize: 10.5, fontWeight: 600, color: 'var(--dag-text-muted)', letterSpacing: 1.2, textTransform: 'uppercase' as const, marginBottom: 6, display: 'block' },
  input: {
    width: '100%', padding: '11px 14px', borderRadius: 10, background: 'var(--dag-input-bg)',
    border: '1px solid var(--dag-border)', color: 'var(--dag-text)', fontSize: 13, outline: 'none',
    fontFamily: "'DM Sans',sans-serif", transition: 'border-color 0.2s',
  } as React.CSSProperties,
  btnSolid: (c = '#00E0C4') => ({
    width: '100%', padding: '12px 0', borderRadius: 10, background: c, color: '#080C14',
    fontSize: 13, fontWeight: 700, cursor: 'pointer', border: 'none', transition: 'opacity 0.2s',
  }),
  mono: { fontFamily: "'DM Mono',monospace" },
};

function ReceiveAddress({ wallet }: { wallet: Wallet }) {
  const pk = getPasskeyWallet();
  const hasName = pk?.name && pk.address === wallet.address;
  // QR encodes the @name if available (easier to type/share than bech32m).
  const qrValue = hasName ? `@${pk!.name}` : fullAddr(wallet.address);

  return (
    <div style={{ display: 'flex', flexDirection: 'column', alignItems: 'center', gap: 14 }}>
      {/* Primary identifier: @name if available */}
      {hasName && (
        <div style={{ textAlign: 'center' }}>
          <div style={{ fontSize: 22, fontWeight: 700, color: '#00E0C4', fontFamily: "'DM Mono',monospace" }}>
            @{pk!.name}
          </div>
          <p style={{ fontSize: 10.5, color: 'var(--dag-text-faint)', marginTop: 3 }}>
            Tell the sender to send to this name
          </p>
        </div>
      )}

      {/* QR Code */}
      <div style={{ background: '#fff', borderRadius: 12, padding: 12, display: 'inline-block' }}>
        <QrCode value={qrValue} size={180} />
      </div>
      <div style={{ fontSize: 9, color: 'var(--dag-text-faint)', textAlign: 'center' }}>
        QR encodes: {hasName ? `@${pk!.name}` : 'bech32m address'}
      </div>

      {/* Address details */}
      <div style={{ width: '100%', textAlign: 'center' }}>
        {!hasName && (
          <div style={{ fontSize: 14, fontWeight: 600, color: 'var(--dag-text)', marginBottom: 4 }}>{wallet.name}</div>
        )}
        <DisplayIdentity address={wallet.address} advanced copyable size="sm" />
      </div>
    </div>
  );
}

function SendToInput({ value, onChange, onScanQr }: { value: string; onChange: (v: string) => void; onScanQr: () => void }) {
  const [resolvedPreview, setResolvedPreview] = useState<{ address: string; label: string } | null>(null);
  const [resolving, setResolving] = useState(false);
  const [resolveError, setResolveError] = useState('');
  const debounceRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  // Live resolution preview — resolves as the user types (debounced).
  useEffect(() => {
    setResolvedPreview(null);
    setResolveError('');
    const input = value.trim();
    if (!input || input.length < 3) return;

    // Already a valid address — show it directly.
    if (isValidAddress(input)) {
      setResolvedPreview({ address: normalizeAddress(input), label: 'address' });
      return;
    }

    // Debounce name resolution.
    if (debounceRef.current) clearTimeout(debounceRef.current);
    debounceRef.current = setTimeout(async () => {
      setResolving(true);
      try {
        const resolved = await resolvePocket(input);
        setResolvedPreview({
          address: resolved.address,
          label: resolved.label ? `@${resolved.parent}.${resolved.label}` : `@${resolved.parent}`,
        });
      } catch {
        // Try /balance fallback for bare names.
        try {
          const bal = await getBalance(input.replace(/^@/, ''));
          if (bal?.address) {
            setResolvedPreview({ address: bal.address, label: `@${input.replace(/^@/, '')}` });
          } else {
            setResolveError('Not found');
          }
        } catch {
          setResolveError('Not found');
        }
      } finally {
        setResolving(false);
      }
    }, 500);

    return () => { if (debounceRef.current) clearTimeout(debounceRef.current); };
  }, [value]);

  return (
    <div>
      <div style={{ display: 'flex', gap: 8 }}>
        <input
          type="text" value={value} onChange={e => onChange(e.target.value)}
          placeholder="@name, @name.pocket, or address"
          style={{ ...S.input, flex: 1 }}
        />
        <button onClick={onScanQr} style={{
          padding: '0 12px', borderRadius: 10, background: 'var(--dag-input-bg)', border: '1px solid var(--dag-border)',
          color: 'var(--dag-text-muted)', cursor: 'pointer', fontSize: 16, flexShrink: 0, height: 42,
        }} title="Scan QR">📷</button>
      </div>

      {/* Live resolution preview */}
      <div style={{ minHeight: 20, marginTop: 5 }}>
        {resolving && (
          <span style={{ fontSize: 10, color: 'var(--dag-text-faint)' }}>Resolving...</span>
        )}
        {!resolving && resolvedPreview && (
          <div style={{ display: 'flex', alignItems: 'center', gap: 6 }}>
            <span style={{ fontSize: 10, color: '#00E0C4' }}>✓</span>
            <span style={{ fontSize: 10, color: '#00E0C4', fontWeight: 600 }}>{resolvedPreview.label}</span>
            <span style={{ fontSize: 10, color: 'var(--dag-text-faint)', fontFamily: "'DM Mono',monospace" }}>
              → {resolvedPreview.address.slice(0, 10)}...{resolvedPreview.address.slice(-6)}
            </span>
          </div>
        )}
        {!resolving && resolveError && value.trim().length >= 3 && (
          <span style={{ fontSize: 10, color: '#EF4444' }}>✗ {resolveError}</span>
        )}
      </div>
    </div>
  );
}

interface SendPageProps {
  wallets: Wallet[];
  balances: Map<string, WalletBalance>;
  unlocked: boolean;
  network?: string;
}

export function SendPage({ wallets, balances, unlocked, network }: SendPageProps) {
  const { toast } = useToast();
  const m = useIsMobile();
  const [selectedIdx, setSelectedIdx] = useState(0);
  const [to, setTo] = useState('');
  // Pre-fill recipient from ?to= query param (e.g., from "Send to this pocket" link).
  useEffect(() => {
    const params = new URLSearchParams(window.location.search);
    const prefill = params.get('to');
    if (prefill) setTo(prefill);
  }, []);
  const [amount, setAmount] = useState('');
  const [fee, setFee] = useState('0.0001');
  const [memo, setMemo] = useState('');
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState('');
  const [success, setSuccess] = useState('');
  const [faucetLoading, setFaucetLoading] = useState(false);
  const [faucetMsg, setFaucetMsg] = useState('');
  const [receiveIdx, setReceiveIdx] = useState(0);
  const [showScanner, setShowScanner] = useState(false);

  const pw = getPasskeyWallet();

  if (!unlocked) {
    return (
      <div style={{ padding: '18px 26px', fontFamily: "'DM Sans',sans-serif", display: 'flex', alignItems: 'center', justifyContent: 'center', minHeight: 300 }}>
        <p style={{ color: 'var(--dag-text-muted)', fontSize: 13 }}>Unlock your wallet to send transactions.</p>
      </div>
    );
  }

  const wallet = wallets[selectedIdx];
  const balance = wallet ? balances.get(wallet.address) : undefined;
  const receiveWallet = wallets[receiveIdx];
  const memoBytes = new TextEncoder().encode(memo).length;

  const handleSend = async () => {
    if (!wallet) return;
    setError(''); setSuccess('');
    const sats = Math.floor(parseFloat(amount) * SATS);
    const feeSats = Math.round(parseFloat(fee) * SATS);
    if (isNaN(sats) || sats <= 0) { setError('Amount must be positive'); return; }
    if (isNaN(feeSats) || feeSats < 10000) { setError('Minimum fee is 0.0001 UDAG'); return; }
    if (memoBytes > 256) { setError('Memo exceeds 256 bytes'); return; }

    setLoading(true);
    try {
      // Resolve recipient: accept bech32m/hex addresses, @name, or @name.pocket
      let resolvedAddr: string;
      const input = to.trim();

      if (isValidAddress(input)) {
        // Direct address — use as-is.
        resolvedAddr = normalizeAddress(input);
      } else {
        // Try resolving as a name or name.pocket.
        try {
          const resolved = await resolvePocket(input);
          resolvedAddr = resolved.address;
        } catch (e) {
          if (e instanceof NameNotFoundError) {
            // Fallback: try /balance/{input} which accepts names without @
            try {
              const bal = await getBalance(input.replace(/^@/, ''));
              if (bal?.address) {
                resolvedAddr = bal.address;
              } else {
                setError(`Name "${input}" not found`); setLoading(false); return;
              }
            } catch {
              setError(`Name "${input}" not found`); setLoading(false); return;
            }
          } else if (e instanceof PocketNotFoundError) {
            setError(e.message); setLoading(false); return;
          } else {
            setError('Invalid recipient — use an address, @name, or @name.pocket'); setLoading(false); return;
          }
        }
      }

      const passkey = getPasskeyInfo();
      if (passkey && wallet.address === passkey.address) {
        const balRes = await fetch(`${getNodeUrl()}/balance/${wallet.address}`, { signal: AbortSignal.timeout(5000) });
        const balData = await balRes.json();
        await signAndSubmitWithPasskey(resolvedAddr, sats, feeSats, balData.nonce ?? 0, memo.trim() || undefined);
      } else {
        const body: Record<string, unknown> = { secret_key: wallet.secret_key, to: resolvedAddr, amount: sats, fee: feeSats };
        if (memo.trim()) body.memo = memo.trim();
        await postTx(body);
      }
      const displayTo = isValidAddress(input) ? shortAddr(input) : input;
      const msg = `Sent ${formatUdag(sats)} UDAG to ${displayTo}`;
      setSuccess(msg); toast(msg, 'success');
      setTimeout(() => setSuccess(''), 5000);
      setTo(''); setAmount(''); setMemo('');
    } catch (e: unknown) {
      const msg = e instanceof Error ? e.message : 'Transaction failed';
      setError(msg); toast(msg, 'error');
    } finally { setLoading(false); }
  };

  const handleFaucet = async () => {
    const w = wallets[receiveIdx];
    if (!w) return;
    setFaucetMsg(''); setFaucetLoading(true);
    try {
      await postFaucet({ address: w.address, amount: 10_000_000_000 });
      setFaucetMsg('✓ 100 UDAG requested');
      toast('100 UDAG requested', 'success');
    } catch (e: unknown) {
      setFaucetMsg(e instanceof Error ? e.message : 'Failed');
    } finally { setFaucetLoading(false); }
  };

  // Wallet selector as inline styled dropdown
  function WalletSelect({ idx, onChange }: { idx: number; onChange: (i: number) => void }) {
    return (
      <select value={idx} onChange={e => onChange(Number(e.target.value))} style={{
        ...S.input, appearance: 'none', cursor: 'pointer', backgroundImage: `url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='12' height='12' viewBox='0 0 12 12'%3E%3Cpath d='M3 5l3 3 3-3' fill='none' stroke='var(--dag-text-muted)' stroke-width='1.5'/%3E%3C/svg%3E")`,
        backgroundRepeat: 'no-repeat', backgroundPosition: 'right 12px center',
      }}>
        {wallets.map((w, i) => (
          <option key={w.address} value={i} style={{ background: 'var(--dag-bg)', color: 'var(--dag-text)' }}>
            {w.name} — {((balances.get(w.address)?.balance ?? 0) / SATS).toFixed(2)} UDAG
          </option>
        ))}
      </select>
    );
  }

  return (
    <div style={{ padding: m ? '12px 14px' : '18px 26px', fontFamily: "'DM Sans',sans-serif" }}>
      <style>{`@keyframes slideUp{from{opacity:0;transform:translateY(10px)}to{opacity:1;transform:translateY(0)}} input:focus,select:focus,textarea:focus{border-color:rgba(0,224,196,0.3)!important}`}</style>

      <PageHeader title="Send & Receive" subtitle="Transfer UDAG or receive funds" />

      <div style={{ display: 'grid', gridTemplateColumns: m ? '1fr' : '1fr 1fr', gap: m ? 14 : 16, animation: 'slideUp 0.4s ease' }}>
        {/* ── Send ── */}
        <div style={{ display: 'flex', flexDirection: 'column', gap: 14 }}>
          <div style={S.card}>
            <div style={{ display: 'flex', alignItems: 'center', gap: 7, marginBottom: 16 }}>
              <span style={{ color: '#00E0C4', fontSize: 16 }}>⇄</span>
              <span style={{ fontSize: 14, fontWeight: 600, color: 'var(--dag-text-secondary)' }}>Send UDAG</span>
              {pw && wallet?.address === pw.address && (
                <span style={{ fontSize: 8.5, background: 'rgba(0,224,196,0.12)', color: '#00E0C4', padding: '1px 6px', borderRadius: 4, fontWeight: 600 }}>PASSKEY</span>
              )}
            </div>

            <div style={{ display: 'flex', flexDirection: 'column', gap: 12 }}>
              <div>
                <span style={S.label}>From Wallet</span>
                <WalletSelect idx={selectedIdx} onChange={setSelectedIdx} />
                {balance && <div style={{ fontSize: 10.5, color: 'var(--dag-subheading)', marginTop: 4, ...S.mono }}>Available: {((balance.balance ?? 0) / SATS).toFixed(4)} UDAG</div>}
              </div>

              <div>
                <span style={S.label}>Send to</span>
                <SendToInput value={to} onChange={v => { setTo(v); setError(''); setSuccess(''); }} onScanQr={() => setShowScanner(true)} />
              </div>

              <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 10 }}>
                <div>
                  <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
                    <span style={S.label}>Amount (UDAG)</span>
                    {balance && (
                      <button
                        onClick={() => {
                          const feeSats = Math.round(parseFloat(fee || '0.0001') * SATS);
                          const max = Math.max(0, (balance.balance ?? 0) - feeSats);
                          setAmount((max / SATS).toFixed(4));
                          setError(''); setSuccess('');
                        }}
                        style={{ background: 'none', border: 'none', color: '#00E0C4', fontSize: 9.5, cursor: 'pointer', fontWeight: 700, letterSpacing: 0.5, padding: 0 }}
                      >
                        MAX
                      </button>
                    )}
                  </div>
                  <input type="number" min="0" step="0.01" value={amount} onChange={e => { setAmount(e.target.value); setError(''); setSuccess(''); }} placeholder="0.00" style={S.input} />
                </div>
                <div>
                  <span style={S.label}>Fee (min 0.0001)</span>
                  <input type="number" min="0.0001" step="0.0001" value={fee} onChange={e => setFee(e.target.value)} style={S.input} />
                </div>
              </div>

              <div>
                <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
                  <span style={S.label}>Memo (optional)</span>
                  <span style={{ fontSize: 10, color: memoBytes > 256 ? '#EF4444' : 'var(--dag-text-faint)' }}>{memoBytes}/256</span>
                </div>
                <textarea value={memo} onChange={e => { setMemo(e.target.value); setError(''); setSuccess(''); }} placeholder="Optional message" rows={2}
                  style={{ ...S.input, resize: 'none', maxHeight: 80 } as React.CSSProperties} />
              </div>

              {error && <div style={{ fontSize: 11, color: '#EF4444', background: 'rgba(239,68,68,0.06)', border: '1px solid rgba(239,68,68,0.15)', borderRadius: 8, padding: '8px 12px' }}>{error}</div>}
              {success && <div style={{ fontSize: 11, color: '#00E0C4', background: 'rgba(0,224,196,0.06)', border: '1px solid rgba(0,224,196,0.15)', borderRadius: 8, padding: '8px 12px' }}>{success}</div>}

              <button
                onClick={handleSend}
                disabled={loading || !to.trim() || !amount || parseFloat(amount) <= 0}
                style={{
                  ...primaryButtonStyle, width: '100%', padding: '12px 0',
                  opacity: loading || !to.trim() || !amount || parseFloat(amount) <= 0 ? 0.35 : 1,
                  cursor: loading || !to.trim() || !amount ? 'not-allowed' : 'pointer',
                }}
              >
                {loading ? 'Sending...' : pw && wallet?.address === pw.address ? '◎ Send with Biometrics' : '⇄ Send'}
              </button>
            </div>
          </div>

          {/* Faucet */}
          {network !== 'mainnet' && (
            <div style={S.card}>
              <div style={{ display: 'flex', alignItems: 'center', gap: 7, marginBottom: 12 }}>
                <span style={{ color: '#FFB800', fontSize: 14 }}>⬡</span>
                <span style={{ fontSize: 13, fontWeight: 600, color: 'var(--dag-text-secondary)' }}>Testnet Faucet</span>
                <span style={{ fontSize: 8.5, background: 'rgba(255,184,0,0.12)', color: '#FFB800', padding: '1px 6px', borderRadius: 4, fontWeight: 600 }}>TESTNET</span>
              </div>
              <p style={{ fontSize: 11, color: 'var(--dag-subheading)', marginBottom: 10 }}>Request 100 free UDAG for testing.</p>
              {faucetMsg && <div style={{ fontSize: 11, color: faucetMsg.startsWith('✓') ? '#00E0C4' : '#EF4444', marginBottom: 8 }}>{faucetMsg}</div>}
              <button onClick={handleFaucet} disabled={faucetLoading} style={{ ...secondaryButtonStyle, width: '100%', padding: '12px 0', opacity: faucetLoading ? 0.5 : 1 }}>
                {faucetLoading ? 'Requesting...' : '⬡ Request 100 UDAG'}
              </button>
            </div>
          )}
        </div>

        {/* ── Receive ── */}
        <div style={S.card}>
          <div style={{ display: 'flex', alignItems: 'center', gap: 7, marginBottom: 16 }}>
            <span style={{ color: '#0066FF', fontSize: 16 }}>◎</span>
            <span style={{ fontSize: 14, fontWeight: 600, color: 'var(--dag-text-secondary)' }}>Receive UDAG</span>
          </div>

          <p style={{ fontSize: 11.5, color: 'var(--dag-subheading)', marginBottom: 14 }}>Share your address or QR code to receive funds.</p>

          {wallets.length === 0 ? (
            <div style={{ textAlign: 'center', padding: '30px 0' }}>
              <div style={{ fontSize: 28, opacity: 0.1, marginBottom: 10 }}>◎</div>
              <p style={{ fontSize: 12, color: 'var(--dag-text-muted)' }}>Create a wallet first to receive UDAG</p>
            </div>
          ) : (
            <>
              <div style={{ marginBottom: 14 }}>
                <span style={S.label}>Wallet</span>
                <WalletSelect idx={receiveIdx} onChange={setReceiveIdx} />
              </div>

              {receiveWallet && (
                <ReceiveAddress wallet={receiveWallet} />
              )}
            </>
          )}
        </div>
      </div>

      <QrScanner open={showScanner} onClose={() => setShowScanner(false)} onScan={(data) => {
        const cleaned = data.replace(/^(ultradag|udag):\/?\/?/i, '').split('?')[0];
        setTo(cleaned);
      }} />
    </div>
  );
}
