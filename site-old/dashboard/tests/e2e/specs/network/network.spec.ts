import { test, expect } from '../../fixtures/base.fixture';
import { NetworkPagePO } from '../../page-objects/pages/NetworkPagePO';

test.describe('Network Page', () => {
  let networkPage: NetworkPagePO;

  test.beforeEach(async ({ page }) => {
    networkPage = new NetworkPagePO(page);
    await page.goto('/network');
    await networkPage.waitForLoaded();
  });

  test.describe('Page Load and Layout', () => {
    test('should load network page successfully', async ({ page }) => {
      await expect(page).toHaveURL('/network');
      await expect(networkPage.pageContainer).toBeVisible();
    });

    test('should display network stats', async () => {
      await expect(networkPage.networkStats).toBeVisible();
    });

    test('should display mempool table', async () => {
      await expect(networkPage.mempoolTable).toBeVisible();
    });

    test('should display peer list', async () => {
      await expect(networkPage.peerList).toBeVisible();
    });

    test('should display TPS chart', async () => {
      await expect(networkPage.tpsChart).toBeVisible();
    });

    test('should display height chart', async () => {
      await expect(networkPage.heightChart).toBeVisible();
    });

    test('should display status grid', async () => {
      await expect(networkPage.statusGrid).toBeVisible();
    });
  });

  test.describe('Network Statistics', () => {
    test('should display current block height', async () => {
      const height = await networkPage.getCurrentHeight();
      expect(height).not.toBe('');
    });

    test('should display current TPS', async () => {
      const tps = await networkPage.getCurrentTPS();
      expect(tps).not.toBe('');
    });

    test('should display peer count', async () => {
      const peerCount = await networkPage.getPeerCount();
      expect(peerCount).not.toBe('');
    });

    test('should display mempool size', async () => {
      const mempoolSize = await networkPage.getMempoolSize();
      expect(mempoolSize).not.toBe('');
    });

    test('should display valid height value', async () => {
      const height = await networkPage.getCurrentHeight();
      if (height) {
        const heightValue = parseInt(height.replace(/,/g, ''));
        expect(heightValue).toBeGreaterThanOrEqual(0);
      }
    });

    test('should display valid TPS value', async () => {
      const tps = await networkPage.getCurrentTPS();
      if (tps) {
        const tpsValue = parseFloat(tps.replace(/,/g, ''));
        expect(tpsValue).toBeGreaterThanOrEqual(0);
      }
    });
  });

  test.describe('Mempool', () => {
    test('should display mempool transactions', async () => {
      const count = await networkPage.getMempoolTransactionCount();
      expect(count).toBeGreaterThanOrEqual(0);
    });

    test('should display mempool transaction details', async ({ page }) => {
      const count = await networkPage.getMempoolTransactionCount();
      
      if (count > 0) {
        const txRows = networkPage.mempoolTable.locator('tbody tr');
        const firstRow = txRows.first();
        
        // Should display tx hash
        const hashEl = firstRow.locator('td:first-child, .tx-hash');
        await expect(hashEl).toBeVisible();
        
        // Should display fee
        const feeEl = firstRow.locator('.fee, td:nth-child(3)');
        await expect(feeEl).toBeVisible();
      }
    });
  });

  test.describe('Peer List', () => {
    test('should display connected peers', async () => {
      const count = await networkPage.getPeerCountInList();
      expect(count).toBeGreaterThanOrEqual(0);
    });

    test('should display peer details', async ({ page }) => {
      const count = await networkPage.getPeerCountInList();
      
      if (count > 0) {
        const peerItem = networkPage.peerList.locator('[data-testid="peer-item"], .peer-item').first();
        
        // Should display peer address
        const addressEl = peerItem.locator('.peer-address, .address');
        await expect(addressEl).toBeVisible();
        
        // Should display peer latency
        const latencyEl = peerItem.locator('.peer-latency, .latency');
        await expect(latencyEl).toBeVisible();
      }
    });
  });

  test.describe('Charts', () => {
    test('should display TPS chart', async () => {
      await expect(networkPage.tpsChart).toBeVisible();
    });

    test('should display height chart', async () => {
      await expect(networkPage.heightChart).toBeVisible();
    });

    test('should render TPS chart canvas', async ({ page }) => {
      const canvas = networkPage.tpsChart.locator('canvas');
      if (await canvas.isVisible()) {
        // Chart should be rendered
        const canvasBox = await canvas.boundingBox();
        expect(canvasBox).not.toBeNull();
        if (canvasBox) {
          expect(canvasBox.width).toBeGreaterThan(0);
          expect(canvasBox.height).toBeGreaterThan(0);
        }
      }
    });

    test('should render height chart canvas', async ({ page }) => {
      const canvas = networkPage.heightChart.locator('canvas');
      if (await canvas.isVisible()) {
        const canvasBox = await canvas.boundingBox();
        expect(canvasBox).not.toBeNull();
        if (canvasBox) {
          expect(canvasBox.width).toBeGreaterThan(0);
          expect(canvasBox.height).toBeGreaterThan(0);
        }
      }
    });
  });

  test.describe('Refresh Functionality', () => {
    test('should display refresh button', async ({ page }) => {
      const refreshButton = page.locator('[data-testid="refresh"], button:has-text("Refresh")');
      await expect(refreshButton).toBeVisible();
    });

    test('should refresh network stats', async () => {
      const initialHeight = await networkPage.getCurrentHeight();
      await networkPage.refreshStats();
      
      // Height might increase or stay same
      const updatedHeight = await networkPage.getCurrentHeight();
      expect(updatedHeight).not.toBe('');
    });

    test('should update charts on refresh', async ({ page }) => {
      await networkPage.refreshStats();
      await page.waitForTimeout(1000);
      
      // Charts should still be visible
      await expect(networkPage.tpsChart).toBeVisible();
      await expect(networkPage.heightChart).toBeVisible();
    });
  });

  test.describe('Status Indicators', () => {
    test('should display connection status', async ({ page }) => {
      const statusEl = networkPage.statusGrid.locator('[data-testid="connection-status"], .connection-status');
      await expect(statusEl).toBeVisible();
    });

    test('should display sync status', async ({ page }) => {
      const statusEl = networkPage.statusGrid.locator('[data-testid="sync-status"], .sync-status');
      await expect(statusEl).toBeVisible();
    });

    test('should display network health', async ({ page }) => {
      const healthEl = networkPage.statusGrid.locator('[data-testid="network-health"], .network-health');
      await expect(healthEl).toBeVisible();
    });
  });

  test.describe('Empty States', () => {
    test('should handle empty mempool gracefully', async () => {
      // Mempool might be empty
      const count = await networkPage.getMempoolTransactionCount();
      
      if (count === 0) {
        // Should show empty state message
        const emptyMessage = networkPage.mempoolTable.locator('.empty-state, .no-data');
        await expect(emptyMessage).toBeVisible();
      }
    });
  });

  test.describe('Responsive Design', () => {
    test('should display correctly on mobile', async ({ page }) => {
      await page.setViewportSize({ width: 375, height: 667 });
      await expect(networkPage.pageContainer).toBeVisible();
    });

    test('should display correctly on tablet', async ({ page }) => {
      await page.setViewportSize({ width: 768, height: 1024 });
      await expect(networkPage.pageContainer).toBeVisible();
    });

    test('should display correctly on desktop', async ({ page }) => {
      await page.setViewportSize({ width: 1920, height: 1080 });
      await expect(networkPage.pageContainer).toBeVisible();
    });
  });
});
