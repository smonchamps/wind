// ADR 0029 (PLAN-HORIZON-NETTOYAGE, volet A) : la profondeur
// d'historique importée en local. À l'ajout d'un compte, le guichet
// offre le choix (défaut « 1 an », D2) ; un compte d'AVANT le réglage
// est réputé « tout » (D4) ; le choix se révise aux Réglages > Comptes
// (D3) et persiste en base — la porte de la rangée montre la valeur.
import { test, expect } from '@playwright/test';
import { launchAppV2, closeApp } from '../launch.mjs';

let app;
let browser;
let page;

test.describe.configure({ mode: 'serial' });

test.beforeAll(async () => {
  ({ app, browser, page } = await launchAppV2({
    comptes: [{ email: 'un@exemple.fr', messages: 4 }],
  }));
});

test.afterAll(async () => {
  await closeApp({ app, browser });
});

test('un compte d’avant le réglage est réputé « tout » (D4)', async () => {
  await expect(page.locator('[data-testid="row"]').first()).toBeVisible();
  await page.locator('[data-testid="settings"]').click();
  await expect(page.locator('[data-testid="account-horizon"]')).toHaveText(
    'Tout depuis le début',
  );
});

test('réviser l’horizon aux Réglages : la porte suit, et le choix survit à la fermeture', async () => {
  await page.locator('[data-testid="account-horizon"]').click();
  await expect(page.locator('[data-testid="settings-horizon"]')).toBeVisible();
  await page
    .locator('[data-testid="horizon-select"]')
    .selectOption('6m');
  // La porte de la rangée montre l'état choisi — application immédiate.
  await expect(page.locator('[data-testid="account-horizon"]')).toHaveText('6 mois');

  // La persistance se prouve au RETOUR : fermer la surimpression,
  // la rouvrir — la valeur vient de la BASE, pas d'un état d'écran.
  await page.locator('[data-testid="settings-done"]').click();
  await page.locator('[data-testid="settings"]').click();
  await expect(page.locator('[data-testid="account-horizon"]')).toHaveText('6 mois');
});

test('le guichet d’ajout offre le choix, défaut « 1 an » (D2)', async () => {
  await page.locator('[data-testid="settings-add"]').click();
  const select = page.locator('[data-testid="desk-horizon"]');
  await expect(select).toBeVisible();
  // Le défaut est « 1 an » (D2) — jamais « tout » : le choix d'importer
  // moins est le comportement livré, celui d'importer tout un geste.
  await expect(select).toHaveValue('1a');
  await expect(select.locator('option')).toHaveCount(7);
  await expect(select.locator('option').last()).toHaveText('Tout depuis le début');
  await page.locator('[data-testid="settings-done"]').click();
});
