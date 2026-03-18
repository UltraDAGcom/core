import { useState, useEffect, useCallback } from 'react';
import * as keystore from '../lib/keystore.ts';
import type { Wallet } from '../lib/keystore.ts';

export function useKeystore() {
  const [unlocked, setUnlocked] = useState(keystore.isUnlocked());
  const [hasStore, setHasStore] = useState(keystore.hasKeystore());
  const [wallets, setWallets] = useState<Wallet[]>(keystore.getWallets());

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

  const create = useCallback(async (password: string) => {
    await keystore.create(password);
  }, []);

  const unlock = useCallback(async (password: string) => {
    return keystore.unlock(password);
  }, []);

  const lock = useCallback(() => {
    keystore.lock();
  }, []);

  const addWallet = useCallback(async (name: string, secretKey: string, address: string) => {
    await keystore.addWallet(name, secretKey, address);
  }, []);

  const removeWallet = useCallback(async (index: number) => {
    await keystore.removeWallet(index);
  }, []);

  const importBlob = useCallback((json: string) => {
    return keystore.importBlob(json);
  }, []);

  const exportBlob = useCallback(() => {
    return keystore.exportBlob();
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
  };
}

export function useWalletSelector() {
  const { wallets, unlocked } = useKeystore();
  const [selectedIdx, setSelectedIdx] = useState(0);
  const selected = wallets[selectedIdx] ?? null;
  return { wallets, unlocked, selected, selectedIdx, setSelectedIdx };
}
