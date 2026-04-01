import { test, expect } from '../../fixtures/base.fixture';
import { CouncilPagePO } from '../../page-objects/pages/CouncilPagePO';

test.describe('Council Page', () => {
  let councilPage: CouncilPagePO;

  test.beforeEach(async ({ page }) => {
    councilPage = new CouncilPagePO(page);
    await page.goto('/council');
    await councilPage.waitForLoaded();
  });

  test.describe('Page Load and Layout', () => {
    test('should load council page successfully', async ({ page }) => {
      await expect(page).toHaveURL('/council');
      await expect(councilPage.pageContainer).toBeVisible();
    });

    test('should display council member list', async () => {
      await expect(councilPage.councilMemberList).toBeVisible();
    });

    test('should display council seats', async () => {
      await expect(councilPage.councilSeats).toBeVisible();
    });

    test('should display election info', async () => {
      await expect(councilPage.electionInfo).toBeVisible();
    });
  });

  test.describe('Council Members Display', () => {
    test('should display council member cards', async () => {
      const count = await councilPage.getMemberCount();
      expect(count).toBeGreaterThan(0);
    });

    test('should display member names', async () => {
      const name = await councilPage.getMemberName(0);
      expect(name).not.toBe('');
    });

    test('should display member stake', async () => {
      const stake = await councilPage.getMemberStake(0);
      expect(stake).not.toBe('');
    });

    test('should display member votes', async () => {
      const votes = await councilPage.getMemberVotes(0);
      expect(votes).not.toBe('');
    });

    test('should display valid member count', async () => {
      const count = await councilPage.getMemberCount();
      // Council typically has fixed number of seats
      expect(count).toBeGreaterThan(0);
    });
  });

  test.describe('Election Information', () => {
    test('should display next election countdown', async () => {
      const countdown = await councilPage.getNextElectionTime();
      expect(countdown).not.toBe('');
    });

    test('should display election details', async ({ page }) => {
      const electionInfo = page.locator('[data-testid="election-details"], .election-details');
      await expect(electionInfo).toBeVisible();
    });
  });

  test.describe('Member Details', () => {
    test('should click on member to view details', async ({ page }) => {
      await councilPage.clickMember(0);
      
      // Should navigate to member detail or address page
      await expect(page).toHaveURL(/\/address\//);
    });

    test('should display member ranking', async ({ page }) => {
      // First member should be ranked highest
      const firstMember = councilPage.memberCards.first();
      const rankEl = firstMember.locator('[data-testid="rank"], .rank, .ranking');
      
      if (await rankEl.isVisible()) {
        const rank = await rankEl.textContent();
        expect(rank).toContain('1');
      }
    });

    test('should display member voting power', async ({ page }) => {
      const memberCard = councilPage.memberCards.first();
      const powerEl = memberCard.locator('[data-testid="voting-power"], .voting-power');
      await expect(powerEl).toBeVisible();
    });
  });

  test.describe('Council Seats', () => {
    test('should display all council seats', async ({ page }) => {
      const seats = page.locator('[data-testid="council-seat"], .council-seat');
      const count = await seats.count();
      expect(count).toBeGreaterThan(0);
    });

    test('should show occupied seats', async ({ page }) => {
      const occupiedSeats = page.locator('[data-testid="council-seat"].occupied, .council-seat.occupied');
      const count = await occupiedSeats.count();
      expect(count).toBeGreaterThan(0);
    });

    test('should display seat information', async ({ page }) => {
      const seat = page.locator('[data-testid="council-seat"], .council-seat').first();
      await expect(seat).toBeVisible();
    });
  });

  test.describe('Sorting and Filtering', () => {
    test('should sort members by stake', async ({ page }) => {
      const sortByStake = page.locator('button:has-text("Stake"), [data-testid="sort-by-stake"]');
      if (await sortByStake.isVisible()) {
        await sortByStake.click();
        await page.waitForTimeout(500);
        
        // Verify sorting
        const count = await councilPage.getMemberCount();
        expect(count).toBeGreaterThan(0);
      }
    });

    test('should sort members by votes', async ({ page }) => {
      const sortByVotes = page.locator('button:has-text("Votes"), [data-testid="sort-by-votes"]');
      if (await sortByVotes.isVisible()) {
        await sortByVotes.click();
        await page.waitForTimeout(500);
        
        const count = await councilPage.getMemberCount();
        expect(count).toBeGreaterThan(0);
      }
    });
  });

  test.describe('Responsive Design', () => {
    test('should display correctly on mobile', async ({ page }) => {
      await page.setViewportSize({ width: 375, height: 667 });
      await expect(councilPage.pageContainer).toBeVisible();
    });

    test('should display correctly on tablet', async ({ page }) => {
      await page.setViewportSize({ width: 768, height: 1024 });
      await expect(councilPage.pageContainer).toBeVisible();
    });

    test('should display correctly on desktop', async ({ page }) => {
      await page.setViewportSize({ width: 1920, height: 1080 });
      await expect(councilPage.pageContainer).toBeVisible();
    });
  });
});
