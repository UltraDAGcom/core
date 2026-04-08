import { useState, useEffect, useCallback } from 'react';
import * as pw from '../lib/passkey-wallet';
import type { PasskeyWallet } from '../lib/passkey-wallet';

export function usePasskeyWallet() {
  const [wallet, setWallet] = useState<PasskeyWallet | null>(pw.getPasskeyWallet());
  const [unlocked, setUnlocked] = useState(pw.isUnlocked());

  useEffect(() => {
    const unsub = pw.onPasskeyChange(() => {
      setWallet(pw.getPasskeyWallet());
      setUnlocked(pw.isUnlocked());
    });
    return unsub;
  }, []);

  const unlock = useCallback(async () => {
    return pw.unlockWithBiometric();
  }, []);

  const lock = useCallback(() => {
    pw.lock();
  }, []);

  const destroy = useCallback(() => {
    pw.destroy();
  }, []);

  return {
    wallet,
    hasWallet: pw.hasPasskeyWallet(),
    unlocked,
    unlock,
    lock,
    destroy,
  };
}
