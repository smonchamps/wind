// Measurement bench (PLAN-AUDIT-V2, STOP 2 field): the app's private RAM
// (exe + e2e profile's WebView2) at rest, on the Feed's first
// page, after 160 cards scrolled, and back in the
// Inbox — on a decor of 200 letters with 100 KB bodies. Outside the
// gate: only runs under WIND_BANC_RAM=1.
//
//   $env:WIND_BANC_RAM = '1'; npx playwright test tests/bench-ram-feed.spec.js --reporter=list --retries=0
//
import { execSync } from 'node:child_process';
import path from 'node:path';
import { test, expect } from '@playwright/test';
import { launchAppV2, closeApp } from '../launch.mjs';

let app; let browser; let page;

test.skip(!process.env.WIND_BANC_RAM, 'measurement bench: WIND_BANC_RAM=1 to run it');
test.beforeAll(async () => {
  if (!process.env.WIND_BANC_RAM) return;
  ({ app, browser, page } = await launchAppV2({
    accounts: [{ email: 'principal@exemple.fr', messages: 200, ko: 100 }],
  }));
});
test.afterAll(async () => {
  if (app) await closeApp({ app, browser });
});

const ram = (label) => {
  const profile = path.join(process.cwd(), '..', 'target', 'e2e', 'webview2');
  const out = execSync(
    `powershell -ExecutionPolicy Bypass -File measure-ram.ps1 -AppPid ${app.pid} -Profil "${profile}"`,
    { encoding: 'utf8' },
  ).trim();
  console.log(`RAM ${label} : ${out}`);
};

test('private RAM: rest, Feed page 1, 160 cards, back', async () => {
  test.setTimeout(240000);
  await expect(page.locator('[data-testid="row"]').first()).toBeVisible();
  await page.waitForTimeout(8000);
  ram('rest, classic mode');
  await page.locator('[data-testid="organized-mode"]').click();
  await expect(page.locator('[data-testid="organized-mode"]')).toHaveAttribute('aria-checked', 'true');
  await page.evaluate(async () => {
    const invoke = window.__TAURI__.core.invoke;
    for (let n = 0; n < 16; n += 1) {
      await invoke('route_sender', { address: `expediteur${n}@exemple.fr`, destination: 'feed', rule: null });
    }
  });
  await page.locator('[data-testid="nav-folder"][data-category="feed"]').click();
  const cards = page.locator('[data-testid="feed-card"]');
  await expect(cards.first()).toBeVisible();
  await page.waitForTimeout(2000);
  ram('feed page 1');
  for (let i = 0; i < 12; i += 1) {
    await cards.last().scrollIntoViewIfNeeded();
    await page.waitForTimeout(1500);
  }
  const n = await cards.count();
  const iframes = await page.locator('[data-testid="feed-card"] iframe.body').count();
  console.log(`cards ${n}, live iframes ${iframes}`);
  await page.waitForTimeout(8000);
  ram(`feed ${n} cards scrolled`);
  await page.locator('[data-testid="nav-folder"][data-category="inbox"]').click();
  await page.waitForTimeout(8000);
  ram('back to Inbox');
  await page.waitForTimeout(25000);
  ram('back to Inbox + 25 s');
});
