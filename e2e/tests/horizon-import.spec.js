// ADR 0029 (PLAN-HORIZON-NETTOYAGE, panel A): the depth
// of history imported locally. When adding an account, the desk
// offers the choice (default "1 year", D2); an account from BEFORE the setting
// is deemed "everything" (D4); the choice is revised in Settings > Accounts
// (D3) and persists to the database — the row's door shows the value.
import { test, expect } from '@playwright/test';
import { launchAppV2, closeApp } from '../launch.mjs';

let app;
let browser;
let page;

test.describe.configure({ mode: 'serial' });

test.beforeAll(async () => {
  ({ app, browser, page } = await launchAppV2({
    accounts: [{ email: 'un@exemple.fr', messages: 4 }],
  }));
});

test.afterAll(async () => {
  await closeApp({ app, browser });
});

test('an account from before the setting is deemed "everything" (D4)', async () => {
  await expect(page.locator('[data-testid="row"]').first()).toBeVisible();
  await page.locator('[data-testid="settings"]').click();
  await expect(page.locator('[data-testid="account-horizon"]')).toHaveText(
    'Everything from the beginning',
  );
});

test('revising the horizon in Settings: the door follows, and the choice survives closing', async () => {
  await page.locator('[data-testid="account-horizon"]').click();
  await expect(page.locator('[data-testid="settings-horizon"]')).toBeVisible();
  await page
    .locator('[data-testid="horizon-select"]')
    .selectOption('6m');
  // The row's door shows the chosen state — immediate application.
  await expect(page.locator('[data-testid="account-horizon"]')).toHaveText('6 months');

  // Persistence is proven on the RETURN: close the overlay,
  // reopen it — the value comes from the DATABASE, not a screen state.
  await page.locator('[data-testid="settings-done"]').click();
  await page.locator('[data-testid="settings"]').click();
  await expect(page.locator('[data-testid="account-horizon"]')).toHaveText('6 months');
});

test('the add desk offers the choice, default "1 year" (D2)', async () => {
  await page.locator('[data-testid="settings-add"]').click();
  const select = page.locator('[data-testid="desk-horizon"]');
  await expect(select).toBeVisible();
  // The default is "1 year" (D2) — never "everything": the choice to import
  // less is the shipped behavior, importing everything is a deliberate action.
  await expect(select).toHaveValue('1a');
  await expect(select.locator('option')).toHaveCount(7);
  await expect(select.locator('option').last()).toHaveText('Everything from the beginning');
  await page.locator('[data-testid="settings-done"]').click();
});
