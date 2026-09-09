import { test, expect } from '@playwright/test';
import { launchAppV2, closeApp, purgeLocals } from '../launch.mjs';

let app, browser, page;

for (const surface of ['onboarding', 'settings', 'reconnect']) {
  const onboarding = surface === 'onboarding';
  const reconnect = surface === 'reconnect';
  test(`${surface}: live manual OAuth can be cancelled and retried`, async ({}, testInfo) => {
    ({ app, browser, page } = await launchAppV2(onboarding ? { fresh: true, lang: 'fr' }
      : { accounts: [{ email: 'fixture@example.fr', messages: 1 },
        ...(reconnect ? [{ email: 'dead@gmail.com', messages: 1, disconnected: true }] : [])], lang: 'fr' }));
    if (onboarding) {
      await purgeLocals(page, ['wind-accueil-fait', 'wind-accueil-commence']);
      await page.reload();
    } else {
      await page.locator('[data-testid="settings"]').click();
      if (!reconnect) await page.locator('[data-testid="settings-add"]').click();
    }
    await page.evaluate(() => {
      Object.defineProperty(navigator, 'clipboard', { configurable: true, value: {
        writeText: async (text) => { window.copiedAuthorization = text; },
      } });
      let id = 0;
      let reject;
      window.oauthCalls = [];
      window.__e2eOAuth = (command, args) => {
        window.oauthCalls.push({ command, flowId: args?.flowId });
        if (command === 'oauth_begin') return Promise.resolve(String(++id));
        if (command === 'oauth_status') return Promise.resolve({ manual: true,
          url: `https://accounts.example.invalid/authorize?fixture=${id}` });
        if (command === 'oauth_cancel') {
          reject?.(new Error('authorization cancelled'));
          return Promise.resolve(true);
        }
        return new Promise((_, fail) => { reject = fail; });
      };
    });
    if (!reconnect) await page.locator('[data-testid="onboarding-address"]').fill('fixture@gmail.com');
    const submit = page.locator(reconnect ? '[data-testid="account-reconnect"]' : '[data-testid="desk-continue"]');
    await submit.click();
    const link = page.getByLabel('Lien d’autorisation', { exact: true }); // lang:fr
    await expect(link).toHaveValue('https://accounts.example.invalid/authorize?fixture=1');
    await expect(link).toHaveAttribute('readonly', '');
    if (reconnect) {
      const address = page.locator('.account .address', { hasText: 'dead@gmail.com' });
      expect((await address.boundingBox()).width).toBeGreaterThan(80);
      expect(await page.locator('[data-testid="settings-pane"]').evaluate(
        element => element.scrollWidth - element.clientWidth)).toBeLessThanOrEqual(1);
    }
    await page.screenshot({ path: testInfo.outputPath('oauth-manual.png') });
    await page.getByRole('button', { name: 'Copier le lien', exact: true }).click(); // lang:fr
    await expect(page.getByRole('button', { name: 'Lien copié', exact: true })).toBeVisible(); // lang:fr
    expect(await page.evaluate(() => window.copiedAuthorization))
      .toBe('https://accounts.example.invalid/authorize?fixture=1');
    await page.getByRole('button', { name: 'Annuler la connexion', exact: true }).click(); // lang:fr
    await expect(submit).toBeEnabled();
    await expect(link).toHaveCount(0);
    await expect(page.locator('[data-testid="onboarding-error"]')).toHaveCount(0);
    await submit.click();
    await expect(link).toHaveValue('https://accounts.example.invalid/authorize?fixture=2');
    if (onboarding) {
      await page.getByRole('button', { name: 'Annuler la connexion', exact: true }).click(); // lang:fr
    } else {
      await page.keyboard.press('Escape');
      await expect.poll(() => page.evaluate(() => window.oauthCalls
        .some(call => call.command === 'oauth_cancel' && call.flowId === '2'))).toBe(true);
    }
  });
}

// Backlog 86 (E6): a PARTIAL Google consent (mail box unticked) used
// to surface the raw Rust error, in English, inside the French desk.
// The failure now crosses the IPC with the stable code
// `missing_mail_scope` (fault.rs, the app_location_readonly
// convention) and the desk speaks the catalogue's language — telling
// the user which box to tick, never the raw diagnostic.
test('a partial consent speaks the catalogue language, never the wire marker', async () => {
  ({ app, browser, page } = await launchAppV2({ fresh: true, lang: 'fr' }));
  await purgeLocals(page, ['wind-accueil-fait', 'wind-accueil-commence']);
  await page.reload();
  await page.evaluate(() => {
    let id = 0;
    window.__e2eOAuth = (command) => {
      if (command === 'oauth_begin') return Promise.resolve(String(++id));
      if (command === 'oauth_status') return Promise.resolve({ manual: true,
        url: `https://accounts.example.invalid/authorize?fixture=${id}` });
      if (command === 'oauth_cancel') return Promise.resolve(true);
      // The typed wire shape fault.rs serializes for coded errors.
      return Promise.reject({
        code: 'missing_mail_scope', retryable: false,
        message: 'the Google consent does not include mail access (granted: []) — start again and tick the corresponding box on the authorization screen',
      });
    };
  });
  await page.locator('[data-testid="onboarding-address"]').fill('fixture@gmail.com');
  await page.locator('[data-testid="desk-continue"]').click();
  const error = page.locator('[data-testid="onboarding-error"]');
  await expect(error).toBeVisible();
  await expect(error).toContainText('Google'); // lang:fr — the localized message names the provider
  await expect(error).toContainText('cochez'); // lang:fr — and says which gesture fixes it
  await expect(error).not.toContainText('missing_mail_scope');
  await expect(error).not.toContainText('granted:');
});

test.afterEach(async () => {
  await purgeLocals(page, ['wind-accueil-fait', 'wind-accueil-commence']);
  await closeApp({ app, browser });
});
