import { Page } from '@playwright/test';

/**
 * API mocking utilities for E2E tests
 */

/**
 * Mock the node status API response
 */
export async function mockNodeStatus(page: Page, overrides?: Partial<NodeStatusResponse>) {
  const defaultResponse: NodeStatusResponse = {
    connected: true,
    nodeUrl: 'http://localhost:8080',
    network: 'testnet',
    height: 12345,
    tps: 150,
    mempoolSize: 42,
    peerCount: 8,
    ...overrides,
  };

  await page.route('**/api/status', async (route) => {
    await route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify(defaultResponse),
    });
  });
}

/**
 * Mock wallet balances API
 */
export async function mockWalletBalances(page: Page, balances: WalletBalance[]) {
  await page.route('**/api/balances', async (route) => {
    await route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify({ balances }),
    });
  });
}

/**
 * Mock transaction history API
 */
export async function mockTransactionHistory(page: Page, transactions: any[]) {
  await page.route('**/api/transactions/**', async (route) => {
    await route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify({ transactions, total: transactions.length }),
    });
  });
}

/**
 * Mock vertex/round data API
 */
export async function mockVertexData(page: Page, vertices: any[]) {
  await page.route('**/api/vertices/**', async (route) => {
    await route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify({ vertices, total: vertices.length }),
    });
  });
}

/**
 * Mock validator data API
 */
export async function mockValidators(page: Page, validators: any[]) {
  await page.route('**/api/validators', async (route) => {
    await route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify({ validators, total: validators.length }),
    });
  });
}

/**
 * Mock governance proposals API
 */
export async function mockProposals(page: Page, proposals: any[]) {
  await page.route('**/api/proposals', async (route) => {
    await route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify({ proposals, total: proposals.length }),
    });
  });
}

/**
 * Mock council members API
 */
export async function mockCouncilMembers(page: Page, members: any[]) {
  await page.route('**/api/council', async (route) => {
    await route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify({ members, total: members.length }),
    });
  });
}

/**
 * Mock network stats API
 */
export async function mockNetworkStats(page: Page, stats: NetworkStats) {
  await page.route('**/api/network/stats', async (route) => {
    await route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify(stats),
    });
  });
}

/**
 * Mock keygen API (for wallet creation)
 */
export async function mockKeygen(page: Page, address: string, secretKey: string) {
  await page.route('**/api/keygen', async (route) => {
    await route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify({ address, secretKey }),
    });
  });
}

/**
 * Mock transaction submission API
 */
export async function mockSubmitTransaction(page: Page, txHash: string) {
  await page.route('**/api/tx/submit', async (route) => {
    await route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify({ hash: txHash, status: 'pending' }),
    });
  });
}

/**
 * Types for API responses
 */
export interface NodeStatusResponse {
  connected: boolean;
  nodeUrl: string;
  network: string;
  height: number;
  tps: number;
  mempoolSize: number;
  peerCount: number;
}

export interface WalletBalance {
  address: string;
  balance: number;
  staked: number;
  delegated: number;
  pending: number;
}

export interface NetworkStats {
  height: number;
  tps: number;
  totalTransactions: number;
  totalVertices: number;
  activeValidators: number;
  totalStaked: number;
  inflationRate: number;
}
