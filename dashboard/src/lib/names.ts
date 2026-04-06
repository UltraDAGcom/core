/**
 * Name + pocket resolution for the UltraDAG name registry.
 *
 * Syntax:
 *   "@alice"          → parent name "alice", no pocket (resolves to alice's primary address)
 *   "@alice.savings"  → parent name "alice", pocket label "savings"
 *   "alice.savings"   → same (@ is optional)
 *   "alice"           → same as @alice
 *
 * One-level only: "alice.foo.bar" is invalid.
 */

import { getNodeUrl } from './api';

export interface ResolvedAddress {
  address: string;        // hex address
  bech32?: string;        // bech32m address (from RPC)
  parent: string;         // parent name ("alice")
  label: string | null;   // pocket label, or null for main
  isPerpetual: boolean;   // whether the parent name is permanent
  availablePockets: string[]; // labels of all pockets on this name (for pocket picker)
}

export class NameNotFoundError extends Error {
  name_: string;
  constructor(name: string) {
    super(`Name not found: @${name}`);
    this.name_ = name;
  }
}

export class PocketNotFoundError extends Error {
  parent_: string;
  label_: string;
  available_: string[];
  constructor(parent: string, label: string, available: string[]) {
    super(
      available.length > 0
        ? `@${parent}.${label} not found. Available: ${available.map(l => `@${parent}.${l}`).join(', ')}`
        : `@${parent} has no pockets.`,
    );
    this.parent_ = parent;
    this.label_ = label;
    this.available_ = available;
  }
}

/**
 * Parse user input into (parent, label) pair.
 * Returns null label when no pocket is specified.
 */
export function parsePocketName(input: string): { parent: string; label: string | null } {
  const cleaned = input.replace(/^@/, '').toLowerCase().trim();
  const dotIndex = cleaned.indexOf('.');
  if (dotIndex === -1) {
    return { parent: cleaned, label: null };
  }
  const parent = cleaned.slice(0, dotIndex);
  const label = cleaned.slice(dotIndex + 1);
  // Reject nested dots ("alice.foo.bar")
  if (label.includes('.')) {
    throw new Error('Nested pocket labels are not supported (max one dot).');
  }
  if (!label) {
    throw new Error('Empty pocket label. Use @name or @name.label.');
  }
  return { parent, label };
}

/**
 * Resolve a name (with optional pocket) to an on-chain address.
 *
 * 1. Fetches /name/info/{parent} to get the owner address + profile.
 * 2. If no label: returns the owner address.
 * 3. If label: finds the matching pocket in the profile.
 *
 * Throws NameNotFoundError or PocketNotFoundError as appropriate.
 */
export async function resolvePocket(input: string): Promise<ResolvedAddress> {
  const { parent, label } = parsePocketName(input);

  const res = await fetch(`${getNodeUrl()}/name/info/${encodeURIComponent(parent)}`, {
    signal: AbortSignal.timeout(8000),
  });

  if (!res.ok) {
    throw new NameNotFoundError(parent);
  }

  const data = await res.json();
  const pockets: Array<{ label: string; address: string; address_bech32?: string }> =
    data.profile?.pockets ?? [];
  const availableLabels = pockets.map((p: { label: string }) => p.label);

  if (!label) {
    return {
      address: data.owner,
      bech32: data.owner_bech32,
      parent,
      label: null,
      isPerpetual: data.is_perpetual === true,
      availablePockets: availableLabels,
    };
  }

  const pocket = pockets.find((p: { label: string }) => p.label === label);
  if (!pocket) {
    throw new PocketNotFoundError(parent, label, availableLabels);
  }

  return {
    address: pocket.address,
    bech32: pocket.address_bech32,
    parent,
    label,
    isPerpetual: data.is_perpetual === true,
    availablePockets: availableLabels,
  };
}
