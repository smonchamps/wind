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
  await expect(page.locator('[data-testid="ligne"]').first()).toBeVisible();
  // Au classique : pas de Nettoyage.
  await expect(
    page.locator('[data-testid="nav-dossier"][data-categorie="nettoyage"]'),
  ).toHaveCount(0);

  await page.locator('[data-testid="mode-organise"]').click();
  const rang = page.locator('[data-testid="nav-dossier"][data-categorie="nettoyage"]');
  await expect(rang).toContainText('Nettoyage de printemps');
  await rang.click();

  // L'intro : titre avec glyphe, sous-texte CE mot pour mot, plage
  // (défaut 1 an), périmètre (défaut Réception seule), Démarrer.
  await expect(page.locator('[data-testid="nettoyage-titre"] svg')).toHaveCount(1);
  await expect(page.locator('[data-testid="nettoyage"]')).toContainText(
    'En lançant un nettoyage de printemps, vous allez pouvoir trier vos archives',
  );
  await expect(page.locator('[data-testid="nettoyage-plage"]')).toHaveCount(6);
  await expect(
    page.locator('[data-testid="nettoyage-plage"][data-plage="1a"]'),
  ).toHaveAttribute('aria-checked', 'true');
  await expect(page.locator('[data-testid="nettoyage-perimetre"]')).toHaveCount(4);
  await expect(
    page.locator('[data-testid="nettoyage-perimetre"][data-perimetre="reception"]'),
  ).toHaveAttribute('aria-checked', 'true');
  await expect(page.locator('[data-testid="nettoyage-demarrer"]')).toBeVisible();
});

test('démarrer ouvre le tri : groupes par expéditeur, progression à 0 %, navigation dans un groupe', async () => {
  // Le décor semé date de 2020 (gabarit) : la plage « tout » couvre —
  // et prouve au passage que le choix de plage est bien envoyé.
  await page.locator('[data-testid="nettoyage-plage"][data-plage="tout"]').click();
  await page.locator('[data-testid="nettoyage-demarrer"]').click();
  await expect(page.locator('[data-testid="nettoyage-groupe"]').first()).toBeVisible();
  await expect(page.locator('[data-testid="nettoyage-progression"]')).toContainText('0 %');

  // Naviguer dans un groupe : ses messages se montrent — et se replient.
  await page.locator('[data-testid="nettoyage-ouvrir"]').first().click();
  await expect(page.locator('[data-testid="nettoyage-messages"]')).toBeVisible();
  await page.locator('[data-testid="nettoyage-ouvrir"]').first().click();
  await expect(page.locator('[data-testid="nettoyage-messages"]')).toHaveCount(0);
});

test('le Oui traite le groupe entier ; le Non fait quitter la Réception à son courrier (D5)', async () => {
  const groupes = page.locator('[data-testid="nettoyage-groupe"]');
  const avant = await groupes.count();
  expect(avant).toBeGreaterThan(1);

  // Oui de groupe : il quitte la liste, la progression avance.
  await page.locator('[data-testid="nettoyage-oui"]').first().click();
  await expect(groupes).toHaveCount(avant - 1);
  await expect(page.locator('[data-testid="nettoyage-progression"]')).not.toContainText('0 %');

  // Non (défaut livré : Corbeille) : le groupe part, et son STOCK de
  // la plage quitte la boîte locale — la Réception ne le montre plus.
  const nomNon = await groupes.first().locator('.exp').innerText();
  await page.locator('[data-testid="nettoyage-non"]').first().click();
  await expect(groupes).toHaveCount(avant - 2);
  await page.locator('[data-testid="nav-dossier"][data-categorie="reception"]').click();
  await expect(page.locator('[data-testid="liste"]')).not.toContainText(nomNon);
});

test('la session PERSISTE (D8) : un rechargement reprend le tri où il en était', async () => {
  await page.reload();
  await expect(page.locator('[data-testid="mode-organise"]')).toHaveAttribute(
    'aria-checked',
    'true',
  );
  await page.locator('[data-testid="nav-dossier"][data-categorie="nettoyage"]').click();
  // Pas l'intro : le tri, avec sa progression déjà entamée.
  await expect(page.locator('[data-testid="nettoyage-demarrer"]')).toHaveCount(0);
  await expect(page.locator('[data-testid="nettoyage-progression"]')).not.toContainText('0 %');
});

test('terminer rend l’intro ; quitter le mode rend la nav classique', async () => {
  await page.locator('[data-testid="nettoyage-terminer"]').click();
  await expect(page.locator('[data-testid="nettoyage-demarrer"]')).toBeVisible();

  await page.locator('[data-testid="mode-organise"]').click();
  await expect(page.locator('[data-testid="nav-dossier"]')).toHaveCount(6);
  await expect(
    page.locator('[data-testid="nav-dossier"][data-categorie="nettoyage"]'),
  ).toHaveCount(0);
});
