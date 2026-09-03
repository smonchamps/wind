// Reconnecting an account with a dead token (field finding 2026-08-20):
// `invalid_grant` (expired or revoked token) left the user
// stranded — the slot said "Account not reconnected" but no action
// relaunched the consent flow. Now the state is SEEN in Settings
// (link_off + "Disconnected") and is repaired in place ("Reconnect").
//
// The decor: two accounts in the paper trail, one of them WITHOUT a session
// (the launcher's `disconnected: true` — seeded in the database, absent from
// WIND_E2E_ACCOUNT). The e2e isolation purges the OAuth configuration: the
// consent flow cannot start, which makes the FAILURE deterministic —
// that's what we verify (stated in place, action replayable); success
// requires a real browser, that belongs to the field.
import { test, expect } from '@playwright/test';
import { launchAppV2, closeApp } from '../launch.mjs';

let app;
let browser;
let page;

test.describe.configure({ mode: 'serial' });

test.beforeAll(async () => {
  ({ app, browser, page } = await launchAppV2({
    accounts: [
      { email: 'sain@exemple.fr', messages: 4 },
      { email: 'mort@exemple.fr', messages: 3, disconnected: true },
    ],
  }));
});

test.afterAll(async () => {
  await closeApp({ app, browser });
});

test('a dead token is SEEN in Settings — the healthy account, meanwhile, does not change', async () => {
  await expect(page.locator('[data-testid="row"]').first()).toBeVisible();
  await page.locator('[data-testid="settings"]').click();

  const rows = page.locator('[data-testid="settings-accounts"]');
  await expect(rows).toContainText('sain@exemple.fr');
  await expect(rows).toContainText('mort@exemple.fr');
  // ONE single "Disconnected" state, one single "Reconnect" action — on the
  // right row.
  await expect(page.locator('[data-testid="account-disconnected"]')).toHaveCount(1);
  await expect(page.locator('[data-testid="account-reconnect"]')).toHaveCount(1);
  const dead = rows.locator('div.account', { hasText: 'mort@exemple.fr' });
  await expect(dead.locator('[data-testid="account-disconnected"]')).toContainText('Déconnecté');
  const healthy = rows.locator('div.account', { hasText: 'sain@exemple.fr' });
  await expect(healthy.locator('[data-testid="account-disconnected"]')).toHaveCount(0);
});

test('the reconnection failure is stated IN PLACE, and the action can be replayed', async () => {
  await page.locator('[data-testid="account-reconnect"]').click();
  await expect(page.locator('[data-testid="reconnection-error"]')).toBeVisible();
  // The button comes back: the failure is not a dead end.
  await expect(page.locator('[data-testid="account-reconnect"]')).toBeEnabled();
});
