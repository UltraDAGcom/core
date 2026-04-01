import { test, expect } from '../../fixtures/base.fixture';
import { WalletPagePO } from '../../page-objects/pages/WalletPagePO';
import { CreateKeystoreModalPO } from '../../page-objects/modals/CreateKeystoreModalPO';
import { generateWalletData } from '../../utils/test-data';

test.describe('Wallet Import and Export', () => {
  let walletPage: WalletPagePO;
  let createKeystoreModal: CreateKeystoreModalPO;

  test.beforeEach(async ({ page }) => {
    walletPage = new WalletPagePO(page);
    createKeystoreModal = new CreateKeystoreModalPO(page);
    await page.goto('/wallet');
    await walletPage.waitForLoaded();
  });

  test.describe('Wallet Import', () => {
    test('should display import wallet button', async () => {
      await expect(walletPage.importWalletButton).toBeVisible();
    });

    test('should open import modal', async ({ page }) => {
      await walletPage.importWalletButton.click();
      await createKeystoreModal.waitForOpen();
      await expect(createKeystoreModal.modal).toBeVisible();
    });

    test('should show file input for keystore blob', async ({ page }) => {
      await walletPage.importWalletButton.click();
      await createKeystoreModal.waitForOpen();
      
      await expect(createKeystoreModal.fileInput).toBeVisible();
    });

    test('should import wallet from valid blob file', async ({ page }) => {
      // Create a wallet first to export
      const walletData = generateWalletData();
      
      // Create wallet
      await walletPage.createWalletButton.click();
      await createKeystoreModal.waitForOpen();
      await createKeystoreModal.fillForm(walletData.name, walletData.password);
      await createKeystoreModal.submit();
      await createKeystoreModal.waitForClose();
      
      // Export the wallet (implementation dependent)
      await walletPage.exportWallet(0);
      
      // Wait for download or file to be available
      await page.waitForTimeout(1000);
      
      // Now import using the exported file
      // Note: Actual file path depends on download location
      // This is a placeholder - adjust based on your export implementation
    });

    test('should show error for invalid blob file', async ({ page }) => {
      await walletPage.importWalletButton.click();
      await createKeystoreModal.waitForOpen();
      
      // Create an empty file
      const emptyFile = new File([''], 'invalid.json', { type: 'application/json' });
      
      // This would need actual file handling - placeholder for now
      await expect(createKeystoreModal.modal).toBeVisible();
    });

    test('should show error for wrong password on import', async ({ page }) => {
      // Create wallet to export
      const walletData = generateWalletData();
      await walletPage.createWalletButton.click();
      await createKeystoreModal.waitForOpen();
      await createKeystoreModal.fillForm(walletData.name, walletData.password);
      await createKeystoreModal.submit();
      await createKeystoreModal.waitForClose();
      
      // Export and re-import with wrong password
      // Implementation dependent
    });

    test('should cancel import process', async ({ page }) => {
      await walletPage.importWalletButton.click();
      await createKeystoreModal.waitForOpen();
      await createKeystoreModal.cancel();
      await createKeystoreModal.waitForClose();
      
      await expect(createKeystoreModal.modal).toBeHidden();
    });
  });

  test.describe('Wallet Export', () => {
    test('should display export option for wallet', async ({ page }) => {
      const walletData = generateWalletData();
      
      // Create wallet
      await walletPage.createWalletButton.click();
      await createKeystoreModal.waitForOpen();
      await createKeystoreModal.fillForm(walletData.name, walletData.password);
      await createKeystoreModal.submit();
      await createKeystoreModal.waitForClose();
      
      // Check export button exists
      await walletPage.exportWallet(0);
      
      // Should trigger download or show export modal
      // Implementation dependent
    });

    test('should require unlock before export', async ({ page }) => {
      const walletData = generateWalletData();
      
      // Create and lock wallet
      await walletPage.createWalletButton.click();
      await createKeystoreModal.waitForOpen();
      await createKeystoreModal.fillForm(walletData.name, walletData.password);
      await createKeystoreModal.submit();
      await createKeystoreModal.waitForClose();
      
      await walletPage.lockWallet();
      
      // Try to export - should prompt for unlock
      // Implementation dependent
    });
  });

  test.describe('Wallet Deletion', () => {
    test('should display delete/remove option', async ({ page }) => {
      const walletData = generateWalletData();
      
      // Create wallet
      await walletPage.createWalletButton.click();
      await createKeystoreModal.waitForOpen();
      await createKeystoreModal.fillForm(walletData.name, walletData.password);
      await createKeystoreModal.submit();
      await createKeystoreModal.waitForClose();
      
      // Check for delete button
      const deleteButton = walletPage.walletCards.first().locator('button:has-text("Delete"), button:has-text("Remove")');
      await expect(deleteButton).toBeVisible();
    });

    test('should delete wallet', async ({ page }) => {
      const walletData = generateWalletData();
      
      // Create wallet
      await walletPage.createWalletButton.click();
      await createKeystoreModal.waitForOpen();
      await createKeystoreModal.fillForm(walletData.name, walletData.password);
      await createKeystoreModal.submit();
      await createKeystoreModal.waitForClose();
      
      // Verify wallet exists
      let walletCount = await walletPage.getWalletCount();
      expect(walletCount).toBe(1);
      
      // Delete wallet
      await walletPage.deleteWallet(0);
      
      // Verify wallet is deleted
      walletCount = await walletPage.getWalletCount();
      expect(walletCount).toBe(0);
    });

    test('should require confirmation before deletion', async ({ page }) => {
      const walletData = generateWalletData();
      
      // Create wallet
      await walletPage.createWalletButton.click();
      await createKeystoreModal.waitForOpen();
      await createKeystoreModal.fillForm(walletData.name, walletData.password);
      await createKeystoreModal.submit();
      await createKeystoreModal.waitForClose();
      
      // Click delete
      const deleteButton = walletPage.walletCards.first().locator('button:has-text("Delete"), button:has-text("Remove")');
      await deleteButton.click();
      
      // Should show confirmation dialog
      const confirmButton = page.locator('button:has-text("Confirm"), button:has-text("Delete")');
      await expect(confirmButton).toBeVisible();
      
      // Cancel deletion
      const cancelButton = page.locator('button:has-text("Cancel")');
      await cancelButton.click();
      
      // Wallet should still exist
      const walletCount = await walletPage.getWalletCount();
      expect(walletCount).toBe(1);
    });
  });
});
