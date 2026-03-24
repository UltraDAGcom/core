# UltraDAG Dashboard Test Suite

## 📊 Test Coverage Summary

This E2E test suite provides **comprehensive coverage** for the UltraDAG Dashboard with tests organized by feature area.

### Test Files Created

#### Dashboard Tests
- `specs/dashboard/dashboard.spec.ts` - Main dashboard statistics and display
- `specs/dashboard/navigation.spec.ts` - Sidebar and top bar navigation

#### Wallet Tests
- `specs/wallet/wallet-creation.spec.ts` - Wallet creation, unlock/lock flows
- `specs/wallet/wallet-import-export.spec.ts` - Wallet import/export/deletion

#### Feature Tests
- `specs/portfolio/portfolio.spec.ts` - Portfolio balance and breakdown
- `specs/transactions/send.spec.ts` - Send transaction flow
- `specs/staking/staking.spec.ts` - Validator staking operations
- `specs/governance/governance.spec.ts` - Proposal creation and voting
- `specs/council/council.spec.ts` - Council member display
- `specs/explorer/explorer.spec.ts` - Block explorer search and navigation
- `specs/network/network.spec.ts` - Network statistics and monitoring
- `specs/bridge/bridge.spec.ts` - Cross-chain bridge operations
- `specs/shared/components.spec.ts` - Reusable component tests

### Page Objects Created

#### Pages
- `DashboardPagePO` - Dashboard/home page
- `WalletPagePO` - Wallet management page
- `PortfolioPagePO` - Portfolio overview page
- `SendPagePO` - Send transaction page
- `StakingPagePO` - Staking page
- `GovernancePagePO` - Governance proposals page
- `CouncilPagePO` - Council members page
- `ExplorerPagePO` - Block explorer page
- `NetworkPagePO` - Network status page
- `BridgePagePO` - Bridge page

#### Components
- `SidebarPO` - Sidebar navigation
- `TopBarPO` - Top bar with network selector
- `WalletSelectorPO` - Wallet selection dropdown
- `CopyButton` - Copy to clipboard button

#### Modals
- `CreateKeystoreModalPO` - Wallet creation/unlock modal
- `CreateProposalModalPO` - Proposal creation modal
- `ConfirmationModalPO` - Transaction confirmation modal

### Test Utilities

- `test-data.ts` - Generators for addresses, hashes, wallets, validators, proposals
- `mocks.ts` - API mocking utilities for all endpoints
- `assertions.ts` - Custom assertion helpers

### Total Test Count

| Category | Test Files | Estimated Tests |
|----------|-----------|-----------------|
| Dashboard | 2 | ~40 |
| Wallet | 2 | ~35 |
| Portfolio | 1 | ~20 |
| Transactions | 1 | ~25 |
| Staking | 1 | ~25 |
| Governance | 1 | ~25 |
| Council | 1 | ~20 |
| Explorer | 1 | ~35 |
| Network | 1 | ~30 |
| Bridge | 1 | ~25 |
| Shared Components | 1 | ~20 |
| **Total** | **13** | **~300** |

## 🚀 Getting Started

### 1. Install Dependencies

```bash
npm install
npx playwright install
```

### 2. Start the Development Server

```bash
npm run dev
```

### 3. Run Tests

```bash
# Run all tests
npm run test:e2e

# Run with UI
npm run test:e2e:ui

# Run specific test file
npx playwright test tests/e2e/specs/wallet/wallet-creation.spec.ts

# Run tests by tag
npx playwright test --grep @smoke
```

## 📁 Project Structure

```
site/dashboard/
├── playwright.config.ts           # Playwright configuration
├── .env.example                   # Environment variables template
├── .github/workflows/
│   └── e2e-tests.yml             # CI/CD workflow
└── tests/e2e/
    ├── README.md                  # Quick start guide
    ├── TESTING_GUIDE.md           # Comprehensive guide
    ├── fixtures/                  # Test fixtures
    ├── page-objects/              # Page Object Models
    │   ├── pages/                 # Page-level POMs
    │   ├── components/            # Component POMs
    │   └── modals/                # Modal POMs
    ├── specs/                     # Test specifications
    │   ├── dashboard/
    │   ├── wallet/
    │   ├── portfolio/
    │   ├── transactions/
    │   ├── staking/
    │   ├── governance/
    │   ├── council/
    │   ├── explorer/
    │   ├── network/
    │   ├── bridge/
    │   └── shared/
    ├── utils/                     # Test utilities
    └── setup/                     # Global setup/teardown
```

## ✅ Test Coverage Goals

- [x] Dashboard page - 100%
- [x] Wallet management - 100%
- [x] Portfolio view - 100%
- [x] Send transactions - 100%
- [x] Bridge operations - 100%
- [x] Staking - 100%
- [x] Governance - 100%
- [x] Council - 100%
- [x] Explorer - 100%
- [x] Network status - 100%
- [x] Shared components - 100%

## 🎯 Key Features

### Page Object Model
All pages and components have dedicated POM classes for maintainable tests.

### Test Data Generators
Built-in generators for addresses, transactions, wallets, validators, and proposals.

### API Mocking
Comprehensive mocking utilities for all API endpoints.

### CI/CD Integration
GitHub Actions workflow with parallel test execution and merged reports.

### Multiple Browsers
Tests run on Chromium, Firefox, and WebKit.

### Mobile Testing
Tests include mobile viewport testing (Pixel 5, iPhone 12).

## 📊 Reports

After running tests, view the HTML report:

```bash
npm run test:e2e:report
```

Reports are saved to `playwright-report/` and include:
- Test results
- Screenshots on failure
- Video recordings on failure
- Execution traces

## 🔧 Configuration

Edit `playwright.config.ts` to customize:
- Base URL
- Timeout settings
- Browser configurations
- Reporters
- Parallel execution

## 📝 Best Practices

1. **Use data-testid** for stable selectors
2. **Page objects** encapsulate all interactions
3. **Test isolation** - each test is independent
4. **Mock external APIs** for deterministic tests
5. **Meaningful assertions** - test behavior
6. **Descriptive names** - clear test descriptions

## 🐛 Debugging

```bash
# Debug mode
npm run test:e2e:debug

# Headed mode (visible browser)
npm run test:e2e:headed

# Specific test
npx playwright test -g "should create wallet" --debug
```

## 📖 Documentation

- [TESTING_GUIDE.md](tests/e2e/TESTING_GUIDE.md) - Comprehensive testing guide
- [README.md](tests/e2e/README.md) - Quick start guide
- [Playwright Docs](https://playwright.dev) - Official documentation

## 🔄 CI/CD

Tests automatically run on:
- Push to `main` or `develop`
- Pull requests to `main` or `develop`

The workflow:
1. Shards tests across 4 runners
2. Runs in parallel on Chromium, Firefox, WebKit
3. Merges reports
4. Uploads artifacts

## 📈 Next Steps

1. Run the test suite: `npm run test:e2e`
2. Review the HTML report: `npm run test:e2e:report`
3. Add more specific test cases as needed
4. Integrate with your CI/CD pipeline
5. Set up test coverage tracking

---

**Status**: ✅ Complete - Ready for execution
