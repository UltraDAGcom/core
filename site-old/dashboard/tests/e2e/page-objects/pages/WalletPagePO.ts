import { Page, Locator } from '@playwright/test';

/**
 * Page Object for the Wallet page
 */
export class WalletPagePO {
  readonly page: Page;
  readonly pageContainer: Locator;
  readonly walletList: Locator;
  readonly createWalletButton: Locator;
  readonly importWalletButton: Locator;
  readonly generateKeyButton: Locator;
  readonly unlockButton: Locator;
  readonly lockButton: Locator;
  readonly passwordInput: Locator;
  readonly walletCards: Locator;

  constructor(page: Page) {
    this.page = page;
    this.pageContainer = page.locator('[data-testid="wallet-page"], .wallet-page');
    this.walletList = page.locator('[data-testid="wallet-list"], .wallet-list');
    this.createWalletButton = page.locator('[data-testid="create-wallet"], button:has-text("Create Wallet"), button:has-text("New Wallet")');
    this.importWalletButton = page.locator('[data-testid="import-wallet"], button:has-text("Import"), button:has-text("Import Wallet")');
    this.generateKeyButton = page.locator('[data-testid="generate-key"], button:has-text("Generate"), button:has-text("Generate Keypair")');
    this.unlockButton = page.locator('[data-testid="unlock-wallet"], button:has-text("Unlock")');
    this.lockButton = page.locator('[data-testid="lock-wallet"], button:has-text("Lock")');
    this.passwordInput = page.locator('input[type="password"][placeholder*="password"], input[name="password"]');
    this.walletCards = this.walletList.locator('[data-testid="wallet-card"], .wallet-card, .wallet-item');
  }

  async createWallet(name: string, password: string): Promise<void> {
    await this.createWalletButton.click();
    
    // Fill in the form in the modal
    await this.page.locator('input[placeholder*="name"], input[name="walletName"]').fill(name);
    await this.page.locator('input[type="password"][placeholder*="password"]').first().fill(password);
    await this.page.locator('input[type="password"][placeholder*="confirm"]').fill(password);
    
    // Submit
    await this.page.locator('button[type="submit"], button:has-text("Create")').click();
  }

  async unlockWallet(password: string): Promise<void> {
    if (await this.unlockButton.isVisible()) {
      await this.unlockButton.click();
      await this.passwordInput.fill(password);
      await this.page.locator('button:has-text("Unlock"), button[type="submit"]').click();
    }
  }

  async lockWallet(): Promise<void> {
    if (await this.lockButton.isVisible()) {
      await this.lockButton.click();
    }
  }

  async getWalletCount(): Promise<number> {
    return this.walletCards.count();
  }

  async getWalletName(index: number = 0): Promise<string> {
    const card = this.walletCards.nth(index);
    const nameEl = card.locator('[data-testid="wallet-name"], .wallet-name, h3, .name');
    return nameEl.textContent() || '';
  }

  async getWalletBalance(index: number = 0): Promise<string> {
    const card = this.walletCards.nth(index);
    const balanceEl = card.locator('[data-testid="wallet-balance"], .wallet-balance, .balance');
    return balanceEl.textContent() || '';
  }

  async selectWallet(index: number = 0): Promise<void> {
    const card = this.walletCards.nth(index);
    await card.click();
  }

  async deleteWallet(index: number = 0): Promise<void> {
    const card = this.walletCards.nth(index);
    const deleteButton = card.locator('[data-testid="delete-wallet"], button:has-text("Delete"), button:has-text("Remove")');
    if (await deleteButton.isVisible()) {
      await deleteButton.click();
      // Confirm deletion
      await this.page.locator('button:has-text("Confirm"), button:has-text("Delete"), button:has-text("Remove")').click();
    }
  }

  async exportWallet(index: number = 0): Promise<void> {
    const card = this.walletCards.nth(index);
    const exportButton = card.locator('[data-testid="export-wallet"], button:has-text("Export")');
    if (await exportButton.isVisible()) {
      await exportButton.click();
    }
  }

  async isWalletUnlocked(): Promise<boolean> {
    return this.lockButton.isVisible();
  }

  async isVisible(): Promise<boolean> {
    return this.pageContainer.isVisible();
  }

  async waitForLoaded(): Promise<void> {
    await this.pageContainer.waitFor({ state: 'visible' });
  }
}
