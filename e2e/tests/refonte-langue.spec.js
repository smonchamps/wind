// La langue de l'interface (PLAN-LANGUES, E1) : audit des catalogues
// (mêmes clés en fr et en — le repli ne doit jamais servir en
// production), bascule immédiate depuis Réglages > Affichage, balayage
// des écrans majeurs en anglais, aller-retour RÉEL (recharger relit la
// préférence en base, pas un état de composant), puis retour au
// français — la langue canonique des autres parcours (L-6).
import { test, expect } from '@playwright/test';
import { launchAppV2, closeApp } from '../launch.mjs';
import { FR } from '../../apps/desktop/ui-v2/src/lib/catalog.fr.js';
import { EN } from '../../apps/desktop/ui-v2/src/lib/catalog.en.js';

let app;
let browser;
let page;

test.describe.configure({ mode: 'serial' });

test.beforeAll(async () => {
  ({ app, browser, page } = await launchAppV2());
});

test.afterAll(async () => {
  await closeApp({ app, browser });
});

test('les catalogues fr et en portent exactement les mêmes clés', () => {
  expect(Object.keys(EN).sort()).toEqual(Object.keys(FR).sort());
});

// D4 (E5b): English is the default; the suite pins the WebView to `--lang=fr`
// (launch.mjs), so the first launch DETECTS French — the default itself is
// tested by e2e/language.test.mjs.
test('the pinned French system language is detected at the first launch', async () => {
  await expect(page.locator('[data-testid="ligne"]').first()).toBeVisible();
  await expect(page.locator('[data-testid="ecrire"]')).toContainText('Écrire');
  await expect(page.locator('html')).toHaveAttribute('lang', 'fr');
});

test('la bascule en anglais est immédiate, sans redémarrage', async () => {
  await page.locator('[data-testid="reglages"]').click();
  await page.locator('[data-testid="reglages-groupe"][data-groupe="affichage"]').click();
  const choix = page.locator('[data-testid="affichage-langue"]');
  await expect(choix).toHaveValue('fr');
  await choix.selectOption('en');
  // La surimpression elle-même a déjà basculé…
  await expect(page.locator('[data-testid="reglages-termine"]')).toHaveText('Done');
  await page.locator('[data-testid="reglages-termine"]').click();
  // …et les écrans majeurs aussi : entête, nav, onglets, volet, langue
  // du document (lecteurs d'écran).
  await expect(page.locator('[data-testid="ecrire"]')).toContainText('Compose');
  await expect(page.locator('[data-testid="nav-dossier"][data-categorie="inbox"]')).toContainText('Inbox');
  await expect(page.locator('[data-testid="nav-dossier"][data-categorie="trash"]')).toContainText('Trash');
  await expect(page.locator('[data-testid="onglet"][data-onglet="nonlus"]')).toContainText('Unread');
  await expect(page.locator('[data-testid="volet-lecture"]')).toContainText('Select a message to read it.');
  await expect(page.locator('html')).toHaveAttribute('lang', 'en');
});

test("l'aller-retour est réel : recharger relit la préférence en base", async () => {
  await page.reload();
  await expect(page.locator('[data-testid="ligne"]').first()).toBeVisible();
  await expect(page.locator('[data-testid="ecrire"]')).toContainText('Compose');
  await expect(page.locator('html')).toHaveAttribute('lang', 'en');
});

test('le retour au français rétablit les formes exactes du prototype', async () => {
  await page.locator('[data-testid="reglages"]').click();
  await page.locator('[data-testid="reglages-groupe"][data-groupe="affichage"]').click();
  await page.locator('[data-testid="affichage-langue"]').selectOption('fr');
  await expect(page.locator('[data-testid="reglages-termine"]')).toHaveText('Terminé');
  await page.locator('[data-testid="reglages-termine"]').click();
  await expect(page.locator('[data-testid="ecrire"]')).toContainText('Écrire');
  await expect(page.locator('[data-testid="volet-lecture"]')).toContainText('Sélectionnez un message pour le lire.');
  await expect(page.locator('html')).toHaveAttribute('lang', 'fr');
});

// D4 (E5b, 2026-09-03): English is the default of a REAL first launch —
// an empty database, a WebView whose locale is neither French nor
// English. The pure decision is unit-tested; this is the wiring
// (`lang_get` → `detectLanguage` → `<html lang>` → the onboarding text).
test('a first launch on a non-French system speaks English (D4)', async () => {
  await closeApp({ app, browser });
  ({ app, browser, page } = await launchAppV2({ vierge: true, lang: 'de-DE' }));
  // The suite's WebView2 profile already carries the onboarding-done flag
  // (localStorage), so the empty database opens on the header, not on the
  // onboarding: the header is the witness.
  await expect(page.locator('[data-testid="champ-recherche"]')).toHaveAttribute('placeholder', 'Search messages, people, files');
  await expect(page.locator('html')).toHaveAttribute('lang', 'en');
});
