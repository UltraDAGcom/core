import { test, expect } from '../../fixtures/base.fixture';

test.describe('Dashboard Page', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto('/');
    await page.waitForLoadState('domcontentloaded', { timeout: 10000 });
  });

  test.describe('Page Load', () => {
    test('should redirect to dashboard page', async ({ page }) => {
      // Page redirects from / to /dashboard/
      await expect(page).toHaveURL(/\/dashboard/);
    });

    test('should have a title', async ({ page }) => {
      const title = await page.title();
      expect(title).not.toBe('');
    });

    test('should load without critical JavaScript errors', async ({ page }) => {
      // Check for console errors (excluding expected network errors)
      let hasCriticalError = false;
      page.on('console', msg => {
        if (msg.type() === 'error' && !msg.text().includes('Failed to fetch')) {
          hasCriticalError = true;
        }
      });
      
      // Wait a bit for any errors to appear
      await page.waitForTimeout(1000);
      expect(hasCriticalError).toBeFalsy();
    });
  });

  test.describe('Layout', () => {
    test('should have root element', async ({ page }) => {
      // React app should have #root
      const root = page.locator('#root');
      await expect(root).toBeAttached();
    });

    test('should have sidebar navigation', async ({ page }) => {
      // Sidebar uses 'aside' element
      const hasAside = await page.locator('aside').isVisible();
      const hasNavLinks = await page.locator('a[href*="/wallet"], a[href*="/explorer"], a[href*="/staking"]').count() > 0;
      
      expect(hasAside || hasNavLinks).toBeTruthy();
    });

    test('should have top bar header', async ({ page }) => {
      // TopBar uses 'header' element
      const hasHeader = await page.locator('header').isVisible();
      expect(hasHeader).toBeTruthy();
    });
  });

  test.describe('Dashboard Content', () => {
    test('should display dashboard heading', async ({ page }) => {
      const heading = page.locator('h1, h2').first();
      const text = await heading.textContent();
      expect(text?.toLowerCase()).toMatch(/dashboard|connecting|unable/);
    });

    test('should display metric cards or loading state', async ({ page }) => {
      // Look for metric/stat cards or loading indicators
      const hasCards = await page.locator('[class*="card"], div:has-text("DAG Round"), div:has-text("Total Supply")').count() > 0;
      const hasLoading = await page.locator('text=Connecting').isVisible();
      const hasError = await page.locator('text=Unable').isVisible();
      
      expect(hasCards || hasLoading || hasError).toBeTruthy();
    });

    test('should display network information or loading', async ({ page }) => {
      // Should show some network data or loading state
      const hasData = await page.locator('text=/validators|peers|treasury|supply|round/i').isVisible();
      const hasLoading = await page.locator('text=Connecting').isVisible();
      
      expect(hasData || hasLoading).toBeTruthy();
    });
  });
});
