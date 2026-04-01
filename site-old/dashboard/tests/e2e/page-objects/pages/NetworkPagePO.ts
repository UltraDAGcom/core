import { Page, Locator } from '@playwright/test';

/**
 * Page Object for the Network page
 */
export class NetworkPagePO {
  readonly page: Page;
  readonly pageContainer: Locator;
  readonly networkStats: Locator;
  readonly mempoolTable: Locator;
  readonly peerList: Locator;
  readonly tpsChart: Locator;
  readonly heightChart: Locator;
  readonly statusGrid: Locator;

  constructor(page: Page) {
    this.page = page;
    this.pageContainer = page.locator('[data-testid="network-page"], .network-page');
    this.networkStats = page.locator('[data-testid="network-stats"], .network-stats');
    this.mempoolTable = page.locator('[data-testid="mempool-table"], .mempool-table, table.mempool');
    this.peerList = page.locator('[data-testid="peer-list"], .peer-list');
    this.tpsChart = page.locator('[data-testid="tps-chart"], .tps-chart, canvas#tpsChart');
    this.heightChart = page.locator('[data-testid="height-chart"], .height-chart, canvas#heightChart');
    this.statusGrid = page.locator('[data-testid="status-grid"], .status-grid');
  }

  async getCurrentHeight(): Promise<string> {
    const stat = this.networkStats.locator('[data-testid="height"], .stat:has-text("Height"), .stat:has-text("Block Height")');
    return (await stat.textContent()) || '';
  }

  async getCurrentTPS(): Promise<string> {
    const stat = this.networkStats.locator('[data-testid="tps"], .stat:has-text("TPS"), .stat:has-text("Transactions/sec")');
    return (await stat.textContent()) || '';
  }

  async getPeerCount(): Promise<string> {
    const stat = this.networkStats.locator('[data-testid="peer-count"], .stat:has-text("Peers"), .stat:has-text("Peer Count")');
    return (await stat.textContent()) || '';
  }

  async getMempoolSize(): Promise<string> {
    const stat = this.networkStats.locator('[data-testid="mempool-size"], .stat:has-text("Mempool"), .stat:has-text("Pending")');
    return (await stat.textContent()) || '';
  }

  async getMempoolTransactionCount(): Promise<number> {
    const rows = this.mempoolTable.locator('tbody tr, [role="row"]');
    return rows.count();
  }

  async getPeerCountInList(): Promise<number> {
    const peers = this.peerList.locator('[data-testid="peer-item"], .peer-item, tr');
    return peers.count();
  }

  async refreshStats(): Promise<void> {
    const refreshButton = this.page.locator('[data-testid="refresh"], button:has-text("Refresh")');
    if (await refreshButton.isVisible()) {
      await refreshButton.click();
    }
  }

  async isVisible(): Promise<boolean> {
    return this.pageContainer.isVisible();
  }

  async waitForLoaded(): Promise<void> {
    await this.pageContainer.waitFor({ state: 'visible' });
  }
}
