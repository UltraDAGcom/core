import { useState } from 'react';
import { useToast } from '../../hooks/useToast';
import { primaryButtonStyle } from '../../lib/theme';
import { getNodeUrl } from '../../lib/api';
import type { Wallet } from '../../lib/keystore';

interface EditProfileModalProps {
  name: string;
  wallet: Wallet;
  currentBio: string | null;
  currentWebsite: string | null;
  currentGithub: string | null;
  currentTwitter: string | null;
  currentExternalAddresses: Array<[string, string]>;
  onClose: () => void;
  onSuccess: () => void;
}

const inputStyle: React.CSSProperties = {
  width: '100%', padding: '10px 14px', borderRadius: 10,
  background: 'var(--dag-input-bg)', border: '1px solid var(--dag-border)',
  color: 'var(--dag-text)', fontSize: 13, outline: 'none',
  fontFamily: "'DM Sans',sans-serif",
};

const labelStyle: React.CSSProperties = {
  fontSize: 10, color: 'var(--dag-text-faint)', letterSpacing: 1,
  display: 'block', marginBottom: 4, fontWeight: 600,
};

export function EditProfileModal({ name, wallet, currentBio, currentWebsite, currentGithub, currentTwitter, currentExternalAddresses, onClose, onSuccess }: EditProfileModalProps) {
  const [bio, setBio] = useState(currentBio ?? '');
  const [website, setWebsite] = useState(currentWebsite ?? '');
  const [github, setGithub] = useState(currentGithub ?? '');
  const [twitter, setTwitter] = useState(currentTwitter ?? '');
  const [ethAddr, setEthAddr] = useState(currentExternalAddresses.find(([k]) => k === 'eth')?.[1] ?? '');
  const [btcAddr, setBtcAddr] = useState(currentExternalAddresses.find(([k]) => k === 'btc')?.[1] ?? '');
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState('');
  const { toast } = useToast();

  // Legacy wallets (with secret_key) can save. Passkey wallets can't sign
  // UpdateProfileTx yet — SmartOp variant needs to be added to the protocol.
  const isPasskey = !wallet.secret_key;

  const handleSave = async () => {
    if (isPasskey) {
      setError('Profile editing is not yet available for passkey wallets. Use a legacy (secret-key) wallet to edit profile metadata.');
      return;
    }
    setError('');
    setLoading(true);

    try {
      // Build metadata entries — only include non-empty fields
      const metadata: Array<[string, string]> = [];
      if (bio.trim()) metadata.push(['bio', bio.trim()]);
      if (website.trim()) metadata.push(['website', website.trim()]);
      if (github.trim()) metadata.push(['github', github.trim().replace(/^@/, '')]);
      if (twitter.trim()) metadata.push(['twitter', twitter.trim().replace(/^@/, '')]);

      // Build external addresses
      const externalAddresses: Array<[string, string]> = [];
      if (ethAddr.trim()) externalAddresses.push(['eth', ethAddr.trim()]);
      if (btcAddr.trim()) externalAddresses.push(['btc', btcAddr.trim()]);

      const res = await fetch(`${getNodeUrl()}/profile/update`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          secret_key: wallet.secret_key,
          name,
          metadata,
          external_addresses: externalAddresses,
          fee: 10000,
        }),
        signal: AbortSignal.timeout(10000),
      });

      if (!res.ok) {
        const text = await res.text();
        throw new Error(text || `HTTP ${res.status}`);
      }

      toast('Profile updated!', 'success');
      onSuccess();
      onClose();
    } catch (e: unknown) {
      const msg = e instanceof Error ? e.message : 'Failed to update profile';
      setError(msg);
      toast(msg, 'error');
    } finally {
      setLoading(false);
    }
  };

  return (
    <div style={{ position: 'fixed', inset: 0, background: 'rgba(0,0,0,0.6)', display: 'flex', alignItems: 'center', justifyContent: 'center', zIndex: 50, padding: 16 }}
      onClick={e => { if (e.target === e.currentTarget) onClose(); }}>
      <div style={{
        background: 'var(--dag-card)', border: '1px solid var(--dag-border)', borderRadius: 14,
        padding: '20px 22px', width: '100%', maxWidth: 460, maxHeight: '90vh', overflowY: 'auto',
      }} onClick={e => e.stopPropagation()}>

        <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: 16 }}>
          <h3 style={{ fontSize: 16, fontWeight: 700, color: 'var(--dag-text)' }}>Edit Profile — @{name}</h3>
          <button onClick={onClose} style={{ background: 'none', border: 'none', color: 'var(--dag-text-faint)', fontSize: 18, cursor: 'pointer' }}>✕</button>
        </div>

        {isPasskey && (
          <div role="note" style={{ marginBottom: 14, padding: '10px 12px', background: 'rgba(255,184,0,0.06)', border: '1px solid rgba(255,184,0,0.15)', borderRadius: 8, fontSize: 11, color: '#FFB800' }}>
            Profile editing from passkey wallets requires a protocol update (<code style={{ fontSize: 10 }}>SmartOpType::UpdateProfile</code> variant). Use a legacy secret-key wallet to edit for now.
          </div>
        )}

        <div style={{ display: 'flex', flexDirection: 'column', gap: 12 }}>
          <div>
            <label style={labelStyle}>BIO</label>
            <textarea value={bio} onChange={e => setBio(e.target.value)} maxLength={256}
              placeholder="Tell the world about yourself..." rows={3}
              style={{ ...inputStyle, resize: 'vertical', minHeight: 60 }} />
            <div style={{ textAlign: 'right', fontSize: 9, color: 'var(--dag-text-faint)', marginTop: 2 }}>{bio.length}/256</div>
          </div>

          <div>
            <label style={labelStyle}>WEBSITE</label>
            <input type="url" value={website} onChange={e => setWebsite(e.target.value)}
              placeholder="https://your-site.com" style={inputStyle} />
          </div>

          <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 10 }}>
            <div>
              <label style={labelStyle}>GITHUB</label>
              <input type="text" value={github} onChange={e => setGithub(e.target.value)}
                placeholder="username" style={inputStyle} />
            </div>
            <div>
              <label style={labelStyle}>X / TWITTER</label>
              <input type="text" value={twitter} onChange={e => setTwitter(e.target.value)}
                placeholder="handle" style={inputStyle} />
            </div>
          </div>

          <div style={{ paddingTop: 8, borderTop: '1px solid var(--dag-border)' }}>
            <div style={{ ...labelStyle, marginBottom: 8 }}>EXTERNAL ADDRESSES</div>
            <div style={{ display: 'flex', flexDirection: 'column', gap: 8 }}>
              <div>
                <label style={{ ...labelStyle, fontSize: 9 }}>ETHEREUM</label>
                <input type="text" value={ethAddr} onChange={e => setEthAddr(e.target.value)}
                  placeholder="0x..." style={inputStyle} />
              </div>
              <div>
                <label style={{ ...labelStyle, fontSize: 9 }}>BITCOIN</label>
                <input type="text" value={btcAddr} onChange={e => setBtcAddr(e.target.value)}
                  placeholder="bc1..." style={inputStyle} />
              </div>
            </div>
          </div>
        </div>

        {error && (
          <div style={{ marginTop: 12, fontSize: 11, color: '#EF4444', background: 'rgba(239,68,68,0.06)', border: '1px solid rgba(239,68,68,0.15)', borderRadius: 8, padding: '8px 12px' }}>
            {error}
          </div>
        )}

        <button onClick={handleSave} disabled={isPasskey || loading} style={{
          ...primaryButtonStyle, width: '100%', padding: '12px 0', marginTop: 16,
          opacity: isPasskey || loading ? 0.4 : 1,
          cursor: isPasskey || loading ? 'default' : 'pointer',
        }}>
          {loading ? 'Saving...' : 'Save Profile'}
        </button>
      </div>
    </div>
  );
}
