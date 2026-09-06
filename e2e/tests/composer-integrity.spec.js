import { test, expect } from '@playwright/test';
import { launchAppV2, closeApp } from '../launch.mjs';
import { writeFile } from 'node:fs/promises';

let app;
let browser;
let page;

test.beforeAll(async () => {
  ({ app, browser, page } = await launchAppV2());
});

test.afterAll(async () => {
  await closeApp({ app, browser });
});

async function holdCalls() {
  await page.evaluate(() => {
    window.__e2eLog = [];
    window.__e2eHold = new Promise((release) => { window.releaseComposeCalls = release; });
  });
}

async function releaseCalls() {
  await page.evaluate(() => {
    delete window.__e2eHold;
    window.releaseComposeCalls?.();
    delete window.releaseComposeCalls;
    delete window.__e2eAttachments;
  });
}

test('the sender selector is vertically centered beside its label', async ({}, testInfo) => {
  await page.locator('[data-testid="write"]').click();
  const from = page.locator('select[data-testid="compose-from"]');
  await expect(from).toBeVisible();
  const centers = await from.evaluate((select) => {
    const label = select.closest('.rank').querySelector('.label').getBoundingClientRect();
    const value = select.getBoundingClientRect();
    return { label: label.y + label.height / 2, value: value.y + value.height / 2 };
  });
  expect(Math.abs(centers.label - centers.value)).toBeLessThanOrEqual(1);
  await page.locator('[data-testid="compose"]').screenshot({ path: testInfo.outputPath('sender-centered.png') });
  await page.locator('[data-testid="compose-cancel"]').click();
  await expect(page.locator('[data-testid="compose"]')).toHaveCount(0);
});

test('an unavailable reply source is stated and cannot be sent as a complete reply', async () => {
  await page.locator('[data-testid="row"]').first().click();
  await page.evaluate(() => { window.__e2eFailure = ['reply_context']; });
  await page.locator('[data-testid="reply"]').first().click();
  const compose = page.locator('[data-testid="compose"]');
  await expect(compose).toBeVisible();
  await expect(compose.getByRole('alert')).toContainText(/source message/i);
  await expect(page.locator('[data-testid="compose-send"]')).toBeDisabled();
  await page.locator('[data-testid="compose-body"]').fill('Preserved after context failure.');
  await page.locator('[data-testid="compose-cancel"]').click();
  await expect(compose).toHaveCount(0);
  const drafts = await page.evaluate(() => window.__TAURI__.core.invoke('list_drafts'));
  expect(drafts.some((draft) => draft.body === 'Preserved after context failure.')).toBe(true);
});

test('failed close keeps unsaved content and a retry persists it', async ({}, testInfo) => {
  await page.locator('[data-testid="write"]').click();
  const compose = page.locator('[data-testid="compose"]');
  await expect(compose).toBeVisible();
  await page.locator('[data-testid="compose-subject"]').fill('Unsaved audit draft');
  await page.locator('[data-testid="compose-body"]').fill('This text must survive a storage failure.');
  await page.evaluate(() => { window.__e2eFailure = ['save_draft']; });
  await page.locator('[data-testid="compose-cancel"]').click();

  await expect(compose).toBeVisible();
  await expect(page.locator('[data-testid="compose-body"]'))
    .toHaveText('This text must survive a storage failure.');
  await expect(compose.getByRole('alert')).toContainText(/could not be saved/i);
  await expect(page.getByText('Draft saved.', { exact: true })).toHaveCount(0);
  await page.screenshot({ path: testInfo.outputPath('save-failed.png') });

  await page.locator('[data-testid="compose-cancel"]').click();
  await expect(compose).toHaveCount(0);
  const saved = await page.evaluate(() => window.__TAURI__.core.invoke('list_drafts'));
  expect(saved.some((draft) => draft.subject === 'Unsaved audit draft'
    && draft.body === 'This text must survive a storage failure.')).toBe(true);
});

test('closing waits for a pending attachment and retains the file', async ({}, testInfo) => {
  const file = testInfo.outputPath('pending-attachment.txt');
  await writeFile(file, 'Attachment retained across close.');
  await page.locator('[data-testid="write"]').click();
  await page.evaluate((filePath) => { window.__e2eAttachments = [filePath]; }, file);
  await holdCalls();
  try {
    await page.locator('[data-testid="compose-attach"]').click();
    await expect.poll(() => page.evaluate(() =>
      window.__e2eLog.some((entry) => entry.command === 'attach_files'))).toBe(true);
    await page.locator('[data-testid="compose-cancel"]').click();
    await expect(page.locator('[data-testid="compose"]')).toBeVisible();
  } finally {
    await releaseCalls();
  }
  await expect(page.locator('[data-testid="compose"]')).toHaveCount(0);
  const files = await page.evaluate(async () => {
    const drafts = await window.__TAURI__.core.invoke('list_drafts');
    return (await Promise.all(drafts.map((draft) =>
      window.__TAURI__.core.invoke('draft_attachments', { draftId: draft.id })))).flat();
  });
  expect(files.some((entry) => entry.name === 'pending-attachment.txt'
    && entry.size === Buffer.byteLength('Attachment retained across close.'))).toBe(true);
});

