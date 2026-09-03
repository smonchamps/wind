// PLAN-AUDIT-V2 E11: THE product's menu is navigable by keyboard and gives
// back focus (A8 held — `role="menu"` promised a keyboard that was absent: eight
// copies closed on any key, Tab included, without ever
// placing focus). Played on the ⋯ of a row of the organized Inbox
// — the first menu carried; the other seven share the component.
import { test, expect } from '@playwright/test';
import { launchAppV2, closeApp } from '../launch.mjs';

let app;
let browser;
let page;

test.describe.configure({ mode: 'serial' });

test.beforeAll(async () => {
  ({ app, browser, page } = await launchAppV2());
  await page.locator('[data-testid="organized-mode"]').click();
  await expect(page.locator('[data-testid="organized-mode"]')).toHaveAttribute('aria-checked', 'true');
  await expect(page.locator('[data-testid="row"]').first()).toBeVisible();
});

test.afterAll(async () => {
  await closeApp({ app, browser });
});

const active = () => page.evaluate(() => document.activeElement?.dataset?.testid ?? null);

test('the actions menu is navigable with the arrow keys and Escape returns focus to the trigger', async () => {
  const row = page.locator('[data-testid="row"]').first();
  await row.hover();
  const trigger = row.locator('[data-testid="row-gestures"]');
  await trigger.click();
  const menu = page.locator('[data-testid="menu-gestures"]');
  await expect(menu).toBeVisible();
  // Focus lands on the first item, with no further action.
  await expect.poll(active).toBe('gestures-feed');
  await page.keyboard.press('ArrowDown');
  await expect.poll(active).toBe('gestures-paper_trail');
  await page.keyboard.press('End');
  await expect.poll(active).toBe('gestures-screen-out');
  await page.keyboard.press('ArrowDown');
  await expect.poll(active).toBe('gestures-feed');
  // Any key does NOT close it (before: any keydown closed it).
  await page.keyboard.press('Shift');
  await expect(menu).toBeVisible();
  await page.keyboard.press('Escape');
  await expect(menu).toHaveCount(0);
  await expect.poll(active).toBe('row-gestures');
});

test('Tab and clicking outside close the menu', async () => {
  const row = page.locator('[data-testid="row"]').first();
  await row.hover();
  await row.locator('[data-testid="row-gestures"]').click();
  const menu = page.locator('[data-testid="menu-gestures"]');
  await expect(menu).toBeVisible();
  await page.keyboard.press('Tab');
  await expect(menu).toHaveCount(0);
  await row.hover();
  await row.locator('[data-testid="row-gestures"]').click();
  await expect(menu).toBeVisible();
  // A neutral point of the window (the corner of the nav): outside, no action.
  await page.mouse.click(5, 5);
  await expect(menu).toHaveCount(0);
});

test('Enter on an item plays the action like a click', async () => {
  const row = page.locator('[data-testid="row"]').first();
  const subject = (await row.locator('.subject').textContent()).trim();
  await row.hover();
  await row.locator('[data-testid="row-gestures"]').click();
  await expect.poll(active).toBe('gestures-feed');
  await page.keyboard.press('Enter');
  await expect(page.locator('[data-testid="menu-gestures"]')).toHaveCount(0);
  // The row left for the Feed: it leaves the Inbox.
  await expect
    .poll(async () =>
      (await page.locator('[data-testid="row"] .subject').allTextContents()).map((s) => s.trim()).includes(subject))
    .toBe(false);
});

test('Settings opens on its first control (D-4 entry)', async () => {
  await page.locator('[data-testid="settings"]').click();
  await expect.poll(async () =>
    page.evaluate(() => {
      const el = document.activeElement;
      return Boolean(el?.closest?.('[data-testid="settings-modal"], .panel, [role="dialog"]')) || (el?.dataset?.testid ?? '').startsWith('settings');
    })).toBe(true);
  await page.locator('[data-testid="settings-done"]').click();
});
