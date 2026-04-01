import { Page, Locator } from '@playwright/test';

/**
 * Page Object for the Create Proposal Modal
 */
export class CreateProposalModalPO {
  readonly page: Page;
  readonly modal: Locator;
  readonly titleInput: Locator;
  readonly descriptionInput: Locator;
  readonly depositInput: Locator;
  readonly submitButton: Locator;
  readonly cancelButton: Locator;
  readonly errorMessage: Locator;

  constructor(page: Page) {
    this.page = page;
    this.modal = page.locator('[data-testid="create-proposal-modal"], .create-proposal-modal, [role="dialog"]:has-text("Create Proposal")');
    this.titleInput = this.modal.locator('input[name="title"], input[placeholder*="title"]');
    this.descriptionInput = this.modal.locator('textarea[name="description"], textarea[placeholder*="description"]');
    this.depositInput = this.modal.locator('input[name="deposit"], input[placeholder*="deposit"], input[type="number"]');
    this.submitButton = this.modal.locator('button:has-text("Create"), button:has-text("Submit"), button[type="submit"]');
    this.cancelButton = this.modal.locator('button:has-text("Cancel")');
    this.errorMessage = this.modal.locator('[data-testid="error-message"], .error-message, .text-error');
  }

  async fillTitle(title: string): Promise<void> {
    await this.titleInput.fill(title);
  }

  async fillDescription(description: string): Promise<void> {
    await this.descriptionInput.fill(description);
  }

  async fillDeposit(deposit: string): Promise<void> {
    await this.depositInput.fill(deposit);
  }

  async fillForm(title: string, description: string, deposit?: string): Promise<void> {
    await this.fillTitle(title);
    await this.fillDescription(description);
    if (deposit) {
      await this.fillDeposit(deposit);
    }
  }

  async submit(): Promise<void> {
    await this.submitButton.click();
  }

  async cancel(): Promise<void> {
    await this.cancelButton.click();
  }

  async getErrorMessage(): Promise<string> {
    return (await this.errorMessage.textContent()) || '';
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
