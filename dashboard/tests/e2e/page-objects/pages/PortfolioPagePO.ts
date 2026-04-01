import { Page, Locator } from '@playwright/test';

/**
 * Page Object for the Portfolio page
 */
export class PortfolioPagePO {
  readonly page: Page;
  readonly pageContainer: Locator;
  readonly totalBalanceCard: Locator;
  readonly totalStakedCard: Locator;
  readonly totalDelegatedCard: Locator;
  readonly walletBreakdown: Locator;
  readonly balanceChart: Locator;
  readonly walletList: Locator;

  constructor(page: Page) {
    this.page = page;
    this.pageContainer = page.locator('[data-testid="portfolio-page"], .portfolio-page');
    this.totalBalanceCard = page.locator('[data-testid="total-balance"], .total-balance, .balance-card:has-text("Total Balance")');
    this.totalStakedCard = page.locator('[data-testid="total-staked"], .total-staked, .balance-card:has-text("Staked")');
    this.totalDelegatedCard = page.locator('[data-testid="total-delegated"], .total-delegated, .balance-card:has-text("Delegated")');
    this.walletBreakdown = page.locator('[data-testid="wallet-breakdown"], .wallet-breakdown');
    this.balanceChart = page.locator('[data-testid="balance-chart"], .balance-chart, canvas#balanceChart');
    this.walletList = page.locator('[data-testid="portfolio-wallets"], .portfolio-wallets');
  }

  async getTotalBalance(): Promise<string> {
    const balanceEl = this.totalBalanceCard.locator('[data-testid="balance-value"], .balance-value, .amount');
    return (await balanceEl.textContent()) || '';
  }

  async getTotalStaked(): Promise<string> {
    const stakedEl = this.totalStakedCard.locator('[data-testid="staked-value"], .staked-value, .amount');
    return (await stakedEl.textContent()) || '';
  }

  async getTotalDelegated(): Promise<string> {
    const delegatedEl = this.totalDelegatedCard.locator('[data-testid="delegated-value"], .delegated-value, .amount');
    return (await delegatedEl.textContent()) || '';
  }

  async getWalletCount(): Promise<number> {
    const wallets = this.walletList.locator('[data-testid="wallet-item"], .wallet-item, tr');
    return wallets.count();
  }

  async getWalletBalance(walletName: string): Promise<string> {
    const row = this.walletList.locator(`tr:has-text("${walletName}"), .wallet-item:has-text("${walletName}")`);
    const balanceEl = row.locator('[data-testid="balance"], .balance, .amount');
    return (await balanceEl.textContent()) || '';
  }

  async clickOnWallet(walletName: string): Promise<void> {
    const row = this.walletList.locator(`tr:has-text("${walletName}"), .wallet-item:has-text("${walletName}")`);
    await row.click();
  }

  async isVisible(): Promise<boolean> {
    return this.pageContainer.isVisible();
  }

  async waitForLoaded(): Promise<void> {
    await this.pageContainer.waitFor({ state: 'visible' });
  }
}
