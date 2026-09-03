// Removing an account from Settings: the gesture is destructive
// locally (local mail erased, connection forgotten — never the
// server), so it CONFIRMS in place, and everything that showed the
// account collapses: the Settings row, the nav mailbox, the list.
//
// Standalone suite: it MUTILATES its decor (an account disappears for
// good) — sharing it with another serialized suite would make
// their assertions depend on execution order.
import { test, expect } from '@playwright/test';
import { launchAppV2, closeApp } from '../launch.mjs';

let app;
let browser;
let page;

test.describe.configure({ mode: 'serial' });

test.beforeAll(async () => {
  ({ app, browser, page } = await launchAppV2({
    accounts: [
      { email: 'un@exemple.fr', messages: 6 },
      { email: 'deux@exemple.fr', messages: 4 },
    ],
  }));
});

test.afterAll(async () => {
  await closeApp({ app, browser });
});

test('removal confirms — and cancelling touches nothing', async () => {
  await expect(page.locator('[data-testid="row"]').first()).toBeVisible();
  // A80/D7: TWO accounts, so accounts mix — each
  // row states its mailbox. The removal, below, will make one disappear:
  // that's the counterpart to this assertion.
  await expect(page.locator('[data-testid="row-mailbox"]').first()).toBeVisible();
  // 6 + 4 messages, one thread out of five (uid 5 replies to 4): 5 + 4
  // conversations. The total left the nav (A29, W2-D4) — it's read
  // on the perf line of the status bar.
  await expect(page.locator('[data-testid="perf"]')).toContainText('9 conversations');
  await page.locator('[data-testid="settings"]').click();
  await expect(page.locator('[data-testid="account-remove"]')).toHaveCount(2);

  // PLAN-RETOURS-9 (D2): the gesture is STATED — the icon carries its text,
  // in the product's vocabulary ("remove", nothing is deleted
  // from the server).
  await expect(page.locator('[data-testid="account-remove"]').first()).toContainText(
    'Remove account',
  );

  // First click: the confirmation card, not the removal.
  await page.locator('[data-testid="account-remove"]').first().click();
  await expect(page.locator('[data-testid="settings-removal"]')).toBeVisible();
  await expect(page.locator('[data-testid="settings-removal"]')).toContainText(
    'un@exemple.fr',
  );
  await expect(page.locator('[data-testid="settings-removal"]')).toContainText(
    'Nothing is deleted on the server',
  );

  // Cancel: the card collapses, both accounts are still there.
  await page.locator('[data-testid="removal-cancel"]').click();
  await expect(page.locator('[data-testid="settings-removal"]')).toHaveCount(0);
  await expect(page.locator('[data-testid="account-remove"]')).toHaveCount(2);
  await page.locator('[data-testid="settings-done"]').click();
});

test('confirmed: the account leaves Settings, the nav and the list', async () => {
  await page.locator('[data-testid="settings"]').click();

  // The second account (deux@exemple.fr) leaves; the first stays.
  await page.locator('[data-testid="account-remove"]').nth(1).click();
  await expect(page.locator('[data-testid="settings-removal"]')).toContainText(
    'deux@exemple.fr',
  );
  await page.locator('[data-testid="removal-confirm"]').click();

  await expect(page.locator('[data-testid="toast"]')).toContainText('Account removed.');
  await expect(page.locator('[data-testid="account-remove"]')).toHaveCount(1);
  await expect(page.locator('[data-testid="settings-accounts"]')).not.toContainText(
    'deux@exemple.fr',
  );
  await page.locator('[data-testid="settings-done"]').click();

  // The nav: "All mailboxes" + the one remaining account.
  await expect(page.locator('[data-testid="nav-mailbox"]')).toHaveCount(2);
  await expect(page.locator('[data-testid="nav"]')).not.toContainText('deux@exemple.fr');

  // The unified list no longer shows anything but the remaining account's mail:
  // the 5 conversations of un@exemple.fr, plus the 4 gone (the
  // account of the rendered rows is authoritative — the list fits on one page).
  await expect(page.locator('[data-testid="row"]')).toHaveCount(5);

  // A80/D7 (2026-08-25 review): only ONE account remains — accounts
  // no longer mix, and "on <its own address>" on
  // every row would be the refrain D7 refuses. The rule therefore bears
  // on the NUMBER of accounts, not just the chosen view: that's
  // what this assertion holds, and the unified mailbox is indeed the
  // current view here.
  await expect(page.locator('[data-testid="row-mailbox"]')).toHaveCount(0);
});
