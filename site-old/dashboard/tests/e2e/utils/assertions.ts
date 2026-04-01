import { expect, Locator, Page } from '@playwright/test';

/**
 * Custom assertion utilities for UltraDAG E2E tests
 */

/**
 * Assert that a locator is visible within a timeout
 */
export async function assertVisible(
  locator: Locator,
  timeout: number = 5000,
  message?: string
): Promise<void> {
  await expect(locator).toBeVisible({ timeout });
}

/**
 * Assert that a locator is hidden within a timeout
 */
export async function assertHidden(
  locator: Locator,
  timeout: number = 5000,
  message?: string
): Promise<void> {
  await expect(locator).toBeHidden();
}

/**
 * Assert that a locator contains specific text
 */
export async function assertText(
  locator: Locator,
  expectedText: string | RegExp,
  message?: string
): Promise<void> {
  await expect(locator).toHaveText(expectedText);
}

/**
 * Assert that a locator has a specific attribute
 */
export async function assertAttribute(
  locator: Locator,
  attribute: string,
  expectedValue: string | RegExp,
  message?: string
): Promise<void> {
  await expect(locator).toHaveAttribute(attribute, expectedValue);
}

/**
 * Assert that a locator has a specific CSS class
 */
export async function assertClass(
  locator: Locator,
  className: string,
  message?: string
): Promise<void> {
  await expect(locator).toHaveClass(new RegExp(`\\b${className}\\b`));
}

/**
 * Assert that a locator does not have a specific CSS class
 */
export async function assertNotClass(
  locator: Locator,
  className: string,
  message?: string
): Promise<void> {
  const classes = await locator.getAttribute('class');
  expect(classes).not.toContain(className);
}

/**
 * Assert that a locator is enabled
 */
export async function assertEnabled(
  locator: Locator,
  message?: string
): Promise<void> {
  await expect(locator).toBeEnabled();
}

/**
 * Assert that a locator is disabled
 */
export async function assertDisabled(
  locator: Locator,
  message?: string
): Promise<void> {
  await expect(locator).toBeDisabled();
}

/**
 * Assert that a locator is checked
 */
export async function assertChecked(
  locator: Locator,
  message?: string
): Promise<void> {
  await expect(locator).toBeChecked();
}

/**
 * Assert that a locator is not checked
 */
export async function assertNotChecked(
  locator: Locator,
  message?: string
): Promise<void> {
  await expect(locator).not.toBeChecked();
}

/**
 * Assert that a page URL matches a pattern
 */
export async function assertUrl(
  page: Page,
  expectedUrl: string | RegExp,
  message?: string
): Promise<void> {
  await expect(page).toHaveURL(expectedUrl);
}

/**
 * Assert that a locator exists in the DOM
 */
export async function assertExists(
  locator: Locator,
  message?: string
): Promise<void> {
  await expect(locator).toBeAttached();
}

/**
 * Assert that a locator does not exist in the DOM
 */
export async function assertNotExists(
  locator: Locator,
  message?: string
): Promise<void> {
  await expect(locator).not.toBeAttached();
}

/**
 * Assert that a locator has a specific count
 */
export async function assertCount(
  locator: Locator,
  expectedCount: number,
  message?: string
): Promise<void> {
  await expect(locator).toHaveCount(expectedCount);
}

/**
 * Assert that a value is within a range
 */
export function assertInRange(
  actual: number,
  min: number,
  max: number,
  message?: string
): void {
  expect(actual).toBeGreaterThanOrEqual(min);
  expect(actual).toBeLessThanOrEqual(max);
}

/**
 * Assert that two values are approximately equal
 */
export function assertApproximately(
  actual: number,
  expected: number,
  tolerance: number = 0.01,
  message?: string
): void {
  expect(Math.abs(actual - expected)).toBeLessThanOrEqual(tolerance);
}

/**
 * Assert that a list contains specific items
 */
export async function assertListContains(
  locator: Locator,
  expectedItems: string[],
  message?: string
): Promise<void> {
  const items = await locator.allTextContents();
  for (const item of expectedItems) {
    expect(items).toContainEqual(expect.stringContaining(item));
  }
}

/**
 * Assert that a table has specific column headers
 */
export async function assertTableHeaders(
  tableLocator: Locator,
  expectedHeaders: string[],
  message?: string
): Promise<void> {
  const headers = tableLocator.locator('th, thead td, [role="columnheader"]');
  const headerTexts = await headers.allTextContents();
  expect(headerTexts).toEqual(expectedHeaders);
}

/**
 * Assert that a modal/dialog is open
 */
export async function assertModalOpen(
  page: Page,
  modalLocator?: Locator,
  message?: string
): Promise<void> {
  const modal = modalLocator || page.locator('[role="dialog"], .modal, [data-testid="modal"]');
  await expect(modal).toBeVisible();
}

/**
 * Assert that a modal/dialog is closed
 */
export async function assertModalClosed(
  page: Page,
  modalLocator?: Locator,
  message?: string
): Promise<void> {
  const modal = modalLocator || page.locator('[role="dialog"], .modal, [data-testid="modal"]');
  await expect(modal).toBeHidden();
}

/**
 * Assert that a toast notification appears with specific message
 */
export async function assertToast(
  page: Page,
  expectedMessage: string | RegExp,
  type?: 'success' | 'error' | 'warning' | 'info'
): Promise<void> {
  const toast = page.locator('[data-testid="toast"], .toast, [role="alert"]');
  await expect(toast).toBeVisible();
  await expect(toast).toHaveText(expectedMessage);
  
  if (type) {
    await expect(toast).toHaveClass(new RegExp(`\\b${type}\\b`));
  }
}

/**
 * Assert that a loading spinner is visible
 */
export async function assertLoading(
  page: Page,
  loaderLocator?: Locator
): Promise<void> {
  const loader = loaderLocator || page.locator('[data-testid="loading"], .loading, .spinner, [role="progressbar"]');
  await expect(loader).toBeVisible();
}

/**
 * Assert that a loading spinner is hidden
 */
export async function assertNotLoading(
  page: Page,
  loaderLocator?: Locator
): Promise<void> {
  const loader = loaderLocator || page.locator('[data-testid="loading"], .loading, .spinner, [role="progressbar"]');
  await expect(loader).toBeHidden();
}
