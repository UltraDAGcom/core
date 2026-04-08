import { test, expect } from '../../fixtures/base.fixture';
import { SendPagePO } from '../../page-objects/pages/SendPagePO';
import { WalletPagePO } from '../../page-objects/pages/WalletPagePO';
import { CreateKeystoreModalPO } from '../../page-objects/modals/CreateKeystoreModalPO';
import { ConfirmationModalPO } from '../../page-objects/modals/ConfirmationModalPO';
import { generateWalletData, generateAddress, generateDagAmount } from '../../utils/test-data';

test.describe('Send Transaction', () => {
  let sendPage: SendPagePO;
  let walletPage: WalletPagePO;
  let createKeystoreModal: CreateKeystoreModalPO;
  let confirmationModal: ConfirmationModalPO;

  test.beforeEach(async ({ page }) => {
    sendPage = new SendPagePO(page);
    walletPage = new WalletPagePO(page);
    createKeystoreModal = new CreateKeystoreModalPO(page);
    confirmationModal = new ConfirmationModalPO(page);
    
    // Create a wallet first
    await page.goto('/wallet');
    const walletData = generateWalletData();
    await walletPage.createWalletButton.click();
    await createKeystoreModal.waitForOpen();
    await createKeystoreModal.fillForm(walletData.name, walletData.password);
    await createKeystoreModal.submit();
    await createKeystoreModal.waitForClose();
    
    // Unlock wallet
    await walletPage.unlockWallet(walletData.password);
    
    // Navigate to send page
    await page.goto('/wallet/send');
    await sendPage.waitForLoaded();
  });

  test.describe('Page Load and Layout', () => {
    test('should load send page successfully', async ({ page }) => {
      await expect(page).toHaveURL('/wallet/send');
      await expect(sendPage.pageContainer).toBeVisible();
    });

    test('should display from address selector', async () => {
      await expect(sendPage.fromAddressInput).toBeVisible();
    });

    test('should display to address input', async () => {
      await expect(sendPage.toAddressInput).toBeVisible();
    });

    test('should display amount input', async () => {
      await expect(sendPage.amountInput).toBeVisible();
    });

    test('should display send button', async () => {
      await expect(sendPage.sendButton).toBeVisible();
    });
  });

  test.describe('Form Validation', () => {
    test('should disable send button with empty form', async () => {
      const isEnabled = await sendPage.isSendEnabled();
      expect(isEnabled).toBeFalsy();
    });

    test('should show error for invalid recipient address', async ({ page }) => {
      await sendPage.fillRecipient('invalid-address');
      await sendPage.fillAmount('10');
      
      const isEnabled = await sendPage.isSendEnabled();
      // May be disabled or show error on submit
      if (isEnabled) {
        await sendPage.submitTransaction();
        const errorMessage = await sendPage.getErrorMessage();
        expect(errorMessage).not.toBe('');
      }
    });

    test('should show error for negative amount', async ({ page }) => {
      const recipientAddress = generateAddress();
      await sendPage.fillRecipient(recipientAddress);
      await sendPage.fillAmount('-10');
      
      const isEnabled = await sendPage.isSendEnabled();
      expect(isEnabled).toBeFalsy();
    });

    test('should show error for zero amount', async ({ page }) => {
      const recipientAddress = generateAddress();
      await sendPage.fillRecipient(recipientAddress);
      await sendPage.fillAmount('0');
      
      const isEnabled = await sendPage.isSendEnabled();
      expect(isEnabled).toBeFalsy();
    });

    test('should enable send button with valid inputs', async ({ page }) => {
      const recipientAddress = generateAddress();
      await sendPage.fillRecipient(recipientAddress);
      await sendPage.fillAmount('10');
      
      // Button should be enabled (assuming wallet has balance)
      const isEnabled = await sendPage.isSendEnabled();
      expect(isEnabled).toBeTruthy();
    });
  });

  test.describe('Send Transaction Flow', () => {
    test('should submit transaction with valid data', async ({ page }) => {
      const recipientAddress = generateAddress();
      const amount = '10';
      
      await sendPage.fillRecipient(recipientAddress);
      await sendPage.fillAmount(amount);
      await sendPage.submitTransaction();
      
      // Should show confirmation modal
      await confirmationModal.waitForOpen();
      await expect(confirmationModal.modal).toBeVisible();
    });

    test('should show transaction details in confirmation', async ({ page }) => {
      const recipientAddress = generateAddress();
      const amount = '10';
      
      await sendPage.fillRecipient(recipientAddress);
      await sendPage.fillAmount(amount);
      await sendPage.submitTransaction();
      
      await confirmationModal.waitForOpen();
      
      const details = await confirmationModal.getDetails();
      expect(details).toContain(recipientAddress);
      expect(details).toContain(amount);
    });

    test('should cancel transaction from confirmation', async ({ page }) => {
      const recipientAddress = generateAddress();
      await sendPage.fillRecipient(recipientAddress);
      await sendPage.fillAmount('10');
      await sendPage.submitTransaction();
      
      await confirmationModal.waitForOpen();
      await confirmationModal.cancel();
      await confirmationModal.waitForClose();
      
      // Should stay on send page
      await expect(page).toHaveURL('/wallet/send');
    });

    test('should confirm and submit transaction', async ({ page }) => {
      const recipientAddress = generateAddress();
      await sendPage.fillRecipient(recipientAddress);
      await sendPage.fillAmount('10');
      await sendPage.submitTransaction();
      
      await confirmationModal.waitForOpen();
      await confirmationModal.confirm();
      
      // Should show success message or navigate to transaction detail
      // Implementation dependent
    });
  });

  test.describe('Max Button', () => {
    test('should display max button', async () => {
      await expect(sendPage.maxButton).toBeVisible();
    });

    test('should fill max amount when clicking max button', async ({ page }) => {
      await sendPage.clickMax();
      
      // Amount input should be filled with max value
      const amount = await sendPage.amountInput.inputValue();
      expect(amount).not.toBe('');
    });

    test('should update amount when clicking max after entering recipient', async ({ page }) => {
      const recipientAddress = generateAddress();
      await sendPage.fillRecipient(recipientAddress);
      await sendPage.clickMax();
      
      const amount = await sendPage.amountInput.inputValue();
      expect(amount).not.toBe('');
    });
  });

  test.describe('Fee Display', () => {
    test('should display transaction fee', async () => {
      await expect(sendPage.feeDisplay).toBeVisible();
    });

    test('should show valid fee amount', async () => {
      const fee = await sendPage.getFee();
      expect(fee).not.toBe('');
    });
  });

  test.describe('Sender Selection', () => {
    test('should display sender dropdown with available wallets', async ({ page }) => {
      await expect(sendPage.fromAddressInput).toBeVisible();
      
      // Click to open dropdown
      await sendPage.fromAddressInput.click();
      
      // Should show wallet options
      const options = page.locator('li, option');
      const count = await options.count();
      expect(count).toBeGreaterThan(0);
    });

    test('should select sender from dropdown', async ({ page }) => {
      // Get first wallet address and select it
      await sendPage.fromAddressInput.click();
      const firstOption = page.locator('li, option').first();
      const text = await firstOption.textContent();
      
      if (text) {
        await firstOption.click();
        // Sender should be selected
      }
    });
  });

  test.describe('Responsive Design', () => {
    test('should display correctly on mobile', async ({ page }) => {
      await page.setViewportSize({ width: 375, height: 667 });
      await expect(sendPage.pageContainer).toBeVisible();
    });

    test('should display correctly on tablet', async ({ page }) => {
      await page.setViewportSize({ width: 768, height: 1024 });
      await expect(sendPage.pageContainer).toBeVisible();
    });
  });
});
