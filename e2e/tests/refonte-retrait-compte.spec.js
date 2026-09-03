// Retrait d'un compte depuis les Réglages : le geste est destructeur
// localement (courrier local effacé, connexion oubliée — jamais le
// serveur), donc il se CONFIRME sur place, et tout ce qui montrait le
// compte se replie : rangée des Réglages, boîte de la nav, liste.
//
// Suite à part : elle MUTILE son décor (un compte disparaît pour de
// bon) — la partager avec une autre suite sérialisée ferait dépendre
// leurs assertions de l'ordre d'exécution.
import { test, expect } from '@playwright/test';
import { launchAppV2, closeApp } from '../launch.mjs';

let app;
let browser;
let page;

test.describe.configure({ mode: 'serial' });

test.beforeAll(async () => {
  ({ app, browser, page } = await launchAppV2({
    comptes: [
      { email: 'un@exemple.fr', messages: 6 },
      { email: 'deux@exemple.fr', messages: 4 },
    ],
  }));
});

test.afterAll(async () => {
  await closeApp({ app, browser });
});

test("le retrait se confirme — et l'annulation ne touche à rien", async () => {
  await expect(page.locator('[data-testid="row"]').first()).toBeVisible();
  // A80/D7 : DEUX comptes, donc les comptes se mélangent — chaque
  // rangée dit sa boîte. Le retrait, plus bas, en fera disparaître un :
  // c'est le pendant de cette assertion.
  await expect(page.locator('[data-testid="row-mailbox"]').first()).toBeVisible();
  // 6 + 4 messages, un fil sur cinq (uid 5 répond au 4) : 5 + 4
  // conversations. Le total a quitté la nav (A29, W2-D4) — il se lit
  // à la ligne de perf de la barre de statut.
  await expect(page.locator('[data-testid="perf"]')).toContainText('9 conversations');
  await page.locator('[data-testid="settings"]').click();
  await expect(page.locator('[data-testid="account-remove"]')).toHaveCount(2);

  // PLAN-RETOURS-9 (D2) : le geste se DIT — l'icône porte son texte,
  // dans le vocabulaire du produit (« retirer », rien n'est supprimé
  // du serveur).
  await expect(page.locator('[data-testid="account-remove"]').first()).toContainText(
    'Retirer le compte',
  );

  // Premier clic : la carte de confirmation, pas le retrait.
  await page.locator('[data-testid="account-remove"]').first().click();
  await expect(page.locator('[data-testid="settings-removal"]')).toBeVisible();
  await expect(page.locator('[data-testid="settings-removal"]')).toContainText(
    'un@exemple.fr',
  );
  await expect(page.locator('[data-testid="settings-removal"]')).toContainText(
    'Rien n’est supprimé sur le serveur',
  );

  // Annuler : la carte se replie, les deux comptes sont toujours là.
  await page.locator('[data-testid="removal-cancel"]').click();
  await expect(page.locator('[data-testid="settings-removal"]')).toHaveCount(0);
  await expect(page.locator('[data-testid="account-remove"]')).toHaveCount(2);
  await page.locator('[data-testid="settings-done"]').click();
});

test('confirmé : le compte quitte les Réglages, la nav et la liste', async () => {
  await page.locator('[data-testid="settings"]').click();

  // Le second compte (deux@exemple.fr) part ; le premier reste.
  await page.locator('[data-testid="account-remove"]').nth(1).click();
  await expect(page.locator('[data-testid="settings-removal"]')).toContainText(
    'deux@exemple.fr',
  );
  await page.locator('[data-testid="removal-confirm"]').click();

  await expect(page.locator('[data-testid="toast"]')).toContainText('Compte retiré.');
  await expect(page.locator('[data-testid="account-remove"]')).toHaveCount(1);
  await expect(page.locator('[data-testid="settings-accounts"]')).not.toContainText(
    'deux@exemple.fr',
  );
  await page.locator('[data-testid="settings-done"]').click();

  // La nav : « Toutes les boîtes » + le seul compte restant.
  await expect(page.locator('[data-testid="nav-mailbox"]')).toHaveCount(2);
  await expect(page.locator('[data-testid="nav"]')).not.toContainText('deux@exemple.fr');

  // La liste unifiée ne montre plus que le courrier du compte restant :
  // les 5 conversations d'un@exemple.fr, plus les 4 disparues (le
  // compte des lignes rendues fait foi — la liste tient sur une page).
  await expect(page.locator('[data-testid="row"]')).toHaveCount(5);

  // A80/D7 (revue du 2026-08-25) : il ne reste qu'UN compte — les
  // comptes ne se mélangent plus, et « sur <sa propre adresse> » sur
  // chaque rangée serait le refrain que D7 refuse. La règle porte donc
  // sur le NOMBRE de comptes, pas seulement sur la vue choisie : c'est
  // ce que cette assertion tient, et la boîte unifiée est bien la vue
  // courante ici.
  await expect(page.locator('[data-testid="row-mailbox"]')).toHaveCount(0);
});
