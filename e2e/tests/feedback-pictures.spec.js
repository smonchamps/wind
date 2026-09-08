// PLAN-BATCH-2026-09 E3: pictures (jpg/png) on the feedback form. The
// picker seam is `__e2eAttachments` (the native dialog cannot be
// driven); the send rides the historical `queue_send` road with the
// new `attachmentPaths`, and the Rust side owns the gates: pictures
// only, three at most (D6). The byte-level journaling proof lives in
// mail-core (`enqueue_outbox_full_stores_the_given_files_with_the_send`).
import { test, expect } from '@playwright/test';
import { writeFile } from 'node:fs/promises';
import { launchAppV2, closeApp } from '../launch.mjs';

let app;
let browser;
let page;

test.describe.configure({ mode: 'serial' });

test.beforeAll(async () => {
  ({ app, browser, page } = await launchAppV2({
    accounts: [{ email: 'principal@exemple.fr', messages: 1 }],
  }));
  await expect(page.locator('[data-testid="row"]').first()).toBeVisible();
});

test.afterAll(async () => {
  await closeApp({ app, browser });
});

test('a png rides the feedback into the outbox', async ({}, info) => {
  const picture = info.outputPath('capture.png');
  await writeFile(picture, Buffer.from([0x89, 0x50, 0x4e, 0x47, 1, 2, 3, 4]));
  await page.evaluate((path) => { window.__e2eAttachments = [path]; }, picture);
  await page.locator('[data-testid="feedback"]').click();
  const card = page.locator('[data-testid="back-card"]');
  await expect(card).toBeVisible();
  await card.locator('[data-testid="back-text"]').fill('The screen looks like this.');
  await card.locator('[data-testid="back-attach"]').click();
  // The chip: name shown, removable — but kept here, it must SEND.
  await expect(card.locator('[data-testid="back-picture"]')).toContainText('capture.png');
  await card.locator('[data-testid="back-send"]').click();
  await expect(page.locator('[data-testid="back-card"]')).toHaveCount(0);
  await expect(page.locator('[data-testid="toast"]')).toContainText('Thank you');
  // The decor account has no server: the send stays JOURNALED in the
  // outbox ("never a lost send" — pictures included, same transaction).
  await expect(page.locator('[data-testid="progress"]')).toContainText(
    'Outbox · 1 message waiting',
  );
});

test('anything but jpg/png is refused by the shell, and the form stays open', async ({}, info) => {
  const stray = info.outputPath('notes.txt');
  await writeFile(stray, 'not a picture');
  await page.evaluate((path) => { window.__e2eAttachments = [path]; }, stray);
  await page.locator('[data-testid="feedback"]').click();
  const card = page.locator('[data-testid="back-card"]');
  await expect(card).toBeVisible();
  await card.locator('[data-testid="back-text"]').fill('With a stray file.');
  // The UI cannot vouch for a path (the picker filter is cosmetic):
  // the chip appears, the REFUSAL is Rust's at the send.
  await card.locator('[data-testid="back-attach"]').click();
  await expect(card.locator('[data-testid="back-picture"]')).toContainText('notes.txt');
  await card.locator('[data-testid="back-send"]').click();
  await expect(page.locator('[data-testid="toast"]')).toContainText('pictures only');
  // Nothing lost, nothing sent: the form and its text are still there,
  // the outbox still carries the ONE send of the previous test.
  await expect(card).toBeVisible();
  await expect(page.locator('[data-testid="progress"]')).toContainText(
    'Outbox · 1 message waiting',
  );
  await card.locator('[data-testid="back-picture"] button').click();
  await expect(card.locator('[data-testid="back-picture"]')).toHaveCount(0);
  await page.keyboard.press('Escape');
  await expect(page.locator('[data-testid="back-card"]')).toHaveCount(0);
});

test('the fourth picture is refused at the gesture (D6: three at most)', async ({}, info) => {
  const paths = [];
  for (const name of ['one.png', 'two.jpg', 'three.png', 'four.jpg']) {
    const path = info.outputPath(name);
    await writeFile(path, Buffer.from([1, 2, 3]));
    paths.push(path);
  }
  await page.evaluate((all) => { window.__e2eAttachments = all; }, paths);
  await page.locator('[data-testid="feedback"]').click();
  const card = page.locator('[data-testid="back-card"]');
  await expect(card).toBeVisible();
  await card.locator('[data-testid="back-attach"]').click();
  await expect(card.locator('[data-testid="back-picture"]')).toHaveCount(3);
  await expect(page.locator('[data-testid="toast"]')).toContainText('Three pictures at most');
  await page.keyboard.press('Escape');
  await expect(page.locator('[data-testid="back-card"]')).toHaveCount(0);
});
