import { test, expect } from '@playwright/test';
import { launchAppV2, closeApp } from '../launch.mjs';

let app, browser, page;

test('generic repair preserves mailbox identity, freezes verification and clears the password on close', async ({}, testInfo) => {
  ({ app, browser, page } = await launchAppV2({ lang: 'fr', accounts: [
    { email: 'healthy@example.fr', messages: 1 },
    { email: 'owner@example.fr', messages: 1, disconnected: true, generic: true },
  ] }));
  await page.locator('[data-testid="settings"]').click();
  await page.locator('[data-testid="account-reconnect"]').click();
  const form = page.locator('[data-testid="generic-repair"]');
  await expect(form).toBeVisible();
  for (const [selector, value] of [['#ob-adresse', 'owner@example.fr'],
    ['#ob-username', 'custom-login'], ['#ob-imap', 'imap.example.fr']]) {
    await expect(form.locator(selector)).toHaveValue(value);
    await expect(form.locator(selector)).toBeDisabled();
  }
  await expect(form.locator('#ob-mdp')).toHaveValue('');
  await expect(form.locator('[data-testid="desk-horizon"]')).toHaveCount(0);
  await form.locator('#ob-smtp-port').fill('587');
  await form.locator('#ob-mdp').fill('synthetic-new-password');
  await page.screenshot({ path: testInfo.outputPath('generic-repair.png') });
  await page.evaluate(() => {
    window.__e2eLog = [];
    window.__e2eFailure = ['repair_generic_account'];
    window.__e2eHoldCommands = ['repair_generic_account'];
    window.__e2eHold = new Promise(resolve => { window.releaseRepair = resolve; });
  });
  await form.locator('[data-testid="desk-continue"]').click();
  for (const selector of ['#ob-mdp', '#ob-imap-port', '#ob-smtp', '#ob-smtp-port']) {
    await expect(form.locator(selector)).toBeDisabled();
  }
  expect(await page.evaluate(() => window.__e2eLog
    .filter(call => call.command === 'repair_generic_account').length)).toBe(1);
  await page.evaluate(() => { delete window.__e2eHold; window.releaseRepair(); });
  await expect(form.locator('[data-testid="onboarding-error"]')).toBeVisible();
  await expect(form.locator('#ob-mdp')).toBeEnabled();
  await expect(form.locator('#ob-mdp')).toHaveValue('synthetic-new-password');
  await expect(form.locator('#ob-smtp-port')).toHaveValue('587');
  await form.getByRole('button', { name: 'Replier', exact: true }).click(); // lang:fr
  await page.locator('[data-testid="account-reconnect"]').click();
  await expect(form.locator('#ob-mdp')).toHaveValue('');
  await expect(form.locator('#ob-smtp-port')).toHaveValue('465');
});

test('a connected generic account also offers repair and the settings contain no password', async () => {
  ({ app, browser, page } = await launchAppV2({ accounts: [
    { email: 'owner@example.fr', messages: 1, generic: true },
  ] }));
  await page.locator('[data-testid="settings"]').click();
  await expect(page.locator('[data-testid="account-disconnected"]')).toHaveCount(0);
  await page.locator('[data-testid="account-reconnect"]').click();
  await expect(page.locator('[data-testid="generic-repair"]')).toBeVisible();
  await expect(page.locator('#ob-mdp')).toHaveValue('');
  const settings = await page.evaluate(async () => {
    const nav = await window.__TAURI__.core.invoke('nav_snapshot');
    return window.__TAURI__.core.invoke('generic_connection_settings', { accountId: nav[0].account_id });
  });
  expect(settings.username).toBe('custom-login');
  expect(Object.keys(settings).sort()).toEqual(['email', 'imapHost', 'imapPort', 'smtpHost', 'smtpPort', 'username']);
});

test.afterEach(async () => { await closeApp({ app, browser }); });
