// PLAN-HORIZON-NETTOYAGE volet B — le Nettoyage de printemps, 5e
// section du Mode organisé. L'intro (plage + périmètre + Démarrer),
// le tri par GROUPES d'expéditeur au vocabulaire du Portier (le
// verdict vaut pour le groupe : stock de la plage ET avenir — D5), la
// barre de progression en haut, la navigation DANS un groupe (voir,
// jamais trier au message), la session PERSISTÉE (D8 : reprise après
// rechargement), et la sortie propre.
import { test, expect } from '@playwright/test';
import { launchAppV2, closeApp } from '../launch.mjs';

let app;
let browser;
let page;

test.describe.configure({ mode: 'serial' });

test.beforeAll(async () => {
  ({ app, browser, page } = await launchAppV2({
    comptes: [{ email: 'principal@exemple.fr', messages: 6 }],
  }));
});

test.afterAll(async () => {
  await closeApp({ app, browser });
});

test('la 5e section n’existe qu’en mode organisé, et son intro dit le texte CE', async () => {
  await expect(page.locator('[data-testid="row"]').first()).toBeVisible();
  // Au classique : pas de Nettoyage.
  await expect(
    page.locator('[data-testid="nav-folder"][data-category="cleanup"]'),
  ).toHaveCount(0);

  await page.locator('[data-testid="organized-mode"]').click();
  const rang = page.locator('[data-testid="nav-folder"][data-category="cleanup"]');
  await expect(rang).toContainText('Nettoyage de printemps');
  await rang.click();

  // L'intro : titre avec glyphe, sous-texte CE mot pour mot, plage
  // (défaut 1 an), périmètre (défaut Réception seule), Démarrer.
  await expect(page.locator('[data-testid="cleanup-title"] svg')).toHaveCount(1);
  await expect(page.locator('[data-testid="cleanup"]')).toContainText(
    'En lançant un nettoyage de printemps, vous allez pouvoir trier vos archives',
  );
  await expect(page.locator('[data-testid="cleanup-range"]')).toHaveCount(6);
  await expect(
    page.locator('[data-testid="cleanup-range"][data-range="1a"]'),
  ).toHaveAttribute('aria-checked', 'true');
  await expect(page.locator('[data-testid="cleanup-scope"]')).toHaveCount(4);
  await expect(
    page.locator('[data-testid="cleanup-scope"][data-scope="inbox"]'),
  ).toHaveAttribute('aria-checked', 'true');
  await expect(page.locator('[data-testid="cleanup-start"]')).toBeVisible();
});

test('démarrer ouvre le tri : groupes par expéditeur, progression à 0 %, navigation dans un groupe', async () => {
  // Le décor semé date de 2020 (gabarit) : la plage « tout » couvre —
  // et prouve au passage que le choix de plage est bien envoyé.
  await page.locator('[data-testid="cleanup-range"][data-range="all"]').click();
  await page.locator('[data-testid="cleanup-start"]').click();
  await expect(page.locator('[data-testid="cleanup-group"]').first()).toBeVisible();
  await expect(page.locator('[data-testid="cleanup-progress"]')).toContainText('0 %');

  // Naviguer dans un groupe : ses messages se montrent — et se replient.
  await page.locator('[data-testid="cleanup-open"]').first().click();
  await expect(page.locator('[data-testid="cleanup-messages"]')).toBeVisible();
  await page.locator('[data-testid="cleanup-open"]').first().click();
  await expect(page.locator('[data-testid="cleanup-messages"]')).toHaveCount(0);
});

test('le Oui traite le groupe entier ; le Non fait quitter la Réception à son courrier (D5)', async () => {
  const groupes = page.locator('[data-testid="cleanup-group"]');
  const avant = await groupes.count();
  expect(avant).toBeGreaterThan(1);

  // Oui de groupe : il quitte la liste, la progression avance.
  await page.locator('[data-testid="cleanup-yes"]').first().click();
  await expect(groupes).toHaveCount(avant - 1);
  await expect(page.locator('[data-testid="cleanup-progress"]')).not.toContainText('0 %');

  // Non (défaut livré : Corbeille) : le groupe part, et son STOCK de
  // la plage quitte la boîte locale — la Réception ne le montre plus.
  const nomNon = await groupes.first().locator('.sender').innerText();
  await page.locator('[data-testid="cleanup-no"]').first().click();
  await expect(groupes).toHaveCount(avant - 2);
  await page.locator('[data-testid="nav-folder"][data-category="inbox"]').click();
  await expect(page.locator('[data-testid="list"]')).not.toContainText(nomNon);
});

test('la session PERSISTE (D8) : un rechargement reprend le tri où il en était', async () => {
  await page.reload();
  await expect(page.locator('[data-testid="organized-mode"]')).toHaveAttribute(
    'aria-checked',
    'true',
  );
  await page.locator('[data-testid="nav-folder"][data-category="cleanup"]').click();
  // Pas l'intro : le tri, avec sa progression déjà entamée.
  await expect(page.locator('[data-testid="cleanup-start"]')).toHaveCount(0);
  await expect(page.locator('[data-testid="cleanup-progress"]')).not.toContainText('0 %');
});

test('terminer rend l’intro ; quitter le mode rend la nav classique', async () => {
  await page.locator('[data-testid="cleanup-finish"]').click();
  await expect(page.locator('[data-testid="cleanup-start"]')).toBeVisible();

  await page.locator('[data-testid="organized-mode"]').click();
  await expect(page.locator('[data-testid="nav-folder"]')).toHaveCount(6);
  await expect(
    page.locator('[data-testid="nav-folder"][data-category="cleanup"]'),
  ).toHaveCount(0);
});