test('text edited while a close-save is pending is persisted before closing', async () => {
  await page.locator('[data-testid="write"]').click();
  await page.locator('[data-testid="compose-subject"]').fill('Edited during close');
  await page.locator('[data-testid="compose-body"]').fill('Before the save.');
  await holdCalls();
  try {
    await page.locator('[data-testid="compose-cancel"]').click();
    await expect.poll(() => page.evaluate(() =>
      window.__e2eLog.some((entry) => entry.command === 'save_draft'))).toBe(true);
    await page.locator('[data-testid="compose-body"]').fill('Edited while the save was pending.');
  } finally {
    await releaseCalls();
  }
  await expect(page.locator('[data-testid="compose"]')).toHaveCount(0);
  const saved = await page.evaluate(() => window.__TAURI__.core.invoke('list_drafts'));
  expect(saved.find((draft) => draft.subject === 'Edited during close')?.body)
    .toBe('Edited while the save was pending.');
});

test('a failed native window close keeps the composition available for retry', async () => {
  await page.locator('[data-testid="write"]').click();
  await page.locator('[data-testid="compose-body"]').fill('Native close must preserve this text.');
  await page.evaluate(async () => {
    window.__e2eFailure = ['save_draft'];
    await window.__TAURI__.event.emit('tauri://close-requested');
  });
  await expect(page.locator('[data-testid="compose"]')).toBeVisible();
  // The 2 s autosave must not impersonate handling the native request.
  await expect(page.locator('[data-testid="compose"] [role="alert"]')).toContainText(/could not be saved/i, { timeout: 800 });
  await expect(page.locator('[data-testid="compose-body"]')).toHaveText('Native close must preserve this text.');
  await page.locator('[data-testid="compose-cancel"]').click();
  await expect(page.locator('[data-testid="compose"]')).toHaveCount(0);
});

test('an edited draft keeps its opening files after its mirror disappears', async ({}, testInfo) => {
  const path = testInfo.outputPath('opening-version.txt');
  await writeFile(path, 'The opening version owns these bytes.');
  await page.locator('[data-testid="write"]').click();
  await page.locator('[data-testid="compose-subject"]').fill('Remote replacement fixture');
  await page.evaluate((path) => { window.__e2eAttachments = [path]; }, path);
  await page.locator('[data-testid="compose-attach"]').click();
  await expect(page.locator('[data-testid="compose"]')).toContainText('opening-version.txt');
  await page.locator('[data-testid="compose-cancel"]').click();
  await expect(page.locator('[data-testid="compose"]')).toHaveCount(0);
  await page.evaluate(() => { delete window.__e2eAttachments; });
  await page.locator('[data-testid="nav-folder"][data-category="drafts"]').click();
  await page.locator('[data-testid="row-draft"]', { hasText: 'Remote replacement fixture' }).click();
  await expect(page.locator('[data-testid="compose"]')).toContainText('opening-version.txt');
  await page.evaluate(async () => {
    const drafts = await window.__TAURI__.core.invoke('list_drafts');
    const source = drafts.find((draft) => draft.subject === 'Remote replacement fixture');
    await window.__TAURI__.core.invoke('delete_draft', { id: source.id });
  });
  await page.locator('[data-testid="compose-body"]').fill('Edited after the mirror disappeared.');
  await page.locator('[data-testid="compose-cancel"]').click();
  await expect(page.locator('[data-testid="compose"]')).toHaveCount(0);
  const result = await page.evaluate(async () => {
    const drafts = await window.__TAURI__.core.invoke('list_drafts');
    const saved = drafts.find((draft) => draft.subject === 'Remote replacement fixture');
    return { body: saved.body, files: await window.__TAURI__.core.invoke('draft_attachments', { draftId: saved.id }) };
  });
  expect(result.body).toBe('Edited after the mirror disappeared.');
  expect(result.files.map((file) => file.name)).toEqual(['opening-version.txt']);
});

test('a draft import failure after a successful inbox poll offers retry', async () => {
  await page.locator('[data-testid="nav-folder"][data-category="inbox"]').click();
  const button = page.locator('[data-testid="btn-poll"]');
  await expect(button).toBeEnabled();
  await page.evaluate(() => {
    window.__e2eSyncSummary = {
      accounts: 1, accounts_failed: 0, fetched: 0, deleted: 0, replayed: 0,
      elapsed_ms: 1, errors: ['remote drafts: simulated import failure'],
    };
  });
  try {
    await button.click();
    await expect(button).toBeEnabled();
    await expect(button).toHaveText(/try again/i, { timeout: 3000 });
    await expect(page.locator('[data-testid="progress"]')).toContainText(/sync failed/i);
    await page.evaluate(() => { window.__e2eSyncSummary.errors = []; });
    await button.click();
    await expect(button).toHaveText(/^\s*sync\s*$/i);
    await expect(page.locator('[data-testid="progress"]')).toContainText(/up to date/i);
    await page.evaluate(() => {
      window.__e2eSyncSummary.accounts_failed = 1;
      window.__e2eSyncSummary.errors = ['second account unavailable'];
    });
    await button.click();
    await expect(button).toHaveText(/try again/i);
    await expect(page.locator('[data-testid="progress"]')).toContainText(/1 account of 2 unreachable/i);
  } finally {
    await page.evaluate(() => { delete window.__e2eSyncSummary; });
  }
});
