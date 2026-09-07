import { test, expect } from '@playwright/test';
import { launchAppV2, closeApp, purgeLocals } from '../launch.mjs';

let app, browser, page;
test.beforeAll(async () => {
  ({ app, browser, page } = await launchAppV2({ accounts: [
    { email: 'owner@example.fr', messages: 8 },
  ] }));
  await purgeLocals(page);
  await page.reload();
});
test.afterAll(async () => { await closeApp({ app, browser }); });
test.afterEach(async () => { await purgeLocals(page); await page.reload(); });

test('keyboard toggles, ranges, clears and archives exactly the checked messages', async () => {
  const rows = page.locator('[data-testid="row"]');
  const checked = page.locator('[data-testid="row-checkbox"][aria-checked="true"]');
  await expect(rows).toHaveCount(7);
  // Enter the list by keyboard, then keep every selection gesture keyboard-only.
  for (let n = 0; n < 40 && !(await rows.first().evaluate(el => el === document.activeElement)); n++) {
    await page.keyboard.press('Tab');
  }
  await expect(rows.first()).toBeFocused();
  await page.keyboard.press('Control+Space');
  await expect(checked).toHaveCount(1);
  await page.keyboard.press('Tab');
  await expect(rows.nth(1)).toBeFocused();
  await page.keyboard.press('Tab');
  await expect(rows.nth(2)).toBeFocused();
  await page.keyboard.press('Shift+Space');
  await expect(checked).toHaveCount(3);
  await expect(page.locator('[data-testid="bar-selection"]')).toContainText('3 selected');
  await page.keyboard.press('Control+Space');
  await expect(checked).toHaveCount(2);
  await page.keyboard.press('Escape');
  await expect(checked).toHaveCount(0);
  await expect(rows.nth(2)).toBeFocused();
  await page.keyboard.press('Control+Space');
  await page.keyboard.press('Tab');
  await page.keyboard.press('Shift+Space');
  await expect(checked).toHaveCount(2);
  const leaving = await rows.filter({ has: page.locator('[aria-checked="true"]') }).locator('.subject').allTextContents();
  await page.keyboard.press('e');
  await expect(rows).toHaveCount(5);
  await expect(checked).toHaveCount(0);
  for (const subject of leaving) await expect(rows.filter({ hasText: subject.trim() })).toHaveCount(0);
});

test('keyboard selection stays in the list in two-pane mode', async () => {
  await page.locator('[data-testid="settings"]').click();
  await page.locator('[data-testid="settings-group"][data-group="affichage"]').click();
  await page.locator('[data-testid="display-panes"]').selectOption('2');
  await page.locator('[data-testid="settings-done"]').click();
  const row = page.locator('[data-testid="row"]').first();
  await row.focus();
  await page.keyboard.press('Control+Space');
  await expect(page.locator('[data-testid="bar-selection"]')).toContainText('1 selected');
  await expect(row).toBeFocused();
  await page.keyboard.press('Escape');
  await expect(page.locator('[data-testid="bar-selection"]')).toHaveCount(0);
});
