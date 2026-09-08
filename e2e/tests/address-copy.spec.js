// PLAN-SWEEP-2026-09 — backlog 110 (beta, Mona): copying a sender's
// address was impossible — the whole message header is one toggle
// target, a selection dies as a click. The address is now a trigger:
// its menu offers "Copy the address", the toast says it happened.
import { test, expect } from '@playwright/test';
import { launchAppV2, closeApp } from '../launch.mjs';

let app;
let browser;
let page;

test.describe.configure({ mode: 'serial' });

test.beforeAll(async () => {
  ({ app, browser, page } = await launchAppV2({
    accounts: [{ email: 'principal@exemple.fr', messages: 3 }],
  }));
  await expect(page.locator('[data-testid="row"]').first()).toBeVisible();
  await page.locator('[data-testid="row"]').first().click();
  await expect(page.locator('[data-testid="message-expanded"]').first()).toBeVisible();
});

test.afterAll(async () => {
  await closeApp({ app, browser });
});

test('the sender address opens its menu; Copy says so and closes (backlog 110)', async () => {
  const address = page.locator('[data-testid="sender-address"]').first();
  await expect(address).toBeVisible();
  await address.click();
  await expect(page.locator('[data-testid="address-menu"]')).toBeVisible();
  // The message must NOT have collapsed: the trigger owns its click.
  await expect(page.locator('[data-testid="message-expanded"]').first()).toBeVisible();
  await page.locator('[data-testid="address-copy"]').click();
  await expect(page.locator('[data-testid="toast"]')).toContainText('Address copied');
  await expect(page.locator('[data-testid="address-menu"]')).toHaveCount(0);
});

test('a second click on the address closes the menu, never reopens', async () => {
  const address = page.locator('[data-testid="sender-address"]').first();
  await address.click();
  await expect(page.locator('[data-testid="address-menu"]')).toBeVisible();
  await address.click();
  await expect(page.locator('[data-testid="address-menu"]')).toHaveCount(0);
});
