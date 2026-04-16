import { useState, useEffect, useCallback } from 'react';
import { getPasskeyWallet } from '../lib/passkey-wallet';
import { signAndSubmitSmartOp } from '../lib/webauthn-sign';
import { VerifiedAddressInput } from '../components/shared/VerifiedAddressInput';
import { Pagination } from '../components/shared/Pagination';
import { useIsMobile } from '../hooks/useIsMobile';
import { PageHeader } from '../components/shared/PageHeader';
import { primaryButtonStyle, buttonStyle as themeButtonStyle } from '../lib/theme';

/**
 * Fetch the current nonce for the wallet address. Used before any SmartOp
 * submission so the tx lands at the expected sequence number.
 */
async function fetchNonce(nodeUrl: string, addr: string): Promise<number> {
  const res = await fetch(`${nodeUrl}/balance/${addr}`, { signal: AbortSignal.timeout(5000) });
  const data = await res.json();
  return data.nonce ?? 0;
}

interface AuthorizedKey { key_id: string; key_type: string; label: string; daily_limit: number | null }
interface SmartAccountInfo {
  address: string; created_at_round: number; authorized_keys: AuthorizedKey[];
  has_recovery: boolean; guardian_count: number | null; recovery_threshold: number | null;
  has_pending_recovery: boolean; has_policy: boolean; instant_limit: number | null;
  vault_threshold: number | null; daily_limit: number | null; pending_vault_count: number;
  pending_key_removal: { key_id: string; executes_at_round: number } | null;
}
interface NameInfo { name: string; address: string; expiry_round: number | null; is_perpetual?: boolean }

const SATS = 100_000_000;
const fmt = (s: number) => (s / SATS).toFixed(4);

const S = {
  card: { background: 'var(--dag-card)', border: '1px solid var(--dag-border)', borderRadius: 14, padding: '20px 22px' } as React.CSSProperties,
  label: { fontSize: 10.5, fontWeight: 600, color: 'var(--dag-text-muted)', letterSpacing: 1.2, textTransform: 'uppercase' as const, marginBottom: 6 },
  input: { width: '100%', padding: '10px 14px', borderRadius: 10, background: 'var(--dag-input-bg)', border: '1px solid var(--dag-border)', color: 'var(--dag-text)', fontSize: 13, outline: 'none', fontFamily: "'DM Sans',sans-serif" } as React.CSSProperties,
  btn: (c = '#00E0C4') => ({ padding: '7px 14px', borderRadius: 8, background: `${c}12`, border: `1px solid ${c}25`, color: c, fontSize: 11, fontWeight: 600, cursor: 'pointer', transition: 'all 0.2s' }),
  btnSolid: (c = '#00E0C4') => ({ padding: '8px 16px', borderRadius: 8, background: c, color: '#080C14', fontSize: 11.5, fontWeight: 700, cursor: 'pointer', border: 'none' }),
  mono: { fontFamily: "'DM Mono',monospace" },
  stat: { background: 'var(--dag-card)', borderRadius: 10, padding: '10px 13px' } as React.CSSProperties,
};

function Section({ icon, color, title, action, children }: { icon: string; color: string; title: string; action?: React.ReactNode; children: React.ReactNode }) {
  return (
    <div style={S.card}>
      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: 16 }}>
        <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
          <span style={{ color, fontSize: 16 }}>{icon}</span>
          <span style={{ fontSize: 13.5, fontWeight: 600, color: 'var(--dag-text-secondary)' }}>{title}</span>
        </div>
        {action}
      </div>
      {children}
    </div>
  );
}

