import { Page, Locator } from '@playwright/test';

/**
 * Page Object for the CopyButton component
 */
export class CopyButton {
  readonly page: Page;
  readonly button: Locator;

  constructor(page: Page, locator?: string) {
    this.page = page;
    this.button = page.locator(locator || '[data-testid="copy-button"], button.copy-button, .copy-button');
  }

  async click(): Promise<void> {
    await this.button.click();
  }

  async hover(): Promise<void> {
    await this.button.hover();
  }

  async isVisible(): Promise<boolean> {
    return this.button.isVisible();
  }

  async isDisabled(): Promise<boolean> {
    return this.button.isDisabled();
  }

  async getTooltipText(): Promise<string> {
    await this.button.hover();
    const tooltip = this.page.locator('[role="tooltip"], .tooltip');
    return (await tooltip.textContent()) || '';
  }
}
