/**
 * PasskeySignIn — a small inline panel used inside PasskeyOnboarding's
 * welcome screen. Lets a user who already registered a passkey (possibly
 * synced via iCloud Keychain / Google Password Manager) sign in on a fresh
 * browser without going through the create-new-account flow.
 *
 * Flow:
 *   - User enters their @name or udag1… bech32 address.
 *   - We call signInWithPasskey() which talks to the node, triggers a
 *     WebAuthn discoverable credentials prompt, verifies the returned
 *     signature client-side against the SmartAccount's authorized pubkeys,
 *     and on success populates localStorage via savePasskeyWallet().
 *   - On success we call onSuccess with the resolved address + name.
 */

import { useCallback, useState } from 'react';
import { primaryButtonStyle } from '../../lib/theme';
import { signInWithPasskey } from '../../lib/passkey-signin';

interface PasskeySignInProps {
  onSuccess: (address: string, name: string | null) => void;
  onCancel: () => void;
}

export function PasskeySignIn({ onSuccess, onCancel }: PasskeySignInProps) {
  const [nameOrAddress, setNameOrAddress] = useState('');
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState('');

  const canSubmit = nameOrAddress.trim().length > 0 && !loading;

  const handleSubmit = useCallback(async () => {
    if (!canSubmit) return;
    setError('');
    setLoading(true);
    try {
      const { wallet } = await signInWithPasskey(nameOrAddress);
      onSuccess(wallet.address, wallet.name);
    } catch (e: unknown) {
      const msg = e instanceof Error ? e.message : String(e);
      setError(msg);
    } finally {
      setLoading(false);
    }
  }, [nameOrAddress, canSubmit, onSuccess]);

  const inputStyle: React.CSSProperties = {
    width: '100%',
    padding: '13px 14px',
    borderRadius: 12,
    background: 'var(--dag-input-bg)',
    border: '1px solid var(--dag-border)',
    color: 'var(--dag-text)',
    fontSize: 14,
    outline: 'none',
    fontFamily: "'DM Sans',sans-serif",
    transition: 'border-color 0.2s',
  };

  return (
    <div
      style={{
        background: 'var(--dag-card)',
        border: '1px solid var(--dag-border)',
        borderRadius: 16,
        padding: 20,
        marginBottom: 16,
        animation: 'slideUp 0.3s ease',
      }}
    >
      <div
        style={{
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'space-between',
          marginBottom: 14,
        }}
      >
        <h3 style={{ fontSize: 14.5, fontWeight: 700, color: 'var(--dag-text)', margin: 0 }}>
          Sign in with existing passkey
        </h3>
        <button
          onClick={onCancel}
          aria-label="Close sign-in panel"
          style={{
            background: 'none',
            border: 'none',
            color: 'var(--dag-text-muted)',
            fontSize: 18,
            cursor: 'pointer',
            padding: 0,
            lineHeight: 1,
          }}
        >
          ×
        </button>
      </div>

      <p
        style={{
          fontSize: 11.5,
          color: 'var(--dag-text-muted)',
          lineHeight: 1.5,
          marginTop: 0,
          marginBottom: 14,
        }}
      >
        Enter your <strong>@name</strong> or address. We'll look up your account
        and prompt you to authenticate with your passkey (Face ID, fingerprint,
        Windows Hello, etc).
      </p>

      <input
        type="text"
        value={nameOrAddress}
        onChange={(e) => {
          setNameOrAddress(e.target.value);
          setError('');
        }}
        onKeyDown={(e) => e.key === 'Enter' && handleSubmit()}
        placeholder="@alice or udag1…"
        disabled={loading}
        autoFocus
        autoCapitalize="none"
        autoCorrect="off"
        spellCheck={false}
        style={inputStyle}
      />

      {error && (
        <div
          style={{
            marginTop: 12,
            padding: '10px 12px',
            borderRadius: 10,
            background: 'rgba(255, 80, 80, 0.08)',
            border: '1px solid rgba(255, 80, 80, 0.2)',
            color: '#ff6b6b',
            fontSize: 11.5,
            lineHeight: 1.5,
          }}
        >
          {error}
        </div>
      )}

      <button
        onClick={handleSubmit}
        disabled={!canSubmit}
        style={{
          ...primaryButtonStyle,
          width: '100%',
          marginTop: 14,
          padding: '13px 0',
          borderRadius: 12,
          fontSize: 14,
          opacity: canSubmit ? 1 : 0.5,
          cursor: canSubmit ? 'pointer' : 'not-allowed',
          display: 'inline-flex',
          alignItems: 'center',
          justifyContent: 'center',
          gap: 8,
        }}
      >
        {loading ? (
          <>
            <span
              style={{
                width: 14,
                height: 14,
                border: '2px solid rgba(255,255,255,0.3)',
                borderTopColor: '#fff',
                borderRadius: '50%',
                animation: 'spin 0.8s linear infinite',
                display: 'inline-block',
              }}
            />
            Verifying…
          </>
        ) : (
          <>Sign in with passkey →</>
        )}
      </button>
    </div>
  );
}
