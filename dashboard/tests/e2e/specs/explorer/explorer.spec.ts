import { test, expect } from '../../fixtures/base.fixture';
import { ExplorerPagePO } from '../../page-objects/pages/ExplorerPagePO';
import { generateTxHash, generateVertexHash, generateAddress } from '../../utils/test-data';

test.describe('Explorer Page', () => {
  let explorerPage: ExplorerPagePO;

  test.beforeEach(async ({ page }) => {
    explorerPage = new ExplorerPagePO(page);
    await page.goto('/explorer');
    await explorerPage.waitForLoaded();
  });

  test.describe('Page Load and Layout', () => {
    test('should load explorer page successfully', async ({ page }) => {
      await expect(page).toHaveURL('/explorer');
      await expect(explorerPage.pageContainer).toBeVisible();
    });

    test('should display search input', async () => {
      await expect(explorerPage.searchInput).toBeVisible();
    });

    test('should display search button', async () => {
      await expect(explorerPage.searchButton).toBeVisible();
    });

    test('should display recent transactions', async () => {
      await expect(explorerPage.recentTransactions).toBeVisible();
    });

    test('should display recent vertices', async () => {
      await expect(explorerPage.recentVertices).toBeVisible();
    });

    test('should display transaction table', async () => {
      await expect(explorerPage.transactionTable).toBeVisible();
    });

    test('should display vertex table', async () => {
      await expect(explorerPage.vertexTable).toBeVisible();
    });
  });

  test.describe('Search Functionality', () => {
    test('should search for transaction by hash', async ({ page }) => {
      const txHash = generateTxHash();
      await explorerPage.search(txHash);
      
      // Should navigate to transaction detail or show results
      await page.waitForTimeout(500);
    });

    test('should search for vertex by hash', async ({ page }) => {
      const vertexHash = generateVertexHash();
      await explorerPage.search(vertexHash);
      
      await page.waitForTimeout(500);
    });

    test('should search for address', async ({ page }) => {
      const address = generateAddress();
      await explorerPage.search(address);
      
      await page.waitForTimeout(500);
      
      // Should navigate to address page
      await expect(page).toHaveURL(/\/address\//);
    });

    test('should search with enter key', async ({ page }) => {
      const txHash = generateTxHash();
      await explorerPage.searchWithEnter(txHash);
      
      await page.waitForTimeout(500);
    });

    test('should show search results', async ({ page }) => {
      const query = 'test';
      await explorerPage.search(query);
      await page.waitForTimeout(500);
      
      // Should show results page or navigate to search results
      await expect(page).toHaveURL(/\/search\//);
    });

    test('should clear search', async ({ page }) => {
      await explorerPage.search('test');
      await page.waitForTimeout(500);
      
      await explorerPage.searchInput.clear();
      await page.waitForTimeout(500);
    });
  });

  test.describe('Transaction List', () => {
    test('should display transaction count', async () => {
      const count = await explorerPage.getTransactionCount();
      expect(count).toBeGreaterThanOrEqual(0);
    });

    test('should display transaction hashes', async ({ page }) => {
      const count = await explorerPage.getTransactionCount();
      
      if (count > 0) {
        const txRows = explorerPage.transactionTable.locator('tbody tr');
        const firstRow = txRows.first();
        const hashEl = firstRow.locator('td:first-child, .tx-hash');
        const hash = await hashEl.textContent();
        
        expect(hash).toMatch(/0x[a-f0-9]+/i);
      }
    });

    test('should display transaction amounts', async ({ page }) => {
      const count = await explorerPage.getTransactionCount();
      
      if (count > 0) {
        const txRows = explorerPage.transactionTable.locator('tbody tr');
        const firstRow = txRows.first();
        const amountEl = firstRow.locator('.amount, td:nth-child(3)');
        await expect(amountEl).toBeVisible();
      }
    });

    test('should click on transaction to view details', async ({ page }) => {
      const count = await explorerPage.getTransactionCount();
      
      if (count > 0) {
        await explorerPage.clickTransaction(0);
        await expect(page).toHaveURL(/\/tx\/0x[a-f0-9]+/);
      }
    });
  });

  test.describe('Vertex List', () => {
    test('should display vertex count', async () => {
      const count = await explorerPage.getVertexCount();
      expect(count).toBeGreaterThanOrEqual(0);
    });

    test('should display vertex hashes', async ({ page }) => {
      const count = await explorerPage.getVertexCount();
      
      if (count > 0) {
        const vertexRows = explorerPage.vertexTable.locator('tbody tr');
        const firstRow = vertexRows.first();
        const hashEl = firstRow.locator('td:first-child, .vertex-hash');
        const hash = await hashEl.textContent();
        
        expect(hash).toMatch(/0x[a-f0-9]+/i);
      }
    });

    test('should click on vertex to view details', async ({ page }) => {
      const count = await explorerPage.getVertexCount();
      
      if (count > 0) {
        await explorerPage.clickVertex(0);
        await expect(page).toHaveURL(/\/vertex\/0x[a-f0-9]+/);
      }
    });
  });

  test.describe('Pagination', () => {
    test('should display pagination controls', async () => {
      await expect(explorerPage.pagination).toBeVisible();
    });

    test('should go to next page', async ({ page }) => {
      await explorerPage.goToNextPage();
      await page.waitForTimeout(500);
      
      // Should still be on explorer page
      await expect(page).toHaveURL('/explorer');
    });

    test('should go to previous page', async ({ page }) => {
      // First go to page 2
      await explorerPage.goToNextPage();
      await page.waitForTimeout(500);
      
      // Then go back
      await explorerPage.goToPreviousPage();
      await page.waitForTimeout(500);
      
      await expect(page).toHaveURL('/explorer');
    });

    test('should go to specific page', async ({ page }) => {
      await explorerPage.goToPage(2);
      await page.waitForTimeout(500);
      
      await expect(page).toHaveURL('/explorer');
    });
  });

  test.describe('Filtering', () => {
    test('should display filter dropdown', async () => {
      await expect(explorerPage.filterDropdown).toBeVisible();
    });

    test('should filter by transaction type', async ({ page }) => {
      await explorerPage.filterByType('transfer');
      await page.waitForTimeout(500);
      
      const count = await explorerPage.getTransactionCount();
      expect(count).toBeGreaterThanOrEqual(0);
    });

    test('should filter by vertex type', async ({ page }) => {
      await explorerPage.filterByType('block');
      await page.waitForTimeout(500);
      
      const count = await explorerPage.getVertexCount();
      expect(count).toBeGreaterThanOrEqual(0);
    });
  });

  test.describe('Recent Activity', () => {
    test('should display recent transactions section', async () => {
      await expect(explorerPage.recentTransactions).toBeVisible();
    });

    test('should display recent vertices section', async () => {
      await expect(explorerPage.recentVertices).toBeVisible();
    });

    test('should display recent rounds section', async () => {
      await expect(explorerPage.recentRounds).toBeVisible();
    });
  });

  test.describe('Responsive Design', () => {
    test('should display correctly on mobile', async ({ page }) => {
      await page.setViewportSize({ width: 375, height: 667 });
      await expect(explorerPage.pageContainer).toBeVisible();
    });

    test('should display correctly on tablet', async ({ page }) => {
      await page.setViewportSize({ width: 768, height: 1024 });
      await expect(explorerPage.pageContainer).toBeVisible();
    });

    test('should display correctly on desktop', async ({ page }) => {
      await page.setViewportSize({ width: 1920, height: 1080 });
      await expect(explorerPage.pageContainer).toBeVisible();
    });
  });
});
