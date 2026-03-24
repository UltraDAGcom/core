# Playwright End-to-End Testing Guide

## Overview

This directory contains comprehensive E2E tests for the UltraDAG Dashboard using Playwright. The test suite follows the Page Object Model (POM) pattern for maintainability and is organized by feature areas.

## Quick Start

### Installation

```bash
# Install dependencies
npm install

# Install Playwright browsers
npx playwright install
```

### Running Tests

```bash
# Run all tests
npm run test:e2e

# Run with UI
npm run test:e2e:ui

# Run in headed mode (visible browser)
npm run test:e2e:headed

# Run in debug mode
npm run test:e2e:debug

# Show test report
npm run test:e2e:report
```

## Project Structure

```
tests/e2e/
├── fixtures/              # Test fixtures
│   ├── base.fixture.ts    # Base test with common fixtures
│   └── wallet.fixture.ts  # Wallet-specific fixtures
├── page-objects/          # Page Object Models
│   ├── pages/             # Page-level POMs
│   ├── components/        # Reusable component POMs
│   └── modals/            # Modal/dialog POMs
├── specs/                 # Test specifications
│   ├── dashboard/         # Dashboard tests
│   ├── wallet/            # Wallet tests
│   ├── portfolio/         # Portfolio tests
│   ├── transactions/      # Transaction tests
│   ├── bridge/            # Bridge tests
│   ├── staking/           # Staking tests
│   ├── governance/        # Governance tests
│   ├── council/           # Council tests
│   ├── explorer/          # Explorer tests
│   ├── network/           # Network tests
│   └── shared/            # Shared component tests
├── utils/                 # Test utilities
│   ├── test-data.ts       # Test data generators
│   ├── mocks.ts           # API mocking utilities
│   └── assertions.ts      # Custom assertions
└── setup/                 # Test setup/teardown
    ├── global-setup.ts
    └── global-teardown.ts
```

## Writing Tests

### Basic Test Structure

```typescript
import { test, expect } from '../../fixtures/base.fixture';
import { MyPagePO } from '../../page-objects/pages/MyPagePO';

test.describe('My Feature', () => {
  let myPage: MyPagePO;

  test.beforeEach(async ({ page }) => {
    myPage = new MyPagePO(page);
    await page.goto('/my-page');
    await myPage.waitForLoaded();
  });

  test('should do something', async () => {
    await expect(myPage.element).toBeVisible();
  });
});
```

### Using Fixtures

```typescript
import { test, expect } from '../../fixtures/base.fixture';

test.describe('My Test', () => {
  test('should use sidebar', async ({ sidebar }) => {
    await sidebar.goToWallet();
    // ...
  });

  test('should use wallet fixture', async ({ wallet }) => {
    await wallet.createWallet({ name: 'Test', password: 'SecurePass123!' });
    // ...
  });
});
```

### Using Test Data Generators

```typescript
import { generateWalletData, generateAddress } from '../../utils/test-data';

test('should create wallet', async () => {
  const walletData = generateWalletData();
  // walletData = { name: 'TestWallet123', password: 'SecurePass456!' }
  
  const address = generateAddress();
  // address = 'dag1...'
});
```

## Test Tags

Use tags to categorize tests:

```typescript
test('should login', { tag: '@smoke' }, async () => {
  // ...
});

test.describe('Critical Flows', { tag: '@regression' }, () => {
  // ...
});
```

Available tags:
- `@smoke` - Critical path tests
- `@regression` - Full regression suite
- `@wallet` - Wallet-related tests
- `@transaction` - Transaction tests
- `@governance` - Governance tests
- `@explorer` - Explorer tests

## Running Specific Tests

```bash
# By tag
npx playwright test --grep @smoke

# By file
npx playwright test tests/e2e/specs/wallet/wallet-creation.spec.ts

# By folder
npx playwright test tests/e2e/specs/wallet

# By test name
npx playwright test -g "should create wallet"

# By browser
npx playwright test --project=chromium
npx playwright test --project=firefox
```

## Page Object Model

### Creating a Page Object

```typescript
// page-objects/pages/MyPagePO.ts
import { Page, Locator } from '@playwright/test';

export class MyPagePO {
  readonly page: Page;
  readonly myElement: Locator;

  constructor(page: Page) {
    this.page = page;
    this.myElement = page.locator('[data-testid="my-element"]');
  }

  async clickMyElement(): Promise<void> {
    await this.myElement.click();
  }

  async isVisible(): Promise<boolean> {
    return this.myElement.isVisible();
  }
}
```

### Best Practices

1. **Use data-testid attributes** for stable selectors
2. **Encapsulate interactions** in methods
3. **Return promises** from async methods
4. **Use TypeScript** for type safety
5. **Keep POMs focused** on a single page/component

## API Mocking

```typescript
import { mockNodeStatus } from '../../utils/mocks';

test('should display status', async ({ page }) => {
  await mockNodeStatus(page, { connected: true, height: 12345 });
  await page.goto('/');
  // ...
});
```

## Custom Assertions

```typescript
import { assertVisible, assertText } from '../../utils/assertions';

test('should display correctly', async ({ page }) => {
  const element = page.locator('[data-testid="my-element"]');
  await assertVisible(element);
  await assertText(element, 'Expected Text');
});
```

## CI/CD

Tests run automatically on:
- Push to `main` or `develop`
- Pull requests to `main` or `develop`

### GitHub Actions

The workflow shards tests across 4 runners for parallel execution. Reports are merged and uploaded as artifacts.

## Debugging

### Debug Mode

```bash
npm run test:e2e:debug
```

### Trace Viewer

```bash
# After a failed test
npx playwright show-trace tests/e2e/test-results/trace.zip
```

### Screenshots and Videos

- Screenshots are taken on failure
- Videos are recorded on failure
- Find them in `tests/e2e/test-results/`

## Best Practices

1. **Test isolation**: Each test should be independent
2. **Deterministic tests**: Use mocks for external dependencies
3. **Meaningful assertions**: Test behavior, not implementation
4. **Descriptive names**: Test names should describe expected behavior
5. **Cleanup**: Use `test.afterEach` for cleanup
6. **Retry flaky tests**: Use `test.describe.configure({ retries: 2 })`

## Common Issues

### Element Not Found

- Check if the page has loaded
- Use `waitForLoaded()` on page objects
- Check for dynamic content with `waitForSelector`

### Test Flakiness

- Add explicit waits
- Use mocks for API calls
- Avoid timing-dependent assertions

### Slow Tests

- Run tests in parallel
- Use sharding in CI
- Mock expensive API calls

## Coverage Goals

- **Dashboard**: 100% coverage
- **Wallet**: 100% coverage
- **Transactions**: 100% coverage
- **Staking**: 100% coverage
- **Governance**: 100% coverage
- **Explorer**: 100% coverage
- **Network**: 100% coverage
- **Bridge**: 100% coverage
- **Shared Components**: 100% coverage

## Contributing

1. Create page objects for new pages
2. Write tests in the appropriate `specs/` folder
3. Use test data generators for random data
4. Add mocks for API calls
5. Run tests locally before committing
6. Update this guide for new patterns

## Resources

- [Playwright Documentation](https://playwright.dev)
- [Playwright Best Practices](https://playwright.dev/docs/best-practices)
- [Page Object Model Pattern](https://playwright.dev/docs/pom)
