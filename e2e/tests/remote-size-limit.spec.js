import { test, expect } from '@playwright/test';
import { launchAppV2, closeApp } from '../launch.mjs';

let app, browser, page;
test.beforeAll(async ({}, testInfo) => {
  testInfo.setTimeout(180000);
  ({ app, browser, page } = await launchAppV2());
});
test.afterAll(async () => { await closeApp({ app, browser }); });

test('an oversized message retains its envelope and explains the webmail fallback', async ({}, testInfo) => {
  await page.evaluate(() => {
    window.__e2eAfterBody = () => Promise.reject({
      code: 'remote_message_too_large', limit: 33554432,
      message: 'synthetic size refusal',
    });
  });
  const row = page.locator('[data-testid="row"]').first();
  await row.click();
  const pane = page.locator('[data-testid="reading-pane"]');
  await expect(pane.locator('[data-testid="body-failure"]')).toContainText(
    'This message exceeds the 32 MiB download limit. Open it in your webmail to read it and its attachments.');
  await expect(pane.locator('[data-testid="body-retry"]')).toHaveCount(0);
  await expect(pane.locator('[data-testid="thread-subject"]')).not.toBeEmpty();
  await expect(row).toBeVisible();
  await expect.poll(() => pane.locator('[data-testid="message-expanded"]').evaluate((card) => {
    const notice = card.querySelector('[data-testid="body-failure"]').getBoundingClientRect();
    const actions = card.querySelector('[data-testid="actions-message"]').getBoundingClientRect();
    return actions.top >= notice.bottom;
  })).toBe(true);
  await pane.locator('[data-testid="body-failure"]').scrollIntoViewIfNeeded();
  await page.screenshot({ path: testInfo.outputPath('remote-size-limit.png') });
  await page.evaluate(() => { delete window.__e2eAfterBody; });
  await page.locator('[data-testid="row"]').nth(1).click();
  await expect(pane.locator('[data-testid="body-failure"]')).toHaveCount(0);
  await expect(pane.frameLocator('[data-testid="message-expanded"] iframe').first().locator('body')).not.toBeEmpty();
});
