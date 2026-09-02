// Terrain STOP 2 PLAN-AUDIT-V2, passe 3 (2026-09-02, verdict CE sur
// captures) : les barres du fil. Au VOLET, la barre de tri (Archiver /
// Signaler comme spam / Épingler) est collée sous l'entête du fil ; à
// l'ÉCRAN 03, ses boutons vivent dans la barre d'entête de la scène ;
// la barre de réponse d'un message FLOTTE en bas du message — elle
// reste visible au défilement, à 12 px du pied du scrollport, et
// s'en tient aux bords de la carte quand sa fin arrive.
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

test("volet : la barre de tri est collée sous l'entête, la barre de réponse flotte en bas du message", async () => {
  await page.locator('[data-testid="ligne"]', { hasText: 'Relecture du contrat Vantis' }).click();
  const barre = page.locator('[data-testid="barre-fil"]');
  await expect(barre).toBeVisible();
  const geo = await page.evaluate(() => {
    const barre = document.querySelector('[data-testid="barre-fil"]');
    const puces = document.querySelector('[data-testid="fil-puces"]');
    const reponse = document.querySelector('[data-testid="actions-message"]');
    return {
      ecart: barre.getBoundingClientRect().top - puces.getBoundingClientRect().bottom,
      barreSticky: getComputedStyle(barre).position,
      reponseSticky: getComputedStyle(reponse).position,
      reponseOmbre: getComputedStyle(reponse).boxShadow,
    };
  });
  // Collée : au plus l'air de 4 px sous les puces ; collante au défilement (R1).
  expect(geo.ecart).toBeLessThanOrEqual(6);
  expect(geo.barreSticky).toBe('sticky');
  // Flottante : collante en bas, élevée (ombre), jamais un filet à plat.
  expect(geo.reponseSticky).toBe('sticky');
  expect(geo.reponseOmbre).not.toBe('none');
});

test("écran 03 : les gestes de tri vivent dans la barre d'entête, la barre de réponse reste visible au défilement", async () => {
  await page.locator('[data-testid="voir-conversation"]').click();
  const entete = page.locator('[data-testid="conversation"] header');
  await expect(entete.locator('[data-testid="archiver"]')).toBeVisible();
  await expect(entete.locator('[data-testid="signaler-spam"]')).toBeVisible();
  await expect(page.locator('[data-testid="barre-fil"].volet')).toHaveCount(0);
  // La barre de réponse du dernier message est visible SANS défiler
  // jusqu'à lui : elle flotte au bas du scrollport, à 12 px du pied.
  const reponse = page.locator('[data-testid="actions-message"]').last();
  await expect(reponse).toBeVisible();
  const marge = await page.evaluate(() => {
    const reponse = [...document.querySelectorAll('[data-testid="actions-message"]')].at(-1);
    const scene = reponse.closest('.scene');
    return scene.getBoundingClientRect().bottom - reponse.getBoundingClientRect().bottom;
  });
  expect(marge).toBeGreaterThanOrEqual(12);
});
