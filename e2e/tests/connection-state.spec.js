import { test, expect } from '@playwright/test';
import { launchAppV2, closeApp } from '../launch.mjs';

let app, browser, page;
test.beforeAll(async ({}, testInfo) => {
  testInfo.setTimeout(180000);
  ({ app, browser, page } = await launchAppV2({ accounts: [
    { email: 'owner@example.fr', messages: 1 },
  ] }));
});
test.afterAll(async () => { await closeApp({ app, browser }); });

test('Settings follows connection failures, recovery and loss of the stored session', async ({}, testInfo) => {
  await page.evaluate(() => {
    const original = window.fetch;
    window.connectionFixture = 'unavailable';
    window.fetch = async (...args) => {
      const response = await original(...args);
      if (new URL(String(args[0]), location.href).pathname !== '/ui_state') return response;
      const state = await response.json();
      const id = state.nav[0].account_id;
      state.connected = window.connectionFixture === 'disconnected' ? [] : ['owner@example.fr'];
      state.connection_states = { [id]: window.connectionFixture };
      return new Response(JSON.stringify(state), { headers: response.headers });
    };
  });
  await page.locator('[data-testid="settings"]').click();
  await expect(page.locator('[data-testid="account-connection-unavailable"]')).toHaveText('Connection unavailable', { timeout: 15000 });
  await expect(page.locator('[data-testid="account-reconnect"]')).toBeVisible();
  await page.screenshot({ path: testInfo.outputPath('connection-unavailable.png') });
  await page.evaluate(() => { window.connectionFixture = 'available'; });
  await expect(page.locator('[data-testid="account-connection-unavailable"]')).toHaveCount(0, { timeout: 15000 });
  await expect(page.locator('[data-testid="account-reconnect"]')).toHaveCount(0);
  await page.evaluate(() => { window.connectionFixture = 'disconnected'; });
  await expect(page.locator('[data-testid="account-disconnected"]')).toBeVisible({ timeout: 15000 });
  await expect(page.locator('[data-testid="account-reconnect"]')).toBeVisible();
});
