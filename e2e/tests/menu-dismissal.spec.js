// PLAN-SWEEP-2026-09 — beta wave 1, backlog 91 and 105: the product
// menu (Menu.svelte, D-47 family) must close on an outside click, and
// a second click on its own trigger must close it, not reopen it.
// Two screens share the component; the beta hit both — the repro
// covers both so the fix is proven in the component, not in a page.
import { test, expect } from '@playwright/test';
import { launchAppV2, closeApp } from '../launch.mjs';

let app;
let browser;
let page;

test.describe.configure({ mode: 'serial' });

test.beforeAll(async () => {
  ({ app, browser, page } = await launchAppV2({
    accounts: [{ email: 'principal@exemple.fr', messages: 6 }],
  }));
  await expect(page.locator('[data-testid="row"]').first()).toBeVisible();
  await page.locator('[data-testid="organized-mode"]').click();
  await page.locator('[data-testid="nav-folder"][data-category="cleanup"]').click();
  await page.locator('[data-testid="cleanup-range"][data-range="all"]').click();
  await page.locator('[data-testid="cleanup-start"]').click();
  await expect(page.locator('[data-testid="cleanup-group"]').first()).toBeVisible();
});

test.afterAll(async () => {
  await closeApp({ app, browser });
});

test('cleanup ⋯: a click outside the menu closes it (backlog 91)', async () => {
  await page.locator('[data-testid="cleanup-mini-yes"]').first().click();
  await expect(page.locator('[data-testid="cleanup-menu"]')).toBeVisible();
  // Anywhere that is neither the menu nor its trigger — the tester
  // clicked the page background.
  await page.locator('[data-testid="cleanup-progress"]').click();
  await expect(page.locator('[data-testid="cleanup-menu"]')).toHaveCount(0);
});

test('cleanup ⋯: a second click on the trigger closes, never reopens', async () => {
  const trigger = page.locator('[data-testid="cleanup-mini-yes"]').first();
  await trigger.click();
  await expect(page.locator('[data-testid="cleanup-menu"]')).toBeVisible();
  await trigger.click();
  await expect(page.locator('[data-testid="cleanup-menu"]')).toHaveCount(0);
});

test('settings › screener Edit: a click outside the menu closes it (backlog 105)', async () => {
  // A Yes on the first group records a screener decision — the
  // Settings list then has a row carrying the Edit trigger.
  await page.locator('[data-testid="cleanup-yes"]').first().click();
  await page.locator('[data-testid="settings"]').click();
  await page.locator('[data-testid="settings-group"][data-group="screener"]').click();
  await page.locator('[data-testid="decision-edit"]').first().click();
  await expect(page.locator('[data-testid="decision-menu"]')).toBeVisible();
  await page.locator('[data-testid="screener-search"]').click();
  await expect(page.locator('[data-testid="decision-menu"]')).toHaveCount(0);
});

test('settings › screener Edit: a second click on the trigger closes, never reopens', async () => {
  const trigger = page.locator('[data-testid="decision-edit"]').first();
  await trigger.click();
  await expect(page.locator('[data-testid="decision-menu"]')).toBeVisible();
  await trigger.click();
  await expect(page.locator('[data-testid="decision-menu"]')).toHaveCount(0);
});
