import { test, expect } from '@playwright/test';
import { launchAppV2, closeApp } from '../launch.mjs';
let app, browser, page;
test.beforeAll(async ({}, info) => {
  info.setTimeout(180000);
  ({ app, browser, page } = await launchAppV2());
});
test.afterAll(async () => { await closeApp({ app, browser }); });

test('partial failures survive successful inbox reports and explain the affected work', async ({}, testInfo) => {
  await page.evaluate(async () => {
    const nav = await window.__TAURI__.core.invoke('nav_snapshot');
    window.__e2eSyncIssues = [{ account_id: nav[0].account_id, mailbox: 'Archive', operation: 'sync', reason: 'server refusal: synthetic folder denied', retry_at: null }];
    window.__e2eSyncSummary = { accounts: 1, accounts_failed: 0, fetched: 0, deleted: 0, replayed: 0, elapsed_ms: 1, errors: [] };
  });
  await expect(page.locator('[data-testid="progress"]')).toContainText('Synchronization incomplete', { timeout: 12000 });
  await page.locator('[data-testid="btn-poll"]').click();
  await expect(page.locator('[data-testid="progress"]')).toContainText('Synchronization incomplete');
  await page.locator('[data-testid="settings"]').click();
  const issue = page.locator('[data-testid="sync-issue"]');
  await expect(issue).toContainText('Archive');
  await expect(issue).toContainText('synthetic folder denied');
  await expect(issue).toContainText('Manual retry required');
  await page.evaluate(() => { window.__e2eSyncIssues[0].retry_at = Math.floor(Date.now() / 1000) + 60; });
  await expect(issue).toContainText('Automatic retry', { timeout: 12000 });
  await page.screenshot({ path: testInfo.outputPath('sync-issues.png') });
  await page.evaluate(() => { window.__e2eSyncIssues = []; });
  await expect(issue).toHaveCount(0, { timeout: 12000 });
});


test('an empty scanned window continues to the next eligible body', async () => {
  await page.keyboard.press('Escape');
  await page.evaluate(() => {
    window.__e2eBackfillCalls = 0;
    window.__e2eBackfill = (command) => {
      if (command === 'backfill_status') return { remaining: 1, percent: 90 };
      window.__e2eBackfillCalls++;
      return window.__e2eBackfillCalls === 1
        ? { fetched: 0, scanned: 200, more: true, remaining: 1, percent: 90, errors: [] }
        : { fetched: 1, scanned: 1, more: false, remaining: 0, percent: 100, errors: [] };
    };
  });
  await page.locator('[data-testid="btn-poll"]').click();
  await expect.poll(() => page.evaluate(() => window.__e2eBackfillCalls)).toBe(2);
  await page.evaluate(() => { delete window.__e2eBackfill; delete window.__e2eSyncIssues; delete window.__e2eSyncSummary; });
});
