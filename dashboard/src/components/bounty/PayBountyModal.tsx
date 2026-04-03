import { useState } from 'react';
import { postTx, isValidAddress, normalizeAddress, getNodeUrl } from '../../lib/api';
import { getPasskeyInfo, signAndSubmitWithPasskey } from '../../lib/webauthn-sign';
import { useToast } from '../../hooks/useToast';
import type { ParsedBounty } from '../../lib/github';
import type { Wallet } from '../../lib/keystore';

interface PayBountyModalProps {
  bounty: ParsedBounty;
  wallets: Wallet[];
  onClose: () => void;
  onSuccess: () => void;
}

const SATS = 100_000_000;

const inputStyle: React.CSSProperties = {
  width: '100%', padding: '10px 14px', borderRadius: 10,
  background: 'var(--dag-input-bg)', border: '1px solid var(--dag-border)',
  color: 'var(--dag-text)', fontSize: 13, outline: 'none',
  fontFamily: "'DM Sans',sans-serif",
};

export function PayBountyModal({ bounty, wallets, onClose, onSuccess }: PayBountyModalProps) {
  const [walletIdx, setWalletIdx] = useState(0);
  const [hunterAddress, setHunterAddress] = useState('');
  const [amount, setAmount] = useState(bounty.reward > 0 ? String(bounty.reward) : '');
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState('');
  const { toast } = useToast();

  const wallet = wallets[walletIdx];
  const memo = `bounty:#${bounty.issue.number}`;
  const fee = 0.0001;

  const handlePay = async () => {
    if (!wallet) return;
    setError('');
    const sats = Math.floor(parseFloat(amount) * SATS);
    const feeSats = Math.round(fee * SATS);
    if (isNaN(sats) || sats <= 0) { setError('Amount must be positive'); return; }
    if (!isValidAddress(hunterAddress.trim())) { setError('Invalid hunter address'); return; }

    setLoading(true);
    try {
      const passkey = getPasskeyInfo();
      if (passkey && wallet.address === passkey.address) {
        const balRes = await fetch(`${getNodeUrl()}/balance/${wallet.address}`, { signal: AbortSignal.timeout(5000) });
        const balData = await balRes.json();
        await signAndSubmitWithPasskey(normalizeAddress(hunterAddress.trim()), sats, feeSats, balData.nonce ?? 0, memo);
      } else {
        await postTx({
          secret_key: wallet.secret_key,
          to: normalizeAddress(hunterAddress.trim()),
          amount: sats, fee: feeSats, memo,
        });
      }
      toast(`Paid ${amount} UDAG for bounty #${bounty.issue.number}`, 'success');
      onSuccess();
      onClose();
    } catch (e: unknown) {
      const msg = e instanceof Error ? e.message : 'Payment failed';
      setError(msg);
      toast(msg, 'error');
    } finally { setLoading(false); }
  };

  return (
    <div style={{ position: 'fixed', inset: 0, background: 'rgba(0,0,0,0.6)', display: 'flex', alignItems: 'center', justifyContent: 'center', zIndex: 50, padding: 16 }}
      onClick={e => { if (e.target === e.currentTarget) onClose(); }}>
      <div style={{
        background: 'var(--dag-card)', border: '1px solid var(--dag-border)', borderRadius: 14,
        padding: '20px 22px', width: '100%', maxWidth: 440, maxHeight: '90vh', overflowY: 'auto',
      }} onClick={e => e.stopPropagation()}>
        <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: 16 }}>
          <h3 style={{ fontSize: 16, fontWeight: 700, color: 'var(--dag-text)' }}>Pay Bounty #{bounty.issue.number}</h3>
          <button onClick={onClose} style={{ background: 'none', border: 'none', color: 'var(--dag-text-faint)', fontSize: 18, cursor: 'pointer' }}>✕</button>
        </div>

        <div style={{
          padding: '10px 12px', background: 'var(--dag-input-bg)', borderRadius: 8, marginBottom: 16,
          fontSize: 11, color: 'var(--dag-text-muted)', lineHeight: 1.5,
        }}>
          {bounty.issue.title.replace(/\[\d+(?:\.\d+)?\s*UDAG\]/i, '').trim()}
        </div>

        {/* Wallet selector */}
        {wallets.length > 1 && (
          <div style={{ marginBottom: 12 }}>
            <label style={{ fontSize: 10, color: 'var(--dag-text-faint)', letterSpacing: 1, display: 'block', marginBottom: 4 }}>FROM WALLET</label>
            <select value={walletIdx} onChange={e => setWalletIdx(Number(e.target.value))} style={{ ...inputStyle, cursor: 'pointer' }}>
              {wallets.map((w, i) => <option key={i} value={i} style={{ background: '#0B1120' }}>{w.name}</option>)}
            </select>
          </div>
        )}

        {/* Hunter address */}
        <div style={{ marginBottom: 12 }}>
          <label style={{ fontSize: 10, color: 'var(--dag-text-faint)', letterSpacing: 1, display: 'block', marginBottom: 4 }}>HUNTER ADDRESS (ULTRA ID or address)</label>
          <input type="text" value={hunterAddress} onChange={e => setHunterAddress(e.target.value)}
            placeholder="@hunter or tudg1..." style={inputStyle} />
        </div>

        {/* Amount */}
        <div style={{ marginBottom: 12 }}>
          <label style={{ fontSize: 10, color: 'var(--dag-text-faint)', letterSpacing: 1, display: 'block', marginBottom: 4 }}>AMOUNT (UDAG)</label>
          <input type="text" value={amount} onChange={e => setAmount(e.target.value)}
            placeholder="0.00" style={inputStyle} />
        </div>

        {/* Memo (read-only) */}
        <div style={{ marginBottom: 16 }}>
          <label style={{ fontSize: 10, color: 'var(--dag-text-faint)', letterSpacing: 1, display: 'block', marginBottom: 4 }}>MEMO (auto-generated)</label>
          <div style={{ ...inputStyle, background: 'var(--dag-input-bg)', opacity: 0.6, fontFamily: "'DM Mono',monospace", fontSize: 12 }}>{memo}</div>
        </div>

        {error && (
          <div style={{ marginBottom: 12, fontSize: 11, color: '#EF4444', background: 'rgba(239,68,68,0.06)', border: '1px solid rgba(239,68,68,0.15)', borderRadius: 8, padding: '8px 12px' }}>
            {error}
          </div>
        )}

        <button onClick={handlePay} disabled={loading} style={{
          width: '100%', padding: '12px 0', borderRadius: 10, border: 'none',
          background: loading ? 'var(--dag-input-bg)' : 'linear-gradient(135deg, #00E0C4, #0066FF)',
          color: loading ? 'var(--dag-text-faint)' : '#fff',
          fontSize: 13, fontWeight: 700, cursor: loading ? 'default' : 'pointer',
          boxShadow: loading ? 'none' : '0 2px 12px rgba(0,224,196,0.2)',
          transition: 'all 0.2s',
        }}>
          {loading ? 'Sending...' : `Pay ${amount || '0'} UDAG`}
        </button>
      </div>
    </div>
  );
}
