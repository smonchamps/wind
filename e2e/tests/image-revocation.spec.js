import { test, expect } from '@playwright/test';
import { launchAppV2, closeApp, injectArrival, purgeLocals } from '../launch.mjs';
let app, browser, page;
const email = 'guest@example.fr';
const pane = () => page.getByTestId('reading-pane');
const frame = () => pane().locator('iframe.body');
const letter = () => page.getByTestId('row').filter({ hasText: 'Image consent test' }).first();
const settingsRule = () => page.getByTestId('sender-images').filter({ hasText: 'letter@example.fr' });
const removeSender = async () => {
  await page.getByTestId('settings').click();
  await page.locator('[data-testid="settings-group"][data-group="affichage"]').click();
  await settingsRule().getByTestId('remove-image-sender').click();
  await page.getByTestId('settings-done').click();
};

test.beforeAll(async () => {
  ({ app, browser, page } = await launchAppV2({ accounts: [{ email, messages: 1 }] }));
  await purgeLocals(page);
  injectArrival({ email, sender: 'letter@example.fr', subject: 'Image consent test', body: 'images' });
  await page.reload();
});
test.afterAll(async () => { await purgeLocals(page); await closeApp({ app, browser }); });

test('message permission can be removed and the next open remains blocked', async ({}, info) => {
  await letter().click();
  await pane().getByTestId('show-images').click();
  await expect(frame()).toHaveAttribute('srcdoc', /images\.exemple/);
  await expect(pane().getByTestId('revoke-message-images')).toBeVisible();
  await page.screenshot({ path: info.outputPath('message-image-permission.png') });
  await pane().getByTestId('revoke-message-images').click();
  await expect(pane().getByTestId('images-guard')).toBeVisible();
  await expect(frame()).not.toHaveAttribute('srcdoc', /images\.exemple/);
  await page.reload();
  await letter().click();
  await expect(pane().getByTestId('images-guard')).toBeVisible();
});

test('removing a sender permission refreshes the currently open message', async () => {
  await letter().click();
  await pane().getByTestId('always-show-images').click();
  await expect(frame()).toHaveAttribute('srcdoc', /images\.exemple/);
  await removeSender();
  await expect(pane().getByTestId('images-guard')).toBeVisible();
  await expect(frame()).not.toHaveAttribute('srcdoc', /images\.exemple/);
});

test('a previously served allowed body cannot arrive after sender revocation', async () => {
  await letter().click();
  await pane().getByTestId('always-show-images').click();
  await expect(frame()).toHaveAttribute('srcdoc', /images\.exemple/);
  await page.getByTestId('row').filter({ hasNotText: 'Image consent test' }).first().click();
  await page.evaluate(() => {
    window.__e2eLog = [];
    window.__e2eAfterBody = value => {
      if (!value.document.includes('images.exemple')) return value;
      delete window.__e2eAfterBody;
      window.__imageReplyCaptured = value.document;
      return new Promise(resolve => { window.__releaseImageReply = () => resolve(value); });
    };
  });
  await letter().click();
  await expect.poll(() => page.evaluate(() => window.__imageReplyCaptured)).toContain('images.exemple');
  await removeSender();
  await page.evaluate(() => window.__releaseImageReply());
  await expect.poll(() => page.evaluate(() => window.__e2eLog.filter(r => r.command === 'message_body').every(r => r.arrival !== null))).toBe(true);
  await expect(pane().getByTestId('images-guard')).toBeVisible();
  await expect(frame()).not.toHaveAttribute('srcdoc', /images\.exemple/);
});

test('a failed grant keeps the images blocked and reports the failure', async () => {
  await letter().click();
  await page.evaluate(() => { window.__e2eFailure = ['allow_images_message']; });
  await pane().getByTestId('show-images').click();
  await expect(page.getByTestId('toast')).toContainText('Could not change image permissions');
  await expect(pane().getByTestId('images-guard')).toBeVisible();
  await expect(frame()).not.toHaveAttribute('srcdoc', /images\.exemple/);
});

test('Feed exposes per-message revocation and applies sender revocation on served pages', async () => {
  injectArrival({ email, sender: 'feed-images@example.fr', subject: 'Feed image consent', body: 'images', n: 25 });
  await page.reload();
  await page.getByTestId('organized-mode').click();
  await page.evaluate(async () => {
    const invoke = window.__TAURI__.core.invoke;
    await invoke('route_sender', { address: 'feed-images@example.fr', destination: 'feed', rule: null });
    await invoke('revoke_images_sender', { address: 'feed-images@example.fr' });
  });
  await page.locator('[data-testid="nav-folder"][data-category="feed"]').click();
  const cards = page.getByTestId('feed-card');
  await expect(cards).toHaveCount(20);
  await cards.last().scrollIntoViewIfNeeded();
  await expect(cards).toHaveCount(25);
  await cards.last().scrollIntoViewIfNeeded();
  await cards.last().getByRole('button', { name: 'Show images', exact: true }).click();
  await expect(cards.last().getByTestId('revoke-message-images')).toBeVisible();
  await cards.last().getByTestId('revoke-message-images').click();
  await expect(cards.last().getByTestId('feed-images-guard')).toBeVisible();
  await cards.last().getByRole('button', { name: 'Always show images from this sender' }).click();
  await expect(cards.last().locator('iframe.body')).toHaveAttribute('srcdoc', /images\.exemple/);
  await page.getByTestId('settings').click();
  await page.locator('[data-testid="settings-group"][data-group="affichage"]').click();
  await page.getByTestId('sender-images').filter({ hasText: 'feed-images@example.fr' }).getByTestId('remove-image-sender').click();
  await page.getByTestId('settings-done').click();
  await cards.last().scrollIntoViewIfNeeded();
  await expect(cards.last().getByTestId('feed-images-guard')).toBeVisible();
  await expect(cards.last().locator('iframe.body')).not.toHaveAttribute('srcdoc', /images\.exemple/);
});
