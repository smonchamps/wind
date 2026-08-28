// PLAN-RETOURS-12 R5 — l'entête du message déplié, en deux lignes :
//   ligne 1 : Nom de l'expéditeur <adresse> [sur Boîte — règle D7]
//   ligne 2 : À : Nom <adresse>, … (et « Cc : … » si des Cc existent, D6)
//
// Les noms des destinataires viennent de l'ANNUAIRE des correspondants
// (décision D4 — to_addrs ne stocke que des adresses nues) ; une adresse
// inconnue s'affiche nue. Décor Clarity : Camille et Sofia sont des
// expéditrices vues, l'annuaire connaît donc leurs noms.
import { test, expect } from '@playwright/test';
import { launchAppV2, closeApp } from '../launch.mjs';

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

test("l'entête déplié dit « Nom <adresse> » puis « À : Nom <adresse> » — le nom vient de l'annuaire", async () => {
  await page.locator('[data-testid="ligne"]').first().click();
  const volet = page.locator('[data-testid="volet-lecture"]');
  const deplie = volet.locator('[data-testid="message-deplie"]');

  // Ligne 1 — l'expéditrice du dernier message du fil Vantis, nom ET
  // adresse sur la même ligne.
  await expect(deplie.locator('.auteur')).toHaveText('Camille Rousseau');
  await expect(deplie.locator('.adr-exp')).toHaveText('<c.rousseau@atelier-nord.fr>');

  // Ligne 2 — « À : » ; sans destinataires stockés (vieux courrier), le
  // repli est l'heuristique du prototype, au MÊME format Nom <adresse>.
  await expect(deplie.locator('[data-testid="ligne-a"]')).toHaveText(
    'À : Paul Mérand <paul.merand@atelier-nord.fr>',
  );
  // Pas de Cc sur ce message : la ligne n'existe pas.
  await expect(deplie.locator('[data-testid="ligne-cc"]')).toHaveCount(0);
  // L'heure longue ne bouge pas.
  await expect(deplie.locator('.tete-message .quand')).toHaveText(/^Aujourd'hui, 09:12$/);
});

test('notre propre message dit ses destinataires stockés, noms résolus — et sa ligne Cc (D6)', async () => {
  const volet = page.locator('[data-testid="volet-lecture"]');
  // Déplier notre réponse (avatar PM, la première carte repliée).
  await volet.locator('[data-testid="message-replie"]').first().click();
  const notre = volet.locator('[data-testid="message-deplie"]').first();

  await expect(notre.locator('.auteur')).toHaveText('Paul Mérand');
  await expect(notre.locator('.adr-exp')).toHaveText('<paul.merand@atelier-nord.fr>');
  // À/Cc stockés (R4) + noms de l'annuaire (les deux sont des
  // expéditrices vues du décor).
  await expect(notre.locator('[data-testid="ligne-a"]')).toHaveText(
    'À : Camille Rousseau <c.rousseau@atelier-nord.fr>',
  );
  await expect(notre.locator('[data-testid="ligne-cc"]')).toHaveText(
    'Cc : Sofia Nardi <s.nardi@atelier-nord.fr>',
  );
});
