import { test, expect } from '@playwright/test';
import { launchAppV2, closeApp, injectArrival } from '../launch.mjs';

let app;
let browser;
let page;
const own = 'principal@exemple.fr';

test.beforeAll(async () => {
  ({ app, browser, page } = await launchAppV2({ accounts: [{ email: own, messages: 2 }] }));
  injectArrival({ email: own, sender: 'robot@example.fr', subject: 'Reply routing',
    replyAddress: 'team@example.fr', to: [own, 'reader@example.fr'],
    cc: ['TEAM@example.fr', 'copy@example.fr', own] });
  injectArrival({ email: own, sender: own, subject: 'Own copy only',
    replyAddress: 'unused@example.fr', to: [], cc: ['copy@example.fr'] });
  await page.reload();
});

test.afterAll(async () => { await closeApp({ app, browser }); });

test('reply-all honors Reply-To, deduplicates across fields and keeps Cc separate', async () => {
  await page.locator('[data-testid="row"]').filter({ hasText: 'Reply routing' }).click();
  await page.locator('[data-testid="reply-all"]').first().click();
  await expect(page.locator('[data-testid="compose-to"]')).toHaveValue('team@example.fr, reader@example.fr');
  await expect(page.locator('[data-testid="compose-cc"]')).toHaveValue('copy@example.fr');
  await page.locator('[data-testid="compose-cancel"]').click();
});

test('reply-all to an own copy-only message keeps Cc without adding oneself or Reply-To', async () => {
  await page.locator('[data-testid="row"]').filter({ hasText: 'Own copy only' }).click();
  await page.locator('[data-testid="reply-all"]').first().click();
  await expect(page.locator('[data-testid="compose-cc"]')).toHaveValue('copy@example.fr');
  await expect(page.locator('[data-testid="compose-to"]')).toHaveValue('');
  await page.locator('[data-testid="compose-cancel"]').click();
});

for (const blind of [false, true]) {
  test(`${blind ? 'Bcc' : 'Cc'}-only composition reaches the durable offline queue`, async () => {
    await page.locator('[data-testid="write"]').click();
    await page.locator(`[data-testid="compose-${blind ? 'bcc' : 'cc'}-button"]`).click();
    await page.locator(`[data-testid="compose-${blind ? 'cci' : 'cc'}"]`).fill('recipient@example.fr');
    const subject = blind ? 'Blind only fixture' : 'Copy only fixture';
    await page.locator('[data-testid="compose-subject"]').fill(subject);
    await page.locator('[data-testid="compose-body"]').fill('Offline fixture, never delivered.');
    await page.locator('[data-testid="compose-send"]').click();
    await expect(page.locator('[data-testid="compose"]')).toHaveCount(0);
    const status = await page.evaluate(() => window.__TAURI__.core.invoke('outbox_status'));
    expect(status.entries.some((entry) => entry.subject === subject && entry.state === 'queued')).toBe(true);
  });
}
