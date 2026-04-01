import { useState, useEffect, useRef } from 'react';
import { getNodeUrl } from '../lib/api';

export interface AddressStatus {
  /** The resolved hex address (null if unresolvable) */
  address: string | null;
  /** Balance in sats (null if not found) */
  balance: number | null;
  /** Registered name (null if none) */
  name: string | null;
  /** Whether the address exists on-chain (has been seen) */
  exists: boolean;
  /** Whether we're currently checking */
  checking: boolean;
  /** Error message if check failed */
  error: string | null;
}

const EMPTY: AddressStatus = { address: null, balance: null, name: null, exists: false, checking: false, error: null };

/**
 * Hook that verifies an address/name against the live node.
 * Debounces 400ms to avoid spamming the API on every keystroke.
 * Accepts hex addresses, bech32m addresses, and registered names.
 */
export function useAddressVerify(input: string): AddressStatus {
  const [status, setStatus] = useState<AddressStatus>(EMPTY);
  const timerRef = useRef<ReturnType<typeof setTimeout>>(undefined);

  useEffect(() => {
    const trimmed = input.trim();

    // Reset if empty or too short
    if (trimmed.length < 3) {
      setStatus(EMPTY);
      return;
    }

    setStatus(prev => ({ ...prev, checking: true, error: null }));

    // Debounce
    clearTimeout(timerRef.current);
    timerRef.current = setTimeout(async () => {
      try {
        // Use the /balance endpoint which accepts hex, bech32m, and names
        const res = await fetch(`${getNodeUrl()}/balance/${encodeURIComponent(trimmed)}`, {
          signal: AbortSignal.timeout(4000),
        });

        if (res.ok) {
          const data = await res.json();
          setStatus({
            address: data.address || trimmed,
            balance: data.balance ?? null,
            name: data.name ?? null,
            exists: true,
            checking: false,
            error: null,
          });
        } else {
          // 400 = invalid address format, 404 = not found
          const isInvalid = res.status === 400;
          setStatus({
            address: null,
            balance: null,
            name: null,
            exists: false,
            checking: false,
            error: isInvalid ? 'Invalid address format' : null,
          });
        }
      } catch {
        setStatus(prev => ({ ...prev, checking: false, error: 'Network error' }));
      }
    }, 400);

    return () => clearTimeout(timerRef.current);
  }, [input]);

  return status;
}
