// Terrain STOP 2 PLAN-AUDIT-V2, passe 2 (2026-09-02) : « DÉJÀ CONSULTÉ »
// chevauchait une rangée, un vide d'une rangée dessous. La bande de
// section est positionnée d'après le MODÈLE de hauteurs (sondes h1/h2)
// ; les rangées s'empilent à leur hauteur RÉELLE. Les sondes ne
// rendaient ni le bloc de boîte ni le ⋯ du mode organisé : 6 px de
// moins par rangée, une rangée entière au bout de vingt. Le filet vise
// ce que l'utilisateur VOIT : la bande est calée sur son vide, et la
// première rangée « Déjà consulté » commence juste sous elle.
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

test("la bande « Déjà consulté » est calée sur son vide, au pixel — boîte unifiée, mode organisé", async () => {
  await expect(page.locator('[data-testid="row"]').first()).toBeVisible();
  await page.locator('[data-testid="organized-mode"]').click();
  await expect(page.locator('[data-testid="organized-mode"]')).toHaveAttribute('aria-checked', 'true');
  await page.locator('[data-testid="nav-folder"][data-category="inbox"]').click();
  const bande = page.locator('[data-testid="section"]', { hasText: 'Déjà consulté' });
  await expect(bande).toBeVisible();
  // Les sondes se lient par ResizeObserver : on laisse un frame ou deux.
  // Tolérance 2 px : le vide tombe sur un sous-pixel (480,83 px mesuré).
  await page.waitForTimeout(500);
  const geo = await page.evaluate(() => {
    const bande = [...document.querySelectorAll('[data-testid="section"]')]
      .find((e) => e.textContent.includes('Déjà consulté'));
    const vide = document.querySelector('.header-space');
    const lignes = [...document.querySelectorAll('.window [data-testid="row"]')];
    const premiereLue = lignes.find((l) => !l.classList.contains('unread'));
    return {
      bande: bande.getBoundingClientRect().top,
      vide: vide?.getBoundingClientRect().top ?? null,
      videBas: vide?.getBoundingClientRect().bottom ?? null,
      premiereLue: premiereLue?.getBoundingClientRect().top ?? null,
    };
  });
  expect(geo.vide).not.toBeNull();
  expect(Math.abs(geo.bande - geo.vide)).toBeLessThanOrEqual(2);
  expect(Math.abs(geo.premiereLue - geo.videBas)).toBeLessThanOrEqual(2);
});
