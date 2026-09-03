// PLAN-RETOURS-12 — retours CE du 2026-08-28.
//
// R1 : un compte ajouté pendant que Wind est ouvert se disait
// « Déconnecté » aux Réglages — le cœur posait bien la session à
// l'ajout (commands.rs, add_oauth_account), mais `compteAjoute()` ne
// rafraîchissait jamais le tableau `connectes`, rempli une seule fois
// au démarrage. Le décor rejoue la racine : le compte est en base et
// sa session vit côté cœur, l'UI ne le sait pas encore.
//
// Le consentement OAuth n'est pas pilotable par Playwright : la
// couture `__e2eAjout` (transport.js, patron __e2ePieces) fait réussir
// l'ajout sans navigateur et fait porter l'adresse par le bilan de
// `connect_accounts` — posée APRÈS le démarrage, elle ne touche pas la
// connexion initiale.
import { test, expect } from '@playwright/test';
import { launchAppV2, closeApp } from '../launch.mjs';

let app;
let browser;
let page;

test.describe.configure({ mode: 'serial' });

test.beforeAll(async () => {
  ({ app, browser, page } = await launchAppV2({
    comptes: [
      { email: 'principal@exemple.fr', messages: 4 },
      { email: 'neuf@gmail.com', messages: 2, deconnecte: true },
    ],
  }));
});

test.afterAll(async () => {
  await closeApp({ app, browser });
});

test("un compte ajouté Wind ouvert est dit connecté aux Réglages, sans redémarrer", async () => {
  await expect(page.locator('[data-testid="ligne"]').first()).toBeVisible();
  await page.locator('[data-testid="reglages"]').click();

  // Le décor dit le bug en puissance : la session du compte n'est pas
  // encore connue de l'UI.
  const rangees = page.locator('[data-testid="reglages-comptes"]');
  await expect(rangees).toContainText('neuf@gmail.com');
  await expect(page.locator('[data-testid="compte-deconnecte"]')).toHaveCount(1);

  // L'ajout, par le vrai guichet des Réglages — la couture ne remplace
  // que le consentement navigateur. Le journal prouve que l'UI RELIT
  // vraiment le cœur (le bilan est complété par la couture : sans cette
  // preuve, le badge pourrait tomber pour une mauvaise raison).
  await page.evaluate(() => {
    window.__e2eAjout = ['neuf@gmail.com'];
    window.__e2eJournal = [];
  });
  await page.locator('[data-testid="reglages-ajouter"]').click();
  await page.locator('[data-testid="onboarding-adresse"]').fill('neuf@gmail.com');
  await page.locator('[data-testid="reglages-guichet"] [data-testid="onboarding-continuer"]').click();

  // Ce que l'utilisateur DOIT voir : plus aucun « Déconnecté » — le
  // compte vient d'être connecté.
  await expect(page.locator('[data-testid="compte-deconnecte"]')).toHaveCount(0);
  const relectures = await page.evaluate(() => {
    const n = window.__e2eJournal.filter((r) => r.command === 'connect_accounts').length;
    // La couture ne survit pas au test : les cycles suivants du décor
    // repassent par le vrai chemin.
    delete window.__e2eAjout;
    delete window.__e2eJournal;
    return n;
  });
  expect(relectures).toBeGreaterThanOrEqual(1);
});
