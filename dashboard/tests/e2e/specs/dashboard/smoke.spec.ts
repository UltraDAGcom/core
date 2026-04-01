import { test, expect } from '../../fixtures/base.fixture';

test.describe('Dashboard Smoke Tests', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto('/');
  });

  test.describe('Basic Page Load', () => {
    test('should redirect to dashboard URL', async ({ page }) => {
      // Wait for redirect
      await page.waitForURL(/\/dashboard/, { timeout: 10000 });
      await expect(page).toHaveURL(/\/dashboard/);
    });

    test('should have valid page title', async ({ page }) => {
      await page.waitForURL(/\/dashboard/, { timeout: 10000 });
      const title = await page.title();
      expect(title.length).toBeGreaterThan(0);
    });

    test('should have React root element', async ({ page }) => {
      await page.waitForURL(/\/dashboard/, { timeout: 10000 });
      const root = page.locator('#root');
      await expect(root).toBeAttached();
    });

    test('should load without critical errors', async ({ page }) => {
      await page.waitForURL(/\/dashboard/, { timeout: 10000 });
      
      let errorCount = 0;
      page.on('console', msg => {
        if (msg.type() === 'error' && !msg.text().includes('Failed to fetch')) {
          errorCount++;
        }
      });
      
      await page.waitForTimeout(2000);
      expect(errorCount).toBe(0);
    });
  });
});
