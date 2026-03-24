import { Page, Locator } from '@playwright/test';

/**
 * Page Object for the Dashboard/Home page
 */
export class DashboardPagePO {
  readonly page: Page;
  readonly pageContainer: Locator;
  readonly header: Locator;
  readonly statsGrid: Locator;
  readonly recentRoundsTable: Locator;
  readonly networkVitals: Locator;
  readonly emissionProgress: Locator;

  constructor(page: Page) {
    this.page = page;
    this.pageContainer = page.locator('body');
    this.header = page.locator('h1:has-text("Dashboard")');
    this.statsGrid = page.locator('.grid:has(.metric-card), div:has-text("DAG Round")');
    this.recentRoundsTable = page.locator('table:has-text("Recent Finalized Rounds"), table');
    this.networkVitals = page.locator('div:has-text("Network Vitals")');
    this.emissionProgress = page.locator('div:has-text("Emission Progress")');
  }

  async getDagRound(): Promise<string | null> {
    const stat = this.page.locator('div:has-text("DAG Round")');
    return stat.textContent();
  }

  async getTotalSupply(): Promise<string | null> {
    const stat = this.page.locator('div:has-text("Total Supply")');
    return stat.textContent();
  }

  async getValidatorCount(): Promise<string | null> {
    const stat = this.page.locator('div:has-text("validators")');
    return stat.textContent();
  }

  async getTreasuryBalance(): Promise<string | null> {
    const stat = this.page.locator('div:has-text("DAO Treasury")');
    return stat.textContent();
  }

  async getRecentRoundCount(): Promise<number> {
    const rows = this.recentRoundsTable.locator('tbody tr');
    return rows.count();
  }

  async clickOnRound(index: number = 0): Promise<void> {
    const rows = this.recentRoundsTable.locator('tbody tr');
    await rows.nth(index).locator('a').click();
  }

  async isVisible(): Promise<boolean> {
    return this.header.isVisible();
  }

  async isLoading(): Promise<boolean> {
    const loader = this.page.locator('text=Connecting to node');
    return loader.isVisible();
  }

  async waitForLoaded(): Promise<void> {
    await this.header.waitFor({ state: 'visible' });
  }
}
