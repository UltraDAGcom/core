import { useState, useCallback } from 'react';
import { Fingerprint, User, CheckCircle, Loader2, ChevronDown, Key, AlertTriangle } from 'lucide-react';
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
          setNameFee(`${data.annual_fee_udag} UDAG/year`);
        }
      }
    } catch {
      setNameError('Could not check availability');
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

  return (
    <div className="max-w-md mx-auto p-6">
      {/* Step 1: Create Passkey */}
      {step === 'passkey' && (
        <div className="text-center space-y-6">
          <div className="w-20 h-20 mx-auto rounded-2xl bg-gradient-to-br from-dag-accent to-purple-500 flex items-center justify-center">
            <Fingerprint className="w-10 h-10 text-white" />
          </div>
          <div>
            <h2 className="text-2xl font-bold text-white">Create Your Wallet</h2>
            <p className="text-dag-muted mt-2">
              Use your fingerprint, face, or security key. No seed phrases required.
            </p>
          </div>

          <button
            onClick={handleCreatePasskey}
            className="w-full py-4 bg-dag-accent hover:bg-dag-accent/90 text-white font-semibold rounded-xl transition-all text-lg"
          >
            <Fingerprint className="w-5 h-5 inline mr-2" />
            Create with Biometrics
          </button>

          {error && (
            <div className="bg-red-500/10 border border-red-500/30 rounded-lg p-3 text-red-400 text-sm text-left">
              <AlertTriangle className="w-4 h-4 inline mr-1" />
              {error}
            </div>
          )}

          <button
            onClick={onFallbackToAdvanced}
            className="text-dag-muted text-sm hover:text-white transition-colors flex items-center gap-1 mx-auto"
          >
            <Key className="w-3 h-3" />
            Advanced: Import with seed phrase or private key
            <ChevronDown className="w-3 h-3" />
          </button>
        </div>
      )}

      {/* Step 2: Choose Username */}
      {step === 'username' && (
        <div className="space-y-6">
          <div className="text-center">
            <div className="w-16 h-16 mx-auto rounded-2xl bg-green-500/20 flex items-center justify-center mb-4">
              <CheckCircle className="w-8 h-8 text-green-400" />
            </div>
            <h2 className="text-2xl font-bold text-white">Passkey Created!</h2>
            <p className="text-dag-muted mt-2">Now choose your username.</p>
          </div>

          <div>
            <label className="text-sm text-dag-muted mb-1 block">Choose a username</label>
            <div className="relative">
              <User className="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-dag-muted" />
              <input
                type="text"
                value={username}
                onChange={(e) => checkName(e.target.value.toLowerCase().replace(/[^a-z0-9-]/g, ''))}
                placeholder="john29"
                maxLength={20}
                className="w-full pl-10 pr-4 py-3 bg-dag-bg border border-dag-border rounded-xl text-white placeholder-slate-600 focus:border-dag-accent focus:outline-none"
                autoFocus
              />
              {checking && <Loader2 className="absolute right-3 top-1/2 -translate-y-1/2 w-4 h-4 text-dag-muted animate-spin" />}
              {!checking && nameAvailable === true && (
                <CheckCircle className="absolute right-3 top-1/2 -translate-y-1/2 w-4 h-4 text-green-400" />
              )}
            </div>
            {nameError && <p className="text-red-400 text-xs mt-1">{nameError}</p>}
            {nameAvailable && nameFee && <p className="text-green-400 text-xs mt-1">{username} is available! ({nameFee})</p>}
          </div>

          <button
            onClick={handleCreate}
            disabled={creating}
            className="w-full py-4 bg-dag-accent hover:bg-dag-accent/90 disabled:opacity-50 text-white font-semibold rounded-xl transition-all text-lg"
          >
            {username.length >= 3 && nameAvailable
              ? `Create Wallet as ${username}`
              : 'Create Wallet'
            }
          </button>

          <button
            onClick={() => handleCreate()}
            className="text-dag-muted text-sm hover:text-white transition-colors block mx-auto"
          >
            Skip username for now
          </button>

          {error && (
            <div className="bg-red-500/10 border border-red-500/30 rounded-lg p-3 text-red-400 text-sm">
              {error}
            </div>
          )}
        </div>
      )}

      {/* Step 3: Creating */}
      {step === 'creating' && (
        <div className="text-center space-y-6 py-12">
          <Loader2 className="w-12 h-12 text-dag-accent animate-spin mx-auto" />
          <div>
            <h2 className="text-xl font-bold text-white">Creating Your Wallet</h2>
            <p className="text-dag-muted mt-2">Funding account and registering on the network...</p>
          </div>
        </div>
      )}

      {/* Step 4: Done */}
      {step === 'done' && result && (
        <div className="text-center space-y-6">
          <div className="w-20 h-20 mx-auto rounded-2xl bg-green-500/20 flex items-center justify-center">
            <CheckCircle className="w-10 h-10 text-green-400" />
          </div>
          <div>
            <h2 className="text-2xl font-bold text-white">You're All Set!</h2>
            {result.name ? (
              <p className="text-dag-accent text-lg font-semibold mt-2">You're {result.name}</p>
            ) : (
              <p className="text-dag-muted mt-2">Your wallet is ready.</p>
            )}
            <p className="text-dag-muted text-sm mt-2 font-mono break-all">{result.address}</p>
          </div>

          <div className="bg-dag-card border border-dag-border rounded-xl p-4 text-left space-y-2">
            <p className="text-sm text-dag-muted">Your wallet is secured by your device's biometrics.</p>
            <p className="text-sm text-dag-muted">
              <strong className="text-white">Tip:</strong> Go to SmartAccount settings to add a backup device and set up recovery guardians.
            </p>
          </div>

          <button
            onClick={() => onComplete(result.address, result.name)}
            className="w-full py-4 bg-dag-accent hover:bg-dag-accent/90 text-white font-semibold rounded-xl transition-all text-lg"
          >
            Open Wallet
          </button>
        </div>
      )}
    </div>
  );
}
