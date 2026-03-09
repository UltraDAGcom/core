# UltraDAG Wallet Improvement Plan

## Current State Analysis

### Problems with Current Wallet
1. **No wallet creation flow** - Users need to manually generate keys via `/keygen`
2. **No key storage** - Users must manually save their secret keys
3. **Raw hex addresses** - 64-character hex strings are not user-friendly
4. **Manual transaction building** - Users must construct JSON payloads manually
5. **No transaction history** - Can't see past transactions
6. **No balance display** - Must manually query `/balance/:address`
7. **No security** - Secret keys exposed in browser (no encryption, no password)
8. **No backup/recovery** - No mnemonic phrases or recovery mechanism
9. **Poor UX** - Technical interface, not suitable for non-technical users
10. **No DAG explorer** - Can't visualize the DAG or see network status

## Improvement Goals

### Phase 1: Core Wallet Functionality (Essential)
**Goal**: Create a secure, user-friendly wallet that anyone can use

#### 1.1 Secure Wallet Creation & Storage
- [ ] **Mnemonic phrase generation** (BIP39-compatible 12-24 words)
- [ ] **HD wallet support** (derive multiple addresses from one seed)
- [ ] **Password encryption** (encrypt wallet data in localStorage)
- [ ] **Wallet creation wizard** (step-by-step onboarding)
- [ ] **Backup reminder** (force users to write down mnemonic)
- [ ] **Recovery flow** (restore wallet from mnemonic)

#### 1.2 User-Friendly Interface
- [ ] **Modern UI framework** (React/Vue/Svelte with TailwindCSS)
- [ ] **Dashboard view** (balance, recent transactions, network status)
- [ ] **Send transaction form** (simple form with validation)
- [ ] **Receive view** (QR code for address, copy button)
- [ ] **Transaction history** (list all transactions for current wallet)
- [ ] **Address book** (save frequently used addresses with labels)

#### 1.3 Security Improvements
- [ ] **Password protection** (unlock wallet with password)
- [ ] **Auto-lock** (lock wallet after inactivity)
- [ ] **Transaction confirmation** (review before signing)
- [ ] **Balance validation** (prevent sending more than available)
- [ ] **Fee estimation** (suggest appropriate fees)

### Phase 2: Enhanced Features (Important)
**Goal**: Make the wallet competitive with modern crypto wallets

#### 2.1 Transaction Management
- [ ] **Transaction status tracking** (pending → confirmed → finalized)
- [ ] **Transaction details view** (full tx info, DAG round, finality status)
- [ ] **Transaction filtering** (by date, amount, status)
- [ ] **Export transactions** (CSV/JSON export)
- [ ] **Transaction notes** (add labels/notes to transactions)

#### 2.2 Multi-Account Support
- [ ] **Multiple wallets** (create/switch between wallets)
- [ ] **Account derivation** (HD wallet with multiple accounts)
- [ ] **Account labels** (name your accounts)
- [ ] **Total portfolio view** (combined balance across accounts)

#### 2.3 Network Visualization
- [ ] **DAG explorer** (visualize DAG structure)
- [ ] **Network status** (validators, rounds, finalization)
- [ ] **Live updates** (WebSocket for real-time updates)
- [ ] **Block/round explorer** (view vertices by round)
- [ ] **Validator info** (see active validators)

### Phase 3: Advanced Features (Nice to Have)
**Goal**: Professional-grade wallet with advanced capabilities

#### 3.1 Advanced Security
- [ ] **Hardware wallet support** (Ledger/Trezor integration)
- [ ] **Multi-signature** (require multiple signatures)
- [ ] **Watch-only addresses** (monitor without private key)
- [ ] **Spending limits** (daily/weekly limits)
- [ ] **2FA support** (optional two-factor authentication)

#### 3.2 Developer Features
- [ ] **Contract interaction** (if smart contracts added later)
- [ ] **API access** (programmatic wallet access)
- [ ] **Testnet mode** (switch between mainnet/testnet)
- [ ] **Advanced settings** (custom RPC endpoints, gas settings)

#### 3.3 User Experience
- [ ] **Mobile responsive** (works on all devices)
- [ ] **PWA support** (install as app)
- [ ] **Dark/light mode** (theme switching)
- [ ] **Multi-language** (i18n support)
- [ ] **Accessibility** (WCAG compliance)

## Technical Architecture

### Frontend Stack (Recommended)
```
Framework: React 18 with TypeScript
Styling: TailwindCSS + shadcn/ui components
State: Zustand or Jotai (lightweight state management)
Crypto: @noble/secp256k1, @scure/bip39, @scure/bip32
Build: Vite (fast builds, HMR)
Icons: Lucide React
Charts: Recharts (for DAG visualization)
```

