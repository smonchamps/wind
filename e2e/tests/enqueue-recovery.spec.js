import { test, expect } from '@playwright/test';
import { launchAppV2, closeApp } from '../launch.mjs';

let app;
let browser;
let page;
test.beforeAll(async () => { ({ app, browser, page } = await launchAppV2()); });
test.afterAll(async () => { await closeApp({ app, browser }); });

async function compose(subject) {
  await page.locator('[data-testid="write"]').click();
  await page.locator('[data-testid="compose-to"]').fill('fixture@example.invalid');
  await page.locator('[data-testid="compose-subject"]').fill(subject);
  await page.locator('[data-testid="compose-body"]').fill('Keep the submitted version.');
}

async function loseAcknowledgement(blockVerification) {
  await page.evaluate((block) => {
    const fetch = window.fetch;
    let lose = true;
    window.blockEnqueueVerification = block;
    window.enqueueRecoveryCalls = [];
    window.restoreEnqueueFetch = () => { window.fetch = fetch; };
    window.fetch = async (...args) => {
      const command = new URL(String(args[0]), location.href).pathname.slice(1);
      window.enqueueRecoveryCalls.push(command);
      const response = await fetch(...args);
      if ((command === 'queue_send' && lose && response.headers.get('Tauri-Response') === 'ok')
          || (command === 'queued_draft_edit' && window.blockEnqueueVerification)) {
        if (command === 'queue_send') lose = false;
        // The actual command has finished before its acknowledgement is lost.
        await response.arrayBuffer();
        const headers = new Headers(response.headers);
        headers.set('Tauri-Response', 'error');
        headers.set('Content-Type', 'application/json');
        return new Response(JSON.stringify('simulated acknowledgement unavailable'), { headers });
      }
      return response;
    };
  }, blockVerification);
}

async function resetFault() {
  await page.evaluate(() => {
    window.restoreEnqueueFetch?.();
    delete window.restoreEnqueueFetch;
    delete window.blockEnqueueVerification;
  });
}

async function queuedCount(subject) {
  return page.evaluate(async (subject) => {
    const status = await window.__TAURI__.core.invoke('outbox_status');
    return status.entries.filter((entry) => entry.subject === subject).length;
  }, subject);
}

test('a lost acknowledgement after commit closes from the durable result, without saving again', async () => {
  const subject = 'Committed acknowledgement lost';
  await compose(subject);
  await loseAcknowledgement(false);
  try {
    await page.locator('[data-testid="compose-send"]').click();
    await expect.poll(() => queuedCount(subject)).toBe(1);
    await expect(page.locator('[data-testid="compose"]')).toHaveCount(0);
    const calls = await page.evaluate(() => window.enqueueRecoveryCalls);
    const submission = calls.indexOf('queue_send');
    expect(submission).toBeGreaterThanOrEqual(0);
    expect(calls.slice(submission + 1)).toContain('queued_draft_edit');
    expect(calls.slice(submission + 1)).not.toContain('save_draft');
    expect(calls.filter((command) => command === 'queue_send')).toHaveLength(1);
  } finally { await resetFault(); }
});

test('unavailable verification freezes the submitted version until an explicit status check', async ({}, testInfo) => {
  const subject = 'Verification pending';
  await compose(subject);
  await loseAcknowledgement(true);
  try {
    await page.locator('[data-testid="compose-send"]').click();
    await expect.poll(() => queuedCount(subject)).toBe(1);
    const check = page.locator('[data-testid="compose-verify-send"]');
    await expect(check).toBeVisible();
    await expect(page.locator('[data-testid="compose"] [role="alert"]')).toContainText('could not be confirmed');
    expect(await page.locator('[data-testid="compose-subject"]').evaluate((field) => !!field.closest('[inert]'))).toBe(true);
    expect(await page.locator('[data-testid="compose-body"]').evaluate((field) => !!field.closest('[inert]'))).toBe(true);
    await page.keyboard.press('Escape');
    await expect(page.locator('[data-testid="compose"]')).toBeVisible();
    await page.screenshot({ path: testInfo.outputPath('enqueue-verification-pending.png') });
    await page.evaluate(() => { window.blockEnqueueVerification = false; });
    await check.click();
    await expect(page.locator('[data-testid="compose"]')).toHaveCount(0);
    expect(await queuedCount(subject)).toBe(1);
    expect(await page.evaluate(() => window.enqueueRecoveryCalls.filter((command) => command === 'queue_send').length)).toBe(1);
  } finally { await resetFault(); }
});

test('a failed enqueue with a live editing session permits correction and a single retry', async () => {
  const subject = 'Enqueue never committed';
  await compose(subject);
  await page.evaluate(() => { window.__e2eFailure = ['queue_send']; });
  await page.locator('[data-testid="compose-send"]').click();
  await expect(page.locator('[data-testid="compose"]')).toBeVisible();
  await expect(page.locator('[data-testid="compose-send"]')).toBeEnabled();
  expect(await queuedCount(subject)).toBe(0);
  await page.locator('[data-testid="compose-subject"]').fill(subject);
  await page.locator('[data-testid="compose-send"]').click();
  await expect(page.locator('[data-testid="compose"]')).toHaveCount(0);
  expect(await queuedCount(subject)).toBe(1);
});
