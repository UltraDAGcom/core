import { Page, Locator } from '@playwright/test';

/**
 * Test data for wallet operations
 */
export interface WalletTestData {
  name: string;
  password: string;
  secretKey?: string;
  address?: string;
}

/**
 * Generate random wallet test data
 */
export function generateWalletData(): WalletTestData {
  const randomId = Math.random().toString(36).substring(2, 8);
  return {
    name: `Test Wallet ${randomId}`,
    password: `SecurePass123!${randomId}`,
  };
}

/**
 * Fixture for wallet-related test operations
 */
export class WalletFixture {
  readonly page: Page;
  readonly createWalletButton: Locator;
  readonly importWalletButton: Locator;

  constructor(page: Page) {
    this.page = page;
    this.createWalletButton = page.locator('button:has-text("Create Wallet"), button:has-text("New Wallet"), [data-testid="create-wallet"]');
    this.importWalletButton = page.locator('button:has-text("Import"), button:has-text("Import Wallet"), [data-testid="import-wallet"]');
  }

  /**
   * Create a new wallet with the given data
   */
  async createWallet(data: WalletTestData): Promise<void> {
    await this.createWalletButton.click();
    
    // Enter wallet name
    await this.page.locator('input[placeholder*="name"], input[name="name"]').fill(data.name);
    
    // Enter password
    await this.page.locator('input[type="password"][placeholder*="password"], input[name="password"]').fill(data.password);
    
    // Confirm password
    await this.page.locator('input[type="password"][placeholder*="confirm"], input[name="confirmPassword"]').fill(data.password);
    
    // Submit
    await this.page.locator('button[type="submit"], button:has-text("Create"), button:has-text("Continue")').click();
  }

  /**
   * Import wallet from blob
   */
  async importWallet(blobPath: string, password: string): Promise<void> {
    await this.importWalletButton.click();
    
    // Upload blob file
    const fileInput = this.page.locator('input[type="file"]');
    await fileInput.setInputFiles(blobPath);
    
    // Enter password
    await this.page.locator('input[type="password"]').fill(password);
    
    // Submit
    await this.page.locator('button[type="submit"], button:has-text("Import")').click();
  }

  /**
   * Lock the wallet
   */
  async lockWallet(): Promise<void> {
    const lockButton = this.page.locator('button:has-text("Lock"), [data-testid="lock-wallet"]');
    if (await lockButton.isVisible()) {
      await lockButton.click();
    }
  }

  /**
   * Unlock the wallet with password
   */
  async unlockWallet(password: string): Promise<void> {
    const unlockButton = this.page.locator('button:has-text("Unlock"), button:has-text("Unlock Wallet")');
    if (await unlockButton.isVisible()) {
      await unlockButton.click();
      await this.page.locator('input[type="password"]').fill(password);
      await this.page.locator('button[type="submit"], button:has-text("Unlock")').click();
    }
  }

  /**
   * Check if wallet is unlocked
   */
  async isUnlocked(): Promise<boolean> {
    const lockButton = this.page.locator('button:has-text("Lock"), [data-testid="lock-wallet"]');
    return lockButton.isVisible();
  }
}
