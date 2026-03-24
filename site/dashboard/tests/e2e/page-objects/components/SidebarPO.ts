import { Page, Locator } from '@playwright/test';

/**
 * Page Object for the Sidebar navigation component
 */
export class SidebarPO {
  readonly page: Page;
  readonly sidebar: Locator;
  readonly navLinks: Locator;
  readonly logoLink: Locator;

  constructor(page: Page) {
    this.page = page;
    // Sidebar uses 'aside' element with flex layout
    this.sidebar = page.locator('aside').first();
    this.navLinks = this.sidebar.locator('a');
    this.logoLink = this.sidebar.locator('a').first();
  }

  async goToDashboard() {
    await this.page.locator('a[href="/"]').first().click();
  }

  async goToWallet() {
    const link = this.sidebar.locator('a').filter({ hasText: /wallet/i }).first();
    if (await link.isVisible()) {
      await link.click();
    } else {
      await this.page.locator('a[href*="/wallet"]').first().click();
    }
  }

  async goToPortfolio() {
    const link = this.sidebar.locator('a').filter({ hasText: /portfolio/i }).first();
    if (await link.isVisible()) {
      await link.click();
    } else {
      await this.page.locator('a[href*="/portfolio"]').first().click();
    }
  }

  async goToSend() {
    const link = this.sidebar.locator('a').filter({ hasText: /send/i }).first();
    if (await link.isVisible()) {
      await link.click();
    } else {
      await this.page.locator('a[href*="/send"]').first().click();
    }
  }

  async goToBridge() {
    const link = this.sidebar.locator('a').filter({ hasText: /bridge/i }).first();
    if (await link.isVisible()) {
      await link.click();
    } else {
      await this.page.locator('a[href*="/bridge"]').first().click();
    }
  }

  async goToStaking() {
    const link = this.sidebar.locator('a').filter({ hasText: /staking/i }).first();
    if (await link.isVisible()) {
      await link.click();
    } else {
      await this.page.locator('a[href*="/staking"]').first().click();
    }
  }

  async goToGovernance() {
    const link = this.sidebar.locator('a').filter({ hasText: /governance/i }).first();
    if (await link.isVisible()) {
      await link.click();
    } else {
      await this.page.locator('a[href*="/governance"]').first().click();
    }
  }

  async goToCouncil() {
    const link = this.sidebar.locator('a').filter({ hasText: /council/i }).first();
    if (await link.isVisible()) {
      await link.click();
    } else {
      await this.page.locator('a[href*="/council"]').first().click();
    }
  }

  async goToExplorer() {
    const link = this.sidebar.locator('a').filter({ hasText: /explorer/i }).first();
    if (await link.isVisible()) {
      await link.click();
    } else {
      await this.page.locator('a[href*="/explorer"]').first().click();
    }
  }

  async goToNetwork() {
    const link = this.sidebar.locator('a').filter({ hasText: /network/i }).first();
    if (await link.isVisible()) {
      await link.click();
    } else {
      await this.page.locator('a[href*="/network"]').first().click();
    }
  }

  async isVisible(): Promise<boolean> {
    return this.sidebar.isVisible();
  }

  async isActiveLink(path: string): Promise<boolean> {
    const activeLink = this.sidebar.locator(`a[href*="${path}"][class*="active"], a[href*="${path}"][aria-current="page"]`);
    return activeLink.isVisible();
  }
}
