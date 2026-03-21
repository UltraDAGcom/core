// UltraDAG Node API client — Bech32m address support (tudg1.../udag1...)

const TESTNET_NODES = [
  'https://ultradag-node-1.fly.dev',
  'https://ultradag-node-2.fly.dev',
  'https://ultradag-node-3.fly.dev',
  'https://ultradag-node-4.fly.dev',
  'https://ultradag-node-5.fly.dev',
];

const MAINNET_NODES = [
  'https://ultradag-mainnet-1.fly.dev',
  'https://ultradag-mainnet-2.fly.dev',
  'https://ultradag-mainnet-3.fly.dev',
  'https://ultradag-mainnet-4.fly.dev',
  'https://ultradag-mainnet-5.fly.dev',
];

export type NetworkType = 'mainnet' | 'testnet';

// Persist network choice in localStorage
function loadNetwork(): NetworkType {
  try {
    const stored = localStorage.getItem('ultradag_network');
    if (stored === 'mainnet' || stored === 'testnet') return stored;
  } catch {}
  return 'testnet'; // default
}

let currentNetwork: NetworkType = loadNetwork();
let currentNode = (currentNetwork === 'mainnet' ? MAINNET_NODES : TESTNET_NODES)[0];
let connected = false;

export function getNetwork(): NetworkType { return currentNetwork; }
export function isMainnet(): boolean { return currentNetwork === 'mainnet'; }
export function getNodeUrl() { return currentNode; }
export function isConnected() { return connected; }

export function switchNetwork(network: NetworkType) {
  currentNetwork = network;
  localStorage.setItem('ultradag_network', network);
  const nodes = network === 'mainnet' ? MAINNET_NODES : TESTNET_NODES;
  currentNode = nodes[0];
  connected = false;
}

async function fetchJson<T>(path: string, options?: RequestInit): Promise<T> {
  const res = await fetch(currentNode + path, {
    ...options,
    headers: { 'Content-Type': 'application/json', ...options?.headers },
  });
  if (!res.ok) {
    const body = await res.text();
    throw new Error(body || `HTTP ${res.status}`);
  }
  return res.json();
}

export async function connectToNode(): Promise<boolean> {
  const nodes = currentNetwork === 'mainnet' ? MAINNET_NODES : TESTNET_NODES;
  for (const node of nodes) {
    try {
      const res = await fetch(node + '/health', { signal: AbortSignal.timeout(5000) });
      if (res.ok) {
        currentNode = node;
        connected = true;
        return true;
      }
    } catch {}
  }
  connected = false;
  return false;
}

// GET endpoints
export const getStatus = () => fetchJson<any>('/status');
export const getBalance = (addr: string) => fetchJson<any>(`/balance/${addr}`);
export const getMempool = () => fetchJson<any>('/mempool');
export const getPeers = () => fetchJson<any>('/peers');
export const getValidators = () => fetchJson<any>('/validators');
export const getStake = (addr: string) => fetchJson<any>(`/stake/${addr}`);
export const getDelegation = (addr: string) => fetchJson<any>(`/delegation/${addr}`);
export const getProposals = () => fetchJson<any>('/proposals');
export const getProposal = (id: number) => fetchJson<any>(`/proposal/${id}`);
export const getCouncil = () => fetchJson<any>('/council');
export const getGovernanceConfig = () => fetchJson<any>('/governance/config');
export const getRound = (n: number) => fetchJson<any>(`/round/${n}`);
export const getVertex = (hash: string) => fetchJson<any>(`/vertex/${hash}`);
export const getTx = (hash: string) => fetchJson<any>(`/tx/${hash}`);
export const getFeeEstimate = () => fetchJson<any>('/fee-estimate');
export const getMetrics = () => fetchJson<any>('/metrics/json');
export const getHealthDetailed = () => fetchJson<any>('/health/detailed');

