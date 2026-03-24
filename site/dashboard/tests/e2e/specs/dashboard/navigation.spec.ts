import { test, expect } from '../../fixtures/base.fixture';
import { SidebarPO } from '../../page-objects/components/SidebarPO';
import { TopBarPO } from '../../page-objects/components/TopBarPO';

test.describe('Navigation', () => {
  let sidebar: SidebarPO;
  let topBar: TopBarPO;

  test.beforeEach(async ({ page }) => {
    sidebar = new SidebarPO(page);
    topBar = new TopBarPO(page);
    await page.goto('/');
    await page.waitForLoadState('networkidle', { timeout: 10000 });
  });

  test.describe('Sidebar Navigation', () => {
    test('should display sidebar or navigation', async ({ page }) => {
      const hasSidebar = await sidebar.isVisible();
      const hasNavLinks = await page.locator('a[href*="/wallet"], a[href*="/explorer"]').first().isVisible();
      expect(hasSidebar || hasNavLinks).toBeTruthy();
    });

    test('should navigate to wallet page', async ({ page }) => {
      await sidebar.goToWallet();
      await page.waitForLoadState('networkidle', { timeout: 5000 });
      await expect(page).toHaveURL(/\/wallet/);
    });

    test('should navigate to explorer page', async ({ page }) => {
      await sidebar.goToExplorer();
      await page.waitForLoadState('networkidle', { timeout: 5000 });
      await expect(page).toHaveURL(/\/explorer/);
    });

    test('should navigate to staking page', async ({ page }) => {
      await sidebar.goToStaking();
      await page.waitForLoadState('networkidle', { timeout: 5000 });
      await expect(page).toHaveURL(/\/staking/);
    });

    test('should navigate to governance page', async ({ page }) => {
      await sidebar.goToGovernance();
      await page.waitForLoadState('networkidle', { timeout: 5000 });
      await expect(page).toHaveURL(/\/governance/);
    });

    test('should navigate to network page', async ({ page }) => {
      await sidebar.goToNetwork();
      await page.waitForLoadState('networkidle', { timeout: 5000 });
      await expect(page).toHaveURL(/\/network/);
    });
  });

  test.describe('Top Bar', () => {
    test('should display top bar or header', async ({ page }) => {
      const hasTopBar = await topBar.isVisible();
      const hasHeader = await page.locator('header').first().isVisible();
      expect(hasTopBar || hasHeader).toBeTruthy();
    });

    test('should display network selector or network name', async ({ page }) => {
      const hasNetwork = await page.locator('text=/testnet|mainnet|network/i').first().isVisible();
      expect(hasNetwork).toBeTruthy();
    });
  });

  test.describe('404 Page', () => {
    test('should display 404 page for non-existent routes', async ({ page }) => {
      await page.goto('/non-existent-page-xyz123');
      await page.waitForLoadState('networkidle', { timeout: 5000 });
      
      // Should show 404 or redirect to dashboard
      const url = page.url();
      const has404 = await page.locator('text=404').isVisible().catch(() => false);
      const isDashboard = url.includes('/dashboard');
      
      expect(has404 || isDashboard).toBeTruthy();
    });
  });
});
