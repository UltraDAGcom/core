/**
 * Test data generators for UltraDAG E2E tests
 */

/**
 * Generate a random UltraDAG address
 * Format: dag1... (bech32 encoded)
 */
export function generateAddress(): string {
  const chars = 'qpzry9x8gf2tvdw0s3jn54khce6mua7l';
  let result = 'dag1';
  for (let i = 0; i < 38; i++) {
    result += chars[Math.floor(Math.random() * chars.length)];
  }
  return result;
}

/**
 * Generate a random transaction hash
 */
export function generateTxHash(): string {
  const chars = '0123456789abcdef';
  let result = '0x';
  for (let i = 0; i < 64; i++) {
    result += chars[Math.floor(Math.random() * chars.length)];
  }
  return result;
}

/**
 * Generate a random vertex hash
 */
export function generateVertexHash(): string {
  const chars = '0123456789abcdef';
  let result = '0x';
  for (let i = 0; i < 64; i++) {
    result += chars[Math.floor(Math.random() * chars.length)];
  }
  return result;
}

/**
 * Generate a random secret key
 */
export function generateSecretKey(): string {
  const chars = '0123456789abcdef';
  let result = '';
  for (let i = 0; i < 64; i++) {
    result += chars[Math.floor(Math.random() * chars.length)];
  }
  return result;
}

/**
 * Generate a random wallet name
 */
export function generateWalletName(): string {
  const adjectives = ['Happy', 'Swift', 'Brave', 'Calm', 'Eager', 'Gentle', 'Proud', 'Wise'];
  const nouns = ['Panda', 'Eagle', 'Tiger', 'Dolphin', 'Falcon', 'Wolf', 'Bear', 'Lion'];
  const adj = adjectives[Math.floor(Math.random() * adjectives.length)];
  const noun = nouns[Math.floor(Math.random() * nouns.length)];
  const num = Math.floor(Math.random() * 1000);
  return `${adj}${noun}${num}`;
}

/**
 * Generate a random password
 */
export function generatePassword(): string {
  return `SecurePass${Math.floor(Math.random() * 1000)}!`;
}

/**
 * Generate random amount of DAG tokens
 */
export function generateDagAmount(min: number = 1, max: number = 1000): number {
  return Number((Math.random() * (max - min) + min).toFixed(6));
}

/**
 * Mock wallet data for testing
 */
export interface MockWallet {
  name: string;
  address: string;
  secretKey: string;
  password: string;
  balance: number;
  staked: number;
  delegated: number;
}

/**
 * Generate a complete mock wallet
 */
export function generateMockWallet(): MockWallet {
  return {
    name: generateWalletName(),
    address: generateAddress(),
    secretKey: generateSecretKey(),
    password: generatePassword(),
    balance: generateDagAmount(100, 10000),
    staked: generateDagAmount(0, 5000),
    delegated: generateDagAmount(0, 2000),
  };
}

/**
 * Mock transaction data
 */
export interface MockTransaction {
  hash: string;
  from: string;
  to: string;
  amount: number;
  timestamp: Date;
  status: 'pending' | 'confirmed' | 'failed';
  type: 'transfer' | 'stake' | 'delegate' | 'vote';
}

/**
 * Generate mock transaction
 */
export function generateMockTransaction(overrides?: Partial<MockTransaction>): MockTransaction {
  const statuses: Array<'pending' | 'confirmed' | 'failed'> = ['confirmed', 'confirmed', 'confirmed', 'pending', 'failed'];
  const types: Array<'transfer' | 'stake' | 'delegate' | 'vote'> = ['transfer', 'stake', 'delegate', 'vote'];
  
  return {
    hash: generateTxHash(),
    from: generateAddress(),
    to: generateAddress(),
    amount: generateDagAmount(),
    timestamp: new Date(Date.now() - Math.random() * 86400000), // Last 24 hours
    status: statuses[Math.floor(Math.random() * statuses.length)],
    type: types[Math.floor(Math.random() * types.length)],
    ...overrides,
  };
}

/**
 * Mock validator data
 */
export interface MockValidator {
  name: string;
  address: string;
  stake: number;
  apy: number;
  commission: number;
  status: 'active' | 'inactive' | 'jailed';
}

/**
 * Generate mock validator
 */
export function generateMockValidator(): MockValidator {
  const validatorNames = ['NodeOne', 'StakeMaster', 'ValidatorPro', 'UltraNode', 'DAGValidator', 'SecureStake'];
  const statuses: Array<'active' | 'inactive' | 'jailed'> = ['active', 'active', 'active', 'inactive', 'jailed'];
  
  return {
    name: validatorNames[Math.floor(Math.random() * validatorNames.length)],
    address: generateAddress(),
    stake: generateDagAmount(10000, 1000000),
    apy: Number((Math.random() * 20 + 5).toFixed(2)), // 5-25%
    commission: Number((Math.random() * 10).toFixed(2)), // 0-10%
    status: statuses[Math.floor(Math.random() * statuses.length)],
  };
}

/**
 * Mock proposal data for governance
 */
export interface MockProposal {
  id: number;
  title: string;
  description: string;
  proposer: string;
  status: 'active' | 'passed' | 'rejected' | 'expired';
  votesFor: number;
  votesAgainst: number;
  votesAbstain: number;
  endTime: Date;
}

/**
 * Generate mock proposal
 */
export function generateMockProposal(): MockProposal {
  const titles = [
    'Increase Block Reward',
    'Reduce Transaction Fees',
    'Add New Validator',
    'Update Consensus Parameters',
    'Community Fund Allocation',
  ];
  const statuses: Array<'active' | 'passed' | 'rejected' | 'expired'> = ['active', 'active', 'passed', 'rejected'];
  
  return {
    id: Math.floor(Math.random() * 1000),
    title: titles[Math.floor(Math.random() * titles.length)],
    description: 'This is a sample governance proposal for testing purposes.',
    proposer: generateAddress(),
    status: statuses[Math.floor(Math.random() * statuses.length)],
    votesFor: generateDagAmount(1000, 100000),
    votesAgainst: generateDagAmount(100, 50000),
    votesAbstain: generateDagAmount(100, 10000),
    endTime: new Date(Date.now() + Math.random() * 604800000), // Next 7 days
  };
}