// POST endpoints
export const postTx = (body: any) => fetchJson<any>('/tx', { method: 'POST', body: JSON.stringify(body) });
export const postTxSubmit = (body: any) => fetchJson<any>('/tx/submit', { method: 'POST', body: JSON.stringify(body) });
export const postStake = (body: any) => fetchJson<any>('/stake', { method: 'POST', body: JSON.stringify(body) });
export const postUnstake = (body: any) => fetchJson<any>('/unstake', { method: 'POST', body: JSON.stringify(body) });
export const postDelegate = (body: any) => fetchJson<any>('/delegate', { method: 'POST', body: JSON.stringify(body) });
export const postUndelegate = (body: any) => fetchJson<any>('/undelegate', { method: 'POST', body: JSON.stringify(body) });
export const postSetCommission = (body: any) => fetchJson<any>('/set-commission', { method: 'POST', body: JSON.stringify(body) });
export const postFaucet = (body: any) => fetchJson<any>('/faucet', { method: 'POST', body: JSON.stringify(body) });
export const postProposal = (body: any) => fetchJson<any>('/proposal', { method: 'POST', body: JSON.stringify(body) });
export const postVote = (body: any) => fetchJson<any>('/vote', { method: 'POST', body: JSON.stringify(body) });

// Bridge endpoints (Validator Federation Bridge)
export const getBridgeNonce = () => fetchJson<{ next_nonce: number }>('/bridge/nonce');
export const getBridgeAttestation = (nonce: number) => fetchJson<any>(`/bridge/attestation/${nonce}`);
export const getBridgeReserve = () => fetchJson<{ reserve_sats: number; reserve_udag: number }>('/bridge/reserve');

// Bech32m encoding/decoding (BIP-350)
const BECH32M_CONST = 0x2bc830a3;
const CHARSET = 'qpzry9x8gf2tvdw0s3jn54khce6mua7l';

function bech32mPolymod(values: number[]): number {
  const GEN = [0x3b6a57b2, 0x26508e6d, 0x1ea119fa, 0x3d4233dd, 0x2a1462b3];
  let chk = 1;
  for (const v of values) {
    const b = chk >> 25;
    chk = ((chk & 0x1ffffff) << 5) ^ v;
    for (let i = 0; i < 5; i++) if ((b >> i) & 1) chk ^= GEN[i];
  }
  return chk;
}

function bech32mHrpExpand(hrp: string): number[] {
  const ret: number[] = [];
  for (let i = 0; i < hrp.length; i++) ret.push(hrp.charCodeAt(i) >> 5);
  ret.push(0);
  for (let i = 0; i < hrp.length; i++) ret.push(hrp.charCodeAt(i) & 31);
  return ret;
}

function bech32mCreateChecksum(hrp: string, data: number[]): number[] {
  const polymod = bech32mPolymod([...bech32mHrpExpand(hrp), ...data, 0, 0, 0, 0, 0, 0]) ^ BECH32M_CONST;
  const ret: number[] = [];
  for (let i = 0; i < 6; i++) ret.push((polymod >> (5 * (5 - i))) & 31);
  return ret;
}

function bech32mVerifyChecksum(hrp: string, data: number[]): boolean {
  return bech32mPolymod([...bech32mHrpExpand(hrp), ...data]) === BECH32M_CONST;
}

function convertBits(data: number[], fromBits: number, toBits: number, pad: boolean): number[] | null {
  let acc = 0, bits = 0;
  const ret: number[] = [];
  const maxv = (1 << toBits) - 1;
  for (const v of data) {
    if (v < 0 || v >> fromBits) return null;
    acc = (acc << fromBits) | v;
    bits += fromBits;
    while (bits >= toBits) {
      bits -= toBits;
      ret.push((acc >> bits) & maxv);
    }
  }
  if (pad) {
    if (bits > 0) ret.push((acc << (toBits - bits)) & maxv);
  } else if (bits >= fromBits || ((acc << (toBits - bits)) & maxv)) {
    return null;
  }
  return ret;
}

export function addressToBech32(hexAddr: string, testnet = currentNetwork !== 'mainnet'): string {
  const hrp = testnet ? 'tudg' : 'udag';
  const bytes: number[] = [];
  for (let i = 0; i < hexAddr.length; i += 2) {
    bytes.push(parseInt(hexAddr.substring(i, i + 2), 16));
  }
  const data5 = convertBits(bytes, 8, 5, true);
  if (!data5) return hexAddr; // fallback
  const checksum = bech32mCreateChecksum(hrp, data5);
  return hrp + '1' + [...data5, ...checksum].map(d => CHARSET[d]).join('');
}

