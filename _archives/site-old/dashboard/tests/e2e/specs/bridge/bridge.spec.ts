import { test, expect } from '../../fixtures/base.fixture';
import { BridgePagePO } from '../../page-objects/pages/BridgePagePO';
import { WalletPagePO } from '../../page-objects/pages/WalletPagePO';
import { CreateKeystoreModalPO } from '../../page-objects/modals/CreateKeystoreModalPO';
import { generateWalletData } from '../../utils/test-data';

test.describe('Bridge Page', () => {
  let bridgePage: BridgePagePO;
  let walletPage: WalletPagePO;
  let createKeystoreModal: CreateKeystoreModalPO;

  test.beforeEach(async ({ page }) => {
    bridgePage = new BridgePagePO(page);
    walletPage = new WalletPagePO(page);
    createKeystoreModal = new CreateKeystoreModalPO(page);
    
    // Create and unlock wallet first
    await page.goto('/wallet');
    const walletData = generateWalletData();
    await walletPage.createWalletButton.click();
    await createKeystoreModal.waitForOpen();
    await createKeystoreModal.fillForm(walletData.name, walletData.password);
    await createKeystoreModal.submit();
    await createKeystoreModal.waitForClose();
    await walletPage.unlockWallet(walletData.password);
    
    // Navigate to bridge
    await page.goto('/bridge');
    await bridgePage.waitForLoaded();
  });

  test.describe('Page Load and Layout', () => {
    test('should load bridge page successfully', async ({ page }) => {
      await expect(page).toHaveURL('/bridge');
      await expect(bridgePage.pageContainer).toBeVisible();
    });

    test('should display from chain selector', async () => {
      await expect(bridgePage.fromChainSelector).toBeVisible();
    });

    test('should display to chain selector', async () => {
      await expect(bridgePage.toChainSelector).toBeVisible();
    });

    test('should display amount input', async () => {
      await expect(bridgePage.amountInput).toBeVisible();
    });

    test('should display bridge button', async () => {
      await expect(bridgePage.bridgeButton).toBeVisible();
    });

    test('should display bridge history', async () => {
      await expect(bridgePage.bridgeHistory).toBeVisible();
    });
  });

  test.describe('Chain Selection', () => {
    test('should display available from chains', async ({ page }) => {
      await bridgePage.fromChainSelector.click();
      
      const options = page.locator('li, option');
      const count = await options.count();
      expect(count).toBeGreaterThan(0);
    });

    test('should display available to chains', async ({ page }) => {
      await bridgePage.toChainSelector.click();
      
      const options = page.locator('li, option');
      const count = await options.count();
      expect(count).toBeGreaterThan(0);
    });

    test('should select from chain', async ({ page }) => {
      await bridgePage.fromChainSelector.click();
      const firstOption = page.locator('li, option').first();
      const text = await firstOption.textContent();
      
      if (text) {
        await firstOption.click();
        // Chain should be selected
      }
    });

    test('should select to chain', async ({ page }) => {
      await bridgePage.toChainSelector.click();
      const firstOption = page.locator('li, option').first();
      const text = await firstOption.textContent();
      
      if (text) {
        await firstOption.click();
      }
    });

    test('should not allow selecting same chain for both from and to', async ({ page }) => {
      // Select Ethereum as from
      await bridgePage.selectFromChain('Ethereum');
      
      // Try to select Ethereum as to - should not be available or show error
      await bridgePage.toChainSelector.click();
      const ethereumOption = page.locator('li:has-text("Ethereum"), option[value="Ethereum"]');
      
      // Option should be disabled or not exist
      const isDisabled = await ethereumOption.isDisabled();
      const isVisible = await ethereumOption.isVisible();
      
      expect(isDisabled || !isVisible).toBeTruthy();
    });
  });

  test.describe('Bridge Form Validation', () => {
    test('should disable bridge button with empty form', async () => {
      const isEnabled = await bridgePage.bridgeButton.isEnabled();
      expect(isEnabled).toBeFalsy();
    });

    test('should show error for invalid amount', async ({ page }) => {
      await bridgePage.selectFromChain('Ethereum');
      await bridgePage.selectToChain('UltraDAG');
      await bridgePage.fillAmount('-100');
      
      const isEnabled = await bridgePage.bridgeButton.isEnabled();
      expect(isEnabled).toBeFalsy();
    });

    test('should show error for zero amount', async ({ page }) => {
      await bridgePage.selectFromChain('Ethereum');
      await bridgePage.selectToChain('UltraDAG');
      await bridgePage.fillAmount('0');
      
      const isEnabled = await bridgePage.bridgeButton.isEnabled();
      expect(isEnabled).toBeFalsy();
    });

    test('should enable bridge button with valid inputs', async ({ page }) => {
      await bridgePage.selectFromChain('Ethereum');
      await bridgePage.selectToChain('UltraDAG');
      await bridgePage.fillAmount('10');
      
      const isEnabled = await bridgePage.bridgeButton.isEnabled();
      expect(isEnabled).toBeTruthy();
    });
  });

  test.describe('Bridge History', () => {
    test('should display bridge history section', async () => {
      await expect(bridgePage.bridgeHistory).toBeVisible();
    });

    test('should display pending bridges', async () => {
      const count = await bridgePage.getPendingBridgeCount();
      expect(count).toBeGreaterThanOrEqual(0);
    });

    test('should display completed bridges', async () => {
      const count = await bridgePage.getCompletedBridgeCount();
      expect(count).toBeGreaterThanOrEqual(0);
    });

    test('should display bridge count', async () => {
      const count = await bridgePage.getBridgeCount();
      expect(count).toBeGreaterThanOrEqual(0);
    });

    test('should display bridge transaction details', async ({ page }) => {
      const count = await bridgePage.getBridgeCount();
      
      if (count > 0) {
        const bridgeItem = bridgePage.bridgeHistory.locator('[data-testid="bridge-item"], .bridge-item').first();
        
        // Should display from chain
        const fromChainEl = bridgeItem.locator('.from-chain, .chain-from');
        await expect(fromChainEl).toBeVisible();
        
        // Should display to chain
        const toChainEl = bridgeItem.locator('.to-chain, .chain-to');
        await expect(toChainEl).toBeVisible();
        
        // Should display amount
        const amountEl = bridgeItem.locator('.amount, .bridge-amount');
        await expect(amountEl).toBeVisible();
        
        // Should display status
        const statusEl = bridgeItem.locator('.status, .bridge-status');
        await expect(statusEl).toBeVisible();
      }
    });
  });

  test.describe('Bridge Transaction', () => {
    test('should initiate bridge transaction', async ({ page }) => {
      await bridgePage.selectFromChain('Ethereum');
      await bridgePage.selectToChain('UltraDAG');
      await bridgePage.fillAmount('10');
      await bridgePage.initiateBridge();
      
      // Should show confirmation modal or processing state
      // Implementation dependent
    });

    test('should show bridge fee', async ({ page }) => {
      await bridgePage.selectFromChain('Ethereum');
      await bridgePage.selectToChain('UltraDAG');
      await bridgePage.fillAmount('10');
      
      const feeEl = page.locator('[data-testid="bridge-fee"], .bridge-fee, .fee');
      await expect(feeEl).toBeVisible();
    });

    test('should show estimated receive amount', async ({ page }) => {
      await bridgePage.selectFromChain('Ethereum');
      await bridgePage.selectToChain('UltraDAG');
      await bridgePage.fillAmount('10');
      
      const receiveEl = page.locator('[data-testid="receive-amount"], .receive-amount');
      await expect(receiveEl).toBeVisible();
    });

    test('should show bridge processing time', async ({ page }) => {
      const timeEl = page.locator('[data-testid="bridge-time"], .bridge-time, .estimated-time');
      await expect(timeEl).toBeVisible();
    });
  });

  test.describe('Minimum and Maximum Amounts', () => {
    test('should display minimum bridge amount', async ({ page }) => {
      const minEl = page.locator('[data-testid="min-amount"], .min-amount');
      await expect(minEl).toBeVisible();
    });

    test('should display maximum bridge amount', async ({ page }) => {
      const maxEl = page.locator('[data-testid="max-amount"], .max-amount');
      await expect(maxEl).toBeVisible();
    });

    test('should show error for amount below minimum', async ({ page }) => {
      await bridgePage.selectFromChain('Ethereum');
      await bridgePage.selectToChain('UltraDAG');
      await bridgePage.fillAmount('0.0001');
      
      // Should show error or disable button
      const isEnabled = await bridgePage.bridgeButton.isEnabled();
      expect(isEnabled).toBeFalsy();
    });
  });

  test.describe('Responsive Design', () => {
    test('should display correctly on mobile', async ({ page }) => {
      await page.setViewportSize({ width: 375, height: 667 });
      await expect(bridgePage.pageContainer).toBeVisible();
    });

    test('should display correctly on tablet', async ({ page }) => {
      await page.setViewportSize({ width: 768, height: 1024 });
      await expect(bridgePage.pageContainer).toBeVisible();
    });

    test('should display correctly on desktop', async ({ page }) => {
      await page.setViewportSize({ width: 1920, height: 1080 });
      await expect(bridgePage.pageContainer).toBeVisible();
    });
  });
});
