// PLAN-RETOURS-9 (D3/D4): an account's custom name. Set
// from Settings > Accounts (card under the row, marker pattern), it
// REPLACES the address in the nav; in Settings it displays WITH
// the address; in the composer the selector says "Name — address"
// (the address remains the functional sending data). Cleared, the address
// comes back everywhere. Decor: two accounts — the name only makes sense
// when it distinguishes.
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

const navMailbox = (label) =>
  page.locator('[data-testid="nav-mailbox"]', { hasText: label });

test('naming an account from Settings: the nav AND the mailbox block take the name', async () => {
  await expect(page.locator('[data-testid="row"]').first()).toBeVisible();
  await expect(navMailbox('un@exemple.fr')).toHaveCount(1);

  await page.locator('[data-testid="settings"]').click();
  await page.locator('[data-testid="account-rename"]').first().click();
  await expect(page.locator('[data-testid="settings-name"]')).toBeVisible();
  await page.locator('[data-testid="name-field"]').fill('Boulot');
  await page.locator('[data-testid="name-save"]').click();
  await expect(page.locator('[data-testid="settings-name"]')).toHaveCount(0);

  // In Settings, the name displays WITH the address (D4) — the address
  // remains the connection's source of truth.
  const row = page.locator('[data-testid="settings-accounts"] .account').first();
  await expect(row).toContainText('Boulot');
  await expect(row).toContainText('un@exemple.fr');
  await page.locator('[data-testid="settings-done"]').click();

  // The nav: the name REPLACES the address; the other account doesn't move.
  await expect(navMailbox('Boulot')).toHaveCount(1);
  await expect(navMailbox('un@exemple.fr')).toHaveCount(0);
  await expect(navMailbox('deux@exemple.fr')).toHaveCount(1);

  // A80/D8: the ROW states the mailbox in full, and the custom
  // name is its label — it's the only `names[…]` branch
  // of `mailboxOf`, the one the unnamed decor never reaches.
  const named = page
    .locator('[data-testid="row-mailbox"]', { hasText: 'Boulot' })
    .first();
  await expect(named).toBeVisible();
  await expect(named.locator('.lbl')).toHaveText('Boulot');
  // The tooltip keeps the address: it remains the technical truth (A78).
  // RETOURS-14 R3 (D4): "Name (address)" — no more em dash.
  await expect(named).toHaveAttribute('title', 'Boulot (un@exemple.fr)');

  // The UNNAMED account falls back to its address — and its tooltip only
  // says it ONCE: "address — address" would be a stutter
  // (2026-08-25 review).
  const anonymous = page
    .locator('[data-testid="row-mailbox"]', { hasText: 'deux@exemple.fr' })
    .first();
  await expect(anonymous).toHaveAttribute('title', 'deux@exemple.fr');
});

test('in the composer, the sender selector says "Name (address)"', async () => {
  await page.locator('[data-testid="write"]').click();
  const from = page.locator('select[data-testid="compose-from"]');
  await expect(from.locator('option').first()).toHaveText('Boulot (un@exemple.fr)');
  await expect(from.locator('option').nth(1)).toHaveText('deux@exemple.fr');
  // Close the composer (empty: nothing to keep) — its overlay
  // would otherwise intercept the next test's clicks.
  await page.locator('[data-testid="compose"] button[aria-label="Fermer"]').click();
  await expect(page.locator('[data-testid="compose"]')).toHaveCount(0);
});

test('clearing the name returns the address to the nav', async () => {
  await page.locator('[data-testid="settings"]').click();
  await page.locator('[data-testid="account-rename"]').first().click();
  await page.locator('[data-testid="name-field"]').fill('');
  await page.locator('[data-testid="name-save"]').click();
  // Wait for the card to close (the write went through) before
  // closing the overlay — otherwise a slow name_set dies in an
  // opaque timeout on the nav.
  await expect(page.locator('[data-testid="settings-name"]')).toHaveCount(0);
  await page.locator('[data-testid="settings-done"]').click();

  await expect(navMailbox('un@exemple.fr')).toHaveCount(1);
  await expect(navMailbox('Boulot')).toHaveCount(0);
});
