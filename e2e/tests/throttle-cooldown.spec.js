import { test, expect } from '@playwright/test';
import { launchAppV2, closeApp } from '../launch.mjs';

// PLAN-THROTTLE-2026-09 E4 — an account told "not now" by its server is
// SAID: the progress line names the account and the resume time, and
// Settings says it under the account. The cooldown itself cannot be
// provoked here (it needs Gmail's real throttle — a refusal of the plan:
// it would lock the Chief Engineer's account for up to 24 h); the seam
// injects what the core would report.
const clock = (epoch) => {
  const at = new Date(epoch * 1000);
  return `${String(at.getHours()).padStart(2, '0')}:${String(at.getMinutes()).padStart(2, '0')}`;
};

let app, browser, page;
test.beforeAll(async ({}, info) => {
  info.setTimeout(180000);
  ({ app, browser, page } = await launchAppV2());
});
test.afterAll(async () => { await closeApp({ app, browser }); });

test('a throttled account says so in the progress line and in Settings, then falls silent', async ({}, testInfo) => {
  const until = await page.evaluate(async () => {
    const nav = await window.__TAURI__.core.invoke('nav_snapshot');
    const until = Math.floor(Date.now() / 1000) + 3600;
    window.__e2eCooldowns = [{ account_id: nav[0].account_id, until, kind: 'throttle' }];
    return until;
  });
  const hhmm = clock(until);

  const progress = page.locator('[data-testid="progress"]');
  await expect(progress).toContainText('limiting downloads', { timeout: 12000 });
  await expect(progress).toContainText(hhmm);
  // The waiting state is not an alert: nothing is asked of the user.
  await expect(page.locator('[data-testid="status"] .alert-dot')).toHaveCount(0);

  await page.locator('[data-testid="settings"]').click();
  const line = page.locator('[data-testid="account-cooldown"]');
  await expect(line).toContainText('limiting downloads');
  await expect(line).toContainText(hhmm);
  await page.screenshot({ path: testInfo.outputPath('throttle-cooldown.png') });

  await page.evaluate(() => { window.__e2eCooldowns = []; });
  await expect(line).toHaveCount(0, { timeout: 12000 });
  await page.keyboard.press('Escape');
  await expect(progress).not.toContainText('limiting downloads', { timeout: 12000 });
  await page.evaluate(() => { delete window.__e2eCooldowns; });
});

test('a spent daily download budget pauses the catch-up until tomorrow, and says so', async () => {
  // The same channel as the throttle (review, altitude 6): computed per
  // account on every status probe, so the line refreshes at midnight
  // without a pump running, and Settings carries it too.
  const until = await page.evaluate(async () => {
    const nav = await window.__TAURI__.core.invoke('nav_snapshot');
    const until = Math.floor(Date.now() / 1000) + 3600;
    window.__e2eCooldowns = [{ account_id: nav[0].account_id, until, kind: 'daily_budget' }];
    return until;
  });
  const hhmm = clock(until);
  const progress = page.locator('[data-testid="progress"]');
  await expect(progress).toContainText('daily download limit', { timeout: 12000 });
  await expect(progress).toContainText(hhmm);
  await expect(page.locator('[data-testid="status"] .alert-dot')).toHaveCount(0);

  await page.locator('[data-testid="settings"]').click();
  const line = page.locator('[data-testid="account-cooldown"]');
  await expect(line).toContainText('download limit');
  await expect(line).toContainText(hhmm);
  await page.keyboard.press('Escape');

  await page.evaluate(() => { delete window.__e2eCooldowns; });
  await expect(progress).not.toContainText('daily download limit', { timeout: 12000 });
});
