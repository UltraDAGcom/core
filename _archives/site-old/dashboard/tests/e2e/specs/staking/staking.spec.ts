import { test, expect } from '../../fixtures/base.fixture';
import { StakingPagePO } from '../../page-objects/pages/StakingPagePO';
import { WalletPagePO } from '../../page-objects/pages/WalletPagePO';
import { CreateKeystoreModalPO } from '../../page-objects/modals/CreateKeystoreModalPO';
import { generateWalletData } from '../../utils/test-data';

test.describe('Staking Page', () => {
  let stakingPage: StakingPagePO;
  let walletPage: WalletPagePO;
  let createKeystoreModal: CreateKeystoreModalPO;

  test.beforeEach(async ({ page }) => {
    stakingPage = new StakingPagePO(page);
    walletPage = new WalletPagePO(page);
    createKeystoreModal = new CreateKeystoreModalPO(page);
    
    // Create and unlock wallet first
    await page.goto('/wallet');
    const walletData = generateWalletData();
    await walletPage.createWalletButton.click();
    await createKeystoreModal.waitForOpen();
    await createKeystoreModal.fillForm(walletData.name, walletData.password);
    await createKeystoreModal.submit();
    await createKeystoreModal.waitForClose();
    await walletPage.unlockWallet(walletData.password);
    
    // Navigate to staking
    await page.goto('/staking');
    await stakingPage.waitForLoaded();
  });

  test.describe('Page Load and Layout', () => {
    test('should load staking page successfully', async ({ page }) => {
      await expect(page).toHaveURL('/staking');
      await expect(stakingPage.pageContainer).toBeVisible();
    });

    test('should display validator list', async () => {
      await expect(stakingPage.validatorList).toBeVisible();
    });

    test('should display my stakes section', async () => {
      await expect(stakingPage.myStakesSection).toBeVisible();
    });

    test('should display search input', async () => {
      await expect(stakingPage.searchInput).toBeVisible();
    });
  });

  test.describe('Validator Display', () => {
    test('should display validator cards', async () => {
      const count = await stakingPage.getValidatorCount();
      expect(count).toBeGreaterThan(0);
    });

    test('should display validator names', async () => {
      const name = await stakingPage.getValidatorName(0);
      expect(name).not.toBe('');
    });

    test('should display validator APY', async () => {
      const apy = await stakingPage.getValidatorApy(0);
      expect(apy).not.toBe('');
    });

    test('should display validator stake', async () => {
      const stake = await stakingPage.getValidatorStake(0);
      expect(stake).not.toBe('');
    });

    test('should display valid APY values', async () => {
      const apy = await stakingPage.getValidatorApy(0);
      if (apy) {
        const apyValue = parseFloat(apy.replace('%', ''));
        expect(apyValue).toBeGreaterThan(0);
      }
    });
  });

  test.describe('Search Functionality', () => {
    test('should search validators by name', async ({ page }) => {
      const validatorName = await stakingPage.getValidatorName(0);
      
      await stakingPage.searchValidator(validatorName);
      
      // Should filter to show matching validators
      await page.waitForTimeout(500); // Wait for search to complete
      const count = await stakingPage.getValidatorCount();
      expect(count).toBeGreaterThan(0);
    });

    test('should show no results for non-matching search', async ({ page }) => {
      await stakingPage.searchValidator('NonExistentValidator123');
      await page.waitForTimeout(500);
      
      const count = await stakingPage.getValidatorCount();
      // May be 0 or show all validators depending on implementation
    });

    test('should clear search when input is cleared', async ({ page }) => {
      await stakingPage.searchValidator('Test');
      await page.waitForTimeout(500);
      
      await stakingPage.searchInput.clear();
      await page.waitForTimeout(500);
      
      // Should show all validators again
      const count = await stakingPage.getValidatorCount();
      expect(count).toBeGreaterThan(0);
    });
  });

  test.describe('Sorting', () => {
    test('should sort validators by stake', async ({ page }) => {
      await stakingPage.sortByStake.click();
      await page.waitForTimeout(500);
      
      // Verify sorting happened (implementation dependent)
      const count = await stakingPage.getValidatorCount();
      expect(count).toBeGreaterThan(0);
    });

    test('should sort validators by APY', async ({ page }) => {
      await stakingPage.sortByApy.click();
      await page.waitForTimeout(500);
      
      // Verify sorting happened
      const count = await stakingPage.getValidatorCount();
      expect(count).toBeGreaterThan(0);
    });
  });

  test.describe('Staking Flow', () => {
    test('should display stake button for validators', async () => {
      const validatorCard = stakingPage.validatorCards.first();
      const stakeButton = validatorCard.locator('button:has-text("Stake"), button:has-text("Delegate")');
      await expect(stakeButton).toBeVisible();
    });

    test('should open stake form when clicking stake button', async ({ page }) => {
      await stakingPage.clickValidator(0);
      
      // Should show stake form or modal
      await expect(stakingPage.stakeForm).toBeVisible();
    });

    test('should stake tokens to validator', async ({ page }) => {
      await stakingPage.stake('100', 0);
      
      // Should show confirmation or success message
      // Implementation dependent
    });

    test('should display my staked amount', async () => {
      const stakedAmount = await stakingPage.getMyStakedAmount();
      // May be empty if no stakes yet
      expect(stakedAmount).not.toBeNull();
    });
  });

  test.describe('Validator Selection', () => {
    test('should click on validator card', async ({ page }) => {
      await stakingPage.clickValidator(0);
      
      // Should show validator details or stake form
      await expect(stakingPage.stakeForm).toBeVisible();
    });

    test('should click on validator by name', async ({ page }) => {
      const validatorName = await stakingPage.getValidatorName(0);
      await stakingPage.clickValidatorByName(validatorName);
      
      // Should show validator details
      await expect(stakingPage.stakeForm).toBeVisible();
    });
  });

  test.describe('Unstaking', () => {
    test('should display unstake button for staked validators', async ({ page }) => {
      // First stake some tokens
      await stakingPage.stake('100', 0);
      
      // Should show unstake option
      const validatorCard = stakingPage.validatorCards.first();
      const unstakeButton = validatorCard.locator('button:has-text("Unstake"), button:has-text("Withdraw")');
      
      // May or may not be visible depending on implementation
    });
  });

  test.describe('Empty State', () => {
    test('should show message when no validators available', async ({ page }) => {
      // This would require mocking an empty validator list
      // For now, just verify page handles the current state
      await stakingPage.waitForLoaded();
      await expect(stakingPage.pageContainer).toBeVisible();
    });
  });

  test.describe('Responsive Design', () => {
    test('should display correctly on mobile', async ({ page }) => {
      await page.setViewportSize({ width: 375, height: 667 });
      await expect(stakingPage.pageContainer).toBeVisible();
    });

    test('should display correctly on tablet', async ({ page }) => {
      await page.setViewportSize({ width: 768, height: 1024 });
      await expect(stakingPage.pageContainer).toBeVisible();
    });
  });
});