export function SmartAccountPage({ walletAddress, nodeUrl }: { walletAddress?: string; nodeUrl: string }) {
  const m = useIsMobile();
  const [info, setInfo] = useState<SmartAccountInfo | null>(null);
  const [nameInfo, setNameInfo] = useState<NameInfo | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState('');
  const [showNameForm, setShowNameForm] = useState(false);
  const [nameInput, setNameInput] = useState('');
  const [nameAvail, setNameAvail] = useState<boolean | null>(null);
  const [nameChecking, setNameChecking] = useState(false);
  const [nameFee, setNameFee] = useState(0);
  const [nameWarn, setNameWarn] = useState('');
  const [showRecoveryForm, setShowRecoveryForm] = useState(false);
  const [guardians, setGuardians] = useState(['', '', '']);
  const [threshold, setThreshold] = useState(2);
  const [showPolicyForm, setShowPolicyForm] = useState(false);
  const [keyPage, setKeyPage] = useState(1);
  const SA_PAGE_SIZE = 10;
  const [policyInstant, setPolicyInstant] = useState('');
  const [policyVault, setPolicyVault] = useState('');
  const [policyDaily, setPolicyDaily] = useState('');
  const [nameSubmitting, setNameSubmitting] = useState(false);
  const [nameError, setNameError] = useState('');
  const [recoverySubmitting, setRecoverySubmitting] = useState(false);
  const [recoveryError, setRecoveryError] = useState('');
  const [policySubmitting, setPolicySubmitting] = useState(false);
  const [policyError, setPolicyError] = useState('');

  // Link another device (AddKey SmartOp) — create a cross-platform passkey on
  // the scanning device, then register its P256 pubkey on-chain.
  const [linking, setLinking] = useState(false);
  const [linkMsg, setLinkMsg] = useState<{ kind: 'ok' | 'err'; text: string } | null>(null);

  // Remove Key — per-row 3-second confirm flow. confirmRemoveKeyId holds the
  // key being confirmed; removeKeyId holds the in-flight removal target.
  const [confirmRemoveKeyId, setConfirmRemoveKeyId] = useState<string | null>(null);
  const [removingKeyId, setRemovingKeyId] = useState<string | null>(null);
  const [removeError, setRemoveError] = useState('');

  const pw = getPasskeyWallet();
  const localName = pw?.name || null;

  const fetchInfo = useCallback(async () => {
    if (!walletAddress) return;
    setLoading(true);
    try {
      const [saRes, nameRes] = await Promise.all([
        fetch(`${nodeUrl}/smart-account/${walletAddress}`, { signal: AbortSignal.timeout(5000) }).catch(() => null),
        fetch(`${nodeUrl}/name/reverse/${walletAddress}`, { signal: AbortSignal.timeout(5000) }).catch(() => null),
      ]);
      if (saRes?.ok) setInfo(await saRes.json()); else setInfo(null);
      if (nameRes?.ok) setNameInfo(await nameRes.json()); else setNameInfo(null);
    } catch { setError('Failed to fetch'); }
    finally { setLoading(false); }
  }, [walletAddress, nodeUrl]);

  useEffect(() => { fetchInfo(); }, [fetchInfo]);

  const handleRegisterName = useCallback(async () => {
    if (!nameAvail || !nameInput || !walletAddress) return;
    setNameError('');
    setNameSubmitting(true);
    try {
      const balRes = await fetch(`${nodeUrl}/balance/${walletAddress}`, { signal: AbortSignal.timeout(5000) });
      const balData = await balRes.json();
      // RegisterName is fee-exempt for 6+ char names (see Rust is_fee_exempt).
      await signAndSubmitSmartOp(
        { RegisterName: { name: nameInput, duration_years: 1 } },
        0,
        balData.nonce ?? 0,
      );
      setShowNameForm(false);
      setNameInput('');
      setNameAvail(null);
      await fetchInfo();
    } catch (e: unknown) {
      setNameError(e instanceof Error ? e.message : 'Failed to register name');
    } finally {
      setNameSubmitting(false);
    }
  }, [nameAvail, nameInput, walletAddress, nodeUrl, fetchInfo]);

  const COIN = 100_000_000n;
  const handleSubmitRecovery = useCallback(async () => {
    setRecoveryError('');
    const cleaned = guardians.map(g => g.trim()).filter(g => g.length > 0);
    if (cleaned.length === 0) { setRecoveryError('Add at least one guardian'); return; }
    if (threshold < 1 || threshold > cleaned.length) { setRecoveryError('Threshold out of range'); return; }
    setRecoverySubmitting(true);
    try {
      const balRes = await fetch(`${nodeUrl}/balance/${walletAddress}`, { signal: AbortSignal.timeout(5000) });
      const balData = await balRes.json();
      // MIN_RECOVERY_DELAY_ROUNDS = 5000 rounds (~2.8 hours at 2s/round).
      await signAndSubmitSmartOp(
        { SetRecovery: { guardians: cleaned, threshold, delay_rounds: 5000 } },
        0,
        balData.nonce ?? 0,
      );
      setShowRecoveryForm(false);
      setGuardians(['', '', '']);
      setThreshold(2);
      await fetchInfo();
    } catch (e: unknown) {
      setRecoveryError(e instanceof Error ? e.message : 'Failed to save guardians');
    } finally {
      setRecoverySubmitting(false);
    }
  }, [guardians, threshold, walletAddress, nodeUrl, fetchInfo]);

  const handleSubmitPolicy = useCallback(async () => {
    setPolicyError('');
    const instant = policyInstant ? BigInt(Math.floor(Number(policyInstant) * 1e8)) : 0n;
    const vault = policyVault ? BigInt(Math.floor(Number(policyVault) * 1e8)) : 0n;
    const daily = policyDaily ? BigInt(Math.floor(Number(policyDaily) * 1e8)) : null;
    if (vault > 0n && instant > vault) { setPolicyError('Instant limit cannot exceed vault threshold'); return; }
    setPolicySubmitting(true);
    try {
      const balRes = await fetch(`${nodeUrl}/balance/${walletAddress}`, { signal: AbortSignal.timeout(5000) });
      const balData = await balRes.json();
      await signAndSubmitSmartOp(
        {
          SetPolicy: {
            instant_limit: instant.toString(),
            vault_threshold: vault.toString(),
            vault_delay_rounds: 5000, // ~2.8h default
            whitelisted_recipients: [],
            daily_limit: daily === null ? null : daily.toString(),
          },
        },
        0,
        balData.nonce ?? 0,
      );
      setShowPolicyForm(false);
      setPolicyInstant(''); setPolicyVault(''); setPolicyDaily('');
      await fetchInfo();
    } catch (e: unknown) {
      setPolicyError(e instanceof Error ? e.message : 'Failed to save policy');
    } finally {
      setPolicySubmitting(false);
    }
  }, [policyInstant, policyVault, policyDaily, walletAddress, nodeUrl, fetchInfo]);
  void COIN;

  const checkName = useCallback(async (name: string) => {
    setNameInput(name); setNameAvail(null); setNameWarn('');
    if (name.length < 3) return;
    setNameChecking(true);
    try {
      const res = await fetch(`${nodeUrl}/name/available/${encodeURIComponent(name)}`, { signal: AbortSignal.timeout(5000) });
      if (res.ok) {
        const d = await res.json();
        setNameAvail(d.available);
        setNameFee(d.annual_fee || 0);
        if (d.similar_warning) setNameWarn(d.similar_warning);
        if (!d.valid) setNameWarn('Invalid name format');
        if (!d.available) setNameWarn('Name taken');
      }
    } catch {} finally { setNameChecking(false); }
  }, [nodeUrl]);

  /**
   * Link another device by creating a cross-platform passkey and registering
   * its P256 pubkey on-chain via SmartOpType::AddKey. Mirrors the onboarding
   * handleLinkDevice flow but uses the already-persisted wallet.
   */
  const handleLinkDevice = useCallback(async () => {
    if (!walletAddress || !pw) return;
    setLinkMsg(null);
    setLinking(true);
    try {
      if (!window.PublicKeyCredential) {
        throw new Error('WebAuthn is not supported on this device.');
      }
      const challenge = crypto.getRandomValues(new Uint8Array(32));
      const credential = await navigator.credentials.create({
        publicKey: {
          challenge,
          rp: { name: 'UltraDAG Wallet', id: window.location.hostname },
          user: {
            id: crypto.getRandomValues(new Uint8Array(16)),
            name: pw.name ? `ultradag-${pw.name}-backup-${Date.now()}` : `ultradag-backup-${Date.now()}`,
            displayName: pw.name ? `UltraDAG @${pw.name} (Backup)` : 'UltraDAG Backup',
          },
          pubKeyCredParams: [{ alg: -7, type: 'public-key' }],
          authenticatorSelection: {
            authenticatorAttachment: 'cross-platform',
            userVerification: 'required',
            residentKey: 'preferred',
          },
          timeout: 120000,
        },
      }) as PublicKeyCredential | null;
      if (!credential) { setLinkMsg({ kind: 'err', text: 'Cancelled.' }); return; }

      // Extract compressed SEC1 P256 pubkey (33 bytes).
      const attestation = credential.response as AuthenticatorAttestationResponse;
      const pubkeyCose = attestation.getPublicKey?.();
      if (!pubkeyCose) throw new Error('Could not extract public key from new device.');
      const spkiKey = new Uint8Array(pubkeyCose);
      const cryptoKey = await crypto.subtle.importKey('spki', spkiKey, { name: 'ECDSA', namedCurve: 'P-256' }, true, ['verify']);
      const rawKey = new Uint8Array(await crypto.subtle.exportKey('raw', cryptoKey));
      let compressed: Uint8Array;
      if (rawKey.length === 65 && rawKey[0] === 0x04) {
        compressed = new Uint8Array(33);
        compressed[0] = (rawKey[64] & 1) === 0 ? 0x02 : 0x03;
        compressed.set(rawKey.slice(1, 33), 1);
      } else if (rawKey.length === 33) {
        compressed = rawKey;
      } else {
        throw new Error('Unexpected key format from new device.');
      }
      const hexNewPubkey = Array.from(compressed).map(b => b.toString(16).padStart(2, '0')).join('');

      const nonce = await fetchNonce(nodeUrl, walletAddress);
      const label = `Backup ${new Date().toLocaleDateString()}`;
      await signAndSubmitSmartOp(
        { AddKey: { key_type: 'p256', pubkey: hexNewPubkey, label } },
        0, // fee-exempt
        nonce,
      );
      setLinkMsg({ kind: 'ok', text: `Linked "${label}". It will appear once the tx finalizes.` });
      // Give the network a moment, then refresh.
      setTimeout(() => { fetchInfo(); }, 1500);
    } catch (e: unknown) {
      const msg = e instanceof Error ? e.message : 'Unknown error';
      if (msg.includes('NotAllowed') || msg.toLowerCase().includes('cancelled')) {
        setLinkMsg({ kind: 'err', text: 'Cancelled. No key was added.' });
      } else {
        setLinkMsg({ kind: 'err', text: `Could not link device: ${msg}` });
      }
    } finally {
      setLinking(false);
    }
  }, [walletAddress, pw, nodeUrl, fetchInfo]);

  /**
   * Initiate time-locked key removal via SmartOpType::RemoveKey. The actual
   * removal executes after KEY_REMOVAL_DELAY_ROUNDS (~2.8h). The engine
   * refuses to remove the last remaining key.
   */
  const handleRemoveKey = useCallback(async (keyIdHex: string) => {
    if (!walletAddress) return;
    setRemoveError('');
    setRemovingKeyId(keyIdHex);
    try {
      const nonce = await fetchNonce(nodeUrl, walletAddress);
      await signAndSubmitSmartOp(
        { RemoveKey: { key_id_to_remove: keyIdHex } },
        0, // fee-exempt
        nonce,
      );
      setConfirmRemoveKeyId(null);
      setTimeout(() => { fetchInfo(); }, 1500);
    } catch (e: unknown) {
      setRemoveError(e instanceof Error ? e.message : 'Failed to remove key');
    } finally {
      setRemovingKeyId(null);
    }
  }, [walletAddress, nodeUrl, fetchInfo]);

  if (!walletAddress) return <div style={{ padding: '18px 26px', color: 'var(--dag-text-muted)', fontSize: 13, fontFamily: "'DM Sans',sans-serif" }}>Create a wallet first.</div>;
  if (loading) return <div style={{ padding: '18px 26px', color: 'var(--dag-text-muted)', fontSize: 13, fontFamily: "'DM Sans',sans-serif", animation: 'pulse 1.5s infinite' }}>Loading SmartAccount...</div>;

  return (
    <div style={{ padding: m ? '12px 14px' : '18px 26px', fontFamily: "'DM Sans',sans-serif" }}>
      <style>{`@keyframes slideUp{from{opacity:0;transform:translateY(10px)}to{opacity:1;transform:translateY(0)}} @keyframes pulse{0%,100%{opacity:1}50%{opacity:.4}} input:focus,select:focus{border-color:rgba(0,224,196,0.3)!important}`}</style>

      <PageHeader
        title="Security"
        subtitle="Keys, recovery, spending limits, and name"
        right={<button onClick={fetchInfo} style={S.btn()}>↻ Refresh</button>}
      />

      {error && <div style={{ fontSize: 11, color: '#EF4444', background: 'rgba(239,68,68,0.06)', border: '1px solid rgba(239,68,68,0.15)', borderRadius: 8, padding: '8px 12px', marginBottom: 14 }}>{error}</div>}

      <div style={{ display: 'grid', gridTemplateColumns: m ? '1fr' : '1fr 1fr', gap: 14, animation: 'slideUp 0.4s ease' }}>

        {/* ── Name ── */}
        <Section icon="◎" color="#00E0C4" title="Your Name"
          action={!nameInfo && !localName && !showNameForm ? <button onClick={() => setShowNameForm(true)} style={S.btn()}>+ Claim</button> : undefined}>
          {nameInfo ? (
            <div>
              <div style={{ display: 'flex', alignItems: 'center', gap: 8, marginBottom: 6, flexWrap: 'wrap' }}>
                <span style={{ fontSize: 24, fontWeight: 700, color: '#00E0C4' }}>@{nameInfo.name}</span>
                <span style={{ fontSize: 8.5, background: 'rgba(0,224,196,0.12)', color: '#00E0C4', padding: '2px 7px', borderRadius: 4, fontWeight: 600 }}>ON-CHAIN</span>
                {nameInfo.is_perpetual && (
                  <span title="Free-tier names (6+ chars) are permanent — no expiry, no renewal required"
                    style={{ fontSize: 8.5, background: 'rgba(255,184,0,0.12)', color: '#FFB800', padding: '2px 7px', borderRadius: 4, fontWeight: 600 }}>
                    ★ PERMANENT
                  </span>
                )}
              </div>
              {nameInfo.is_perpetual ? (
                <p style={{ fontSize: 10.5, color: 'var(--dag-text-faint)' }}>
                  Yours forever — no renewal required.
                </p>
              ) : nameInfo.expiry_round != null ? (
                <p style={{ fontSize: 10.5, color: 'var(--dag-text-faint)' }}>
                  Rented — expires round {nameInfo.expiry_round.toLocaleString()}
                </p>
              ) : null}
            </div>
          ) : localName ? (
            <div>
              <div style={{ display: 'flex', alignItems: 'center', gap: 8, marginBottom: 6 }}>
                <span style={{ fontSize: 24, fontWeight: 700, color: '#00E0C4' }}>{localName}</span>
                <span style={{ fontSize: 8.5, background: 'rgba(255,184,0,0.12)', color: '#FFB800', padding: '2px 7px', borderRadius: 4, fontWeight: 600 }}>LOCAL</span>
              </div>
              <p style={{ fontSize: 10.5, color: 'var(--dag-text-faint)' }}>Saved locally. On-chain registration available via SmartOp.</p>
            </div>
          ) : showNameForm ? (
            <div style={{ display: 'flex', flexDirection: 'column', gap: 10 }}>
              <div style={{ position: 'relative' }}>
                <input type="text" value={nameInput} onChange={e => checkName(e.target.value.toLowerCase().replace(/[^a-z0-9-]/g, ''))}
                  placeholder="john29" maxLength={20} autoFocus style={S.input} />
                {nameChecking && <span style={{ position: 'absolute', right: 12, top: 11, color: 'var(--dag-text-faint)', fontSize: 11 }}>...</span>}
                {nameAvail === true && <span style={{ position: 'absolute', right: 12, top: 10, color: '#00E0C4', fontSize: 14 }}>✓</span>}
              </div>
              {nameWarn && <p style={{ fontSize: 10, color: '#FFB800' }}>{nameWarn}</p>}
              {nameAvail && <p style={{ fontSize: 10, color: '#00E0C4' }}>{nameInput} available! {nameFee > 0 ? `${fmt(nameFee)} UDAG/yr` : 'Free'}</p>}
              {nameError && <p role="alert" style={{ fontSize: 10, color: '#EF4444' }}>{nameError}</p>}
              <div style={{ display: 'flex', gap: 8 }}>
                <button onClick={handleRegisterName} disabled={!nameAvail || !nameInput || nameSubmitting}
                  style={{ ...S.btnSolid(), opacity: !nameAvail || !nameInput || nameSubmitting ? 0.4 : 1 }}>
                  {nameSubmitting ? 'Signing...' : 'Register Name'}
                </button>
                <button onClick={() => { setShowNameForm(false); setNameError(''); }} style={S.btn('var(--dag-text-muted)')}>Cancel</button>
              </div>
            </div>
          ) : (
            <p style={{ fontSize: 11.5, color: 'var(--dag-text-faint)' }}>No name registered. Claim one like <span style={{ color: '#00E0C4' }}>alice</span> or <span style={{ color: '#00E0C4' }}>john29</span>.</p>
          )}
        </Section>

        {/* ── Keys ── */}
        {/*
         * The "+ Link Another Device" button is shown whenever the user has a
         * primary passkey wallet locally (`pw !== null`), regardless of whether
         * the SmartAccount has any keys registered on-chain yet. The AddKey
         * SmartOp envelope carries the primary's `p256_pubkey` for auto-
         * registration, so the first AddKey call from a fresh address correctly
         * registers BOTH the primary and the new backup device in a single tx.
         *
         * Earlier versions of this page gated the button on
         * `info && info.authorized_keys.length > 0`, which meant users whose
         * SmartAccount hadn't been activated yet had no way to add a backup —
         * exactly when they'd want to most.
         */}
        <Section icon="◇" color="#0066FF" title="Authorized Keys"
          action={pw ? (
            <button
              onClick={handleLinkDevice}
              disabled={linking}
              style={{ ...S.btn(), opacity: linking ? 0.5 : 1, cursor: linking ? 'wait' : 'pointer' }}
              title="Create a new passkey on another device and register it on-chain"
            >
              {linking ? 'Linking…' : '+ Link Another Device'}
            </button>
          ) : undefined}>
          {info && info.authorized_keys.length > 0 ? (
            <div style={{ display: 'flex', flexDirection: 'column', gap: 8 }}>
              {info.authorized_keys.slice((keyPage - 1) * SA_PAGE_SIZE, keyPage * SA_PAGE_SIZE).map(key => {
                const isOnlyKey = info.authorized_keys.length <= 1;
                const isPending = info.pending_key_removal?.key_id === key.key_id;
                const isConfirming = confirmRemoveKeyId === key.key_id;
                const isRemoving = removingKeyId === key.key_id;
                return (
                  <div key={key.key_id} style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', gap: 10, background: 'var(--dag-card)', borderRadius: 10, padding: '10px 13px' }}>
                    <div style={{ display: 'flex', alignItems: 'center', gap: 10, minWidth: 0, flex: 1 }}>
                      <div style={{ width: 32, height: 32, flexShrink: 0, borderRadius: 8, display: 'flex', alignItems: 'center', justifyContent: 'center', background: key.key_type === 'p256' ? 'rgba(168,85,247,0.12)' : 'rgba(0,102,255,0.12)', fontSize: 14 }}>
                        {key.key_type === 'p256' ? '◎' : '◇'}
                      </div>
                      <div style={{ minWidth: 0 }}>
                        <div style={{ fontSize: 12, fontWeight: 600, color: 'var(--dag-text)', overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>{key.label}</div>
                        <div style={{ fontSize: 10, color: 'var(--dag-text-faint)', ...S.mono }}>{key.key_id.slice(0, 12)}… ({key.key_type === 'p256' ? 'Passkey' : 'Ed25519'})</div>
                      </div>
                    </div>
                    <div style={{ display: 'flex', alignItems: 'center', gap: 8, flexShrink: 0 }}>
                      <div style={{ fontSize: 10, color: 'var(--dag-subheading)' }}>{key.daily_limit ? `${fmt(key.daily_limit)}/day` : 'No limit'}</div>
                      {isPending ? (
                        <span style={{ fontSize: 9.5, color: '#FFB800', background: 'rgba(255,184,0,0.1)', padding: '3px 7px', borderRadius: 6, fontWeight: 600 }}>REMOVING</span>
                      ) : isConfirming ? (
                        <div style={{ display: 'flex', gap: 4 }}>
                          <button
                            onClick={() => handleRemoveKey(key.key_id)}
                            disabled={isRemoving}
                            style={{ padding: '5px 9px', borderRadius: 6, background: '#EF4444', color: '#fff', border: 'none', fontSize: 10, fontWeight: 700, cursor: isRemoving ? 'wait' : 'pointer', opacity: isRemoving ? 0.6 : 1 }}
                          >
                            {isRemoving ? 'Signing…' : 'Confirm Remove'}
                          </button>
                          <button
                            onClick={() => { setConfirmRemoveKeyId(null); setRemoveError(''); }}
                            disabled={isRemoving}
                            style={{ padding: '5px 9px', borderRadius: 6, background: 'transparent', color: 'var(--dag-text-muted)', border: '1px solid var(--dag-border)', fontSize: 10, fontWeight: 600, cursor: 'pointer' }}
                          >
                            Cancel
                          </button>
                        </div>
                      ) : (
                        <button
                          onClick={() => { setConfirmRemoveKeyId(key.key_id); setRemoveError(''); }}
                          disabled={isOnlyKey || !!info.pending_key_removal}
                          title={isOnlyKey ? 'Cannot remove the only key — add another first' : info.pending_key_removal ? 'Another key removal is already pending' : 'Start time-locked removal'}
                          style={{ padding: '5px 9px', borderRadius: 6, background: 'transparent', color: isOnlyKey || info.pending_key_removal ? 'var(--dag-text-faint)' : '#EF4444', border: `1px solid ${isOnlyKey || info.pending_key_removal ? 'var(--dag-border)' : 'rgba(239,68,68,0.3)'}`, fontSize: 10, fontWeight: 600, cursor: isOnlyKey || info.pending_key_removal ? 'not-allowed' : 'pointer' }}
                        >
                          Remove
                        </button>
                      )}
                    </div>
                  </div>
                );
              })}
              {info.pending_key_removal && (
                <div style={{ fontSize: 10.5, color: '#FFB800', background: 'rgba(255,184,0,0.06)', border: '1px solid rgba(255,184,0,0.15)', borderRadius: 8, padding: '8px 12px', display: 'flex', alignItems: 'center', gap: 6 }}>
                  ◷ Key removal pending — executes at round {info.pending_key_removal.executes_at_round.toLocaleString()} (~2.8h time-lock)
                </div>
              )}
              {linkMsg && (
                <div role="status" style={{
                  fontSize: 10.5,
                  color: linkMsg.kind === 'ok' ? '#00E0C4' : '#EF4444',
                  background: linkMsg.kind === 'ok' ? 'rgba(0,224,196,0.06)' : 'rgba(239,68,68,0.06)',
                  border: `1px solid ${linkMsg.kind === 'ok' ? 'rgba(0,224,196,0.15)' : 'rgba(239,68,68,0.15)'}`,
                  borderRadius: 8, padding: '8px 12px',
                }}>
                  {linkMsg.text}
                </div>
              )}
              {removeError && (
                <div role="alert" style={{ fontSize: 10.5, color: '#EF4444', background: 'rgba(239,68,68,0.06)', border: '1px solid rgba(239,68,68,0.15)', borderRadius: 8, padding: '8px 12px' }}>
                  {removeError}
                </div>
              )}
              <Pagination page={keyPage} totalPages={Math.ceil(info.authorized_keys.length / SA_PAGE_SIZE)} onPageChange={setKeyPage} totalItems={info.authorized_keys.length} pageSize={SA_PAGE_SIZE} />
            </div>
          ) : (
            <div style={{ display: 'flex', flexDirection: 'column', gap: 8 }}>
              <p style={{ fontSize: 11.5, color: 'var(--dag-text-faint)' }}>
                {info
                  ? 'No keys registered on-chain yet. Your primary passkey auto-registers on your first transaction — or tap + Link Another Device above to add a backup device now (both primary and backup will register in the same transaction).'
                  : 'SmartAccount activates when you send or receive a transaction, OR when you tap + Link Another Device above to add a backup passkey.'}
              </p>
              {linkMsg && (
                <div role="status" style={{
                  fontSize: 10.5,
                  color: linkMsg.kind === 'ok' ? '#00E0C4' : '#EF4444',
                  background: linkMsg.kind === 'ok' ? 'rgba(0,224,196,0.06)' : 'rgba(239,68,68,0.06)',
                  border: `1px solid ${linkMsg.kind === 'ok' ? 'rgba(0,224,196,0.15)' : 'rgba(239,68,68,0.15)'}`,
                  borderRadius: 8, padding: '8px 12px',
                }}>
                  {linkMsg.text}
                </div>
              )}
            </div>
          )}
        </Section>

        {/* ── Recovery ── */}
        <Section icon="♛" color="#A855F7" title="Social Recovery"
          action={!info?.has_recovery && !showRecoveryForm ? <button onClick={() => setShowRecoveryForm(true)} style={themeButtonStyle()}>+ Set Up</button> : undefined}>
          {info?.has_recovery ? (
            <div>
              <div style={{ display: 'flex', alignItems: 'center', gap: 6, marginBottom: 6 }}>
                <span style={{ color: '#00E0C4' }}>✓</span>
                <span style={{ fontSize: 12, fontWeight: 600, color: '#00E0C4' }}>Configured</span>
              </div>
              <p style={{ fontSize: 11, color: 'var(--dag-text-muted)' }}>{info.recovery_threshold}-of-{info.guardian_count} guardians required to recover</p>
              <p style={{ fontSize: 10, color: 'var(--dag-text-faint)', marginTop: 4 }}>If you lose all devices, contact your guardians. Each one opens their wallet and approves your recovery. After {info.recovery_threshold} approve, a time-lock starts — then your new device gets access.</p>
              {info.has_pending_recovery && (
                <div style={{ fontSize: 10.5, color: '#FFB800', background: 'rgba(255,184,0,0.06)', border: '1px solid rgba(255,184,0,0.15)', borderRadius: 8, padding: '8px 12px', marginTop: 8 }}>
                  ⚠ A recovery is in progress — if you didn't initiate this, cancel it immediately from any authorized device!
                </div>
              )}
            </div>
          ) : showRecoveryForm ? (
            <div style={{ display: 'flex', flexDirection: 'column', gap: 10 }}>
              {recoveryError && (
                <div role="alert" style={{ fontSize: 10.5, color: '#FF4444', background: 'rgba(255,68,68,0.06)', border: '1px solid rgba(255,68,68,0.15)', borderRadius: 8, padding: '8px 12px' }}>
                  {recoveryError}
                </div>
              )}

              {/* Explainer */}
              <div style={{ background: 'rgba(168,85,247,0.04)', border: '1px solid rgba(168,85,247,0.1)', borderRadius: 10, padding: '12px 14px' }}>
                <div style={{ fontSize: 11.5, fontWeight: 600, color: '#A855F7', marginBottom: 6 }}>How recovery works</div>
                <ol style={{ fontSize: 10.5, color: 'var(--dag-text-muted)', lineHeight: 1.7, margin: 0, paddingLeft: 16 }}>
                  <li>Pick friends or family who have UltraDAG wallets</li>
                  <li>If you lose all devices, call them</li>
                  <li>Each guardian opens <strong style={{ color: 'var(--dag-text-secondary)' }}>their own wallet</strong> and taps "Approve Recovery"</li>
                  <li>When enough guardians approve, a time-lock starts (~2.8 hours)</li>
                  <li>After the time-lock, your new device gets access to your account</li>
                </ol>
                <p style={{ fontSize: 10, color: 'var(--dag-text-faint)', marginTop: 8 }}>Guardians can never access your funds — they can only authorize a key change.</p>
              </div>

              {/* Guardian inputs */}
              <div style={{ fontSize: 10.5, fontWeight: 600, color: 'var(--dag-text-muted)', letterSpacing: 1, marginTop: 4 }}>GUARDIAN WALLET ADDRESSES</div>
              {guardians.map((g, i) => (
                <VerifiedAddressInput key={i} value={g}
                  onChange={v => { const n = [...guardians]; n[i] = v; setGuardians(n); }}
                  placeholder={`Guardian ${i + 1} — paste their wallet address`} />
              ))}
              <button onClick={() => setGuardians([...guardians, ''])} style={{ ...themeButtonStyle(), alignSelf: 'flex-start', fontSize: 10 }}>+ Add another guardian</button>

              <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
                <span style={{ fontSize: 10.5, color: 'var(--dag-subheading)' }}>How many must approve:</span>
                <select value={threshold} onChange={e => setThreshold(Number(e.target.value))}
                  style={{ ...S.input, width: 'auto', padding: '6px 10px' }}>
                  {Array.from({ length: guardians.filter(g => g.length > 0).length || 1 }, (_, i) => i + 1).map(n => (
                    <option key={n} value={n}>{n} of {guardians.filter(g => g.length > 0).length || '?'}</option>
                  ))}
                </select>
              </div>

              <div style={{ display: 'flex', gap: 8 }}>
                <button onClick={handleSubmitRecovery} disabled={recoverySubmitting} style={{ ...primaryButtonStyle, opacity: recoverySubmitting ? 0.5 : 1, cursor: recoverySubmitting ? 'wait' : 'pointer' }}>
                  {recoverySubmitting ? 'Saving…' : 'Save Guardians'}
                </button>
                <button onClick={() => setShowRecoveryForm(false)} style={S.btn('var(--dag-text-muted)')}>Cancel</button>
              </div>
            </div>
          ) : (
            <div>
              <p style={{ fontSize: 11.5, color: 'var(--dag-text-faint)' }}>No recovery guardians set.</p>
              <p style={{ fontSize: 10, color: 'var(--dag-text-faint)', marginTop: 4, lineHeight: 1.6 }}>
                If you lose all your devices, recovery guardians can authorize a new device.
                Pick 3–5 people you trust (friends, family) who have UltraDAG wallets.
                They can never access your funds — only help you regain access.
              </p>
            </div>
          )}
        </Section>

        {/* ── Spending Policy ── */}
        <Section icon="⬡" color="#FFB800" title="Spending Policy"
          action={!info?.has_policy && !showPolicyForm ? <button onClick={() => setShowPolicyForm(true)} style={themeButtonStyle()}>+ Set Up</button> : undefined}>
          {info?.has_policy ? (
            <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 8 }}>
              {[
                { l: 'INSTANT LIMIT', v: info.instant_limit != null ? `${fmt(info.instant_limit)} UDAG` : '—' },
                { l: 'VAULT THRESHOLD', v: info.vault_threshold && info.vault_threshold > 0 ? `${fmt(info.vault_threshold)} UDAG` : '—' },
                { l: 'DAILY CAP', v: info.daily_limit != null ? `${fmt(info.daily_limit)} UDAG` : '—' },
                { l: 'PENDING VAULTS', v: String(info.pending_vault_count) },
              ].map((x, i) => (
                <div key={i} style={S.stat}>
                  <div style={{ fontSize: 9, color: 'var(--dag-subheading)', letterSpacing: 1, marginBottom: 3 }}>{x.l}</div>
                  <div style={{ fontSize: 15, fontWeight: 700, color: 'var(--dag-text)' }}>{x.v}</div>
                </div>
              ))}
            </div>
          ) : showPolicyForm ? (
            <div style={{ display: 'flex', flexDirection: 'column', gap: 10 }}>
              {policyError && (
                <div role="alert" style={{ fontSize: 10.5, color: '#FF4444', background: 'rgba(255,68,68,0.06)', border: '1px solid rgba(255,68,68,0.15)', borderRadius: 8, padding: '8px 12px' }}>
                  {policyError}
                </div>
              )}
              <div>
                <span style={{ ...S.label, display: 'block' }}>Instant limit (UDAG)</span>
                <input type="number" placeholder="1" value={policyInstant} onChange={e => setPolicyInstant(e.target.value)} style={S.input} />
                <p style={{ fontSize: 10, color: 'var(--dag-text-faint)', marginTop: 4 }}>
                  Transfers at or below this amount send immediately, no delay.
                </p>
              </div>
              <div>
                <span style={{ ...S.label, display: 'block' }}>Vault threshold (UDAG)</span>
                <input type="number" placeholder="100" value={policyVault} onChange={e => setPolicyVault(e.target.value)} style={S.input} />
                <p style={{ fontSize: 10, color: 'var(--dag-text-faint)', marginTop: 4 }}>
                  Transfers above this amount are held in a vault for ~2.8h before executing — cancellable with any key.
                </p>
              </div>
              <div>
                <span style={{ ...S.label, display: 'block' }}>Daily cap (UDAG)</span>
                <input type="number" placeholder="10" value={policyDaily} onChange={e => setPolicyDaily(e.target.value)} style={S.input} />
                <p style={{ fontSize: 10, color: 'var(--dag-text-faint)', marginTop: 4 }}>
                  Total outgoing amount allowed per 24h window — resets at the first transfer past the boundary.
                </p>
              </div>
              <div style={{ display: 'flex', gap: 8 }}>
                <button onClick={handleSubmitPolicy} disabled={policySubmitting} style={{ ...primaryButtonStyle, opacity: policySubmitting ? 0.5 : 1, cursor: policySubmitting ? 'wait' : 'pointer' }}>
                  {policySubmitting ? 'Saving…' : 'Save (2.8hr time-lock)'}
                </button>
                <button onClick={() => setShowPolicyForm(false)} style={S.btn('var(--dag-text-muted)')}>Cancel</button>
              </div>
            </div>
          ) : (
            <div>
              <p style={{ fontSize: 11.5, color: 'var(--dag-text-faint)' }}>No spending limits set.</p>
              <p style={{ fontSize: 10, color: 'var(--dag-text-faint)', marginTop: 4 }}>Protect against key theft with daily limits and vault delays.</p>
            </div>
          )}
        </Section>
      </div>
    </div>
  );
}
