// PLAN-SWEEP-2026-09 — backlog 103: in Settings › Screener, the
// verdict is the information the reader came for; a long sender
// address must never eat it. The verdict stays visible, inside the
// row, whatever the address length — the address is the part that
// truncates.
import { test, expect } from '@playwright/test';
import { launchAppV2, closeApp, injectArrival } from '../launch.mjs';

let app;
let browser;
let page;

const LONG_SENDER =
  'no-reply.notifications.eu-west-3.long-service-name.production@subdomain.exemple.fr';

test.describe.configure({ mode: 'serial' });

test.beforeAll(async () => {
  ({ app, browser, page } = await launchAppV2({
    accounts: [{ email: 'principal@exemple.fr', messages: 2 }],
  }));
  await expect(page.locator('[data-testid="row"]').first()).toBeVisible();
  // Organized FIRST: only then does an unknown sender wait at the desk.
  await page.locator('[data-testid="organized-mode"]').click();
  await expect(page.locator('[data-testid="nav-folder"][data-category="screener"]')).toBeVisible();
  // The desk only counts arrivals STRICTLY after the activation epoch
  // (seconds): an arrival within the same second is treated as
  // pre-mode mail. Cross the second boundary before injecting.
  await page.waitForTimeout(1100);
  injectArrival({ email: 'principal@exemple.fr', sender: LONG_SENDER });
  await page.reload();
  await page.locator('[data-testid="nav-folder"][data-category="screener"]').click();
  await page.locator('[data-testid="screener-rank"]', { hasText: 'subdomain.exemple.fr' })
    .locator('[data-testid="screener-yes"]').click();
});

test.afterAll(async () => {
  await closeApp({ app, browser });
});

test('an 80-character address never masks the verdict (backlog 103)', async () => {
  await page.locator('[data-testid="settings"]').click();
  await page.locator('[data-testid="settings-group"][data-group="screener"]').click();
  const row = page.locator('[data-testid="screener-decision"]', { hasText: 'subdomain.exemple.fr' });
  await expect(row).toHaveCount(1);
  const verdict = row.locator('.verdict');
  await expect(verdict).toBeVisible();
  // Visible for real: the verdict's box sits fully INSIDE the row's
  // box — not pushed past the clip by the address.
  const rowBox = await row.boundingBox();
  const vBox = await verdict.boundingBox();
  expect(vBox.width).toBeGreaterThan(10);
  expect(vBox.x + vBox.width).toBeLessThanOrEqual(rowBox.x + rowBox.width + 1);
  // And the address itself truncates rather than wrapping or pushing.
  await expect(row.locator('.address-rule')).toHaveCSS('text-overflow', 'ellipsis');
});
