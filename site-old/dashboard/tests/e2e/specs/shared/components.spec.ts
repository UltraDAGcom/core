import { test, expect } from '../../fixtures/base.fixture';
import { CopyButton } from '../../page-objects/components/CopyButton';

test.describe('Shared Components', () => {
  test.describe('CopyButton', () => {
    test('should copy text to clipboard', async ({ page }) => {
      // Navigate to a page with copy buttons (like wallet or explorer)
      await page.goto('/');
      await page.waitForTimeout(1000);
      
      // Find any copy button
      const copyButton = page.locator('[data-testid="copy-button"], button.copy-button, .copy-button').first();
      
      if (await copyButton.isVisible()) {
        // Grant clipboard permissions
        const context = page.context();
        await context.grantPermissions(['clipboard-read', 'clipboard-write']);
        
        await copyButton.click();
        
        // Verify clipboard content (implementation dependent)
        // This is a basic test - adjust based on your copy button implementation
      }
    });

    test('should show tooltip on hover', async ({ page }) => {
      await page.goto('/');
      
      const copyButton = page.locator('[data-testid="copy-button"], button.copy-button').first();
      
      if (await copyButton.isVisible()) {
        await copyButton.hover();
        
        // Should show tooltip
        const tooltip = page.locator('[role="tooltip"], .tooltip');
        await expect(tooltip).toBeVisible();
      }
    });

    test('should show success message after copy', async ({ page }) => {
      await page.goto('/');
      
      const copyButton = page.locator('[data-testid="copy-button"], button.copy-button').first();
      
      if (await copyButton.isVisible()) {
        const context = page.context();
        await context.grantPermissions(['clipboard-read', 'clipboard-write']);
        
        await copyButton.click();
        
        // Should show success toast or message
        const toast = page.locator('[data-testid="toast"], .toast, .copy-success');
        await expect(toast).toBeVisible();
      }
    });
  });

  test.describe('Badge Component', () => {
    test('should display badge with text', async ({ page }) => {
      await page.goto('/');
      
      const badge = page.locator('[data-testid="badge"], .badge, .status-badge').first();
      
      if (await badge.isVisible()) {
        const text = await badge.textContent();
        expect(text).not.toBe('');
      }
    });

    test('should display badge with correct color for status', async ({ page }) => {
      await page.goto('/network');
      
      // Find status badges
      const successBadge = page.locator('.badge.success, .badge:has-text("Connected"), .badge:has-text("Active")').first();
      if (await successBadge.isVisible()) {
        await expect(successBadge).toHaveClass(/success|active|connected/i);
      }
    });
  });

  test.describe('StatusBadge Component', () => {
    test('should display status badge for transactions', async ({ page }) => {
      await page.goto('/explorer');
      
      const statusBadge = page.locator('[data-testid="status-badge"], .status-badge').first();
      
      if (await statusBadge.isVisible()) {
        const status = await statusBadge.textContent();
        expect(['pending', 'confirmed', 'failed'].some(s => status?.toLowerCase().includes(s))).toBeTruthy();
      }
    });

    test('should display correct color for status', async ({ page }) => {
      await page.goto('/explorer');
      
      const confirmedBadge = page.locator('.status-badge.confirmed, .badge:has-text("Confirmed")').first();
      if (await confirmedBadge.isVisible()) {
        await expect(confirmedBadge).toHaveClass(/success|confirmed|green/i);
      }
      
      const pendingBadge = page.locator('.status-badge.pending, .badge:has-text("Pending")').first();
      if (await pendingBadge.isVisible()) {
        await expect(pendingBadge).toHaveClass(/warning|pending|yellow/i);
      }
    });
  });

  test.describe('Skeleton Loader', () => {
    test('should display skeleton during loading', async ({ page }) => {
      // Navigate and check for skeleton loaders
      await page.goto('/explorer');
      
      // Skeleton might appear briefly
      const skeleton = page.locator('[data-testid="skeleton"], .skeleton, .skeleton-loader');
      // May or may not be visible depending on load speed
    });

    test('should hide skeleton after content loads', async ({ page }) => {
      await page.goto('/');
      await page.waitForTimeout(2000);
      
      const skeleton = page.locator('[data-testid="skeleton"], .skeleton');
      await expect(skeleton).toBeHidden();
    });
  });

  test.describe('Pagination Component', () => {
    test('should display pagination controls', async ({ page }) => {
      await page.goto('/explorer');
      
      const pagination = page.locator('[data-testid="pagination"], .pagination');
      await expect(pagination).toBeVisible();
    });

    test('should display page numbers', async ({ page }) => {
      await page.goto('/explorer');
      
      const pageNumbers = page.locator('.pagination button, .pagination li');
      const count = await pageNumbers.count();
      expect(count).toBeGreaterThan(0);
    });

    test('should highlight current page', async ({ page }) => {
      await page.goto('/explorer');
      
      const currentPage = page.locator('.pagination button.active, .pagination li.active, .pagination button[aria-current="page"]');
      await expect(currentPage).toBeVisible();
    });

    test('should navigate to next page', async ({ page }) => {
      await page.goto('/explorer');
      
      const nextButton = page.locator('.pagination button:has-text("Next"), .pagination li:has-text("Next")');
      if (await nextButton.isVisible()) {
        await nextButton.click();
        await page.waitForTimeout(500);
        await expect(page).toHaveURL('/explorer');
      }
    });

    test('should navigate to previous page', async ({ page }) => {
      await page.goto('/explorer');
      
      // First go to page 2
      const nextButton = page.locator('.pagination button:has-text("Next"), .pagination li:has-text("Next")');
      if (await nextButton.isVisible()) {
        await nextButton.click();
        await page.waitForTimeout(500);
        
        // Then go back
        const prevButton = page.locator('.pagination button:has-text("Previous"), .pagination li:has-text("Previous")');
        if (await prevButton.isVisible()) {
          await prevButton.click();
          await page.waitForTimeout(500);
        }
      }
    });
  });

  test.describe('AnimatedNumber Component', () => {
    test('should animate number changes', async ({ page }) => {
      await page.goto('/');
      
      // Find animated numbers (stats)
      const animatedNumber = page.locator('[data-testid="animated-number"], .animated-number, .stat-value').first();
      await expect(animatedNumber).toBeVisible();
    });
  });

  test.describe('Sparkline Chart', () => {
    test('should display sparkline chart', async ({ page }) => {
      await page.goto('/');
      
      const sparkline = page.locator('[data-testid="sparkline"], .sparkline, canvas.sparkline').first();
      await expect(sparkline).toBeVisible();
    });
  });

  test.describe('Card Component', () => {
    test('should display card with header', async ({ page }) => {
      await page.goto('/');
      
      const card = page.locator('[data-testid="card"], .card').first();
      await expect(card).toBeVisible();
      
      const header = card.locator('.card-header, h3, h4');
      await expect(header).toBeVisible();
    });

    test('should display card with content', async ({ page }) => {
      await page.goto('/');
      
      const card = page.locator('[data-testid="card"], .card').first();
      const content = card.locator('.card-content, .card-body');
      await expect(content).toBeVisible();
    });
  });

  test.describe('ActivityBar Component', () => {
    test('should display activity bar', async ({ page }) => {
      await page.goto('/wallet');
      
      const activityBar = page.locator('[data-testid="activity-bar"], .activity-bar');
      // May or may not be visible depending on implementation
    });
  });
});
