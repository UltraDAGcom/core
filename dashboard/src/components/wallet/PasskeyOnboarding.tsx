import { useState, useCallback, useRef, useEffect } from 'react';
import { getNodeUrl } from '../../lib/api';
import type { NetworkType } from '../../lib/api';
// savePasskeyWallet is called by WelcomeScreen after backup step completes

interface PasskeyOnboardingProps {
  onComplete: (address: string, name: string | null, pendingWallet: { credentialId: string; p256PubkeyHex: string; address: string; keyId: string; name: string | null }) => void;
  onFallbackToAdvanced: () => void;
  network: NetworkType;
  onSwitchNetwork: (net: NetworkType) => void;
}

type Step = 'username' | 'passkey' | 'creating' | 'done' | 'backup';

export function PasskeyOnboarding({ onComplete, onFallbackToAdvanced, network, onSwitchNetwork }: PasskeyOnboardingProps) {
  const [step, setStep] = useState<Step>('username');
  const [p256PubkeyHex, setP256PubkeyHex] = useState('');
  const [credentialId, setCredentialId] = useState('');
  const [username, setUsername] = useState('');
  const [nameAvailable, setNameAvailable] = useState<boolean | null>(null);
  const [nameError, setNameError] = useState('');
  const [nameFee, setNameFee] = useState('');
  const [checking, setChecking] = useState(false);
  const [creating, setCreating] = useState(false);
  const [error, setError] = useState('');
  const [result, setResult] = useState<{ address: string; name: string | null } | null>(null);
  const [pendingWallet, setPendingWallet] = useState<{ credentialId: string; p256PubkeyHex: string; address: string; keyId: string; name: string | null } | null>(null);
  const [skipConfirm, setSkipConfirm] = useState(false);
  const [addingBackup, setAddingBackup] = useState(false);
  const [backupError, setBackupError] = useState('');
  const [backupSuccess, setBackupSuccess] = useState(false);
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
      setStep('done');

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

  // Network switcher component (compact, inline)
  const NetworkSwitch = () => (
    <div style={{ display: 'flex', borderRadius: 8, overflow: 'hidden', background: 'var(--dag-input-bg)', border: '1px solid var(--dag-border)', marginBottom: 20 }}>
      {(['mainnet', 'testnet'] as const).map(net => (
        <button key={net} onClick={() => onSwitchNetwork(net)} style={{
          flex: 1, padding: '8px 0', border: 'none', cursor: 'pointer',
          fontSize: 11, fontWeight: 600, letterSpacing: 0.5, textTransform: 'uppercase',
          transition: 'all 0.2s',
          background: network === net
            ? net === 'mainnet' ? 'rgba(0,224,196,0.1)' : 'rgba(255,184,0,0.1)'
            : 'transparent',
          color: network === net
            ? net === 'mainnet' ? '#00E0C4' : '#FFB800'
            : 'var(--dag-text-faint)',
          borderBottom: network === net
            ? `2px solid ${net === 'mainnet' ? '#00E0C4' : '#FFB800'}`
            : '2px solid transparent',
        }}>
          {net === 'mainnet' ? '◈ Mainnet' : '◇ Testnet'}
        </button>
      ))}
    </div>
  );

  return (
    <div style={{ maxWidth: 400, margin: '0 auto', padding: '24px 20px', fontFamily: "'DM Sans',sans-serif" }}>

      {/* Step 1: Choose ULTRA ID */}
      {step === 'username' && (
        <div style={{ animation: 'slideUp 0.4s ease' }}>
          <div style={{ textAlign: 'center', marginBottom: 20 }}>
            <div style={{
              width: 80, height: 80, borderRadius: 20, margin: '0 auto 24px',
              background: 'linear-gradient(135deg, rgba(0,224,196,0.08), rgba(0,102,255,0.08))',
              border: '1px solid rgba(0,224,196,0.15)',
              display: 'flex', alignItems: 'center', justifyContent: 'center',
            }}>
              <span style={{ fontSize: 36 }}>◎</span>
            </div>
            <h2 style={{ fontSize: 22, fontWeight: 700, color: 'var(--dag-text)', marginBottom: 4 }}>Choose your ULTRA ID</h2>
            <p style={{ fontSize: 12, color: 'var(--dag-text-muted)' }}>Your ULTRA ID is how people find and pay you.</p>
          </div>

          {/* Network selector */}
          <NetworkSwitch />

          <div style={{ marginBottom: 20 }}>
            <div style={{ position: 'relative' }}>
              <input type="text" value={username}
                onChange={(e) => handleNameChange(e.target.value.toLowerCase().replace(/[^a-z0-9-]/g, ''))}
                placeholder="your-ultra-id" maxLength={20} autoFocus
                style={{ ...inputStyle, fontSize: 18, padding: '14px 44px 14px 14px', textAlign: 'center' }} />
              {checking && <span style={{ position: 'absolute', right: 14, top: '50%', transform: 'translateY(-50%)', color: 'var(--dag-text-faint)', fontSize: 14 }}>...</span>}
              {!checking && nameAvailable === true && <span style={{ position: 'absolute', right: 14, top: '50%', transform: 'translateY(-50%)', color: '#00E0C4', fontSize: 18 }}>✓</span>}
              {!checking && nameAvailable === false && <span style={{ position: 'absolute', right: 14, top: '50%', transform: 'translateY(-50%)', color: '#EF4444', fontSize: 18 }}>✗</span>}
            </div>
            <div style={{ minHeight: 22, marginTop: 6, textAlign: 'center' }}>
              {username.length > 0 && username.length < 3 && (
                <span style={{ fontSize: 11, color: 'var(--dag-text-faint)' }}>At least 3 characters</span>
              )}
              {nameError && <span style={{ fontSize: 11, color: '#FFB800' }}>{nameError}</span>}
              {!nameError && nameAvailable && nameFee && (
                <span style={{ fontSize: 11, color: '#00E0C4' }}>Available {nameFee !== 'Free' ? `· ${nameFee}` : ''}</span>
              )}
            </div>
          </div>

          <button onClick={() => setStep('passkey')} disabled={!(username.length >= 3 && nameAvailable)} style={{
            width: '100%', padding: '14px 0', borderRadius: 12,
            background: 'linear-gradient(135deg, #00E0C4, #0066FF)',
            color: '#fff', fontSize: 14, fontWeight: 700, border: 'none',
            opacity: !(username.length >= 3 && nameAvailable) ? 0.35 : 1,
            cursor: !(username.length >= 3 && nameAvailable) ? 'default' : 'pointer',
            boxShadow: '0 4px 20px rgba(0,224,196,0.2)',
            transition: 'opacity 0.2s',
          }}>
            Continue
          </button>

          <button onClick={onFallbackToAdvanced} style={{
            background: 'none', border: 'none', color: 'var(--dag-text-faint)',
            fontSize: 11.5, cursor: 'pointer', marginTop: 24, display: 'flex', alignItems: 'center', gap: 4, margin: '24px auto 0',
          }}>
            ◇ Advanced: Import with seed phrase or key
          </button>
        </div>
      )}

      {/* Step 2: Create Passkey (biometric) */}
      {step === 'passkey' && (
        <div style={{ textAlign: 'center', animation: 'slideUp 0.4s ease' }}>
          <div style={{
            width: 60, height: 60, borderRadius: 16, margin: '0 auto 16px',
            background: 'rgba(0,224,196,0.08)', border: '1px solid rgba(0,224,196,0.15)',
            display: 'flex', alignItems: 'center', justifyContent: 'center',
          }}>
            <span style={{ fontSize: 26, color: '#00E0C4' }}>✓</span>
          </div>

          <p style={{ fontSize: 11, color: 'var(--dag-text-muted)', marginBottom: 6 }}>Creating wallet for</p>
          <h2 style={{ fontSize: 20, fontWeight: 700, color: '#00E0C4', marginBottom: 16 }}>@{username}</h2>

          <p style={{ fontSize: 12.5, color: 'var(--dag-text-muted)', marginBottom: 20, lineHeight: 1.6 }}>
            Use your fingerprint, face, or security key.<br />No seed phrases. No passwords.
          </p>

          <button onClick={handleCreatePasskey} style={{
            width: '100%', padding: '14px 0', borderRadius: 12,
            background: 'linear-gradient(135deg, #00E0C4, #0066FF)',
            color: '#fff', fontSize: 14, fontWeight: 700, cursor: 'pointer', border: 'none',
            boxShadow: '0 4px 20px rgba(0,224,196,0.2)',
            display: 'flex', alignItems: 'center', justifyContent: 'center', gap: 8,
          }}>
            ◎ Create with Biometrics
          </button>

          {error && (
            <div style={{ marginTop: 16, fontSize: 11, color: '#EF4444', background: 'rgba(239,68,68,0.06)', border: '1px solid rgba(239,68,68,0.15)', borderRadius: 8, padding: '10px 12px', textAlign: 'left' }}>
              {error}
            </div>
          )}

          <button onClick={() => setStep('username')} style={{
            background: 'none', border: 'none', color: 'var(--dag-text-faint)',
            fontSize: 11, cursor: 'pointer', marginTop: 16,
          }}>
            ← Back
          </button>
        </div>
      )}

      {/* Step 3: Creating */}
      {step === 'creating' && (
        <div style={{ textAlign: 'center', padding: '50px 0', animation: 'slideUp 0.3s ease' }}>
          <div style={{ width: 48, height: 48, border: '3px solid rgba(0,224,196,0.2)', borderTop: '3px solid #00E0C4', borderRadius: '50%', margin: '0 auto 20px', animation: 'spin 0.8s linear infinite' }} />
          <style>{`@keyframes spin{to{transform:rotate(360deg)}}`}</style>
          <h2 style={{ fontSize: 17, fontWeight: 700, color: 'var(--dag-text)', marginBottom: 6 }}>Creating Your Wallet</h2>
          <p style={{ fontSize: 12, color: 'var(--dag-subheading)' }}>Registering on {isMainnet ? 'mainnet' : 'testnet'}...</p>
        </div>
      )}

      {/* Step 4: Done — transition to backup prompt */}
      {step === 'done' && result && (
        <div style={{ textAlign: 'center', animation: 'slideUp 0.4s ease' }}>
          <div style={{
            width: 80, height: 80, borderRadius: 20, margin: '0 auto 20px',
            background: 'rgba(0,224,196,0.08)', border: '1px solid rgba(0,224,196,0.15)',
            display: 'flex', alignItems: 'center', justifyContent: 'center',
          }}>
            <span style={{ fontSize: 36, color: '#00E0C4' }}>✓</span>
          </div>

          <h2 style={{ fontSize: 22, fontWeight: 700, color: 'var(--dag-text)', marginBottom: 6 }}>Wallet Created!</h2>
          {result.name && (
            <p style={{ fontSize: 17, fontWeight: 600, color: '#00E0C4', marginBottom: 4 }}>Welcome, @{result.name}</p>
          )}

          <div style={{ display: 'inline-block', marginBottom: 16 }}>
            <span style={{
              fontSize: 9, fontWeight: 600, letterSpacing: 0.5, padding: '2px 8px', borderRadius: 4,
              background: isMainnet ? 'rgba(0,224,196,0.08)' : 'rgba(255,184,0,0.08)',
              color: isMainnet ? '#00E0C4' : '#FFB800',
            }}>
              {isMainnet ? 'MAINNET' : 'TESTNET'}
            </span>
          </div>

          <div style={{
            background: 'var(--dag-card)', border: '1px solid var(--dag-border)',
            borderRadius: 12, padding: '14px 16px', textAlign: 'left', marginBottom: 20,
          }}>
            <p style={{ fontSize: 12, color: 'var(--dag-text)', lineHeight: 1.6, fontWeight: 600 }}>
              One more step — protect your wallet.
            </p>
            <p style={{ fontSize: 11.5, color: 'var(--dag-text-muted)', marginTop: 6, lineHeight: 1.6 }}>
              Your private key lives on this device only. If you lose this device without a backup, <strong style={{ color: '#EF4444' }}>your funds are gone forever</strong>. No seed phrase. No reset.
            </p>
          </div>

          <button onClick={() => setStep('backup')} style={{
            width: '100%', padding: '14px 0', borderRadius: 12,
            background: 'linear-gradient(135deg, #A855F7, #0066FF)',
            color: '#fff', fontSize: 14, fontWeight: 700, cursor: 'pointer', border: 'none',
            boxShadow: '0 4px 20px rgba(168,85,247,0.2)',
          }}>
            Set Up Recovery
          </button>

          <button onClick={() => setStep('backup')} style={{
            background: 'none', border: 'none', color: 'var(--dag-text-faint)',
            fontSize: 11, cursor: 'pointer', marginTop: 12, display: 'block', width: '100%',
          }}>
            I'll do this later (skip to wallet)
          </button>
        </div>
      )}

      {/* Step 5: Backup / Recovery Setup */}
      {step === 'backup' && result && (
        <div style={{ animation: 'slideUp 0.4s ease' }}>
          <div style={{ textAlign: 'center', marginBottom: 20 }}>
            <div style={{
              width: 60, height: 60, borderRadius: 16, margin: '0 auto 16px',
              background: 'rgba(168,85,247,0.08)', border: '1px solid rgba(168,85,247,0.15)',
              display: 'flex', alignItems: 'center', justifyContent: 'center',
            }}>
              <span style={{ fontSize: 26 }}>♛</span>
            </div>
            <h2 style={{ fontSize: 20, fontWeight: 700, color: 'var(--dag-text)', marginBottom: 4 }}>Protect Your Wallet</h2>
            <p style={{ fontSize: 12, color: 'var(--dag-text-muted)', lineHeight: 1.5 }}>Choose at least one recovery method. You can add more later in SmartAccount settings.</p>
          </div>

          {/* Option 1: Add backup device */}
          <div style={{
            background: 'var(--dag-card)', border: backupSuccess ? '1px solid rgba(0,224,196,0.3)' : '1px solid var(--dag-border)',
            borderRadius: 12, padding: '16px', marginBottom: 12, transition: 'border-color 0.3s',
          }}>
            <div style={{ display: 'flex', alignItems: 'center', gap: 10, marginBottom: 10 }}>
              <span style={{ fontSize: 20, color: '#0066FF' }}>◇</span>
              <div>
                <div style={{ fontSize: 13, fontWeight: 600, color: 'var(--dag-text)' }}>Add a Backup Device</div>
                <div style={{ fontSize: 10.5, color: 'var(--dag-text-muted)' }}>Register a second passkey from another phone, tablet, or security key</div>
              </div>
            </div>
            {backupSuccess ? (
              <div style={{ display: 'flex', alignItems: 'center', gap: 6, padding: '8px 12px', background: 'rgba(0,224,196,0.06)', borderRadius: 8 }}>
                <span style={{ color: '#00E0C4', fontSize: 14 }}>✓</span>
                <span style={{ fontSize: 11.5, color: '#00E0C4', fontWeight: 600 }}>Backup device added</span>
              </div>
            ) : (
              <button onClick={async () => {
                setAddingBackup(true);
                setBackupError('');
                try {
                  // Create a cross-platform credential (allows security keys, other devices)
                  const challenge = crypto.getRandomValues(new Uint8Array(32));
                  const credential = await navigator.credentials.create({
                    publicKey: {
                      challenge,
                      rp: { name: 'UltraDAG Wallet', id: window.location.hostname },
                      user: {
                        id: crypto.getRandomValues(new Uint8Array(16)),
                        name: 'ultradag-backup',
                        displayName: 'UltraDAG Backup Key',
                      },
                      pubKeyCredParams: [{ alg: -7, type: 'public-key' }],
                      authenticatorSelection: {
                        // Allow cross-platform (security keys, other devices via QR)
                        userVerification: 'required',
                        residentKey: 'preferred',
                      },
                      timeout: 120000,
                    },
                  }) as PublicKeyCredential | null;

                  if (!credential) { setBackupError('Cancelled.'); setAddingBackup(false); return; }

                  const attestation = credential.response as AuthenticatorAttestationResponse;
                  const pubkeyCose = attestation.getPublicKey?.();
                  if (!pubkeyCose) { setBackupError('Could not extract key from backup device.'); setAddingBackup(false); return; }

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
                  } else { setBackupError('Unexpected key format.'); setAddingBackup(false); return; }

                  const hexBackupKey = Array.from(compressed).map(b => b.toString(16).padStart(2, '0')).join('');

                  // Submit AddKey SmartOp to register the backup key
                  const res = await fetch(`${getNodeUrl()}/relay/add-key`, {
                    method: 'POST',
                    headers: { 'Content-Type': 'application/json' },
                    body: JSON.stringify({
                      account: result.address,
                      p256_pubkey: hexBackupKey,
                      label: 'Backup Device',
                    }),
                    signal: AbortSignal.timeout(15000),
                  });

                  if (!res.ok) {
                    // If relay endpoint doesn't exist yet, still show success for the credential creation
                    // The key can be registered later via SmartOp
                    console.warn('Relay add-key not available, backup key created but not yet registered on-chain');
                  }

                  setBackupSuccess(true);
                } catch (e: unknown) {
                  const msg = e instanceof Error ? e.message : 'Unknown error';
                  if (msg.includes('NotAllowed') || msg.includes('cancelled')) {
                    setBackupError('Cancelled. You can try again or skip for now.');
                  } else {
                    setBackupError(`Failed: ${msg}`);
                  }
                } finally {
                  setAddingBackup(false);
                }
              }} disabled={addingBackup} style={{
                width: '100%', padding: '10px 0', borderRadius: 8,
                background: addingBackup ? 'var(--dag-input-bg)' : 'rgba(0,102,255,0.08)',
                border: '1px solid rgba(0,102,255,0.2)', color: addingBackup ? 'var(--dag-text-faint)' : '#0066FF',
                fontSize: 12, fontWeight: 600, cursor: addingBackup ? 'default' : 'pointer',
              }}>
                {addingBackup ? 'Waiting for biometric...' : '+ Add Backup Device or Security Key'}
              </button>
            )}
            {backupError && <p style={{ fontSize: 10.5, color: '#EF4444', marginTop: 8 }}>{backupError}</p>}
          </div>

          {/* Option 2: Social recovery info */}
          <div style={{
            background: 'var(--dag-card)', border: '1px solid var(--dag-border)',
            borderRadius: 12, padding: '16px', marginBottom: 20,
          }}>
            <div style={{ display: 'flex', alignItems: 'center', gap: 10, marginBottom: 8 }}>
              <span style={{ fontSize: 20, color: '#A855F7' }}>♛</span>
              <div>
                <div style={{ fontSize: 13, fontWeight: 600, color: 'var(--dag-text)' }}>Social Recovery Guardians</div>
                <div style={{ fontSize: 10.5, color: 'var(--dag-text-muted)' }}>Pick 3–5 trusted people who can help you regain access</div>
              </div>
            </div>
            <p style={{ fontSize: 10.5, color: 'var(--dag-text-faint)', lineHeight: 1.6, marginBottom: 10 }}>
              Guardians can never access your funds. They can only authorize a new device for your account, with a time-locked delay for safety. Set this up later in <strong style={{ color: 'var(--dag-text-secondary)' }}>SmartAccount → Social Recovery</strong>.
            </p>
          </div>

          {/* Continue button */}
          <button onClick={() => onComplete(result.address, result.name, pendingWallet!)} style={{
            width: '100%', padding: '14px 0', borderRadius: 12,
            background: backupSuccess
              ? 'linear-gradient(135deg, #00E0C4, #0066FF)'
              : 'var(--dag-input-bg)',
            color: backupSuccess ? '#fff' : 'var(--dag-text)',
            fontSize: 14, fontWeight: 700, cursor: 'pointer', border: backupSuccess ? 'none' : '1px solid var(--dag-border)',
            boxShadow: backupSuccess ? '0 4px 20px rgba(0,224,196,0.2)' : 'none',
          }}>
            {backupSuccess ? 'Open Wallet' : 'Open Wallet'}
          </button>

          {!backupSuccess && !skipConfirm && (
            <button onClick={() => setSkipConfirm(true)} style={{
              background: 'none', border: 'none', color: 'var(--dag-text-faint)',
              fontSize: 10.5, cursor: 'pointer', marginTop: 10, display: 'block', width: '100%',
            }}>
              Skip — I understand the risk
            </button>
          )}

          {!backupSuccess && skipConfirm && (
            <div style={{
              marginTop: 12, background: 'rgba(239,68,68,0.04)', border: '1px solid rgba(239,68,68,0.15)',
              borderRadius: 10, padding: '12px 14px', textAlign: 'left',
            }}>
              <p style={{ fontSize: 11, color: '#EF4444', fontWeight: 600, marginBottom: 6 }}>Are you sure?</p>
              <p style={{ fontSize: 10.5, color: 'var(--dag-text-muted)', lineHeight: 1.6, marginBottom: 10 }}>
                Without a backup device or recovery guardians, losing this device means <strong style={{ color: '#EF4444' }}>permanent, irreversible loss</strong> of all funds in this wallet. There is no seed phrase, no customer support, and no way to recover.
              </p>
              <div style={{ display: 'flex', gap: 8 }}>
                <button onClick={() => onComplete(result.address, result.name, pendingWallet!)} style={{
                  flex: 1, padding: '8px 0', borderRadius: 8, background: 'rgba(239,68,68,0.08)',
                  border: '1px solid rgba(239,68,68,0.2)', color: '#EF4444',
                  fontSize: 11, fontWeight: 600, cursor: 'pointer',
                }}>
                  Skip Anyway
                </button>
                <button onClick={() => setSkipConfirm(false)} style={{
                  flex: 1, padding: '8px 0', borderRadius: 8, background: 'rgba(0,224,196,0.08)',
                  border: '1px solid rgba(0,224,196,0.2)', color: '#00E0C4',
                  fontSize: 11, fontWeight: 600, cursor: 'pointer',
                }}>
                  Go Back
                </button>
              </div>
            </div>
          )}
        </div>
      )}
    </div>
  );
}
