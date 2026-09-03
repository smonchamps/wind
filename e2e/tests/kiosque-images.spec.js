// Terrain STOP 2 PLAN-AUDIT-V2 (2026-09-02) : « Toujours afficher les
// images » sans effet au Kiosque après dix pages défilées. La garde
// appelait bien le cœur, puis `charger(0)` — qui ne re-sert que la
// page 0 ; la fusion par clé (E10) gardait telle quelle une carte
// au-delà. Le filet vise ce que l'utilisateur VOIT : la garde d'une
// carte de la page 2 disparaît au clic.
import { test, expect } from '@playwright/test';
import { launchAppV2, closeApp, injecterArrivee } from '../launch.mjs';

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

test("« Toujours afficher les images » lève la garde d'une carte au-delà de la page 0", async () => {
  // 25 lettres d'un même expéditeur, chacune avec une image distante :
  // plus qu'une page de Kiosque (20).
  injecterArrivee({
    email: 'principal@exemple.fr', expediteur: 'lettre@exemple.fr',
    nom: 'La Lettre', sujet: 'Edition', n: 25, corps: 'images',
  });
  await page.reload();
  await expect(page.locator('[data-testid="ligne"]').first()).toBeVisible();
  await page.locator('[data-testid="mode-organise"]').click();
  await expect(page.locator('[data-testid="mode-organise"]')).toHaveAttribute('aria-checked', 'true');
  // Router au Kiosque pose la règle d'images (RETOURS-14, « Oui = règle
  // d'images ») : on la RÉVOQUE — le cas du terrain, des lettres routées
  // avant cette règle, ou dont la règle a été retirée aux Réglages.
  await page.evaluate(async () => {
    const invoke = window.__TAURI__.core.invoke;
    await invoke('route_sender', {
      address: 'lettre@exemple.fr',
      destination: 'feed',
      rule: null,
    });
    await invoke('revoke_images_sender', { address: 'lettre@exemple.fr' });
  });
  await page.locator('[data-testid="nav-dossier"][data-categorie="feed"]').click();
  const cartes = page.locator('[data-testid="kiosque-carte"]');
  await expect(cartes).toHaveCount(20);
  // La page 2 se charge en approchant du bas.
  await cartes.last().scrollIntoViewIfNeeded();
  await expect(cartes).toHaveCount(25);
  const derniere = cartes.last();
  await derniere.scrollIntoViewIfNeeded();
  const garde = derniere.locator('[data-testid="kiosque-garde-images"]');
  await expect(garde).toBeVisible();
  await garde.getByRole('button', { name: 'Toujours afficher les images de cet expéditeur' }).click();
  // Ce que l'utilisateur voit : la garde de CETTE carte s'en va, les
  // images de la lettre sont rendues (l'iframe porte la vraie URL).
  await expect(garde).toHaveCount(0);
  await expect(derniere.locator('iframe.corps')).toHaveAttribute('srcdoc', /images\.exemple\/lettre-/);
  // Et la règle vaut pour ses sœurs déjà servies : la première carte
  // (page 0) n'a plus de garde non plus.
  await expect(cartes.first().locator('[data-testid="kiosque-garde-images"]')).toHaveCount(0);
});
