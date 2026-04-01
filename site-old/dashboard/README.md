# UltraDAG Dashboard v2

A modern, secure React dashboard for managing UltraDAG wallets, staking, governance, and network exploration.

![Dashboard Preview](./preview.png)

## Features

- 🔐 **Client-Side Keystore** - Keys never leave your browser
- 🌐 **Network Switching** - Toggle between mainnet and testnet
- 💰 **Wallet Management** - Create, import, export multiple wallets
- 📊 **Portfolio Tracking** - View balance, staked, and delegated amounts
- 🗳️ **Governance** - Create proposals, vote, track council
- 🔍 **Block Explorer** - Search transactions, vertices, rounds, addresses
- 📡 **Network Status** - Real-time node health and metrics

## Quick Start

### Development

```bash
cd site/dashboard-v2

# Install dependencies
npm install

# Start development server
npm run dev

# Open http://localhost:5173
```

### Production Build

```bash
# Build for production
npm run build

# Preview production build
npm run preview

# Deploy dist/ to your web server
```

## Deployment

### Netlify

1. Connect repository to Netlify
2. Set build command: `npm run build`
3. Set publish directory: `dist`
4. Add redirect rule in `public/_redirects`:
   ```
   /* /index.html 200
   ```

### Docker

```dockerfile
FROM node:20-alpine AS builder
WORKDIR /app
COPY package*.json ./
RUN npm ci
COPY . .
RUN npm run build

FROM nginx:alpine
COPY --from=builder /app/dist /usr/share/nginx/html
COPY nginx.conf /etc/nginx/conf.d/default.conf
EXPOSE 80
```

### Manual

```bash
# Build
npm run build

# Copy to web server
scp -r dist/* user@server:/var/www/ultradag.com/dashboard/
```

## Configuration

### Environment Variables

Create `.env` file:

```env
# Node URLs (comma-separated for failover)
VITE_TESTNET_NODES=https://ultradag-node-1.fly.dev,https://ultradag-node-2.fly.dev
VITE_MAINNET_NODES=https://ultradag-mainnet-1.fly.dev,https://ultradag-mainnet-2.fly.dev

# Default network (mainnet|testnet)
VITE_DEFAULT_NETWORK=testnet

# Feature flags
VITE_ENABLE_FAUCET=true
VITE_ENABLE_BRIDGE=false
```

### Network Configuration

Edit `src/lib/api.ts` to customize node lists:

```typescript
const TESTNET_NODES = [
  'https://ultradag-node-1.fly.dev',
  'https://ultradag-node-2.fly.dev',
  // Add more nodes...
];

const MAINNET_NODES = [
  'https://ultradag-mainnet-1.fly.dev',
  'https://ultradag-mainnet-2.fly.dev',
  // Add more nodes...
];
```

## Security

### Keystore Encryption

- Password: AES-256-GCM encryption
- Keys: Never transmitted over network
- Storage: Encrypted JSON in localStorage

### Best Practices

1. **Use strong passwords** (12+ characters)
2. **Backup keystore** (Export → Save securely)
3. **Never share password** (UltraDAG team will never ask)
4. **Use hardware wallet** for large amounts (coming soon)

## Architecture

```
src/
├── components/       # React components
│   ├── layout/      # Layout, TopBar, Sidebar
│   ├── wallet/      # Wallet cards, modals
│   ├── explorer/    # Search, results
│   └── governance/  # Proposals, voting
├── hooks/           # Custom React hooks
│   ├── useKeystore  # Keystore management
│   ├── useNode      # Node connection
│   └── useToast     # Notifications
├── lib/             # Utilities
│   ├── api.ts       # RPC client
│   ├── keygen.ts    # Key generation
│   └── contracts.ts # Contract ABIs
└── pages/           # Route pages
    ├── DashboardPage
    ├── WalletPage
    ├── PortfolioPage
    ├── SendPage
    ├── StakingPage
    ├── GovernancePage
    └── ExplorerPage
```

## API Integration

### RPC Endpoints

```typescript
import { getBalance, postTx, getProposals } from './lib/api';

// Get balance
const balance = await getBalance('udag1...');

// Submit transaction
const result = await postTx({
  Transfer: {
    from: '...',
    to: '...',
    amount: 100000000,
    fee: 10000,
    nonce: 0,
    pub_key: '...',
    signature: '...'
  }
});

// Get proposals
const proposals = await getProposals();
```

### Error Handling

```typescript
try {
  await postTx(tx);
} catch (error) {
  if (error.message.includes('InsufficientBalance')) {
    // Handle insufficient funds
  } else if (error.message.includes('InvalidNonce')) {
    // Handle nonce error
  } else {
    // Generic error
  }
}
```

## Testing

```bash
# Run tests
npm test

# Run with coverage
npm run test:coverage

# Lint code
npm run lint

# Type check
npm run type-check
```

## Contributing

1. Fork repository
2. Create feature branch (`git checkout -b feature/amazing-feature`)
3. Commit changes (`git commit -m 'Add amazing feature'`)
4. Push to branch (`git push origin feature/amazing-feature`)
5. Open Pull Request

### Code Style

- ESLint + Prettier configured
- TypeScript strict mode
- Functional components with hooks
- Tailwind CSS for styling

## Troubleshooting

### Cannot connect to node

1. Check node status: `curl https://ultradag-node-1.fly.dev/health`
2. Try different node in network switcher
3. Check browser console for errors

### Keystore won't unlock

1. Verify password (case-sensitive)
2. Try importing keystore backup
3. Clear localStorage (will delete keystore)

### Transaction stuck pending

1. Check mempool: `curl https://ultradag-node-1.fly.dev/mempool`
2. Increase fee if mempool is full
3. Wait for network congestion to clear

## Roadmap

- [ ] Hardware wallet support (Ledger, Trezor)
- [ ] Multi-signature wallets
- [ ] Transaction batching
- [ ] Advanced governance analytics
- [ ] Mobile app (React Native)
- [ ] Dark/light theme toggle
- [ ] Transaction notifications
- [ ] Portfolio performance charts

## License

MIT License - see [LICENSE](./LICENSE) for details.

## Support

- **Documentation**: https://ultradag.com/docs
- **Discord**: https://discord.gg/ultradag
- **Twitter**: https://twitter.com/UltraDAGcom
- **Email**: support@ultradag.com
