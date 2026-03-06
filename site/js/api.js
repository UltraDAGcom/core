// RPC client for UltraDAG node

let baseUrl = '';

export function setNodeUrl(url) {
  baseUrl = url.replace(/\/+$/, '');
}

export function getNodeUrl() {
  return baseUrl;
}

async function rpc(path, opts) {
  const res = await fetch(baseUrl + path, opts);
  const data = await res.json();
  if (data.error) throw new Error(data.error);
  return data;
}

export async function getStatus() {
  return rpc('/status');
}

export async function getBalance(address) {
  return rpc('/balance/' + address);
}

export async function getRound(round) {
  return rpc('/round/' + round);
}

export async function getMempool() {
  return rpc('/mempool');
}

export async function keygen() {
  return rpc('/keygen');
}

export async function sendTx({ from_secret, to, amount, fee }) {
  return rpc('/tx', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ from_secret, to, amount, fee }),
  });
}
