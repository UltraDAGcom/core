import { Page, Locator } from '@playwright/test';

/**
 * Page Object for the Bridge page
 */
export class BridgePagePO {
  readonly page: Page;
  readonly pageContainer: Locator;
  readonly fromChainSelector: Locator;
  readonly toChainSelector: Locator;
  readonly amountInput: Locator;
  readonly bridgeButton: Locator;
  readonly bridgeHistory: Locator;
  readonly pendingBridges: Locator;
  readonly completedBridges: Locator;

  constructor(page: Page) {
    this.page = page;
    this.pageContainer = page.locator('[data-testid="bridge-page"], .bridge-page');
    this.fromChainSelector = page.locator('[data-testid="from-chain"], select[name="fromChain"], button:has-text("From")');
    this.toChainSelector = page.locator('[data-testid="to-chain"], select[name="toChain"], button:has-text("To")');
    this.amountInput = page.locator('[data-testid="amount-input"], input[name="amount"], input[type="number"], input[placeholder*="amount"]');
    this.bridgeButton = page.locator('[data-testid="bridge-button"], button:has-text("Bridge"), button[type="submit"]');
    this.bridgeHistory = page.locator('[data-testid="bridge-history"], .bridge-history');
    this.pendingBridges = this.bridgeHistory.locator('[data-status="pending"], .status-pending');
    this.completedBridges = this.bridgeHistory.locator('[data-status="completed"], .status-completed');
  }

  async selectFromChain(chain: string): Promise<void> {
    await this.fromChainSelector.click();
    await this.page.locator(`li:has-text("${chain}"), option[value="${chain}"]`).click();
  }

  async selectToChain(chain: string): Promise<void> {
    await this.toChainSelector.click();
    await this.page.locator(`li:has-text("${chain}"), option[value="${chain}"]`).click();
  }

  async fillAmount(amount: string): Promise<void> {
    await this.amountInput.fill(amount);
  }

  async initiateBridge(): Promise<void> {
    await this.bridgeButton.click();
  }

  async getBridgeCount(): Promise<number> {
    const bridges = this.bridgeHistory.locator('[data-testid="bridge-item"], .bridge-item, tr');
    return bridges.count();
  }

  async getPendingBridgeCount(): Promise<number> {
    return this.pendingBridges.count();
  }

  async getCompletedBridgeCount(): Promise<number> {
    return this.completedBridges.count();
  }

  async isVisible(): Promise<boolean> {
    return this.pageContainer.isVisible();
  }

  async waitForLoaded(): Promise<void> {
    await this.pageContainer.waitFor({ state: 'visible' });
  }
}
