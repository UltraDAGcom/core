# UltraDAG Dashboard E2E Tests

Comprehensive Playwright test suite for the UltraDAG Dashboard.

## Folder Structure

```
tests/e2e/
├── fixtures/           # Test fixtures and base test setup
│   ├── base.fixture.ts
│   └── wallet.fixture.ts
├── page-objects/       # Page Object Models (POMs)
│   ├── pages/          # Page-level POMs
│   ├── components/     # Reusable component POMs
│   └── modals/         # Modal/dialog POMs
├── specs/              # Test specifications organized by feature
│   ├── dashboard/      # Dashboard/Home page tests
│   ├── wallet/         # Wallet management tests
│   ├── portfolio/      # Portfolio page tests
│   ├── transactions/   # Send/transaction tests
│   ├── bridge/         # Bridge functionality tests
│   ├── staking/        # Staking page tests
│   ├── governance/     # Governance and voting tests
│   ├── council/        # Council page tests
│   ├── explorer/       # Block explorer tests
│   ├── network/        # Network status tests
│   └── shared/         # Shared component tests
├── utils/              # Test utilities and helpers
│   ├── test-data.ts    # Test data generators
│   ├── mocks.ts        # API mocks
│   └── assertions.ts   # Custom assertions
└── setup/              # Test setup and teardown
    ├── global-setup.ts
    └── global-teardown.ts
```

## Running Tests

```bash
# Install dependencies
npm install

# Install Playwright browsers
npx playwright install

# Run all tests
npx playwright test

# Run tests in specific folder
npx playwright test tests/e2e/specs/wallet

# Run tests with UI
npx playwright test --ui

# Run tests in headed mode (visible browser)
npx playwright test --headed

# Run specific test file
npx playwright test tests/e2e/specs/wallet/wallet-creation.spec.ts

# Run tests with specific tag
npx playwright test --grep @smoke
npx playwright test --grep @regression
```

## Test Tags

- `@smoke` - Critical path tests
- `@regression` - Full regression suite
- `@wallet` - Wallet-related tests
- `@transaction` - Transaction tests
- `@governance` - Governance tests
- `@explorer` - Explorer tests
