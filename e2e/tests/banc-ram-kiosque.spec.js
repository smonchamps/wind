// Banc de mesure (PLAN-AUDIT-V2, terrain STOP 2) : la RAM privée de
// l'application (exe + WebView2 du profil e2e) au repos, à la première
// page du Kiosque, après 160 cartes défilées, et au retour en
// Réception — sur un décor de 200 lettres à corps de 100 Ko. Hors
// gate : ne tourne que sous WIND_BANC_RAM=1.
//
//   $env:WIND_BANC_RAM = '1'; npx playwright test tests/banc-ram-kiosque.spec.js --reporter=list --retries=0
//
import { execSync } from 'node:child_process';
import path from 'node:path';
import { test, expect } from '@playwright/test';
import { launchAppV2, closeApp } from '../launch.mjs';

let app; let browser; let page;

test.skip(!process.env.WIND_BANC_RAM, 'banc de mesure : WIND_BANC_RAM=1 pour le jouer');
test.beforeAll(async () => {
  if (!process.env.WIND_BANC_RAM) return;
  ({ app, browser, page } = await launchAppV2({
    comptes: [{ email: 'principal@exemple.fr', messages: 200, ko: 100 }],
  }));
});
test.afterAll(async () => {
  if (app) await closeApp({ app, browser });
});

const ram = (etiquette) => {
  const profil = path.join(process.cwd(), '..', 'target', 'e2e', 'webview2');
  const out = execSync(
    `powershell -ExecutionPolicy Bypass -File mesure-ram.ps1 -AppPid ${app.pid} -Profil "${profil}"`,
    { encoding: 'utf8' },
  ).trim();
  console.log(`RAM ${etiquette} : ${out}`);
};

test('RAM privée : repos, Kiosque page 1, 160 cartes, retour', async () => {
  test.setTimeout(240000);
  await expect(page.locator('[data-testid="ligne"]').first()).toBeVisible();
  await page.waitForTimeout(8000);
  ram('repos, mode classique');
  await page.locator('[data-testid="mode-organise"]').click();
  await expect(page.locator('[data-testid="mode-organise"]')).toHaveAttribute('aria-checked', 'true');
  await page.evaluate(async () => {
    const invoke = window.__TAURI__.core.invoke;
    for (let n = 0; n < 16; n += 1) {
      await invoke('router_expediteur', { address: `expediteur${n}@exemple.fr`, destination: 'kiosque', regle: null });
    }
  });
  await page.locator('[data-testid="nav-dossier"][data-categorie="kiosque"]').click();
  const cartes = page.locator('[data-testid="kiosque-carte"]');
  await expect(cartes.first()).toBeVisible();
  await page.waitForTimeout(2000);
  ram('kiosque page 1');
  for (let i = 0; i < 12; i += 1) {
    await cartes.last().scrollIntoViewIfNeeded();
    await page.waitForTimeout(1500);
  }
  const n = await cartes.count();
  const iframes = await page.locator('[data-testid="kiosque-carte"] iframe.corps').count();
  console.log(`cartes ${n}, iframes vivantes ${iframes}`);
  await page.waitForTimeout(8000);
  ram(`kiosque ${n} cartes défilées`);
  await page.locator('[data-testid="nav-dossier"][data-categorie="reception"]').click();
  await page.waitForTimeout(8000);
  ram('retour Réception');
  await page.waitForTimeout(25000);
  ram('retour Réception + 25 s');
});
