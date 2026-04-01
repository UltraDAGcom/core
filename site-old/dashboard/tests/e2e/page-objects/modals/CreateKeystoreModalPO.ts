import { Page, Locator } from '@playwright/test';

/**
 * Page Object for the Create Keystore Modal
 */
export class CreateKeystoreModalPO {
  readonly page: Page;
  readonly modal: Locator;
  readonly nameInput: Locator;
  readonly passwordInput: Locator;
  readonly confirmPasswordInput: Locator;
  readonly createButton: Locator;
  readonly cancelButton: Locator;
  readonly importButton: Locator;
  readonly fileInput: Locator;
  readonly errorMessage: Locator;

  constructor(page: Page) {
    this.page = page;
    this.modal = page.locator('[data-testid="create-keystore-modal"], .create-keystore-modal, [role="dialog"]:has-text("Create Wallet"), [role="dialog"]:has-text("Unlock")');
    this.nameInput = this.modal.locator('input[name="name"], input[placeholder*="name"]');
    this.passwordInput = this.modal.locator('input[type="password"][placeholder*="password"]').first();
    this.confirmPasswordInput = this.modal.locator('input[type="password"][placeholder*="confirm"]');
    this.createButton = this.modal.locator('button:has-text("Create"), button:has-text("Continue"), button[type="submit"]');
    this.cancelButton = this.modal.locator('button:has-text("Cancel")');
    this.importButton = this.modal.locator('button:has-text("Import"), button:has-text("Import Keystore")');
    this.fileInput = this.modal.locator('input[type="file"]');
    this.errorMessage = this.modal.locator('[data-testid="error-message"], .error-message, .text-error');
  }

  async fillName(name: string): Promise<void> {
    await this.nameInput.fill(name);
  }

  async fillPassword(password: string): Promise<void> {
    await this.passwordInput.fill(password);
  }

  async fillConfirmPassword(password: string): Promise<void> {
    await this.confirmPasswordInput.fill(password);
  }

  async fillForm(name: string, password: string): Promise<void> {
    await this.fillName(name);
    await this.fillPassword(password);
    await this.fillConfirmPassword(password);
  }

  async submit(): Promise<void> {
    await this.createButton.click();
  }

  async cancel(): Promise<void> {
    await this.cancelButton.click();
  }

  async importKeystore(filePath: string): Promise<void> {
    await this.importButton.click();
    await this.fileInput.setInputFiles(filePath);
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
