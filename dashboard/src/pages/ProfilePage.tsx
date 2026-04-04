import { useState, useEffect } from 'react';
import { useParams, Link } from 'react-router-dom';
import { useProfile } from '../hooks/useProfile';
import { useKeystore } from '../hooks/useKeystore';
import { useIsMobile } from '../hooks/useIsMobile';
import { getPasskeyWallet } from '../lib/passkey-wallet';
import { getBalance, normalizeAddress, isValidAddress } from '../lib/api';
import { UltraIdCard } from '../components/profile/UltraIdCard';
import { EditProfileModal } from '../components/profile/EditProfileModal';
import { ProfileActivity } from '../components/profile/ProfileActivity';
import { PageHeader } from '../components/shared/PageHeader';
import { buttonStyle as themeButtonStyle } from '../lib/theme';

export function ProfilePage() {
  const { nameOrAddress } = useParams<{ nameOrAddress: string }>();
  const { wallets, unlocked } = useKeystore();
  const m = useIsMobile();

  const [resolvedAddress, setResolvedAddress] = useState<string | null>(null);
  const [resolving, setResolving] = useState(true);
  const [showEdit, setShowEdit] = useState(false);

  // Resolve "me" → current wallet, "@name" → address, hex/bech32 → normalize
  useEffect(() => {
    setResolving(true);
    const resolve = async () => {
      let input = nameOrAddress ?? '';

      // "me" → current wallet address
      if (input === 'me') {
        const pk = getPasskeyWallet();
        const addr = pk?.address ?? wallets[0]?.address;
        if (addr) { setResolvedAddress(addr); setResolving(false); return; }
        setResolvedAddress(null); setResolving(false); return;
      }

      // Strip @ prefix if present
      input = input.replace(/^@/, '');

      // Try as address first (hex or bech32)
      if (isValidAddress(input)) {
        setResolvedAddress(normalizeAddress(input));
        setResolving(false);
        return;
      }

      // Resolve as name via /balance/ endpoint (accepts names)
      try {
        const data = await getBalance(input);
        if (data?.address) { setResolvedAddress(data.address); setResolving(false); return; }
      } catch { /* not found */ }

      setResolvedAddress(null);
      setResolving(false);
    };
    resolve();
  }, [nameOrAddress, wallets]);

  const { profile, badges, loading, error, refresh } = useProfile(resolvedAddress ?? undefined);

  // Check if this is the current user's profile
  const pk = getPasskeyWallet();
  const myAddresses = [pk?.address, ...wallets.map(w => w.address)].filter(Boolean).map(a => a!.toLowerCase());
  const isOwnProfile = resolvedAddress ? myAddresses.includes(resolvedAddress.toLowerCase()) : false;
  const editableWallet = wallets.find(w => w.secret_key && w.address.toLowerCase() === resolvedAddress?.toLowerCase());

  if (resolving || loading) {
    return (
      <div style={{ padding: m ? '12px 14px' : '18px 26px', fontFamily: "'DM Sans',sans-serif" }}>
        <div style={{ textAlign: 'center', padding: '60px 0' }}>
          <div style={{ width: 32, height: 32, border: '2px solid rgba(0,224,196,0.2)', borderTop: '2px solid #00E0C4', borderRadius: '50%', margin: '0 auto 12px', animation: 'spin 0.8s linear infinite' }} />
          <style>{`@keyframes spin{to{transform:rotate(360deg)}}`}</style>
          <p style={{ fontSize: 12, color: 'var(--dag-text-faint)' }}>Loading profile...</p>
        </div>
      </div>
    );
  }

  if (!resolvedAddress || error) {
    return (
      <div style={{ padding: m ? '12px 14px' : '18px 26px', fontFamily: "'DM Sans',sans-serif" }}>
        <div style={{ textAlign: 'center', padding: '60px 0' }}>
          <div style={{ fontSize: 36, color: 'var(--dag-text-faint)', marginBottom: 12, opacity: 0.3 }}>◎</div>
          <p style={{ fontSize: 14, fontWeight: 600, color: 'var(--dag-text)', marginBottom: 4 }}>
            {nameOrAddress === 'me' ? 'No wallet connected' : 'Profile not found'}
          </p>
          <p style={{ fontSize: 12, color: 'var(--dag-text-faint)' }}>
            {nameOrAddress === 'me'
              ? 'Create or unlock a wallet to view your ULTRA ID'
              : `Could not resolve "${nameOrAddress}"`}
          </p>
          <Link to="/" style={{ display: 'inline-block', marginTop: 16, fontSize: 12, color: '#00E0C4', textDecoration: 'none' }}>
            ← Back to Dashboard
          </Link>
        </div>
      </div>
    );
  }

  return (
    <div style={{ fontFamily: "'DM Sans',sans-serif" }}>
      <div style={{ padding: m ? '12px 14px 0' : '18px 26px 0' }}>
        <PageHeader title="ULTRA ID" subtitle="Your on-chain identity" />
      </div>
      <div style={{ padding: m ? '0 14px 14px' : '0 26px 26px', maxWidth: 700, margin: '0 auto' }}>
      {/* ID Card */}
      <div style={{ marginBottom: 20 }}>
        <UltraIdCard
          address={resolvedAddress}
          name={profile?.name ?? null}
          badges={badges}
          balance={profile?.balance ?? 0}
          staked={profile?.staked ?? 0}
          delegatorCount={profile?.delegatorCount}
          createdAtRound={profile?.createdAtRound}
          currentRound={profile?.currentRound}
          bio={profile?.bio}
          size="lg"
        />
      </div>

      {/* Links + Edit button */}
      <div style={{
        background: 'var(--dag-card)', border: '1px solid var(--dag-border)', borderRadius: 14,
        padding: '16px 20px', marginBottom: 20,
      }}>
        <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: profile?.website || profile?.github || profile?.twitter ? 12 : 0 }}>
          <span style={{ fontSize: 10, fontWeight: 600, letterSpacing: 2, color: 'var(--dag-text-faint)', textTransform: 'uppercase' }}>Links</span>
          {isOwnProfile && unlocked && (
            <button onClick={() => setShowEdit(true)} style={{
              ...themeButtonStyle(), padding: '5px 14px', fontSize: 11,
            }}>
              Edit Profile
            </button>
          )}
        </div>

        {(profile?.website || profile?.github || profile?.twitter) ? (
          <div style={{ display: 'flex', flexWrap: 'wrap', gap: 10 }}>
            {profile?.website && (
              <a href={profile.website} target="_blank" rel="noopener noreferrer" style={{ fontSize: 12, color: '#0066FF', textDecoration: 'none' }}>
                🌐 {profile.website.replace(/^https?:\/\//, '')}
              </a>
            )}
            {profile?.github && (
              <a href={`https://github.com/${profile.github}`} target="_blank" rel="noopener noreferrer" style={{ fontSize: 12, color: 'var(--dag-text-secondary)', textDecoration: 'none' }}>
                GitHub: {profile.github}
              </a>
            )}
            {profile?.twitter && (
              <a href={`https://x.com/${profile.twitter.replace(/^@/, '')}`} target="_blank" rel="noopener noreferrer" style={{ fontSize: 12, color: 'var(--dag-text-secondary)', textDecoration: 'none' }}>
                X: @{profile.twitter.replace(/^@/, '')}
              </a>
            )}
          </div>
        ) : (
          <p style={{ fontSize: 12, color: 'var(--dag-text-faint)', textAlign: 'center', padding: '8px 0' }}>
            {isOwnProfile ? 'No links yet — click Edit Profile to add them' : 'No links'}
          </p>
        )}

        {/* External addresses */}
        {profile && profile.externalAddresses.length > 0 && (
          <div style={{ marginTop: 12, paddingTop: 10, borderTop: '1px solid var(--dag-border)' }}>
            <div style={{ fontSize: 9, color: 'var(--dag-text-faint)', letterSpacing: 1, marginBottom: 6 }}>CROSS-CHAIN</div>
            <div style={{ display: 'flex', flexDirection: 'column', gap: 4 }}>
              {profile.externalAddresses.map(([chain, addr]) => (
                <div key={chain} style={{ display: 'flex', justifyContent: 'space-between', fontSize: 11 }}>
                  <span style={{ color: 'var(--dag-text-muted)', fontWeight: 600, textTransform: 'uppercase' }}>{chain}</span>
                  <span style={{ fontFamily: "'DM Mono',monospace", color: 'var(--dag-text-secondary)' }}>
                    {addr.length > 20 ? addr.slice(0, 10) + '...' + addr.slice(-6) : addr}
                  </span>
                </div>
              ))}
            </div>
          </div>
        )}
      </div>

      {/* Activity */}
      {profile && (
        <div style={{
          background: 'var(--dag-card)', border: '1px solid var(--dag-border)', borderRadius: 14,
          padding: '16px 20px',
        }}>
          <div style={{ fontSize: 10, fontWeight: 600, letterSpacing: 2, color: 'var(--dag-text-faint)', textTransform: 'uppercase', marginBottom: 14 }}>
            Activity
          </div>
          <ProfileActivity profile={profile} />
        </div>
      )}

      {/* Edit Modal */}
      {showEdit && profile && (
        <EditProfileModal
          name={profile.name ?? ''}
          wallet={editableWallet ?? wallets[0] ?? { name: '', address: resolvedAddress, secret_key: '' }}
          currentBio={profile.bio}
          currentWebsite={profile.website}
          currentGithub={profile.github}
          currentTwitter={profile.twitter}
          currentExternalAddresses={profile.externalAddresses}
          onClose={() => setShowEdit(false)}
          onSuccess={refresh}
        />
      )}
      </div>
    </div>
  );
}
