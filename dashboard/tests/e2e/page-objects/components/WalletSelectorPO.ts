import { Page, Locator } from '@playwright/test';

/**
 * Page Object for the WalletSelector component
 */
export class WalletSelectorPO {
  readonly page: Page;
  readonly walletSelector: Locator;
  readonly walletDropdown: Locator;
  readonly walletItems: Locator;
  readonly addWalletButton: Locator;

  constructor(page: Page) {
    this.page = page;
    this.walletSelector = page.locator('[data-testid="wallet-selector"], .wallet-selector, button:has-text("Select Wallet")');
    this.walletDropdown = page.locator('[data-testid="wallet-dropdown"], .wallet-dropdown, .wallet-selector-dropdown');
    this.walletItems = this.walletDropdown.locator('li, button, [role="menuitem"]');
    this.addWalletButton = this.walletDropdown.locator('button:has-text("Add"), button:has-text("Create"), [data-testid="add-wallet"]');
  }

  async selectWallet(walletName: string): Promise<void> {
    await this.walletSelector.click();
    await this.walletItems.filter({ hasText: walletName }).click();
  }

  async getSelectedWallet(): Promise<string | null> {
    const text = await this.walletSelector.textContent();
    return text?.trim() || null;
  }

  async getWalletCount(): Promise<number> {
    return this.walletItems.count();
  }

  async addWallet(): Promise<void> {
    await this.walletSelector.click();
    await this.addWalletButton.click();
  }

  async isDropdownOpen(): Promise<boolean> {
    return this.walletDropdown.isVisible();
  }

  async closeDropdown(): Promise<void> {
    await this.page.keyboard.press('Escape');
  }
}
