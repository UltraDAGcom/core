import { Page, Locator } from '@playwright/test';

/**
 * Page Object for the Staking page
 */
export class StakingPagePO {
  readonly page: Page;
  readonly pageContainer: Locator;
  readonly validatorList: Locator;
  readonly myStakesSection: Locator;
  readonly stakeForm: Locator;
  readonly validatorCards: Locator;
  readonly searchInput: Locator;
  readonly sortByStake: Locator;
  readonly sortByApy: Locator;

  constructor(page: Page) {
    this.page = page;
    this.pageContainer = page.locator('[data-testid="staking-page"], .staking-page');
    this.validatorList = page.locator('[data-testid="validator-list"], .validator-list');
    this.myStakesSection = page.locator('[data-testid="my-stakes"], .my-stakes');
    this.stakeForm = page.locator('[data-testid="stake-form"], .stake-form');
    this.validatorCards = this.validatorList.locator('[data-testid="validator-card"], .validator-card, .validator-item');
    this.searchInput = page.locator('[data-testid="search-validator"], input[placeholder*="search"], input[type="search"]');
    this.sortByStake = page.locator('[data-testid="sort-by-stake"], button:has-text("Stake")');
    this.sortByApy = page.locator('[data-testid="sort-by-apy"], button:has-text("APY")');
  }

  async searchValidator(name: string): Promise<void> {
    await this.searchInput.fill(name);
  }

  async clickValidator(index: number = 0): Promise<void> {
    const card = this.validatorCards.nth(index);
    await card.click();
  }

  async clickValidatorByName(name: string): Promise<void> {
    const card = this.validatorCards.filter({ hasText: name });
    await card.click();
  }

  async stake(amount: string, validatorIndex: number = 0): Promise<void> {
    const card = this.validatorCards.nth(validatorIndex);
    const stakeButton = card.locator('button:has-text("Stake"), button:has-text("Delegate")');
    await stakeButton.click();
    
    // Fill stake amount in modal
    await this.page.locator('input[type="number"], input[name="amount"], input[placeholder*="amount"]').fill(amount);
    
    // Confirm
    await this.page.locator('button:has-text("Confirm"), button:has-text("Stake"), button[type="submit"]').click();
  }

  async unstake(validatorIndex: number = 0): Promise<void> {
    const card = this.validatorCards.nth(validatorIndex);
    const unstakeButton = card.locator('button:has-text("Unstake"), button:has-text("Withdraw")');
    if (await unstakeButton.isVisible()) {
      await unstakeButton.click();
      await this.page.locator('button:has-text("Confirm"), button:has-text("Unstake")').click();
    }
  }

  async getValidatorCount(): Promise<number> {
    return this.validatorCards.count();
  }

  async getValidatorName(index: number = 0): Promise<string> {
    const card = this.validatorCards.nth(index);
    const nameEl = card.locator('[data-testid="validator-name"], .validator-name, h3, .name');
    return nameEl.textContent() || '';
  }

  async getValidatorApy(index: number = 0): Promise<string> {
    const card = this.validatorCards.nth(index);
    const apyEl = card.locator('[data-testid="validator-apy"], .validator-apy, .apy');
    return apyEl.textContent() || '';
  }

  async getValidatorStake(index: number = 0): Promise<string> {
    const card = this.validatorCards.nth(index);
    const stakeEl = card.locator('[data-testid="validator-stake"], .validator-stake, .stake');
    return stakeEl.textContent() || '';
  }

  async getMyStakedAmount(): Promise<string> {
    const amountEl = this.myStakesSection.locator('[data-testid="staked-amount"], .staked-amount, .amount');
    return (await amountEl.textContent()) || '';
  }

  async isVisible(): Promise<boolean> {
    return this.pageContainer.isVisible();
  }

  async waitForLoaded(): Promise<void> {
    await this.pageContainer.waitFor({ state: 'visible' });
  }
}
