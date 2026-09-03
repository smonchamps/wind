// PLAN-AUDIT-V2 E11 : LE menu du produit se parcourt au clavier et rend
// le focus (A8 tenu — `role="menu"` promettait un clavier absent : huit
// copies fermaient à n'importe quelle touche, Tab compris, sans jamais
// poser le focus). Joué sur le ⋯ d'une rangée de la Réception organisée
// — le premier menu porté ; les sept autres partagent le composant.
import { test, expect } from '@playwright/test';
import { launchAppV2, closeApp } from '../launch.mjs';

let app;
let browser;
let page;

test.describe.configure({ mode: 'serial' });

test.beforeAll(async () => {
  ({ app, browser, page } = await launchAppV2());
  await page.locator('[data-testid="organized-mode"]').click();
  await expect(page.locator('[data-testid="organized-mode"]')).toHaveAttribute('aria-checked', 'true');
  await expect(page.locator('[data-testid="row"]').first()).toBeVisible();
});

test.afterAll(async () => {
  await closeApp({ app, browser });
});

const actif = () => page.evaluate(() => document.activeElement?.dataset?.testid ?? null);

test('le menu des gestes se parcourt aux flèches et Échap rend le focus au déclencheur', async () => {
  const rangee = page.locator('[data-testid="row"]').first();
  await rangee.hover();
  const declencheur = rangee.locator('[data-testid="row-gestures"]');
  await declencheur.click();
  const menu = page.locator('[data-testid="menu-gestures"]');
  await expect(menu).toBeVisible();
  // Le focus se pose sur le premier item, sans geste de plus.
  await expect.poll(actif).toBe('gestures-feed');
  await page.keyboard.press('ArrowDown');
  await expect.poll(actif).toBe('gestures-paper_trail');
  await page.keyboard.press('End');
  await expect.poll(actif).toBe('gestures-screen-out');
  await page.keyboard.press('ArrowDown');
  await expect.poll(actif).toBe('gestures-feed');
  // Une touche quelconque ne ferme PAS (avant : tout keydown fermait).
  await page.keyboard.press('Shift');
  await expect(menu).toBeVisible();
  await page.keyboard.press('Escape');
  await expect(menu).toHaveCount(0);
  await expect.poll(actif).toBe('row-gestures');
});

test('Tab et le clic dehors ferment le menu', async () => {
  const rangee = page.locator('[data-testid="row"]').first();
  await rangee.hover();
  await rangee.locator('[data-testid="row-gestures"]').click();
  const menu = page.locator('[data-testid="menu-gestures"]');
  await expect(menu).toBeVisible();
  await page.keyboard.press('Tab');
  await expect(menu).toHaveCount(0);
  await rangee.hover();
  await rangee.locator('[data-testid="row-gestures"]').click();
  await expect(menu).toBeVisible();
  // Un point neutre de la fenêtre (le coin de la nav) : dehors, sans geste.
  await page.mouse.click(5, 5);
  await expect(menu).toHaveCount(0);
});

test('Entrée sur un item joue le geste comme un clic', async () => {
  const rangee = page.locator('[data-testid="row"]').first();
  const sujet = (await rangee.locator('.subject').textContent()).trim();
  await rangee.hover();
  await rangee.locator('[data-testid="row-gestures"]').click();
  await expect.poll(actif).toBe('gestures-feed');
  await page.keyboard.press('Enter');
  await expect(page.locator('[data-testid="menu-gestures"]')).toHaveCount(0);
  // La rangée est partie au Kiosque : elle quitte la Réception.
  await expect
    .poll(async () =>
      (await page.locator('[data-testid="row"] .subject').allTextContents()).map((s) => s.trim()).includes(sujet))
    .toBe(false);
});

test('les Réglages ouvrent sur leur premier contrôle (entrée de D-4)', async () => {
  await page.locator('[data-testid="settings"]').click();
  await expect.poll(async () =>
    page.evaluate(() => {
      const el = document.activeElement;
      return Boolean(el?.closest?.('[data-testid="settings-modal"], .panel, [role="dialog"]')) || (el?.dataset?.testid ?? '').startsWith('settings');
    })).toBe(true);
  await page.locator('[data-testid="settings-done"]').click();
});
