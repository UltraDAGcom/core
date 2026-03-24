import { Page, Locator } from '@playwright/test';

/**
 * Page Object for the Council page
 */
export class CouncilPagePO {
  readonly page: Page;
  readonly pageContainer: Locator;
  readonly councilMemberList: Locator;
  readonly councilSeats: Locator;
  readonly electionInfo: Locator;
  readonly memberCards: Locator;
  readonly nextElectionCountdown: Locator;

  constructor(page: Page) {
    this.page = page;
    this.pageContainer = page.locator('[data-testid="council-page"], .council-page');
    this.councilMemberList = page.locator('[data-testid="council-members"], .council-members');
    this.councilSeats = page.locator('[data-testid="council-seats"], .council-seats');
    this.electionInfo = page.locator('[data-testid="election-info"], .election-info');
    this.memberCards = this.councilMemberList.locator('[data-testid="council-member"], .council-member, .member-card');
    this.nextElectionCountdown = page.locator('[data-testid="election-countdown"], .election-countdown');
  }

  async getMemberCount(): Promise<number> {
    return this.memberCards.count();
  }

  async getMemberName(index: number = 0): Promise<string> {
    const card = this.memberCards.nth(index);
    const nameEl = card.locator('[data-testid="member-name"], .member-name, h3, .name');
    return nameEl.textContent() || '';
  }

  async getMemberStake(index: number = 0): Promise<string> {
    const card = this.memberCards.nth(index);
    const stakeEl = card.locator('[data-testid="member-stake"], .member-stake, .stake');
    return stakeEl.textContent() || '';
  }

  async getMemberVotes(index: number = 0): Promise<string> {
    const card = this.memberCards.nth(index);
    const votesEl = card.locator('[data-testid="member-votes"], .member-votes, .votes');
    return votesEl.textContent() || '';
  }

  async getNextElectionTime(): Promise<string> {
    return (await this.nextElectionCountdown.textContent()) || '';
  }

  async clickMember(index: number = 0): Promise<void> {
    const card = this.memberCards.nth(index);
    await card.click();
  }

  async isVisible(): Promise<boolean> {
    return this.pageContainer.isVisible();
  }

  async waitForLoaded(): Promise<void> {
    await this.pageContainer.waitFor({ state: 'visible' });
  }
}
