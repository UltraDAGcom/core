import { useState, useCallback, useRef, useEffect } from 'react';
import { getNodeUrl } from '../../lib/api';
import { primaryButtonStyle } from '../../lib/theme';
import type { NetworkType } from '../../lib/api';
// savePasskeyWallet is called by WelcomeScreen after backup step completes

interface PasskeyOnboardingProps {
  onComplete: (address: string, name: string | null, pendingWallet: { credentialId: string; p256PubkeyHex: string; address: string; keyId: string; name: string | null }) => void;
  onFallbackToAdvanced: () => void;
  network: NetworkType;
  onSwitchNetwork: (net: NetworkType) => void;
}

type Step = 'welcome' | 'username' | 'passkey' | 'creating' | 'success';

/** Device-aware biometric label used in the passkey step CTA. */
function biometricLabel(): string {
  if (typeof navigator === 'undefined') return 'Biometrics';
  const ua = navigator.userAgent;
  const platform = navigator.platform || '';
  if (/iPhone|iPad|iPod/.test(ua)) return 'Face ID';
  if (/Mac/.test(platform)) return 'Touch ID';
  if (/Win/.test(platform)) return 'Windows Hello';
  if (/Android/.test(ua)) return 'Fingerprint';
  return 'Biometrics';
}

export function PasskeyOnboarding({ onComplete, onFallbackToAdvanced, network, onSwitchNetwork }: PasskeyOnboardingProps) {
  const [step, setStep] = useState<Step>('welcome');
  const [p256PubkeyHex, setP256PubkeyHex] = useState('');
  const [credentialId, setCredentialId] = useState('');
  const [username, setUsername] = useState('');
  const [nameAvailable, setNameAvailable] = useState<boolean | null>(null);
  const [nameError, setNameError] = useState('');
  const [nameFee, setNameFee] = useState('');
  const [checking, setChecking] = useState(false);
  const [, setCreating] = useState(false);
  const [error, setError] = useState('');
  const [result, setResult] = useState<{ address: string; name: string | null } | null>(null);
  const [pendingWallet, setPendingWallet] = useState<{ credentialId: string; p256PubkeyHex: string; address: string; keyId: string; name: string | null } | null>(null);
  // Backup-device registration requires SmartOpType::AddKey which doesn't
  // exist in the Rust protocol yet, so the old backup step was removed.
  // Social recovery is surfaced on the final success screen instead.
  const debounceRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const abortRef = useRef<AbortController | null>(null);

  // Step 1: Create passkey via WebAuthn
  const handleCreatePasskey = useCallback(async () => {
    setError('');
    try {
      if (!window.PublicKeyCredential) {
        setError('WebAuthn is not supported on this device. Use the advanced import option.');
        return;
      }

      const challenge = crypto.getRandomValues(new Uint8Array(32));
      const credential = await navigator.credentials.create({
        publicKey: {
          challenge,
          rp: { name: 'UltraDAG Wallet', id: window.location.hostname },
          user: {
            id: crypto.getRandomValues(new Uint8Array(16)),
            name: username ? `ultradag-${username}` : 'ultradag-wallet',
            displayName: username ? `UltraDAG @${username}` : 'UltraDAG Wallet',
          },
          pubKeyCredParams: [
            { alg: -7, type: 'public-key' },  // ES256 (P-256)
          ],
          authenticatorSelection: {
            authenticatorAttachment: 'platform',
            userVerification: 'required',
            residentKey: 'preferred',
          },
          timeout: 60000,
        },
      }) as PublicKeyCredential | null;

      if (!credential) { setError('Passkey creation was cancelled.'); return; }

      const attestation = credential.response as AuthenticatorAttestationResponse;
      const pubkeyCose = attestation.getPublicKey?.();
      if (!pubkeyCose) { setError('Could not extract public key. Try a different browser or use advanced import.'); return; }

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
      } else { setError('Unexpected key format from WebAuthn.'); return; }

      const hexPubkey = Array.from(compressed).map(b => b.toString(16).padStart(2, '0')).join('');
      setP256PubkeyHex(hexPubkey);
      setCredentialId(credential.id);
      // Username already chosen — proceed to account creation
      handleCreateAccount(hexPubkey, credential.id);
    } catch (e: unknown) {
      const msg = e instanceof Error ? e.message : 'Unknown error';
      if (msg.includes('NotAllowed') || msg.includes('cancelled')) {
        setError('Biometric verification was cancelled. Try again or use advanced import.');
      } else {
        setError(`Passkey creation failed: ${msg}`);
      }
    }
  }, []);

  // Step 2: Check name availability (debounced, with abort)
  const checkNameNow = useCallback(async (name: string) => {
    // Abort any in-flight request
    if (abortRef.current) abortRef.current.abort();
    const controller = new AbortController();
    abortRef.current = controller;

    setChecking(true);
    try {
      const res = await fetch(`${getNodeUrl()}/name/available/${encodeURIComponent(name)}`, {
        signal: controller.signal,
      });
      if (controller.signal.aborted) return;
      if (res.ok) {
        const data = await res.json();
        setNameAvailable(data.available);
        if (!data.valid) {
          setNameError('Invalid name format');
        } else if (!data.available) {
          setNameError('This name is taken');
        } else {
          setNameFee(data.annual_fee_udag > 0 ? `${data.annual_fee_udag} UDAG/year` : 'Free');
          if (data.similar_warning) setNameError(data.similar_warning);
        }
      } else if (res.status === 404) {
        setNameError('Name service not available on this network. Try switching networks.');
      } else {
        setNameError('Could not check name');
      }
    } catch (e: unknown) {
      if (e instanceof DOMException && e.name === 'AbortError') return; // Cancelled by new keystroke
      setNameError('Checking...');
      // Silent retry after 2s
      setTimeout(() => { if (!controller.signal.aborted) checkNameNow(name); }, 2000);
      return;
    } finally {
      if (!controller.signal.aborted) setChecking(false);
    }
  }, []);

  const handleNameChange = useCallback((name: string) => {
    setUsername(name);
    setNameAvailable(null);
    setNameError('');
    setNameFee('');

    if (name.length < 3) {
      if (debounceRef.current) clearTimeout(debounceRef.current);
      return;
    }

    // Debounce: wait 400ms after last keystroke before checking
    if (debounceRef.current) clearTimeout(debounceRef.current);
    debounceRef.current = setTimeout(() => checkNameNow(name), 400);
  }, [checkNameNow]);

  // Re-check when network changes
  useEffect(() => {
    if (username.length >= 3) {
      setNameAvailable(null);
      setNameError('');
      if (debounceRef.current) clearTimeout(debounceRef.current);
      debounceRef.current = setTimeout(() => checkNameNow(username), 300);
    }
  }, [network]); // eslint-disable-line react-hooks/exhaustive-deps

  // Create account via relay (called after passkey creation)
  const handleCreateAccount = useCallback(async (pubkey?: string, credId?: string) => {
    const finalPubkey = pubkey || p256PubkeyHex;
    const finalCredId = credId || credentialId;
    setCreating(true);
    setError('');
    setStep('creating');

    try {
      const body: Record<string, unknown> = { p256_pubkey: finalPubkey };
      if (username.length >= 3 && nameAvailable) body.name = username;

      const res = await fetch(`${getNodeUrl()}/relay/create-account`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(body),
        signal: AbortSignal.timeout(15000),
      });

      if (!res.ok) {
        const err = await res.json().catch(() => ({ error: 'Unknown error' }));
        throw new Error(err.error || `Server error: ${res.status}`);
      }

      const data = await res.json();
      const resultData = { address: data.address, name: username.length >= 3 && nameAvailable ? username : null };
      setResult(resultData);
      setStep('success');

      // Don't save yet — wait until backup step completes so the app
      // doesn't detect pk.unlocked and unmount the onboarding flow
      setPendingWallet({
        credentialId: finalCredId, p256PubkeyHex: finalPubkey,
        address: data.address,
        keyId: data.p256_key_id,
        name: resultData.name,
      });
    } catch (e: unknown) {
      setError(e instanceof Error ? e.message : 'Account creation failed');
      setStep('passkey'); // Go back to passkey step on failure
    } finally {
      setCreating(false);
    }
  }, [p256PubkeyHex, username, nameAvailable, credentialId]);

  const inputStyle: React.CSSProperties = {
    width: '100%', padding: '12px 14px', borderRadius: 10, background: 'var(--dag-input-bg)',
    border: '1px solid var(--dag-border)', color: 'var(--dag-text)', fontSize: 14, outline: 'none',
    fontFamily: "'DM Sans',sans-serif", transition: 'border-color 0.2s',
  };

  const isMainnet = network === 'mainnet';

  // ─── Derived UI helpers ────────────────────────────────────────────────
  const bioLabel = biometricLabel();
  const isPerpetualName = username.length >= 6 && username.length <= 20;
  const isPremiumName = username.length >= 3 && username.length <= 5;
  const canContinueUsername = username.length >= 3 && nameAvailable === true;

  const networkPill = (
    <div style={{
      display: 'inline-flex', alignItems: 'center', gap: 6, padding: '4px 10px',
      borderRadius: 999, fontSize: 9.5, fontWeight: 600, letterSpacing: 0.5,
      background: isMainnet ? 'rgba(0,224,196,0.08)' : 'rgba(255,184,0,0.08)',
      color: isMainnet ? '#00E0C4' : '#FFB800',
      border: `1px solid ${isMainnet ? 'rgba(0,224,196,0.2)' : 'rgba(255,184,0,0.2)'}`,
    }}>
      <span style={{ width: 5, height: 5, borderRadius: '50%', background: isMainnet ? '#00E0C4' : '#FFB800' }} />
      {isMainnet ? 'MAINNET' : 'TESTNET'}
    </div>
  );

  return (
    <div style={{ maxWidth: 440, margin: '0 auto', padding: '32px 20px 40px', fontFamily: "'DM Sans',sans-serif" }}>
      <style>{`@keyframes spin{to{transform:rotate(360deg)}} @keyframes pulseGlow{0%,100%{box-shadow:0 0 0 0 rgba(0,224,196,0.35)}50%{box-shadow:0 0 0 14px rgba(0,224,196,0)}}`}</style>

      {/* ───────────────────────────────────────────────────────── */}
      {/* Step 0: Welcome / hero                                    */}
      {/* ───────────────────────────────────────────────────────── */}
      {step === 'welcome' && (
        <div style={{ textAlign: 'center', animation: 'slideUp 0.5s ease' }}>
          {/* Brand mark with soft glow */}
          <div style={{
            width: 96, height: 96, borderRadius: 24, margin: '8px auto 28px',
            background: 'linear-gradient(135deg, rgba(0,224,196,0.12), rgba(0,102,255,0.08))',
            border: '1px solid rgba(0,224,196,0.25)',
            display: 'flex', alignItems: 'center', justifyContent: 'center',
            boxShadow: '0 0 60px rgba(0,224,196,0.12)',
            position: 'relative',
          }}>
            <img src="/media/logo/logo_website.png" alt="UltraDAG" style={{ height: 42, width: 'auto' }} />
          </div>

          <h1 style={{
            fontSize: 30, fontWeight: 800, color: 'var(--dag-text)',
            lineHeight: 1.15, marginBottom: 10, letterSpacing: -0.5,
          }}>
            Your wallet.<br />
            <span style={{
              background: 'linear-gradient(135deg, #00E0C4, #0066FF)',
              WebkitBackgroundClip: 'text', WebkitTextFillColor: 'transparent',
              backgroundClip: 'text',
            }}>Forever.</span>
          </h1>
          <p style={{ fontSize: 13.5, color: 'var(--dag-text-muted)', lineHeight: 1.55, maxWidth: 340, margin: '0 auto 28px' }}>
            A passkey-secured wallet with a permanent @name. No seed phrases. No rental fees. No browser extensions.
          </p>

          {/* Value bullets — 3 rows, icon + label + sub */}
          <div style={{ display: 'flex', flexDirection: 'column', gap: 10, marginBottom: 28, textAlign: 'left' }}>
            {[
              { icon: '◎', color: '#00E0C4', title: `Sign in with ${bioLabel}`, sub: 'Your face or fingerprint unlocks everything.' },
              { icon: '★', color: '#FFB800', title: 'Claim a permanent @name', sub: 'Free forever for 6+ character names.' },
              { icon: '◈', color: '#0066FF', title: 'Keys never leave your device', sub: 'Non-custodial. No seed phrase to lose.' },
            ].map((b, i) => (
              <div key={i} style={{
                display: 'flex', alignItems: 'center', gap: 14, padding: '14px 16px',
                background: 'var(--dag-card)', border: '1px solid var(--dag-border)',
                borderRadius: 14,
              }}>
                <div style={{
                  width: 40, height: 40, borderRadius: 12, flexShrink: 0,
                  background: `${b.color}14`, border: `1px solid ${b.color}30`,
                  display: 'flex', alignItems: 'center', justifyContent: 'center',
                  color: b.color, fontSize: 18,
                }}>{b.icon}</div>
                <div style={{ flex: 1, minWidth: 0 }}>
                  <div style={{ fontSize: 12.5, fontWeight: 600, color: 'var(--dag-text)', marginBottom: 2 }}>{b.title}</div>
                  <div style={{ fontSize: 11, color: 'var(--dag-text-muted)', lineHeight: 1.4 }}>{b.sub}</div>
                </div>
              </div>
            ))}
          </div>

          <button
            onClick={() => setStep('username')}
            aria-label="Start creating wallet"
            style={{
              ...primaryButtonStyle, width: '100%', padding: '15px 0', borderRadius: 14,
              fontSize: 14.5, display: 'inline-flex', alignItems: 'center', justifyContent: 'center', gap: 8,
            }}
          >
            Get Started <span style={{ fontSize: 15 }}>→</span>
          </button>

          <button
            onClick={onFallbackToAdvanced}
            style={{
              background: 'none', border: 'none', color: 'var(--dag-text-muted)',
              fontSize: 12, cursor: 'pointer', marginTop: 18, padding: '6px 10px',
              fontWeight: 500,
            }}
          >
            I already have a wallet →
          </button>

          {/* Network switcher — small and subtle at bottom */}
          <div style={{ marginTop: 24, display: 'flex', alignItems: 'center', justifyContent: 'center', gap: 8 }}>
            {networkPill}
            <button
              onClick={() => onSwitchNetwork(isMainnet ? 'testnet' : 'mainnet')}
              style={{
                background: 'none', border: 'none', color: 'var(--dag-text-faint)',
                fontSize: 10.5, cursor: 'pointer', padding: '4px 8px',
              }}
              aria-label={`Switch to ${isMainnet ? 'testnet' : 'mainnet'}`}
            >
              switch →
            </button>
          </div>
        </div>
      )}

      {/* ───────────────────────────────────────────────────────── */}
      {/* Step 1: Claim your @name                                  */}
      {/* ───────────────────────────────────────────────────────── */}
      {step === 'username' && (
        <div style={{ animation: 'slideUp 0.4s ease' }}>
          <div style={{ textAlign: 'center', marginBottom: 24 }}>
            <div style={{
              width: 72, height: 72, borderRadius: 18, margin: '0 auto 20px',
              background: 'linear-gradient(135deg, rgba(0,224,196,0.1), rgba(0,102,255,0.08))',
              border: '1px solid rgba(0,224,196,0.2)',
              display: 'flex', alignItems: 'center', justifyContent: 'center',
            }}>
              <span style={{ fontSize: 32, color: '#00E0C4' }}>◎</span>
            </div>
            <h2 style={{ fontSize: 24, fontWeight: 700, color: 'var(--dag-text)', marginBottom: 6, letterSpacing: -0.3 }}>
              Claim your @name
            </h2>
            <p style={{ fontSize: 12.5, color: 'var(--dag-text-muted)', lineHeight: 1.5, maxWidth: 340, margin: '0 auto' }}>
              This is how people find and pay you on UltraDAG. <strong style={{ color: 'var(--dag-text-secondary)' }}>6+ characters is free and yours forever.</strong>
            </p>
          </div>

          {/* Live preview chip */}
          <div style={{
            background: 'var(--dag-card)', border: '1px solid var(--dag-border)',
            borderRadius: 14, padding: '18px 18px 14px', marginBottom: 14,
          }}>
            <div style={{ fontSize: 9.5, color: 'var(--dag-text-faint)', letterSpacing: 1.2, textTransform: 'uppercase', marginBottom: 8 }}>
              Preview
            </div>
            <div style={{
              fontSize: username.length > 12 ? 22 : 26, fontWeight: 700,
              color: username ? '#00E0C4' : 'var(--dag-text-faint)',
              fontFamily: "'DM Mono',monospace", marginBottom: 14,
              minHeight: 34, display: 'flex', alignItems: 'center',
            }}>
              @{username || 'yourname'}
              {checking && <span style={{ marginLeft: 10, fontSize: 13, color: 'var(--dag-text-faint)' }}>...</span>}
              {!checking && nameAvailable === true && username.length >= 3 && (
                <span style={{ marginLeft: 10, fontSize: 18, color: '#00E0C4' }}>✓</span>
              )}
              {!checking && nameAvailable === false && (
                <span style={{ marginLeft: 10, fontSize: 18, color: '#EF4444' }}>✗</span>
              )}
            </div>

            <input
              type="text"
              value={username}
              onChange={(e) => handleNameChange(e.target.value.toLowerCase().replace(/[^a-z0-9-]/g, ''))}
              placeholder="type your name"
              maxLength={20}
              autoFocus
              aria-label="Choose your @name"
              style={{
                ...inputStyle,
                fontSize: 14, padding: '12px 14px',
                fontFamily: "'DM Mono',monospace",
              }}
            />

            {/* Status row — fixed height, no layout jump */}
            <div style={{ minHeight: 22, marginTop: 10, display: 'flex', alignItems: 'center', gap: 8, flexWrap: 'wrap' }}>
              {username.length === 0 && (
                <span style={{ fontSize: 10.5, color: 'var(--dag-text-faint)' }}>a–z, 0–9, hyphens · 3–20 chars</span>
              )}
              {username.length > 0 && username.length < 3 && (
                <span style={{ fontSize: 10.5, color: 'var(--dag-text-faint)' }}>at least 3 characters</span>
              )}
              {nameError && <span style={{ fontSize: 10.5, color: '#FFB800' }}>{nameError}</span>}
              {!nameError && nameAvailable && isPerpetualName && (
                <>
                  <span style={{
                    fontSize: 9, fontWeight: 700, letterSpacing: 0.6, padding: '2px 7px', borderRadius: 4,
                    background: 'rgba(255,184,0,0.14)', color: '#FFB800',
                  }}>★ PERMANENT</span>
                  <span style={{ fontSize: 10.5, color: '#00E0C4' }}>Free forever · yours to keep</span>
                </>
              )}
              {!nameError && nameAvailable && isPremiumName && (
                <>
                  <span style={{
                    fontSize: 9, fontWeight: 700, letterSpacing: 0.6, padding: '2px 7px', borderRadius: 4,
                    background: 'rgba(0,102,255,0.14)', color: '#0066FF',
                  }}>PREMIUM</span>
                  <span style={{ fontSize: 10.5, color: 'var(--dag-text-muted)' }}>{nameFee} · renewable</span>
                </>
              )}
            </div>
          </div>

          <button
            onClick={() => setStep('passkey')}
            disabled={!canContinueUsername}
            style={{
              ...primaryButtonStyle, width: '100%', padding: '14px 0', borderRadius: 12,
              fontSize: 14,
              opacity: canContinueUsername ? 1 : 0.35,
              cursor: canContinueUsername ? 'pointer' : 'default',
            }}
          >
            Continue →
          </button>

          <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginTop: 16 }}>
            <button
              onClick={() => setStep('welcome')}
              aria-label="Back to welcome"
              style={{
                background: 'none', border: 'none', color: 'var(--dag-text-faint)',
                fontSize: 11.5, cursor: 'pointer', padding: 0,
              }}
            >
              ← Back
            </button>
            <button
              onClick={onFallbackToAdvanced}
              style={{
                background: 'none', border: 'none', color: 'var(--dag-text-faint)',
                fontSize: 11.5, cursor: 'pointer', padding: 0,
              }}
            >
              Advanced import →
            </button>
          </div>
        </div>
      )}

      {/* ───────────────────────────────────────────────────────── */}
      {/* Step 2: Unlock with biometrics                            */}
      {/* ───────────────────────────────────────────────────────── */}
      {step === 'passkey' && (
        <div style={{ textAlign: 'center', animation: 'slideUp 0.4s ease' }}>
          <div style={{
            width: 96, height: 96, borderRadius: 24, margin: '8px auto 24px',
            background: 'linear-gradient(135deg, rgba(0,224,196,0.1), rgba(0,102,255,0.06))',
            border: '1px solid rgba(0,224,196,0.25)',
            display: 'flex', alignItems: 'center', justifyContent: 'center',
            animation: 'pulseGlow 2.4s ease-in-out infinite',
          }}>
            <span style={{ fontSize: 44, color: '#00E0C4' }}>◉</span>
          </div>

          <p style={{ fontSize: 11, color: 'var(--dag-text-faint)', letterSpacing: 1.2, textTransform: 'uppercase', marginBottom: 8 }}>
            One tap to create
          </p>
          <h2 style={{ fontSize: 26, fontWeight: 700, color: '#00E0C4', marginBottom: 8, letterSpacing: -0.3 }}>
            @{username}
          </h2>

          <p style={{ fontSize: 12.5, color: 'var(--dag-text-muted)', marginBottom: 28, lineHeight: 1.6, maxWidth: 320, margin: '0 auto 28px' }}>
            Authenticate with {bioLabel} to generate your wallet. Your keys are created on this device and never leave it.
          </p>

          <button
            onClick={handleCreatePasskey}
            style={{
              ...primaryButtonStyle, width: '100%', padding: '16px 0', borderRadius: 14,
              fontSize: 14.5,
              display: 'inline-flex', alignItems: 'center', justifyContent: 'center', gap: 10,
            }}
          >
            <span style={{ fontSize: 17 }}>◉</span>
            Continue with {bioLabel}
          </button>

          {error && (
            <div role="alert" style={{
              marginTop: 18, fontSize: 11, color: '#EF4444',
              background: 'rgba(239,68,68,0.06)', border: '1px solid rgba(239,68,68,0.2)',
              borderRadius: 10, padding: '12px 14px', textAlign: 'left', lineHeight: 1.5,
            }}>
              {error}
            </div>
          )}

          <button
            onClick={() => setStep('username')}
            style={{
              background: 'none', border: 'none', color: 'var(--dag-text-faint)',
              fontSize: 11.5, cursor: 'pointer', marginTop: 20, padding: '6px 10px',
            }}
          >
            ← Back
          </button>
        </div>
      )}

      {/* ───────────────────────────────────────────────────────── */}
      {/* Step 3: Creating (spinner)                                */}
      {/* ───────────────────────────────────────────────────────── */}
      {step === 'creating' && (
        <div style={{ textAlign: 'center', padding: '64px 0', animation: 'slideUp 0.3s ease' }}>
          <div style={{
            width: 56, height: 56,
            border: '3px solid rgba(0,224,196,0.15)', borderTop: '3px solid #00E0C4',
            borderRadius: '50%', margin: '0 auto 24px',
            animation: 'spin 0.8s linear infinite',
          }} />
          <h2 style={{ fontSize: 18, fontWeight: 700, color: 'var(--dag-text)', marginBottom: 8 }}>
            Creating your wallet
          </h2>
          <p style={{ fontSize: 12, color: 'var(--dag-text-muted)' }}>
            Registering @{username} on {isMainnet ? 'mainnet' : 'testnet'}...
          </p>
        </div>
      )}

      {/* ───────────────────────────────────────────────────────── */}
      {/* Step 4: Success (merged done + backup)                    */}
      {/* ───────────────────────────────────────────────────────── */}
      {step === 'success' && result && pendingWallet && (
        <div style={{ textAlign: 'center', animation: 'slideUp 0.4s ease' }}>
          <div style={{
            width: 96, height: 96, borderRadius: 24, margin: '8px auto 24px',
            background: 'linear-gradient(135deg, rgba(0,224,196,0.14), rgba(0,102,255,0.08))',
            border: '1px solid rgba(0,224,196,0.3)',
            display: 'flex', alignItems: 'center', justifyContent: 'center',
            boxShadow: '0 0 60px rgba(0,224,196,0.15)',
          }}>
            <span style={{ fontSize: 48, color: '#00E0C4' }}>✓</span>
          </div>

          <h2 style={{ fontSize: 26, fontWeight: 700, color: 'var(--dag-text)', marginBottom: 6, letterSpacing: -0.3 }}>
            You're all set
          </h2>
          {result.name ? (
            <p style={{ fontSize: 17, fontWeight: 600, color: '#00E0C4', marginBottom: 6, fontFamily: "'DM Mono',monospace" }}>
              @{result.name}
            </p>
          ) : (
            <p style={{ fontSize: 13, color: 'var(--dag-text-muted)', marginBottom: 6 }}>
              Wallet created without a name.
            </p>
          )}

          <div style={{ display: 'inline-flex', marginBottom: 24 }}>{networkPill}</div>

          {/* Honest backup notice — no buttons that don't do anything */}
          <div style={{
            background: 'var(--dag-card)', border: '1px solid var(--dag-border)',
            borderRadius: 14, padding: '16px 18px', textAlign: 'left', marginBottom: 24,
          }}>
            <div style={{ display: 'flex', alignItems: 'center', gap: 8, marginBottom: 8 }}>
              <span style={{ fontSize: 15, color: '#FFB800' }}>⚠</span>
              <span style={{ fontSize: 12.5, fontWeight: 600, color: 'var(--dag-text)' }}>
                Protect this device
              </span>
            </div>
            <p style={{ fontSize: 11.5, color: 'var(--dag-text-muted)', lineHeight: 1.6, margin: 0 }}>
              Your key lives on this device only. If you lose it without a backup, <strong style={{ color: '#EF4444' }}>funds are gone forever</strong>. On-chain social recovery is coming soon — for now, keep this device safe and consider using the same passkey across devices via iCloud Keychain or Google Password Manager.
            </p>
          </div>

          <button
            onClick={() => onComplete(result.address, result.name, pendingWallet)}
            style={{
              ...primaryButtonStyle, width: '100%', padding: '15px 0', borderRadius: 14,
              fontSize: 14.5, display: 'inline-flex', alignItems: 'center', justifyContent: 'center', gap: 8,
            }}
          >
            Open Wallet →
          </button>
        </div>
      )}
    </div>
  );
}
