// PLAN-RETOURS-12 — Chief Engineer feedback of 2026-08-28.
//
// R1: an account added while Wind is open was called
// "Disconnected" in Settings — the core did set the session at
// add time (commands.rs, add_oauth_account), but `accountAdded()` never
// refreshed the `connected` array, filled only once
// at startup. The decor replays the root cause: the account is in the database and
// its session lives on the core side, the UI doesn't know it yet.
//
// OAuth consent is not drivable by Playwright: the
// `__e2eAdd` seam (transport.js, __e2eAttachments pattern) makes the
// add succeed without a browser and has the address carried by the outcome of
// `connect_accounts` — set AFTER startup, it doesn't touch the
// initial connection.
import { test, expect } from '@playwright/test';
import { launchAppV2, closeApp } from '../launch.mjs';

let app;
let browser;
let page;

test.describe.configure({ mode: 'serial' });

test.beforeAll(async () => {
  ({ app, browser, page } = await launchAppV2({
    accounts: [
      { email: 'principal@exemple.fr', messages: 4 },
      { email: 'neuf@gmail.com', messages: 2, disconnected: true },
    ],
  }));
});

test.afterAll(async () => {
  await closeApp({ app, browser });
});

test('an account added while Wind is open is said to be connected in Settings, without a restart', async () => {
  await expect(page.locator('[data-testid="row"]').first()).toBeVisible();
  await page.locator('[data-testid="settings"]').click();

  // The decor states the bug in potential: the account's session is not
  // yet known to the UI.
  const rows = page.locator('[data-testid="settings-accounts"]');
  await expect(rows).toContainText('neuf@gmail.com');
  await expect(page.locator('[data-testid="account-disconnected"]')).toHaveCount(1);

  // The add, through the real Settings desk — the seam only replaces
  // the browser consent. The log proves the UI truly REREADS
  // the core (the outcome is completed by the seam: without this
  // proof, the badge could fall for the wrong reason).
  await page.evaluate(() => {
    window.__e2eAdd = ['neuf@gmail.com'];
    window.__e2eLog = [];
  });
  await page.locator('[data-testid="settings-add"]').click();
  await page.locator('[data-testid="onboarding-address"]').fill('neuf@gmail.com');
  await page.locator('[data-testid="settings-desk"] [data-testid="desk-continue"]').click();

  // What the user MUST see: no more "Disconnected" — the
  // account has just been connected.
  await expect(page.locator('[data-testid="account-disconnected"]')).toHaveCount(0);
  const rereads = await page.evaluate(() => {
    const n = window.__e2eLog.filter((r) => r.command === 'connect_accounts').length;
    // The seam doesn't survive the test: the decor's subsequent cycles
    // go back through the real path.
    delete window.__e2eAdd;
    delete window.__e2eLog;
    return n;
  });
  expect(rereads).toBeGreaterThanOrEqual(1);
});