export function bech32ToHex(bech: string): string | null {
  const lower = bech.toLowerCase();
  const pos = lower.lastIndexOf('1');
  if (pos < 1 || pos + 7 > lower.length) return null;
  const hrp = lower.substring(0, pos);
  if (hrp !== 'udag' && hrp !== 'tudg') return null;
  const data: number[] = [];
  for (let i = pos + 1; i < lower.length; i++) {
    const d = CHARSET.indexOf(lower[i]);
    if (d === -1) return null;
    data.push(d);
  }
  if (!bech32mVerifyChecksum(hrp, data)) return null;
  const conv = convertBits(data.slice(0, -6), 5, 8, false);
  if (!conv || conv.length !== 20) return null;
  return conv.map(b => b.toString(16).padStart(2, '0')).join('');
}

export function isValidAddress(s: string): boolean {
  if (/^[0-9a-fA-F]{40}$/.test(s)) return true;
  return bech32ToHex(s) !== null;
}

export function normalizeAddress(s: string): string {
  const hex = bech32ToHex(s);
  if (hex) return hex;
  return s.toLowerCase();
}

// Helpers
export function formatUdag(sats: number): string {
  return (sats / 100_000_000).toLocaleString(undefined, { minimumFractionDigits: 2, maximumFractionDigits: 4 });
}

export function bytesToHex(bytes: number[]): string {
  return bytes.map(b => b.toString(16).padStart(2, '0')).join('');
}

export function formatProposalType(pt: unknown): string {
  if (typeof pt === 'string') return pt;
  if (pt && typeof pt === 'object') {
    const key = Object.keys(pt)[0];
    if (key === 'CouncilMembership') {
      const v = (pt as Record<string, Record<string, unknown>>)[key];
      const addr = Array.isArray(v.address) ? shortAddr(bytesToHex(v.address as number[])) : shortAddr(String(v.address));
      return `Council: ${v.action} ${v.category} (${addr})`;
    }
    if (key === 'ParameterChange') {
      const v = (pt as Record<string, Record<string, unknown>>)[key];
      return `Param: ${v.param} → ${v.value}`;
    }
    if (key === 'TreasurySpend') {
      const v = (pt as Record<string, Record<string, unknown>>)[key];
      const addr = Array.isArray(v.recipient) ? shortAddr(bytesToHex(v.recipient as number[])) : shortAddr(String(v.recipient));
      return `Treasury: ${formatUdag(v.amount as number)} to ${addr}`;
    }
    if (key === 'Text') return 'Text';
    return key;
  }
  return String(pt);
}

export function shortHash(hash: string): string {
  if (!hash || hash.length < 12) return hash || '';
  return hash.slice(0, 8) + '...' + hash.slice(-4);
}

export function shortAddr(addr: string): string {
  if (!addr) return '';
  // If it's already bech32m, shorten it
  if (addr.startsWith('udag1') || addr.startsWith('tudg1')) {
    return addr.length > 20 ? addr.slice(0, 12) + '...' + addr.slice(-6) : addr;
  }
  // If hex, convert to bech32m first then shorten
  if (/^[0-9a-fA-F]{40}$/.test(addr)) {
    const bech = addressToBech32(addr);
    return bech.length > 20 ? bech.slice(0, 12) + '...' + bech.slice(-6) : bech;
  }
  // Fallback for short/unknown
  if (addr.length < 16) return addr;
  return addr.slice(0, 10) + '...' + addr.slice(-6);
}

export function fullAddr(addr: string): string {
  if (!addr) return '';
  if (addr.startsWith('udag1') || addr.startsWith('tudg1')) return addr;
  if (/^[0-9a-fA-F]{40}$/.test(addr)) return addressToBech32(addr);
  return addr;
}

/** Format a bech32m address in grouped chunks for readability: tudg1 mq3n r25w kxgf ... */
export function prettyAddr(addr: string): string {
  const bech = fullAddr(addr);
  if (!bech) return '';
  // Split: prefix (tudg1/udag1) + data in groups of 4
  const sepIdx = bech.indexOf('1');
  if (sepIdx < 1) return bech;
  const prefix = bech.slice(0, sepIdx + 1);
  const data = bech.slice(sepIdx + 1);
  const groups: string[] = [];
  for (let i = 0; i < data.length; i += 4) {
    groups.push(data.slice(i, i + 4));
  }
  return prefix + ' ' + groups.join(' ');
}
