import { Page, Locator } from '@playwright/test';

/**
 * Page Object for the TopBar component
 */
export class TopBarPO {
  readonly page: Page;
  readonly topBar: Locator;
  readonly networkSelector: Locator;
  readonly connectionStatus: Locator;
  readonly walletButton: Locator;
  readonly menuButton: Locator;

  constructor(page: Page) {
    this.page = page;
    // TopBar uses 'header' element
    this.topBar = page.locator('header').first();
    this.networkSelector = this.topBar.locator('button, select').filter({ hasText: /testnet|mainnet|network/i }).first();
    this.connectionStatus = this.topBar.locator('[class*="status"], [class*="connected"]').first();
    this.walletButton = this.topBar.locator('button').filter({ hasText: /wallet|select/i }).first();
    this.menuButton = this.topBar.locator('button').first();
  }

  async getNetwork(): Promise<string> {
    const text = await this.networkSelector.textContent();
    return text?.toLowerCase() || 'unknown';
  }

  async switchNetwork(network: 'testnet' | 'mainnet'): Promise<void> {
    await this.networkSelector.click();
    await this.page.locator(`button:has-text("${network}"), li:has-text("${network}"), option[value="${network}"]`).click();
  }

  async getConnectionStatus(): Promise<string> {
    const status = await this.connectionStatus.getAttribute('data-status');
    return status || (await this.connectionStatus.textContent()) || 'unknown';
  }

  async isConnected(): Promise<boolean> {
    const status = await this.getConnectionStatus();
    return status.toLowerCase().includes('connected') || status === 'connected';
  }

  async openWalletMenu(): Promise<void> {
    await this.walletButton.click();
  }

  async openMenu(): Promise<void> {
    await this.menuButton.click();
  }

  async isVisible(): Promise<boolean> {
    return this.topBar.isVisible();
  }
}
