import { useState } from 'react';
import { postTx, postFaucet, formatUdag, shortAddr, fullAddr, isValidAddress, normalizeAddress, getNodeUrl } from '../lib/api';
import { DisplayIdentity } from '../components/shared/DisplayIdentity';
import { getPasskeyInfo, signAndSubmitWithPasskey } from '../lib/webauthn-sign';
import { getPasskeyWallet } from '../lib/passkey-wallet';
import { QrCode } from '../components/shared/QrCode';
import { QrScanner } from '../components/shared/QrScanner';
import { useToast } from '../hooks/useToast';
import { useIsMobile } from '../hooks/useIsMobile';
import { PageHeader } from '../components/shared/PageHeader';
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
  return (
    <div style={{ display: 'flex', flexDirection: 'column', alignItems: 'center', gap: 16 }}>
      {/* QR Code */}
      <div style={{ background: '#fff', borderRadius: 12, padding: 12, display: 'inline-block' }}>
        <QrCode value={fullAddr(wallet.address)} size={200} />
      </div>

      {/* Primary: ULTRA ID or wallet name */}
      <div style={{ width: '100%', textAlign: 'center' }}>
        <div style={{ fontSize: 16, fontWeight: 700, color: 'var(--dag-text)', marginBottom: 4 }}>{wallet.name}</div>
        <DisplayIdentity address={wallet.address} advanced copyable size="sm" />
      </div>

      <p style={{ fontSize: 10.5, color: 'var(--dag-text-faint)', textAlign: 'center' }}>
        Share your ULTRA ID or scan the QR code to receive UDAG.
      </p>
    </div>
  );
}

function SendToInput({ value, onChange, onScanQr }: { value: string; onChange: (v: string) => void; onScanQr: () => void }) {
  const [mode, setMode] = useState<'name' | 'address'>('name');
  // Auto-detect: if user pastes something that looks like an address, switch to address mode
  const handleChange = (v: string) => {
    onChange(v);
    if (v.startsWith('udag1') || v.startsWith('tudg1') || /^[0-9a-f]{40,64}$/i.test(v)) {
      setMode('address');
    }
  };

  return (
    <div>
      <div style={{ display: 'flex', gap: 8, marginBottom: 6 }}>
        <div style={{ flex: 1, position: 'relative' }}>
          <input
            type="text" value={value} onChange={e => handleChange(e.target.value)}
            placeholder={mode === 'name' ? 'Enter ULTRA ID (e.g. alice)' : 'udag1... or tudg1... address'}
            style={{ ...S.input, paddingRight: 40 }}
          />
          {mode === 'name' && (
            <span style={{ position: 'absolute', right: 14, top: '50%', transform: 'translateY(-50%)', fontSize: 12, color: 'var(--dag-text-faint)' }}>@</span>
          )}
        </div>
        <button onClick={onScanQr} style={{
          padding: '0 12px', borderRadius: 10, background: 'var(--dag-input-bg)', border: '1px solid var(--dag-border)',
          color: 'var(--dag-text-muted)', cursor: 'pointer', fontSize: 16, flexShrink: 0, height: 42,
        }} title="Scan QR">📷</button>
      </div>

      <button onClick={() => setMode(mode === 'name' ? 'address' : 'name')} style={{
        background: 'none', border: 'none', color: 'var(--dag-text-faint)', fontSize: 10,
        cursor: 'pointer', padding: 0, display: 'flex', alignItems: 'center', gap: 4,
      }}>
        <span style={{ fontSize: 8, transform: mode === 'address' ? 'rotate(90deg)' : 'rotate(0deg)', transition: 'transform 0.2s', display: 'inline-block' }}>▶</span>
        {mode === 'name' ? 'Use address instead' : 'Use ULTRA ID instead'}
      </button>
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
    if (!isValidAddress(to.trim())) { setError('Invalid recipient address'); return; }
    if (memoBytes > 256) { setError('Memo exceeds 256 bytes'); return; }

    setLoading(true);
    try {
      const passkey = getPasskeyInfo();
      if (passkey && wallet.address === passkey.address) {
        const balRes = await fetch(`${getNodeUrl()}/balance/${wallet.address}`, { signal: AbortSignal.timeout(5000) });
        const balData = await balRes.json();
        await signAndSubmitWithPasskey(normalizeAddress(to.trim()), sats, feeSats, balData.nonce ?? 0, memo.trim() || undefined);
      } else {
        const body: Record<string, unknown> = { secret_key: wallet.secret_key, to: normalizeAddress(to.trim()), amount: sats, fee: feeSats };
        if (memo.trim()) body.memo = memo.trim();
        await postTx(body);
      }
      const msg = `Sent ${formatUdag(sats)} UDAG to ${shortAddr(to)}`;
      setSuccess(msg); toast(msg, 'success');
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
                <SendToInput value={to} onChange={v => { setTo(v); setError(''); }} onScanQr={() => setShowScanner(true)} />
              </div>

              <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 10 }}>
                <div>
                  <span style={S.label}>Amount (UDAG)</span>
                  <input type="number" min="0" step="0.01" value={amount} onChange={e => setAmount(e.target.value)} placeholder="0.00" style={S.input} />
                </div>
                <div>
                  <span style={S.label}>Fee (UDAG)</span>
                  <input type="number" min="0.0001" step="0.0001" value={fee} onChange={e => setFee(e.target.value)} style={S.input} />
                </div>
              </div>

              <div>
                <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
                  <span style={S.label}>Memo (optional)</span>
                  <span style={{ fontSize: 10, color: memoBytes > 256 ? '#EF4444' : 'var(--dag-text-faint)' }}>{memoBytes}/256</span>
                </div>
                <textarea value={memo} onChange={e => setMemo(e.target.value)} placeholder="Optional message" rows={2}
                  style={{ ...S.input, resize: 'none', maxHeight: 80 } as React.CSSProperties} />
              </div>

              {error && <div style={{ fontSize: 11, color: '#EF4444', background: 'rgba(239,68,68,0.06)', border: '1px solid rgba(239,68,68,0.15)', borderRadius: 8, padding: '8px 12px' }}>{error}</div>}
              {success && <div style={{ fontSize: 11, color: '#00E0C4', background: 'rgba(0,224,196,0.06)', border: '1px solid rgba(0,224,196,0.15)', borderRadius: 8, padding: '8px 12px' }}>{success}</div>}

              <button onClick={handleSend} disabled={loading} style={{ ...S.btnSolid(), opacity: loading ? 0.5 : 1 }}>
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
              <button onClick={handleFaucet} disabled={faucetLoading} style={{ ...S.btnSolid('#FFB800'), opacity: faucetLoading ? 0.5 : 1 }}>
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

          <div style={{ marginBottom: 14 }}>
            <span style={S.label}>Wallet</span>
            <WalletSelect idx={receiveIdx} onChange={setReceiveIdx} />
          </div>

          {receiveWallet && (
            <ReceiveAddress wallet={receiveWallet} />
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
