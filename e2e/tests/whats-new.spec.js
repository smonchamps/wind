// PLAN-BATCH-2026-09 E2: the "What's new" window. Shown ONCE, on the
// first launch after an update — never on a fresh install (the seen
// version is seeded silently), never twice (Continue acknowledges).
// The seam is plain IPC: `whats_new_ack` rewinds the seen version the
// way an update leaves it behind.
import { test, expect } from '@playwright/test';
import { launchAppV2, closeApp } from '../launch.mjs';

let app;
let browser;
let page;

test.describe.configure({ mode: 'serial' });

test.beforeAll(async () => {
  ({ app, browser, page } = await launchAppV2({
    accounts: [{ email: 'principal@exemple.fr', messages: 1 }],
  }));
});

test.afterAll(async () => {
  await closeApp({ app, browser });
});

test('a fresh database never opens the window (first run seeds silently)', async () => {
  await expect(page.locator('[data-testid="row"]').first()).toBeVisible();
  await expect(page.locator('[data-testid="whats-new-modal"]')).toHaveCount(0);
});

test("the first launch after an update shows the new version's notes, once", async () => {
  // The state an update leaves behind: the seen version is an old one.
  await page.evaluate(() =>
    window.__TAURI__.core.invoke('whats_new_ack', { version: '0.1.0' }));
  await page.reload();
  await expect(page.locator('[data-testid="whats-new-modal"]')).toBeVisible();
  const version = await page.evaluate(() =>
    window.__TAURI__.core.invoke('app_version'));
  await expect(page.locator('[data-testid="whats-new-title"]')).toContainText(version);
  // The window never opens empty: the release net (whats_new.rs)
  // guarantees the running version has an entry — the modal must
  // carry it.
  const notes = await page.locator('[data-testid="whats-new-notes"]').innerText();
  expect(notes.trim().length).toBeGreaterThan(0);
  // Continue closes and ACKNOWLEDGES: the window does not come back.
  await page.locator('[data-testid="whats-new-continue"]').click();
  await expect(page.locator('[data-testid="whats-new-modal"]')).toHaveCount(0);
  await page.reload();
  await expect(page.locator('[data-testid="row"]').first()).toBeVisible();
  await expect(page.locator('[data-testid="whats-new-modal"]')).toHaveCount(0);
});
