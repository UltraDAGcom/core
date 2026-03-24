import { useState, useEffect, useCallback, useRef } from 'react';
import * as keystore from '../lib/keystore.ts';
import type { Wallet } from '../lib/keystore.ts';

const AUTO_LOCK_TIMEOUT_MS = 15 * 60 * 1000; // 15 minutes

export function useKeystore() {
  const [unlocked, setUnlocked] = useState(keystore.isUnlocked());
  const [hasStore, setHasStore] = useState(keystore.hasKeystore());
  const [wallets, setWallets] = useState<Wallet[]>(keystore.getWallets());
  const [sessionSecondsLeft, setSessionSecondsLeft] = useState(AUTO_LOCK_TIMEOUT_MS / 1000);
  const lastActivityRef = useRef<number>(Date.now());

  const resetActivity = useCallback(() => {
    lastActivityRef.current = Date.now();
  }, []);

  useEffect(() => {
    const loaded = keystore.loadFromStorage();
    setHasStore(loaded || keystore.hasKeystore());

    const unsub = keystore.onKeystoreChange(() => {
      setUnlocked(keystore.isUnlocked());
      setHasStore(keystore.hasKeystore());
      setWallets([...keystore.getWallets()]);
    });
    return unsub;
  }, []);

  // Auto-lock after 15 minutes of inactivity
  // Track ALL user interactions (mouse, keyboard, touch, scroll)
  // Tick every second to update the countdown timer
  useEffect(() => {
    const onActivity = () => { lastActivityRef.current = Date.now(); };
    const events = ['mousedown', 'keydown', 'touchstart', 'scroll', 'mousemove'];
    events.forEach(e => window.addEventListener(e, onActivity, { passive: true }));

    const interval = setInterval(() => {
      if (keystore.isUnlocked()) {
        const elapsed = Date.now() - lastActivityRef.current;
        const remaining = Math.max(0, Math.ceil((AUTO_LOCK_TIMEOUT_MS - elapsed) / 1000));
        setSessionSecondsLeft(remaining);
        if (remaining <= 0) {
          keystore.lock();
        }
      } else {
        setSessionSecondsLeft(AUTO_LOCK_TIMEOUT_MS / 1000);
      }
    }, 1000);

    return () => {
      events.forEach(e => window.removeEventListener(e, onActivity));
      clearInterval(interval);
    };
  }, []);

  const create = useCallback(async (password: string) => {
    await keystore.create(password);
    resetActivity();
  }, [resetActivity]);

  const unlock = useCallback(async (password: string) => {
    const result = await keystore.unlock(password);
    resetActivity();
    return result;
  }, [resetActivity]);

  const lock = useCallback(() => {
    keystore.lock();
  }, []);

  const addWallet = useCallback(async (name: string, secretKey: string, address: string) => {
    await keystore.addWallet(name, secretKey, address);
    resetActivity();
  }, [resetActivity]);

  const removeWallet = useCallback(async (index: number) => {
    await keystore.removeWallet(index);
    resetActivity();
  }, [resetActivity]);

  const importBlob = useCallback((json: string) => {
    return keystore.importBlob(json);
  }, []);

  const exportBlob = useCallback(() => {
    return keystore.exportBlob();
  }, []);

  const webauthnAvailable = keystore.isWebAuthnAvailable();
  const [webauthnEnrolled, setWebauthnEnrolled] = useState(keystore.isWebAuthnEnrolled());

  // Keep webauthn state in sync
  useEffect(() => {
    const unsub2 = keystore.onKeystoreChange(() => {
      setWebauthnEnrolled(keystore.isWebAuthnEnrolled());
    });
    return unsub2;
  }, []);

  const enrollWebAuthn = useCallback(async () => {
    const ok = await keystore.enrollWebAuthn();
    setWebauthnEnrolled(keystore.isWebAuthnEnrolled());
    resetActivity();
    return ok;
  }, [resetActivity]);

  const unlockWithWebAuthn = useCallback(async () => {
    const ok = await keystore.unlockWithWebAuthn();
    resetActivity();
    return ok;
  }, [resetActivity]);

  const removeWebAuthnCred = useCallback(() => {
    keystore.removeWebAuthn();
    setWebauthnEnrolled(false);
  }, []);

  return {
    unlocked,
    hasStore,
    wallets,
    create,
    unlock,
    lock,
    addWallet,
    removeWallet,
    importBlob,
    exportBlob,
    resetActivity,
    webauthnAvailable,
    webauthnEnrolled,
    enrollWebAuthn,
    unlockWithWebAuthn,
    removeWebAuthn: removeWebAuthnCred,
    sessionSecondsLeft,
    sessionTotalSeconds: AUTO_LOCK_TIMEOUT_MS / 1000,
  };
}

export function useWalletSelector() {
  const { wallets, unlocked } = useKeystore();
  const [selectedIdx, setSelectedIdx] = useState(0);
  const selected = wallets[selectedIdx] ?? null;
  return { wallets, unlocked, selected, selectedIdx, setSelectedIdx };
}
