import { Page, Locator } from '@playwright/test';

/**
 * Page Object for the Send Transaction page
 */
export class SendPagePO {
  readonly page: Page;
  readonly pageContainer: Locator;
  readonly fromAddressInput: Locator;
  readonly toAddressInput: Locator;
  readonly amountInput: Locator;
  readonly sendButton: Locator;
  readonly maxButton: Locator;
  readonly feeDisplay: Locator;
  readonly confirmationModal: Locator;
  readonly confirmButton: Locator;
  readonly cancelButton: Locator;

  constructor(page: Page) {
    this.page = page;
    this.pageContainer = page.locator('[data-testid="send-page"], .send-page');
    this.fromAddressInput = page.locator('[data-testid="from-address"], input[name="from"], input[placeholder*="from"]');
    this.toAddressInput = page.locator('[data-testid="to-address"], input[name="to"], input[placeholder*="recipient"], input[placeholder*="to"]');
    this.amountInput = page.locator('[data-testid="amount-input"], input[name="amount"], input[type="number"], input[placeholder*="amount"]');
    this.sendButton = page.locator('[data-testid="send-button"], button:has-text("Send"), button[type="submit"]');
    this.maxButton = page.locator('[data-testid="max-button"], button:has-text("Max")');
    this.feeDisplay = page.locator('[data-testid="fee-display"], .fee-display, .transaction-fee');
    this.confirmationModal = page.locator('[data-testid="confirmation-modal"], .confirmation-modal, [role="dialog"]:has-text("Confirm")');
    this.confirmButton = this.confirmationModal.locator('button:has-text("Confirm"), button[type="submit"]');
    this.cancelButton = this.confirmationModal.locator('button:has-text("Cancel")');
  }

  async selectSender(address: string): Promise<void> {
    if (await this.fromAddressInput.isVisible()) {
      await this.fromAddressInput.click();
      await this.page.locator(`li:has-text("${address}"), option[value="${address}"]`).click();
    }
  }

  async fillRecipient(address: string): Promise<void> {
    await this.toAddressInput.fill(address);
  }

  async fillAmount(amount: string): Promise<void> {
    await this.amountInput.fill(amount);
  }

  async clickMax(): Promise<void> {
    await this.maxButton.click();
  }

  async getFee(): Promise<string> {
    return (await this.feeDisplay.textContent()) || '';
  }

  async submitTransaction(): Promise<void> {
    await this.sendButton.click();
  }

  async confirmTransaction(): Promise<void> {
    await this.confirmButton.click();
  }

  async cancelTransaction(): Promise<void> {
    await this.cancelButton.click();
  }

  async isConfirmationVisible(): Promise<boolean> {
    return this.confirmationModal.isVisible();
  }

  async isSendEnabled(): Promise<boolean> {
    return this.sendButton.isEnabled();
  }

  async getErrorMessage(): Promise<string> {
    const errorEl = this.page.locator('[data-testid="error-message"], .error-message, .text-error');
    return (await errorEl.textContent()) || '';
  }

  async isVisible(): Promise<boolean> {
    return this.pageContainer.isVisible();
  }

  async waitForLoaded(): Promise<void> {
    await this.pageContainer.waitFor({ state: 'visible' });
  }
}
