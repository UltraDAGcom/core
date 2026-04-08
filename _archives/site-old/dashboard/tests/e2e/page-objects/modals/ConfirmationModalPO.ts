import { Page, Locator } from '@playwright/test';

/**
 * Page Object for generic Confirmation/Transaction Modal
 */
export class ConfirmationModalPO {
  readonly page: Page;
  readonly modal: Locator;
  readonly title: Locator;
  readonly content: Locator;
  readonly confirmButton: Locator;
  readonly cancelButton: Locator;
  readonly details: Locator;

  constructor(page: Page) {
    this.page = page;
    this.modal = page.locator('[data-testid="confirmation-modal"], .confirmation-modal, [role="dialog"]:has-text("Confirm")');
    this.title = this.modal.locator('[data-testid="modal-title"], .modal-title, h2, h3');
    this.content = this.modal.locator('[data-testid="modal-content"], .modal-content, p');
    this.confirmButton = this.modal.locator('button:has-text("Confirm"), button:has-text("Yes"), button[type="submit"]');
    this.cancelButton = this.modal.locator('button:has-text("Cancel"), button:has-text("No")');
    this.details = this.modal.locator('[data-testid="modal-details"], .modal-details, .details');
  }

  async getTitle(): Promise<string> {
    return (await this.title.textContent()) || '';
  }

  async getContent(): Promise<string> {
    return (await this.content.textContent()) || '';
  }

  async getDetails(): Promise<string> {
    return (await this.details.textContent()) || '';
  }

  async confirm(): Promise<void> {
    await this.confirmButton.click();
  }

  async cancel(): Promise<void> {
    await this.cancelButton.click();
  }

  async isVisible(): Promise<boolean> {
    return this.modal.isVisible();
  }

  async waitForOpen(): Promise<void> {
    await this.modal.waitFor({ state: 'visible' });
  }

  async waitForClose(): Promise<void> {
    await this.modal.waitFor({ state: 'hidden' });
  }
}
