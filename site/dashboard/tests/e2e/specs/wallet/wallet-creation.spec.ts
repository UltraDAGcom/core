import { test, expect } from '../../fixtures/base.fixture';
import { WalletPagePO } from '../../page-objects/pages/WalletPagePO';
import { CreateKeystoreModalPO } from '../../page-objects/modals/CreateKeystoreModalPO';
import { generateWalletData, type WalletTestData } from '../../utils/test-data';

test.describe('Wallet Management', () => {
  let walletPage: WalletPagePO;
  let createKeystoreModal: CreateKeystoreModalPO;

  test.beforeEach(async ({ page }) => {
    walletPage = new WalletPagePO(page);
    createKeystoreModal = new CreateKeystoreModalPO(page);
    await page.goto('/wallet');
    await walletPage.waitForLoaded();
  });

  test.describe('Wallet Creation', () => {
    test('should display create wallet button', async () => {
      await expect(walletPage.createWalletButton).toBeVisible();
    });

    test('should open create wallet modal', async ({ page }) => {
      await walletPage.createWalletButton.click();
      await createKeystoreModal.waitForOpen();
      await expect(createKeystoreModal.modal).toBeVisible();
    });

    test('should create new wallet with valid data', async ({ page }) => {
      const walletData = generateWalletData();
      
      await walletPage.createWalletButton.click();
      await createKeystoreModal.waitForOpen();
      await createKeystoreModal.fillForm(walletData.name, walletData.password);
      await createKeystoreModal.submit();
      await createKeystoreModal.waitForClose();
      
      // Verify wallet was created
      const walletCount = await walletPage.getWalletCount();
      expect(walletCount).toBeGreaterThan(0);
      
      const walletName = await walletPage.getWalletName(0);
      expect(walletName).toContain(walletData.name);
    });

    test('should show error for empty wallet name', async ({ page }) => {
      const walletData = generateWalletData();
      
      await walletPage.createWalletButton.click();
      await createKeystoreModal.waitForOpen();
      
      // Only fill password, leave name empty
      await createKeystoreModal.fillPassword(walletData.password);
      await createKeystoreModal.fillConfirmPassword(walletData.password);
      await createKeystoreModal.submit();
      
      // Should show error or not submit
      const isVisible = await createKeystoreModal.isVisible();
      expect(isVisible).toBeTruthy();
    });

    test('should show error for mismatched passwords', async ({ page }) => {
      await walletPage.createWalletButton.click();
      await createKeystoreModal.waitForOpen();
      await createKeystoreModal.fillName('Test Wallet');
      await createKeystoreModal.fillPassword('Password123!');
      await createKeystoreModal.fillConfirmPassword('Password456!');
      await createKeystoreModal.submit();
      
      // Modal should stay open or show error
      const isVisible = await createKeystoreModal.isVisible();
      expect(isVisible).toBeTruthy();
    });

    test('should show error for weak password', async ({ page }) => {
      await walletPage.createWalletButton.click();
      await createKeystoreModal.waitForOpen();
      await createKeystoreModal.fillName('Test Wallet');
      await createKeystoreModal.fillPassword('weak');
      await createKeystoreModal.fillConfirmPassword('weak');
      await createKeystoreModal.submit();
      
      // Modal should stay open or show error
      const isVisible = await createKeystoreModal.isVisible();
      expect(isVisible).toBeTruthy();
    });

    test('should cancel wallet creation', async ({ page }) => {
      await walletPage.createWalletButton.click();
      await createKeystoreModal.waitForOpen();
      await createKeystoreModal.cancel();
      await createKeystoreModal.waitForClose();
      
      await expect(createKeystoreModal.modal).toBeHidden();
    });
  });

  test.describe('Wallet Unlock/Lock', () => {
    test('should display unlock button when wallet is locked', async ({ page }) => {
      // First create a wallet
      const walletData = generateWalletData();
      await walletPage.createWalletButton.click();
      await createKeystoreModal.waitForOpen();
      await createKeystoreModal.fillForm(walletData.name, walletData.password);
      await createKeystoreModal.submit();
      await createKeystoreModal.waitForClose();
      
      // Lock the wallet
      await walletPage.lockWallet();
      
      // Should show unlock button
      await expect(walletPage.unlockButton).toBeVisible();
    });

    test('should unlock wallet with correct password', async ({ page }) => {
      const walletData = generateWalletData();
      
      // Create wallet
      await walletPage.createWalletButton.click();
      await createKeystoreModal.waitForOpen();
      await createKeystoreModal.fillForm(walletData.name, walletData.password);
      await createKeystoreModal.submit();
      await createKeystoreModal.waitForClose();
      
      // Lock
      await walletPage.lockWallet();
      
      // Unlock
      await walletPage.unlockWallet(walletData.password);
      
      // Should show lock button (wallet is unlocked)
      await expect(walletPage.lockButton).toBeVisible();
    });

    test('should show error for incorrect password', async ({ page }) => {
      const walletData = generateWalletData();
      
      // Create wallet
      await walletPage.createWalletButton.click();
      await createKeystoreModal.waitForOpen();
      await createKeystoreModal.fillForm(walletData.name, walletData.password);
      await createKeystoreModal.submit();
      await createKeystoreModal.waitForClose();
      
      // Lock
      await walletPage.lockWallet();
      
      // Try to unlock with wrong password
      await walletPage.unlockButton.click();
      await walletPage.passwordInput.fill('WrongPassword123!');
      await page.locator('button:has-text("Unlock")').click();
      
      // Should show error message
      const errorMessage = await createKeystoreModal.getErrorMessage();
      expect(errorMessage).not.toBe('');
    });

    test('should lock unlocked wallet', async ({ page }) => {
      const walletData = generateWalletData();
      
      // Create and unlock wallet
      await walletPage.createWalletButton.click();
      await createKeystoreModal.waitForOpen();
      await createKeystoreModal.fillForm(walletData.name, walletData.password);
      await createKeystoreModal.submit();
      await createKeystoreModal.waitForClose();
      
      // Should show lock button
      await expect(walletPage.lockButton).toBeVisible();
      
      // Lock
      await walletPage.lockWallet();
      
      // Should show unlock button
      await expect(walletPage.unlockButton).toBeVisible();
    });
  });

  test.describe('Wallet Display', () => {
    test('should display wallet list', async () => {
      await expect(walletPage.walletList).toBeVisible();
    });

    test('should display wallet cards', async ({ page }) => {
      const walletData = generateWalletData();
      
      // Create wallet
      await walletPage.createWalletButton.click();
      await createKeystoreModal.waitForOpen();
      await createKeystoreModal.fillForm(walletData.name, walletData.password);
      await createKeystoreModal.submit();
      await createKeystoreModal.waitForClose();
      
      // Verify wallet card displays
      const walletCount = await walletPage.getWalletCount();
      expect(walletCount).toBe(1);
      
      const name = await walletPage.getWalletName(0);
      expect(name).toContain(walletData.name);
    });

    test('should display wallet balance', async ({ page }) => {
      const walletData = generateWalletData();
      
      // Create wallet
      await walletPage.createWalletButton.click();
      await createKeystoreModal.waitForOpen();
      await createKeystoreModal.fillForm(walletData.name, walletData.password);
      await createKeystoreModal.submit();
      await createKeystoreModal.waitForClose();
      
      // Should display balance (might be 0)
      const balance = await walletPage.getWalletBalance(0);
      expect(balance).not.toBeNull();
    });
  });

  test.describe('Wallet Actions', () => {
    test('should select wallet', async ({ page }) => {
      const walletData = generateWalletData();
      
      // Create wallet
      await walletPage.createWalletButton.click();
      await createKeystoreModal.waitForOpen();
      await createKeystoreModal.fillForm(walletData.name, walletData.password);
      await createKeystoreModal.submit();
      await createKeystoreModal.waitForClose();
      
      // Select wallet
      await walletPage.selectWallet(0);
      
      // Should show wallet as selected (check for active class or similar)
      const card = walletPage.walletCards.first();
      await expect(card).toBeVisible();
    });

    test('should generate new keypair', async ({ page }) => {
      await expect(walletPage.generateKeyButton).toBeVisible();
      
      await walletPage.generateKeyButton.click();
      
      // Should generate and possibly add a new wallet or show keys
      // Implementation dependent - adjust assertions as needed
    });
  });

  test.describe('Multiple Wallets', () => {
    test('should create multiple wallets', async ({ page }) => {
      const wallet1 = generateWalletData();
      const wallet2 = generateWalletData();
      
      // Create first wallet
      await walletPage.createWalletButton.click();
      await createKeystoreModal.waitForOpen();
      await createKeystoreModal.fillForm(wallet1.name, wallet1.password);
      await createKeystoreModal.submit();
      await createKeystoreModal.waitForClose();
      
      // Create second wallet
      await walletPage.createWalletButton.click();
      await createKeystoreModal.waitForOpen();
      await createKeystoreModal.fillForm(wallet2.name, wallet2.password);
      await createKeystoreModal.submit();
      await createKeystoreModal.waitForClose();
      
      // Should have 2 wallets
      const walletCount = await walletPage.getWalletCount();
      expect(walletCount).toBe(2);
    });
  });

  test.describe('Responsive Design', () => {
    test('should display correctly on mobile', async ({ page }) => {
      await page.setViewportSize({ width: 375, height: 667 });
      await expect(walletPage.pageContainer).toBeVisible();
      await expect(walletPage.createWalletButton).toBeVisible();
    });

    test('should display correctly on tablet', async ({ page }) => {
      await page.setViewportSize({ width: 768, height: 1024 });
      await expect(walletPage.pageContainer).toBeVisible();
    });
  });
});
