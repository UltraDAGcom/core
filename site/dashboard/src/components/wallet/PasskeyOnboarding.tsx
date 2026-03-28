import { useState, useCallback } from 'react';
import { getNodeUrl } from '../../lib/api';
import { savePasskeyWallet } from '../../lib/passkey-wallet';

interface PasskeyOnboardingProps {
  onComplete: (address: string, name: string | null) => void;
  onFallbackToAdvanced: () => void;
}

type Step = 'passkey' | 'username' | 'creating' | 'done';

/**
 * Passkey-first onboarding flow:
 * 1. Create passkey via WebAuthn (biometric prompt)
 * 2. Choose username (name availability check)
 * 3. Relay creates account (funded + key registered)
 * 4. Done — wallet ready
 */
export function PasskeyOnboarding({ onComplete, onFallbackToAdvanced }: PasskeyOnboardingProps) {
  const [step, setStep] = useState<Step>('passkey');
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

  // Step 1: Create passkey via WebAuthn
  const handleCreatePasskey = useCallback(async () => {
    setError('');
    try {
      // Check WebAuthn support
      if (!window.PublicKeyCredential) {
        setError('WebAuthn is not supported on this device. Use the advanced import option.');
        return;
      }

      // Create a new P256 credential
      const challenge = crypto.getRandomValues(new Uint8Array(32));
      const credential = await navigator.credentials.create({
        publicKey: {
          challenge,
          rp: { name: 'UltraDAG Wallet', id: window.location.hostname },
          user: {
            id: crypto.getRandomValues(new Uint8Array(16)),
            name: 'ultradag-user',
            displayName: 'UltraDAG Wallet',
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

      if (!credential) {
        setError('Passkey creation was cancelled.');
        return;
      }

      // Extract P256 public key from attestation
      const attestation = credential.response as AuthenticatorAttestationResponse;
      const pubkeyCose = attestation.getPublicKey?.();

      if (!pubkeyCose) {
        // Fallback: extract from attestation object (older browsers)
        setError('Could not extract public key. Try a different browser or use advanced import.');
        return;
      }

      // Convert COSE key to compressed SEC1 format (33 bytes)
      // The raw public key from WebAuthn is in SPKI format — extract the raw P256 point
      const spkiKey = new Uint8Array(pubkeyCose);

      // Import as CryptoKey to export as raw
      const cryptoKey = await crypto.subtle.importKey(
        'spki', spkiKey,
        { name: 'ECDSA', namedCurve: 'P-256' },
        true, ['verify']
      );
      const rawKey = new Uint8Array(await crypto.subtle.exportKey('raw', cryptoKey));

      // rawKey is 65 bytes (uncompressed: 0x04 || x || y)
      // Compress to 33 bytes (0x02 or 0x03 prefix based on y parity)
      let compressed: Uint8Array;
      if (rawKey.length === 65 && rawKey[0] === 0x04) {
        compressed = new Uint8Array(33);
        compressed[0] = (rawKey[64] & 1) === 0 ? 0x02 : 0x03;
        compressed.set(rawKey.slice(1, 33), 1);
      } else if (rawKey.length === 33) {
        compressed = rawKey;
      } else {
        setError('Unexpected key format from WebAuthn.');
        return;
      }

      const hexPubkey = Array.from(compressed).map(b => b.toString(16).padStart(2, '0')).join('');
      setP256PubkeyHex(hexPubkey);
      setCredentialId(credential.id);
      setStep('username');
    } catch (e: unknown) {
      const msg = e instanceof Error ? e.message : 'Unknown error';
      if (msg.includes('NotAllowed') || msg.includes('cancelled')) {
        setError('Biometric verification was cancelled. Try again or use advanced import.');
      } else {
        setError(`Passkey creation failed: ${msg}`);
      }
    }
  }, []);

  // Step 2: Check name availability
  const checkName = useCallback(async (name: string) => {
    setUsername(name);
    setNameAvailable(null);
    setNameError('');
    setNameFee('');

    if (name.length < 3) {
      setNameError(name.length > 0 ? 'Name must be at least 3 characters' : '');
      return;
    }

    setChecking(true);
    try {
      const res = await fetch(`${getNodeUrl()}/name/available/${encodeURIComponent(name)}`, {
        signal: AbortSignal.timeout(5000),
      });
      if (res.ok) {
        const data = await res.json();
        setNameAvailable(data.available);
        if (!data.valid) {
          setNameError('Invalid name format');
        } else if (!data.available) {
          setNameError('This name is taken');
        } else {
          setNameFee(data.annual_fee_udag > 0 ? `${data.annual_fee_udag} UDAG/year` : 'Free');
          if (data.similar_warning) {
            setNameError(data.similar_warning);
          }
        }
      } else if (res.status === 404) {
        setNameError('Name service not available on this network yet. Please switch to testnet or try again later.');
      }
    } catch {
      setNameError('Could not reach the network. Check your connection.');
    } finally {
      setChecking(false);
    }
  }, []);

  // Step 3: Create account via relay
  const handleCreate = useCallback(async () => {
    setCreating(true);
    setError('');
    setStep('creating');

    try {
      const body: Record<string, unknown> = { p256_pubkey: p256PubkeyHex };
      if (username.length >= 3 && nameAvailable) {
        body.name = username;
      }

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
      const resultData = {
        address: data.address,
        name: username.length >= 3 && nameAvailable ? username : null,
      };
      setResult(resultData);
      setStep('done');

      // Store passkey wallet info (no secret keys — secure enclave only)
      savePasskeyWallet({
        credentialId,
        p256PubkeyHex,
        address: data.address,
        keyId: data.p256_key_id,
        name: username.length >= 3 && nameAvailable ? username : null,
      });
    } catch (e: unknown) {
      setError(e instanceof Error ? e.message : 'Account creation failed');
      setStep('username');
    } finally {
      setCreating(false);
    }
  }, [p256PubkeyHex, username, nameAvailable, credentialId]);

  const inputStyle: React.CSSProperties = {
    width: '100%', padding: '12px 14px', borderRadius: 10, background: 'var(--dag-input-bg)',
    border: '1px solid var(--dag-border)', color: 'var(--dag-text)', fontSize: 14, outline: 'none',
    fontFamily: "'DM Sans',sans-serif", transition: 'border-color 0.2s',
  };

  return (
    <div style={{ maxWidth: 400, margin: '0 auto', padding: '24px 20px', fontFamily: "'DM Sans',sans-serif" }}>

      {/* Step 1: Create Passkey */}
      {step === 'passkey' && (
        <div style={{ textAlign: 'center', animation: 'slideUp 0.4s ease' }}>
          <div style={{
            width: 80, height: 80, borderRadius: 20, margin: '0 auto 24px',
            background: 'linear-gradient(135deg, rgba(0,224,196,0.08), rgba(0,102,255,0.08))',
            border: '1px solid rgba(0,224,196,0.15)',
            display: 'flex', alignItems: 'center', justifyContent: 'center',
          }}>
            <span style={{ fontSize: 36 }}>◎</span>
          </div>

          <h2 style={{ fontSize: 22, fontWeight: 700, color: 'var(--dag-text)', marginBottom: 8 }}>Create Your Wallet</h2>
          <p style={{ fontSize: 12.5, color: 'var(--dag-text-muted)', marginBottom: 28, lineHeight: 1.6 }}>
            Use your fingerprint, face, or security key.<br />No seed phrases. No passwords.
          </p>

          <button onClick={handleCreatePasskey} style={{
            width: '100%', padding: '14px 0', borderRadius: 12,
            background: 'linear-gradient(135deg, #00E0C4, #0066FF)',
            color: 'var(--dag-text)', fontSize: 14, fontWeight: 700, cursor: 'pointer', border: 'none',
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

          <button onClick={onFallbackToAdvanced} style={{
            background: 'none', border: 'none', color: 'var(--dag-text-faint)',
            fontSize: 11.5, cursor: 'pointer', marginTop: 24, display: 'flex', alignItems: 'center', gap: 4, margin: '24px auto 0',
          }}>
            ◇ Advanced: Import with seed phrase or key
          </button>
        </div>
      )}

      {/* Step 2: Choose Username */}
      {step === 'username' && (
        <div style={{ animation: 'slideUp 0.4s ease' }}>
          <div style={{ textAlign: 'center', marginBottom: 24 }}>
            <div style={{
              width: 60, height: 60, borderRadius: 16, margin: '0 auto 16px',
              background: 'rgba(0,224,196,0.08)', border: '1px solid rgba(0,224,196,0.15)',
              display: 'flex', alignItems: 'center', justifyContent: 'center',
            }}>
              <span style={{ fontSize: 26, color: '#00E0C4' }}>✓</span>
            </div>
            <h2 style={{ fontSize: 20, fontWeight: 700, color: 'var(--dag-text)', marginBottom: 4 }}>Choose a Username</h2>
            <p style={{ fontSize: 12, color: 'var(--dag-text-muted)' }}>This is how people will find and pay you.</p>
          </div>

          <div style={{ marginBottom: 20 }}>
            <div style={{ position: 'relative' }}>
              <input type="text" value={username}
                onChange={(e) => checkName(e.target.value.toLowerCase().replace(/[^a-z0-9-]/g, ''))}
                placeholder="your-username" maxLength={20} autoFocus
                style={{ ...inputStyle, fontSize: 18, padding: '14px 44px 14px 14px', textAlign: 'center' }} />
              {checking && <span style={{ position: 'absolute', right: 14, top: '50%', transform: 'translateY(-50%)', color: 'var(--dag-text-faint)', fontSize: 14 }}>...</span>}
              {!checking && nameAvailable === true && <span style={{ position: 'absolute', right: 14, top: '50%', transform: 'translateY(-50%)', color: '#00E0C4', fontSize: 18 }}>✓</span>}
              {!checking && nameAvailable === false && <span style={{ position: 'absolute', right: 14, top: '50%', transform: 'translateY(-50%)', color: '#EF4444', fontSize: 18 }}>✗</span>}
            </div>
            {/* Status message below input */}
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

          <button onClick={handleCreate} disabled={creating || !(username.length >= 3 && nameAvailable)} style={{
            width: '100%', padding: '14px 0', borderRadius: 12,
            background: 'linear-gradient(135deg, #00E0C4, #0066FF)',
            color: '#fff', fontSize: 14, fontWeight: 700, border: 'none',
            opacity: (creating || !(username.length >= 3 && nameAvailable)) ? 0.35 : 1,
            cursor: (creating || !(username.length >= 3 && nameAvailable)) ? 'default' : 'pointer',
            boxShadow: '0 4px 20px rgba(0,224,196,0.2)',
            transition: 'opacity 0.2s',
          }}>
            {creating ? 'Creating...' : 'Create Wallet'}
          </button>

          {error && <div style={{ marginTop: 12, fontSize: 11, color: '#EF4444', background: 'rgba(239,68,68,0.06)', border: '1px solid rgba(239,68,68,0.15)', borderRadius: 8, padding: '8px 12px' }}>{error}</div>}
        </div>
      )}

      {/* Step 3: Creating */}
      {step === 'creating' && (
        <div style={{ textAlign: 'center', padding: '50px 0', animation: 'slideUp 0.3s ease' }}>
          <div style={{ width: 48, height: 48, border: '3px solid rgba(0,224,196,0.2)', borderTop: '3px solid #00E0C4', borderRadius: '50%', margin: '0 auto 20px', animation: 'spin 0.8s linear infinite' }} />
          <style>{`@keyframes spin{to{transform:rotate(360deg)}}`}</style>
          <h2 style={{ fontSize: 17, fontWeight: 700, color: 'var(--dag-text)', marginBottom: 6 }}>Creating Your Wallet</h2>
          <p style={{ fontSize: 12, color: 'var(--dag-subheading)' }}>Funding account on the network...</p>
        </div>
      )}

      {/* Step 4: Done */}
      {step === 'done' && result && (
        <div style={{ textAlign: 'center', animation: 'slideUp 0.4s ease' }}>
          <div style={{
            width: 80, height: 80, borderRadius: 20, margin: '0 auto 20px',
            background: 'rgba(0,224,196,0.08)', border: '1px solid rgba(0,224,196,0.15)',
            display: 'flex', alignItems: 'center', justifyContent: 'center',
          }}>
            <span style={{ fontSize: 36, color: '#00E0C4' }}>✓</span>
          </div>

          <h2 style={{ fontSize: 22, fontWeight: 700, color: 'var(--dag-text)', marginBottom: 6 }}>You're All Set!</h2>
          {result.name ? (
            <p style={{ fontSize: 17, fontWeight: 600, color: '#00E0C4', marginBottom: 4 }}>You're {result.name}</p>
          ) : (
            <p style={{ fontSize: 12, color: 'var(--dag-text-muted)', marginBottom: 4 }}>Your wallet is ready.</p>
          )}
          <p style={{ fontSize: 10, color: 'var(--dag-text-faint)', fontFamily: "'DM Mono',monospace", wordBreak: 'break-all', marginBottom: 20 }}>{result.address}</p>

          <div style={{
            background: 'var(--dag-card)', border: '1px solid var(--dag-border)',
            borderRadius: 12, padding: '14px 16px', textAlign: 'left', marginBottom: 20,
          }}>
            <p style={{ fontSize: 11.5, color: 'var(--dag-text-muted)', lineHeight: 1.6 }}>
              Your wallet is secured by your device's biometrics. The private key never leaves the secure enclave.
            </p>
            <p style={{ fontSize: 11.5, color: 'var(--dag-text-muted)', marginTop: 6, lineHeight: 1.6 }}>
              <strong style={{ color: 'var(--dag-text-secondary)' }}>Tip:</strong> Go to SmartAccount to add a backup device and set up recovery guardians.
            </p>
          </div>

          <button onClick={() => onComplete(result.address, result.name)} style={{
            width: '100%', padding: '14px 0', borderRadius: 12,
            background: 'linear-gradient(135deg, #00E0C4, #0066FF)',
            color: 'var(--dag-text)', fontSize: 14, fontWeight: 700, cursor: 'pointer', border: 'none',
            boxShadow: '0 4px 20px rgba(0,224,196,0.2)',
          }}>
            Open Wallet
          </button>
        </div>
      )}
    </div>
  );
}
