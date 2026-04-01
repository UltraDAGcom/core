import { Page, Locator } from '@playwright/test';

/**
 * Page Object for the Governance page
 */
export class GovernancePagePO {
  readonly page: Page;
  readonly pageContainer: Locator;
  readonly proposalList: Locator;
  readonly activeProposals: Locator;
  readonly pastProposals: Locator;
  readonly createProposalButton: Locator;
  readonly proposalCards: Locator;
  readonly votingPowerDisplay: Locator;

  constructor(page: Page) {
    this.page = page;
    this.pageContainer = page.locator('[data-testid="governance-page"], .governance-page');
    this.proposalList = page.locator('[data-testid="proposal-list"], .proposal-list');
    this.activeProposals = page.locator('[data-testid="active-proposals"], .active-proposals');
    this.pastProposals = page.locator('[data-testid="past-proposals"], .past-proposals');
    this.createProposalButton = page.locator('[data-testid="create-proposal"], button:has-text("Create Proposal"), button:has-text("New Proposal")');
    this.proposalCards = this.proposalList.locator('[data-testid="proposal-card"], .proposal-card, .proposal-item');
    this.votingPowerDisplay = page.locator('[data-testid="voting-power"], .voting-power');
  }

  async getProposalCount(): Promise<number> {
    return this.proposalCards.count();
  }

  async getActiveProposalCount(): Promise<number> {
    const cards = this.activeProposals.locator('[data-testid="proposal-card"], .proposal-card');
    return cards.count();
  }

  async getPastProposalCount(): Promise<number> {
    const cards = this.pastProposals.locator('[data-testid="proposal-card"], .proposal-card');
    return cards.count();
  }

  async clickProposal(index: number = 0): Promise<void> {
    const card = this.proposalCards.nth(index);
    await card.click();
  }

  async clickProposalByTitle(title: string): Promise<void> {
    const card = this.proposalCards.filter({ hasText: title });
    await card.click();
  }

  async voteOnProposal(proposalIndex: number, vote: 'for' | 'against' | 'abstain'): Promise<void> {
    const card = this.proposalCards.nth(proposalIndex);
    const voteButton = card.locator(`button:has-text("${vote.charAt(0).toUpperCase() + vote.slice(1)}"), button:has-text("${vote.toUpperCase()}")`);
    await voteButton.click();
    
    // Confirm vote
    await this.page.locator('button:has-text("Confirm Vote"), button:has-text("Submit Vote"), button[type="submit"]').click();
  }

  async getVotingPower(): Promise<string> {
    return (await this.votingPowerDisplay.textContent()) || '';
  }

  async createProposal(title: string, description: string): Promise<void> {
    await this.createProposalButton.click();
    
    // Fill form in modal
    await this.page.locator('input[name="title"], input[placeholder*="title"]').fill(title);
    await this.page.locator('textarea[name="description"], textarea[placeholder*="description"]').fill(description);
    
    // Submit
    await this.page.locator('button:has-text("Create"), button:has-text("Submit"), button[type="submit"]').click();
  }

  async isVisible(): Promise<boolean> {
    return this.pageContainer.isVisible();
  }

  async waitForLoaded(): Promise<void> {
    await this.pageContainer.waitFor({ state: 'visible' });
  }
}
