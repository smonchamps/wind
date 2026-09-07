import { test, expect } from '@playwright/test';
import { launchAppV2, closeApp } from '../launch.mjs';

let app, browser, page;
test.describe.configure({ mode: 'default' });
test.beforeAll(async () => {
  ({ app, browser, page } = await launchAppV2({ accounts: [
    { email: 'owner@example.fr', messages: 3 },
  ] }));
});
test.afterAll(async () => { await closeApp({ app, browser }); });
test.afterEach(async () => { await page.reload(); });

for (const trigger of ['settings', 'write', 'feedback']) {
  test(`${trigger} contains keyboard focus, isolates shortcuts and restores its trigger`, async () => {
    const opener = page.locator(`[data-testid="${trigger}"]`);
    await opener.click();
    const dialog = page.getByRole('dialog');
    await expect(dialog).toBeVisible();
    await expect.poll(() => dialog.evaluate(el => el.contains(document.activeElement))).toBe(true);
    const boundaries = await dialog.evaluate(el => {
      const stops = [...el.querySelectorAll('button,input,textarea,select,a[href],[tabindex],[contenteditable="true"]')]
        .filter(n => n.tabIndex >= 0 && !n.disabled && !n.closest('[inert]') && n.getClientRects().length);
      stops[0].setAttribute('data-focus-edge', 'first');
      stops.at(-1).setAttribute('data-focus-edge', 'last');
      return stops.length;
    });
    expect(boundaries).toBeGreaterThan(1);
    const first = dialog.locator('[data-focus-edge="first"]');
    const last = dialog.locator('[data-focus-edge="last"]');
    await last.focus();
    await page.keyboard.press('Tab');
    await expect(first).toBeFocused();
    await page.keyboard.press('Shift+Tab');
    await expect(last).toBeFocused();
    await opener.evaluate(el => el.focus());
    await expect.poll(() => dialog.evaluate(el => el.contains(document.activeElement))).toBe(true);
    await first.focus();
    await page.keyboard.press('/');
    await expect.poll(() => dialog.evaluate(el => el.contains(document.activeElement))).toBe(true);
    await page.keyboard.press('Escape');
    await expect(dialog).toHaveCount(0);
    await expect(opener).toBeFocused();
    await opener.press('Enter');
    await expect(dialog).toBeVisible();
    await page.keyboard.press('Escape');
    await expect(dialog).toHaveCount(0);
    await expect(opener).toBeFocused();
  });
}
