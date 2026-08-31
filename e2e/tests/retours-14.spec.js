// PLAN-RETOURS-14 R1 (D1) : la barre d'actions du fil (archiver,
// signaler comme spam, épingler…) vit EN TÊTE de la conversation et
// COLLE au défilement — dans le volet de lecture (trois volets) comme
// à l'écran 03. Le filet vise ce que l'utilisateur VOIT : au fond d'un
// long fil, la barre est encore là, en haut du cadre.
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
  // Une fenêtre basse force le défilement du fil : le collant ne se
  // prouve qu'en défilant réellement (filet non vacant).
  await page.setViewportSize({ width: 1180, height: 420 });
});

test.afterAll(async () => {
  await closeApp({ app, browser });
});

test('volet de lecture : la barre du fil est en tête, avant les messages', async () => {
  await expect(page.locator('[data-testid="ligne"]').first()).toBeVisible();
  await page.locator('[data-testid="ligne"]').first().click();
  const volet = page.locator('[data-testid="volet-lecture"]');
  await expect(volet.locator('[data-testid="archiver"]')).toBeVisible();

  // L'ordre VU : la barre au-dessus du premier message.
  const barre = await volet.locator('[data-testid="archiver"]').boundingBox();
  const message = await volet.locator('[data-testid="message-deplie"], [data-testid="message-replie"]').first().boundingBox();
  expect(barre.y).toBeLessThan(message.y);
});

test('volet de lecture : la barre reste visible au fond du fil (collante)', async () => {
  const volet = page.locator('[data-testid="volet-lecture"]');
  // Défiler le CADRE jusqu'au fond — et vérifier qu'il y avait bien de
  // quoi défiler, sinon le test ne prouverait rien.
  const defile = await volet.evaluate((el) => {
    el.scrollTop = el.scrollHeight;
    return el.scrollHeight > el.clientHeight;
  });
  expect(defile).toBe(true);

  // Collée en tête du cadre, pas partie avec le flot. Le poll
  // re-défile à chaque mesure : le chargement tardif d'une iframe de
  // corps peut regrandir le flot après le premier scroll.
  await expect
    .poll(async () => {
      await volet.evaluate((el) => { el.scrollTop = el.scrollHeight; });
      const cadre = await volet.boundingBox();
      const barre = await volet.locator('[data-testid="archiver"]').boundingBox();
      if (!barre) return false;
      const offset = barre.y - cadre.y;
      // Bornée des DEUX côtés : sans collant, la barre serait partie
      // AU-DESSUS du cadre (offset négatif) — le filet doit le voir.
      return offset >= 0 && offset < 48;
    })
    .toBe(true);
  await volet.evaluate((el) => { el.scrollTop = 0; });
});

test('écran 03 : la barre est en tête et colle au défilement de la scène', async () => {
  // Plus bas encore : l'écran 03 d'un fil d'un message est court — il
  // faut assez de défilement pour que la barre ATTEIGNE le haut.
  await page.setViewportSize({ width: 1180, height: 320 });
  await page.locator('[data-testid="voir-conversation"]').click();
  const conv = page.locator('[data-testid="conversation"]');
  await expect(conv.locator('[data-testid="archiver"]')).toBeVisible();

  const scene = conv.locator('.scene');
  const defile = await scene.evaluate((el) => {
    el.scrollTop = el.scrollHeight;
    return el.scrollHeight > el.clientHeight;
  });
  expect(defile).toBe(true);

  await expect
    .poll(async () => {
      await scene.evaluate((el) => { el.scrollTop = el.scrollHeight; });
      const cadre = await scene.boundingBox();
      const barre = await conv.locator('[data-testid="archiver"]').boundingBox();
      if (!barre) return false;
      const offset = barre.y - cadre.y;
      return offset >= 0 && offset < 48;
    })
    .toBe(true);

  // Retour à l'écran 02 pour les tests suivants.
  await page.locator('[data-testid="retour-boite"]').click();
  await expect(page.locator('[data-testid="volet-lecture"]')).toBeVisible();
});