### Wallet Structure
```typescript
interface Wallet {
  id: string;
  name: string;
  encryptedSeed: string; // Encrypted mnemonic
  accounts: Account[];
  createdAt: number;
  lastAccessed: number;
}

interface Account {
  index: number;
  label: string;
  address: string;
  publicKey: string;
  // Private key derived on-demand from seed
}

interface Transaction {
  hash: string;
  from: string;
  to: string;
  amount: number;
  fee: number;
  nonce: number;
  timestamp: number;
  status: 'pending' | 'confirmed' | 'finalized';
  round?: number;
  note?: string;
}
```

### Security Model
```
1. User creates wallet → generates 12-word mnemonic
2. User sets password → derives encryption key (PBKDF2)
3. Mnemonic encrypted with AES-256-GCM → stored in localStorage
4. On unlock → decrypt mnemonic → derive keys in memory
5. On lock/timeout → clear all keys from memory
6. Transactions signed in-memory, never expose private keys
```

### API Integration
```typescript
class UltraDAGClient {
  async getBalance(address: string): Promise<Balance>
  async sendTransaction(tx: SignedTransaction): Promise<TxHash>
  async getTransactionHistory(address: string): Promise<Transaction[]>
  async getNetworkStatus(): Promise<NetworkStatus>
  async getRound(round: number): Promise<RoundInfo>
  // WebSocket for live updates
  subscribeToAddress(address: string, callback: (tx: Transaction) => void)
}
```

## Implementation Phases

### Phase 1A: Foundation (Week 1)
- [ ] Set up React + TypeScript + Vite project
- [ ] Install dependencies (crypto libs, UI components)
- [ ] Create basic layout (header, sidebar, main content)
- [ ] Implement wallet creation (mnemonic generation)
- [ ] Implement wallet encryption/decryption
- [ ] Create unlock screen

### Phase 1B: Core Features (Week 2)
- [ ] Dashboard with balance display
- [ ] Send transaction form
- [ ] Receive screen with QR code
- [ ] Transaction signing and submission
- [ ] Basic transaction history
- [ ] Address book

### Phase 1C: Security & Polish (Week 3)
- [ ] Auto-lock functionality
- [ ] Transaction confirmation dialogs
- [ ] Error handling and validation
- [ ] Loading states and feedback
- [ ] Backup/recovery flow
- [ ] Testing and bug fixes

### Phase 2: Enhanced Features (Week 4-5)
- [ ] Multi-account support
- [ ] Advanced transaction management
- [ ] Network visualization
- [ ] WebSocket integration
- [ ] Export functionality

### Phase 3: Advanced Features (Week 6+)
- [ ] Hardware wallet support
- [ ] Mobile optimization
- [ ] PWA setup
- [ ] Multi-language support
- [ ] Advanced settings

## Success Metrics

### User Experience
- [ ] New user can create wallet in < 2 minutes
- [ ] Sending transaction takes < 30 seconds
- [ ] Zero exposed private keys in UI
- [ ] Works on mobile and desktop
- [ ] Accessible to non-technical users

### Security
- [ ] All private keys encrypted at rest
- [ ] Auto-lock after 5 minutes inactivity
- [ ] Mnemonic backup required before use
- [ ] Transaction confirmation required
- [ ] No private key exposure in browser console/network

### Performance
- [ ] Page load < 2 seconds
- [ ] Transaction submission < 1 second
- [ ] Balance updates in real-time
- [ ] Smooth animations (60fps)

## File Structure
```
site/
├── src/
│   ├── components/
│   │   ├── wallet/
│   │   │   ├── CreateWallet.tsx
│   │   │   ├── UnlockWallet.tsx
│   │   │   ├── Dashboard.tsx
│   │   │   ├── SendTransaction.tsx
│   │   │   ├── ReceiveAddress.tsx
│   │   │   └── TransactionHistory.tsx
│   │   ├── ui/ (shadcn components)
│   │   └── layout/
│   ├── lib/
│   │   ├── crypto.ts (wallet crypto functions)
│   │   ├── storage.ts (encrypted localStorage)
│   │   ├── api.ts (UltraDAG API client)
│   │   └── utils.ts
│   ├── hooks/
│   │   ├── useWallet.ts
│   │   ├── useBalance.ts
│   │   └── useTransactions.ts
│   ├── store/
│   │   └── walletStore.ts
│   ├── App.tsx
│   └── main.tsx
├── public/
├── index.html
├── package.json
├── vite.config.ts
└── tailwind.config.js
```

## Next Steps

1. **Review this plan** - Confirm approach and priorities
2. **Choose tech stack** - Finalize framework and libraries
3. **Create mockups** - Design the UI/UX flows
4. **Start Phase 1A** - Set up project foundation
5. **Iterate rapidly** - Build, test, improve

## Questions to Answer

1. **Target users**: Technical crypto users or mainstream users?
2. **Mobile priority**: Mobile-first or desktop-first?
3. **Feature priority**: Security vs. features vs. UX?
4. **Timeline**: How fast do we need this?
5. **Branding**: Colors, logo, style guide for UltraDAG?
