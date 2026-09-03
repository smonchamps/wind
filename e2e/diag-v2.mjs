// Andon P1 diagnostic: break down the "list page" budget into its
// three stages — core query (elapsed_us of MessagePage), IPC
// transport, Svelte render — so the arbitration happens on the
// figures for the right stage. Reuses the bench's binary and database
// (no build, no seed): run AFTER measure-v2.mjs.
//
//   node diag-v2.mjs
import { spawn } from 'node:child_process';
import { mkdirSync, rmSync } from 'node:fs';
import path from 'node:path';
import { chromium } from '@playwright/test';
import { purgeOAuth } from './isolation.mjs';
import { allocateCdpPort } from './port-cdp.mjs';
import { browserArgs } from './browser-args.mjs';

const root = path.resolve(import.meta.dirname, '..');
const db = process.env.MEASURE_DB || path.join(root, 'target', 'e2e', 'measure-v2.db');
const profile = path.join(root, 'target', 'e2e', 'webview2-mesure-v2');
mkdirSync(profile, { recursive: true });
for (const folder of ['Cache', 'Code Cache']) {
  rmSync(path.join(profile, 'EBWebView', 'Default', folder), { recursive: true, force: true });
}

// Free CDP port on every launch (PLAN-ISOLATION-E2E, D2).
const port = await allocateCdpPort();

const env = {
  ...process.env,
  WIND_DB_PATH: db,
  WIND_E2E_ACCOUNT: 'mesure@exemple.fr',
  // The PRODUCTION arguments + the CDP port (2026-08-16 review): the
  // variable overrides the Tauri config at the WebView2 loader level —
  // without this override, the bench would measure a classic-scrollbar
  // geometry (~15 px reserved) that the user never sees (overlay A44).
  WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: browserArgs(root, port),
  WEBVIEW2_USER_DATA_FOLDER: profile,
};
purgeOAuth(env);

const app = spawn(path.join(root, 'target', 'release', 'wind-desktop.exe'), [], {
  env,
  stdio: 'ignore',
});

let browser = null;
for (let attempt = 0; attempt < 60 && !browser; attempt++) {
  try {
    browser = await chromium.connectOverCDP(`http://127.0.0.1:${port}`);
  } catch {
    await new Promise((resolve) => setTimeout(resolve, 500));
  }
}
if (!browser) {
  app.kill();
  throw new Error(`CDP unreachable on port ${port}.`);
}

try {
  // We wait for the PAGE, not the port (lesson from launch.mjs).
  let page = null;
  for (let attempt = 0; attempt < 60 && !page; attempt++) {
    page = browser
      .contexts()
      .flatMap((context) => context.pages())
      .find((candidate) => candidate.url().includes('tauri.localhost'));
    if (!page) await new Promise((resolve) => setTimeout(resolve, 500));
  }
  if (!page) throw new Error('Tauri window not found after 30 s — did the process start?');
  await page.locator('[data-testid="row"]').first().waitFor({ timeout: 60000 });

  // 1. Core + IPC stage: RAW call, with no render at all. `elapsed_us`
  //    is measured INSIDE the Rust command — the difference with the
  //    wall clock is the IPC (serialization included).
  const raw = await page.evaluate(async () => {
    const call = window.__TAURI__.core.invoke;
    const output = [];
    for (const offset of [0, 1000, 10000, 50000, 100000, 200000]) {
      const reps = [];
      for (let n = 0; n < 5; n++) {
        const t0 = performance.now();
        // The path the UI REALLY takes (list_messages, v1, is removed
        // at B2): the unified per-category inbox.
        const p = await call('list_category', {
          category: 'inbox', accountId: null, unread: false, offset, limit: 200,
        });
        reps.push({ wall: performance.now() - t0, core: p.elapsed_us / 1000 });
      }
      reps.sort((x, y) => x.wall - y.wall);
      const med = reps[2];
      output.push(`offset ${String(offset).padStart(6)} : core ${med.core.toFixed(1)} ms · wall ${med.wall.toFixed(1)} ms (median of 5)`);
    }
    return output;
  });
  for (const line of raw) console.log(line);

  // 2. Render stage: jump to pages ALREADY served -> the service is a
  //    no-op, all that's left is Svelte + reflow.
  const render = await page.evaluate(async () => {
    await window.__mesure.page(100000); // warm-up: served pages
    const reps = [];
    for (let n = 0; n < 20; n++) {
      reps.push(await window.__mesure.page(100000 + (n % 2) * 5)); // same window
    }
    reps.sort((a, b) => a - b);
    return `render only (served pages): median ${reps[10].toFixed(1)} ms · max ${reps[19].toFixed(1)} ms`;
  });
  console.log(render);

  // 3. PROXIMITY scrolling (the real gesture): jumps of ± one window
  //    around a position, fresh neighboring pages.
  const nearby = await page.evaluate(async () => {
    const base = 150000;
    await window.__mesure.page(base);
    const reps = [];
    for (let n = 1; n <= 20; n++) {
      reps.push(await window.__mesure.page(base + n * 12)); // ~one window
    }
    reps.sort((a, b) => a - b);
    return `nearby scrolling (window to window): median ${reps[10].toFixed(1)} ms · p95 ${reps[18].toFixed(1)} ms`;
  });
  console.log(nearby);
} finally {
  if (browser) await browser.close();
  app.kill();
}
