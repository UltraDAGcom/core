import { test as base, expect, Page } from '@playwright/test';
import { SidebarPO } from '../page-objects/components/SidebarPO';
import { TopBarPO } from '../page-objects/components/TopBarPO';
import { WalletSelectorPO } from '../page-objects/components/WalletSelectorPO';

/**
 * Base test fixture that extends Playwright's base test
 * Provides common functionality for all tests
 */
export interface TestFixtures {
  sidebar: SidebarPO;
  topBar: TopBarPO;
  walletSelector: WalletSelectorPO;
  navigateTo: (path: string) => Promise<void>;
}

export const test = base.extend<TestFixtures>({
  sidebar: async ({ page }, use) => {
    const sidebar = new SidebarPO(page);
    await use(sidebar);
  },

  topBar: async ({ page }, use) => {
    const topBar = new TopBarPO(page);
    await use(topBar);
  },

  walletSelector: async ({ page }, use) => {
    const walletSelector = new WalletSelectorPO(page);
    await use(walletSelector);
  },

  navigateTo: async ({ page }, use) => {
    const navigateTo = async (path: string) => {
      await page.goto(path);
    };
    await use(navigateTo);
  },
});

export { expect };
