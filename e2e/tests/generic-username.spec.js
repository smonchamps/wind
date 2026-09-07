import { test, expect } from '@playwright/test';
import { launchAppV2, closeApp, purgeLocals } from '../launch.mjs';

let app;
let browser;
let page;

for (const onboarding of [true, false]) {
  test(`${onboarding ? 'onboarding' : 'settings'}: generic identity is explicit and the checked form stays frozen`, async ({}, testInfo) => {
    ({ app, browser, page } = await launchAppV2(onboarding
      ? { fresh: true, lang: 'fr' }
      : { accounts: [{ email: 'fixture@example.fr', messages: 1 }], lang: 'fr' }));
    if (onboarding) {
      await purgeLocals(page, ['wind-accueil-fait', 'wind-accueil-commence']);
      await page.reload();
    } else {
      await page.locator('[data-testid="settings"]').click();
      await page.locator('[data-testid="settings-add"]').click();
    }
    await page.locator('[data-testid="onboarding-address"]').fill('contact@exemple.fr');
    await page.locator('[data-testid="desk-continue"]').click();
    const username = page.getByLabel('Identifiant de connexion', { exact: true }); // lang:fr
    await expect(username).toHaveValue('contact@exemple.fr');
    await expect(username).toHaveAccessibleDescription(
      'Votre adresse e-mail, sauf si votre fournisseur indique un autre identifiant.'); // lang:fr
    await page.locator('[data-testid="onboarding-address"]').fill('service@exemple.fr');
    await expect(username).toHaveValue('service@exemple.fr');
    await username.fill('service-client');
    await page.locator('[data-testid="onboarding-address"]').fill('contact@exemple.fr');
    await expect(username).toHaveValue('service-client');
    await page.screenshot({ path: testInfo.outputPath('generic-username.png') });
    await page.locator('#ob-mdp').fill('synthetic-secret');
    await page.evaluate(() => {
      window.__e2eLog = [];
      window.__e2eFailure = ['add_generic_account'];
      window.__e2eHoldCommands = ['add_generic_account'];
      window.__e2eHold = new Promise((resolve) => { window.releaseGeneric = resolve; });
    });
    await page.locator('[data-testid="desk-continue"]').click();
    for (const selector of ['[data-testid="onboarding-address"]', '#ob-username', '#ob-mdp',
      '#ob-imap', '#ob-imap-port', '#ob-smtp', '#ob-smtp-port', '[data-testid="desk-horizon"]']) {
      await expect(page.locator(selector)).toBeDisabled();
    }
    const sent = await page.evaluate(() => window.__e2eLog.filter((entry) => entry.command === 'add_generic_account'));
    expect(sent).toHaveLength(1);
    expect(sent[0].username).toBe('service-client');
    await page.evaluate(() => { delete window.__e2eHold; window.releaseGeneric(); });
    await expect(page.locator('[data-testid="onboarding-error"]')).toBeVisible();
    await expect(username).toBeEnabled();
    await expect(username).toHaveValue('service-client');
    await expect(page.locator('#ob-mdp')).toHaveValue('synthetic-secret');

    await username.fill('');
    await page.evaluate(() => { window.__e2eFailure = ['add_generic_account']; });
    await page.locator('[data-testid="desk-continue"]').click();
    await expect.poll(() => page.evaluate(() => window.__e2eLog
      .filter((entry) => entry.command === 'add_generic_account').at(-1)?.username)).toBe('contact@exemple.fr');
  });
}

test.afterEach(async () => {
  await purgeLocals(page, ['wind-accueil-fait', 'wind-accueil-commence']);
  await closeApp({ app, browser });
});
