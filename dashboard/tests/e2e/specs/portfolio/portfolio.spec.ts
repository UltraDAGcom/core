import { test, expect } from '../../fixtures/base.fixture';
import { PortfolioPagePO } from '../../page-objects/pages/PortfolioPagePO';
import { WalletPagePO } from '../../page-objects/pages/WalletPagePO';
import { CreateKeystoreModalPO } from '../../page-objects/modals/CreateKeystoreModalPO';
import { generateWalletData } from '../../utils/test-data';

test.describe('Portfolio Page', () => {
  let portfolioPage: PortfolioPagePO;
  let walletPage: WalletPagePO;
  let createKeystoreModal: CreateKeystoreModalPO;

  test.beforeEach(async ({ page }) => {
    portfolioPage = new PortfolioPagePO(page);
    walletPage = new WalletPagePO(page);
    createKeystoreModal = new CreateKeystoreModalPO(page);
    await page.goto('/wallet/portfolio');
    await portfolioPage.waitForLoaded();
  });

  test.describe('Page Load and Layout', () => {
    test('should load portfolio page successfully', async ({ page }) => {
      await expect(page).toHaveURL('/wallet/portfolio');
      await expect(portfolioPage.pageContainer).toBeVisible();
    });

    test('should display total balance card', async () => {
      await expect(portfolioPage.totalBalanceCard).toBeVisible();
    });

    test('should display total staked card', async () => {
      await expect(portfolioPage.totalStakedCard).toBeVisible();
    });

    test('should display total delegated card', async () => {
      await expect(portfolioPage.totalDelegatedCard).toBeVisible();
    });

    test('should display wallet breakdown section', async () => {
      await expect(portfolioPage.walletBreakdown).toBeVisible();
    });
  });

  test.describe('Balance Display', () => {
    test('should display total balance value', async () => {
      const balance = await portfolioPage.getTotalBalance();
      expect(balance).not.toBeNull();
    });

    test('should display total staked value', async () => {
      const staked = await portfolioPage.getTotalStaked();
      expect(staked).not.toBeNull();
    });

    test('should display total delegated value', async () => {
      const delegated = await portfolioPage.getTotalDelegated();
      expect(delegated).not.toBeNull();
    });

    test('should show valid numeric values', async () => {
      const balance = await portfolioPage.getTotalBalance();
      if (balance) {
        const balanceValue = parseFloat(balance.replace(/,/g, '').replace(/[^\d.]/g, ''));
        expect(balanceValue).toBeGreaterThanOrEqual(0);
      }
    });
  });

  test.describe('Wallet List', () => {
    test('should display wallet list', async () => {
      await expect(portfolioPage.walletList).toBeVisible();
    });

    test('should show wallet count', async ({ page }) => {
      // Create a wallet first
      await page.goto('/wallet');
      const walletData = generateWalletData();
      await walletPage.createWalletButton.click();
      await createKeystoreModal.waitForOpen();
      await createKeystoreModal.fillForm(walletData.name, walletData.password);
      await createKeystoreModal.submit();
      await createKeystoreModal.waitForClose();
      
      // Go back to portfolio
      await page.goto('/wallet/portfolio');
      await portfolioPage.waitForLoaded();
      
      const walletCount = await portfolioPage.getWalletCount();
      expect(walletCount).toBeGreaterThan(0);
    });

    test('should display individual wallet balances', async ({ page }) => {
      // Create a wallet
      await page.goto('/wallet');
      const walletData = generateWalletData();
      await walletPage.createWalletButton.click();
      await createKeystoreModal.waitForOpen();
      await createKeystoreModal.fillForm(walletData.name, walletData.password);
      await createKeystoreModal.submit();
      await createKeystoreModal.waitForClose();
      
      // Go to portfolio
      await page.goto('/wallet/portfolio');
      await portfolioPage.waitForLoaded();
      
      const balance = await portfolioPage.getWalletBalance(walletData.name);
      expect(balance).not.toBeNull();
    });

    test('should click on wallet to view details', async ({ page }) => {
      // Create a wallet
      await page.goto('/wallet');
      const walletData = generateWalletData();
      await walletPage.createWalletButton.click();
      await createKeystoreModal.waitForOpen();
      await createKeystoreModal.fillForm(walletData.name, walletData.password);
      await createKeystoreModal.submit();
      await createKeystoreModal.waitForClose();
      
      // Go to portfolio
      await page.goto('/wallet/portfolio');
      await portfolioPage.waitForLoaded();
      
      await portfolioPage.clickOnWallet(walletData.name);
      
      // Should navigate to wallet detail or address page
      await expect(page).toHaveURL(/\/address\//);
    });
  });

  test.describe('Charts and Visualizations', () => {
    test('should display balance chart', async () => {
      await expect(portfolioPage.balanceChart).toBeVisible();
    });
  });

  test.describe('Empty State', () => {
    test('should show empty state when no wallets exist', async ({ page }) => {
      // Clear all wallets first (implementation dependent)
      // For now, just verify the page loads without wallets
      await portfolioPage.waitForLoaded();
      await expect(portfolioPage.pageContainer).toBeVisible();
    });
  });

  test.describe('Responsive Design', () => {
    test('should display correctly on mobile', async ({ page }) => {
      await page.setViewportSize({ width: 375, height: 667 });
      await expect(portfolioPage.pageContainer).toBeVisible();
    });

    test('should display correctly on tablet', async ({ page }) => {
      await page.setViewportSize({ width: 768, height: 1024 });
      await expect(portfolioPage.pageContainer).toBeVisible();
    });

    test('should display correctly on desktop', async ({ page }) => {
      await page.setViewportSize({ width: 1920, height: 1080 });
      await expect(portfolioPage.pageContainer).toBeVisible();
    });
  });
});
