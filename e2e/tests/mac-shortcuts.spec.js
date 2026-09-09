// Backlog 109 — the shortcuts read the platform (first mac field
// report): on macOS, ⌫ (Backspace) deletes — the key Mail.app users
// expect, "Suppr" does not exist on their keyboard — and the Settings
// reference shows the mac labels (⌫, ⌘Space). On Windows nothing
// moves: Backspace stays inert, the labels stay "Del"/"Ctrl+Space".
//
// The platform probe carries an e2e seam (`window.__e2ePlatform`, the
// __e2eLinks pattern): one machine proves BOTH directions — the net
// is broken by flipping the seam, never vacant.
import { test, expect } from '@playwright/test';
import { launchAppV2, closeApp } from '../launch.mjs';

let app, browser, page;

test.describe.configure({ mode: 'serial' });

test.beforeAll(async () => {
  ({ app, browser, page } = await launchAppV2({ accounts: [
    { email: 'owner@example.fr', messages: 8 },
  ] }));
});

test.afterAll(async () => { await closeApp({ app, browser }); });

const rows = () => page.locator('[data-testid="row"]');

test('under the mac seam, Backspace deletes the selected conversation', async () => {
  await page.addInitScript(() => { window.__e2ePlatform = 'mac'; });
  await page.reload();
  await expect(rows().first()).toBeVisible();
  const n = await rows().count();
  const leaving = (await rows().first().locator('.subject').textContent()).trim();
  await rows().first().click();
  await page.keyboard.press('Backspace');
  await expect(page.locator('[data-testid="toast"]')).toContainText('deleted');
  await expect(rows()).toHaveCount(n - 1);
  await expect(rows().filter({ hasText: leaving })).toHaveCount(0);
});

test('under the mac seam, the Settings reference speaks mac (⌫, ⌘Space)', async () => {
  await page.locator('[data-testid="settings"]').click();
  await page.locator('[data-testid="settings-group"][data-group="raccourcis"]').click();
  const panel = page.locator('[data-testid="settings-shortcuts"]');
  await expect(panel).toContainText('⌫');
  await expect(panel).toContainText('⌘Space');
  await expect(panel).not.toContainText('Ctrl+Space');
  await page.locator('[data-testid="settings-done"]').click();
});

test('under the windows seam, Backspace does nothing and the labels stay Del/Ctrl+Space', async () => {
  await page.addInitScript(() => { window.__e2ePlatform = 'windows'; });
  await page.reload();
  await expect(rows().first()).toBeVisible();
  const n = await rows().count();
  await rows().first().click();
  await page.keyboard.press('Backspace');
  // Nothing leaves — the same count, no toast racing: assert on the
  // stable truth (a deletion would change the count).
  await page.waitForTimeout(300);
  await expect(rows()).toHaveCount(n);
  await page.locator('[data-testid="settings"]').click();
  await page.locator('[data-testid="settings-group"][data-group="raccourcis"]').click();
  const panel = page.locator('[data-testid="settings-shortcuts"]');
  await expect(panel).toContainText('Ctrl+Space');
  await expect(panel).not.toContainText('⌘Space');
  await page.locator('[data-testid="settings-done"]').click();
});
