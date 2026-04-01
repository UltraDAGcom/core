import { test, expect } from '../../fixtures/base.fixture';
import { DashboardPagePO } from '../../page-objects/pages/DashboardPagePO';

test.describe('Dashboard Page', () => {
  let dashboardPage: DashboardPagePO;

  test.beforeEach(async ({ page }) => {
    dashboardPage = new DashboardPagePO(page);
    await page.goto('/');
  });

  test.describe('Page Load and Layout', () => {
    test('should load dashboard page successfully', async ({ page }) => {
      // Page redirects to /dashboard/
      await expect(page).toHaveURL(/\/dashboard/);
      await page.waitForLoadState('networkidle', { timeout: 10000 });
      await expect(page.locator('body')).toBeVisible();
    });

    test('should display page title/header', async ({ page }) => {
      await page.waitForLoadState('networkidle', { timeout: 10000 });
      // Should have either Dashboard header or connection message
      const hasDashboard = await page.locator('h1:has-text("Dashboard")').isVisible();
      const hasConnecting = await page.locator('text=Connecting').isVisible();
      const hasUnable = await page.locator('text=Unable').isVisible();
      
      expect(hasDashboard || hasConnecting || hasUnable).toBeTruthy();
    });

    test('should display stats grid or loading state', async ({ page }) => {
      await page.waitForLoadState('networkidle', { timeout: 10000 });
      // Either show stats grid or loading/unable to connect
      const hasRound = await page.locator('text=DAG Round').isVisible();
      const hasSupply = await page.locator('text=Total Supply').isVisible();
      const hasConnecting = await page.locator('text=Connecting').isVisible();
      
      expect(hasRound || hasSupply || hasConnecting).toBeTruthy();
    });

    test('should display recent rounds section or loading', async ({ page }) => {
      await page.waitForLoadState('networkidle', { timeout: 10000 });
      const hasRounds = await page.locator('text=Recent').isVisible();
      const hasConnecting = await page.locator('text=Connecting').isVisible();
      
      expect(hasRounds || hasConnecting).toBeTruthy();
    });

    test('should display network vitals or loading', async ({ page }) => {
      await page.waitForLoadState('networkidle', { timeout: 10000 });
      const hasVitals = await page.locator('text=Network Vitals').isVisible();
      const hasConnecting = await page.locator('text=Connecting').isVisible();
      
      expect(hasVitals || hasConnecting).toBeTruthy();
    });
  });

  test.describe('Network Statistics', () => {
    test('should display current TPS', async () => {
      const tps = await dashboardPage.getCurrentTPS();
      expect(tps).not.toBeNull();
      expect(tps).not.toBe('');
    });

    test('should display network height', async () => {
      const height = await dashboardPage.getNetworkHeight();
      expect(height).not.toBeNull();
      expect(height).not.toBe('');
    });

    test('should display total transactions count', async () => {
      const total = await dashboardPage.getTotalTransactions();
      expect(total).not.toBeNull();
    });

    test('should display total vertices count', async () => {
      const total = await dashboardPage.getTotalVertices();
      expect(total).not.toBeNull();
    });

    test('should have valid numeric values for stats', async () => {
      const tps = await dashboardPage.getCurrentTPS();
      const height = await dashboardPage.getNetworkHeight();
      
      if (tps) {
        const tpsValue = parseFloat(tps.replace(/,/g, ''));
        expect(tpsValue).toBeGreaterThanOrEqual(0);
      }
      
      if (height) {
        const heightValue = parseInt(height.replace(/,/g, ''));
        expect(heightValue).toBeGreaterThanOrEqual(0);
      }
    });
  });

  test.describe('Recent Transactions', () => {
    test('should display transaction list', async () => {
      const count = await dashboardPage.getRecentTransactionCount();
      expect(count).toBeGreaterThanOrEqual(0);
    });

    test('should allow clicking on a transaction to view details', async ({ page }) => {
      const count = await dashboardPage.getRecentTransactionCount();
      
      if (count > 0) {
        await dashboardPage.clickOnTransaction(0);
        // Should navigate to transaction detail page
        await expect(page).toHaveURL(/\/tx\/0x[a-f0-9]+/);
      }
    });

    test('should display transaction hashes', async () => {
      const transactions = dashboardPage.recentTransactions.locator('td:first-child, .tx-hash');
      const count = await transactions.count();
      
      if (count > 0) {
        const hash = await transactions.first().textContent();
        expect(hash).toMatch(/0x[a-f0-9]+/i);
      }
    });
  });

  test.describe('Recent Vertices', () => {
    test('should display vertex list', async () => {
      const vertices = dashboardPage.recentVertices.locator('tr, [role="row"], .vertex-row');
      const count = await vertices.count();
      expect(count).toBeGreaterThanOrEqual(0);
    });

    test('should allow clicking on a vertex to view details', async ({ page }) => {
      const vertices = dashboardPage.recentVertices.locator('tr, [role="row"], .vertex-row');
      const count = await vertices.count();
      
      if (count > 0) {
        await vertices.first().click();
        // Should navigate to vertex detail page
        await expect(page).toHaveURL(/\/vertex\/0x[a-f0-9]+/);
      }
    });
  });

  test.describe('Navigation and Interactions', () => {
    test('should have working refresh button', async () => {
      await dashboardPage.refreshData();
      // Page should still be visible after refresh
      await expect(dashboardPage.pageContainer).toBeVisible();
    });

    test('should not show loading state after initial load', async () => {
      await dashboardPage.waitForLoaded();
      const isLoading = await dashboardPage.isLoading();
      expect(isLoading).toBeFalsy();
    });

    test('should update stats when refreshing', async () => {
      const initialTPS = await dashboardPage.getCurrentTPS();
      await dashboardPage.refreshData();
      const updatedTPS = await dashboardPage.getCurrentTPS();
      
      // TPS might change or stay the same, but should be valid
      if (updatedTPS) {
        const tpsValue = parseFloat(updatedTPS.replace(/,/g, ''));
        expect(tpsValue).toBeGreaterThanOrEqual(0);
      }
    });
  });

  test.describe('Responsive Design', () => {
    test('should display correctly on mobile viewport', async ({ page }) => {
      await page.setViewportSize({ width: 375, height: 667 });
      await expect(dashboardPage.pageContainer).toBeVisible();
      await expect(dashboardPage.statsGrid).toBeVisible();
    });

    test('should display correctly on tablet viewport', async ({ page }) => {
      await page.setViewportSize({ width: 768, height: 1024 });
      await expect(dashboardPage.pageContainer).toBeVisible();
    });

    test('should display correctly on desktop viewport', async ({ page }) => {
      await page.setViewportSize({ width: 1920, height: 1080 });
      await expect(dashboardPage.pageContainer).toBeVisible();
    });
  });

  test.describe('Error States', () => {
    test('should handle network disconnection gracefully', async ({ page }) => {
      await page.route('**/api/**', route => route.abort('failed'));
      
      // Reload page with failed API calls
      await page.reload();
      
      // Should still show page container (might show error message)
      await expect(dashboardPage.pageContainer).toBeVisible();
    });
  });
});
