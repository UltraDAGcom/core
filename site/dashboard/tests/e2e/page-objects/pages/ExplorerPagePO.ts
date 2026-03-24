import { Page, Locator } from '@playwright/test';

/**
 * Page Object for the Explorer page
 */
export class ExplorerPagePO {
  readonly page: Page;
  readonly pageContainer: Locator;
  readonly searchInput: Locator;
  readonly searchButton: Locator;
  readonly recentTransactions: Locator;
  readonly recentVertices: Locator;
  readonly recentRounds: Locator;
  readonly transactionTable: Locator;
  readonly vertexTable: Locator;
  readonly filterDropdown: Locator;
  readonly pagination: Locator;

  constructor(page: Page) {
    this.page = page;
    this.pageContainer = page.locator('[data-testid="explorer-page"], .explorer-page');
    this.searchInput = page.locator('[data-testid="search-input"], input[placeholder*="search"], input[type="search"]');
    this.searchButton = page.locator('[data-testid="search-button"], button:has-text("Search")');
    this.recentTransactions = page.locator('[data-testid="recent-transactions"], .recent-transactions');
    this.recentVertices = page.locator('[data-testid="recent-vertices"], .recent-vertices');
    this.recentRounds = page.locator('[data-testid="recent-rounds"], .recent-rounds');
    this.transactionTable = page.locator('[data-testid="transaction-table"], table.transactions');
    this.vertexTable = page.locator('[data-testid="vertex-table"], table.vertices');
    this.filterDropdown = page.locator('[data-testid="filter-dropdown"], select[name="filter"]');
    this.pagination = page.locator('[data-testid="pagination"], .pagination');
  }

  async search(query: string): Promise<void> {
    await this.searchInput.fill(query);
    await this.searchButton.click();
  }

  async searchWithEnter(query: string): Promise<void> {
    await this.searchInput.fill(query);
    await this.searchInput.press('Enter');
  }

  async filterByType(type: string): Promise<void> {
    await this.filterDropdown.selectOption(type);
  }

  async getTransactionCount(): Promise<number> {
    const rows = this.transactionTable.locator('tbody tr, [role="row"]');
    return rows.count();
  }

  async getVertexCount(): Promise<number> {
    const rows = this.vertexTable.locator('tbody tr, [role="row"]');
    return rows.count();
  }

  async clickTransaction(index: number = 0): Promise<void> {
    const rows = this.transactionTable.locator('tbody tr');
    await rows.nth(index).click();
  }

  async clickVertex(index: number = 0): Promise<void> {
    const rows = this.vertexTable.locator('tbody tr');
    await rows.nth(index).click();
  }

  async goToNextPage(): Promise<void> {
    const nextButton = this.pagination.locator('button:has-text("Next"), li:has-text("Next")');
    if (await nextButton.isVisible()) {
      await nextButton.click();
    }
  }

  async goToPreviousPage(): Promise<void> {
    const prevButton = this.pagination.locator('button:has-text("Previous"), li:has-text("Previous")');
    if (await prevButton.isVisible()) {
      await prevButton.click();
    }
  }

  async goToPage(pageNumber: number): Promise<void> {
    const pageButton = this.pagination.locator(`button:has-text("${pageNumber}"), li:has-text("${pageNumber}")`);
    if (await pageButton.isVisible()) {
      await pageButton.click();
    }
  }

  async isVisible(): Promise<boolean> {
    return this.pageContainer.isVisible();
  }

  async waitForLoaded(): Promise<void> {
    await this.pageContainer.waitFor({ state: 'visible' });
  }
}
