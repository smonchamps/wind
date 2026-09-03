// P1 bench for the redesign (PLAN-UI-V2 §5): the budgets on the v2
// shell (Svelte, one continuous list with A SINGLE template since
// A29), database seeded at real scale — 256,312 messages by default.
//
//   node measure-v2.mjs
//
//   MESURE_DB          database path (default: target/e2e/measure-v2.db)
//   MESURE_COMPTES     "email:count" pairs separated by commas
//                      (default: mesure@exemple.fr:256312)
//   MESURE_REUTILISER  =1 to keep the database in place
//
// The shipped Tauri config points at `ui` (v1). The bench TEMPORARILY
// swaps `frontendDist` to `ui-v2/dist`, builds, then RESTORES the
// config before any measurement — the repository is never left dirty,
// even on failure (finally). This is the price accepted in P1 for not
// touching the shipped UI; if the loop becomes painful, P2 will decide
// otherwise.
//
// Protocol (mirrors the ADR 0015 spike, but with the real core + real
// IPC):
// - startup: wall clock, spawn -> first row visible;
// - page: 300 jumps spread over the depth (deterministic LCG), each
//   jump waits for the SERVICE (IPC) + the render + a forced reflow;
// - theme: 60 hot switches across the 7 themes;
// - open: 20 messages among the 400 most recent (the seeded bodies
//   cover the 500 most recent);
// - RAM: private working sets after 30 s (measure-ram.ps1, ADR 0002).
import { spawn, execSync } from 'node:child_process';
import { existsSync, mkdirSync, rmSync } from 'node:fs';
import path from 'node:path';
import { chromium } from '@playwright/test';
import { buildV2, purgeHttpCache } from './rebuild-v2.mjs';
import { purgeOAuth } from './isolation.mjs';
import { allocateCdpPort } from './port-cdp.mjs';
import { browserArgs } from './browser-args.mjs';

const root = path.resolve(import.meta.dirname, '..');

// --- 1. Build ui-v2 then the shell that embeds it ---------------------
buildV2(root);

// --- 2. Database seeded at scale ---------------------------------------
const db = process.env.MESURE_DB || path.join(root, 'target', 'e2e', 'measure-v2.db');
const accounts = (process.env.MESURE_COMPTES || 'mesure@exemple.fr:256312')
  .split(',')
  .map((entry) => {
    const [email, count] = entry.split(':');
    return { email: email.trim(), count: Number(count) };
  });

if (process.env.MESURE_REUTILISER && existsSync(db)) {
  console.log(`reused database: ${db}`);
} else {
  rmSync(db, { force: true });
  mkdirSync(path.dirname(db), { recursive: true });
  for (const { email, count } of accounts) {
    execSync(
      `cargo run -p mail-core --example seed_inbox --release -- "${db}" ${count} ${email}`,
      { cwd: root, stdio: 'inherit' },
    );
  }
}

// --- 3. Launch the real window, attach over CDP -------------------------
const profile = path.join(root, 'target', 'e2e', 'webview2-mesure-v2');
mkdirSync(profile, { recursive: true });
purgeHttpCache(profile);

// Free CDP port on every launch: the bench can run while an e2e gate
// plays on the same machine (PLAN-ISOLATION-E2E, D2).
const port = await allocateCdpPort();

const env = {
  ...process.env,
  WIND_DB_PATH: db,
  WIND_E2E_ACCOUNT: accounts[0].email,
  // The PRODUCTION arguments + the CDP port (2026-08-16 review): the
  // variable overrides the Tauri config at the WebView2 loader level —
  // without this override, the bench would measure a classic-scrollbar
  // geometry (~15 px reserved) that the user never sees (overlay A44).
  WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: browserArgs(root, port),
  WEBVIEW2_USER_DATA_FOLDER: profile,
};
purgeOAuth(env);

const t0 = performance.now();
const app = spawn(path.join(root, 'target', 'release', 'wind-desktop.exe'), [], {
  env,
  stdio: 'ignore',
});

let browser = null;
for (let attempt = 0; attempt < 300 && !browser; attempt++) {
  try {
    browser = await chromium.connectOverCDP(`http://127.0.0.1:${port}`);
  } catch {
    await new Promise((resolve) => setTimeout(resolve, 100));
  }
}
if (!browser) {
  app.kill();
  throw new Error(
    `CDP unreachable on port ${port} after 30 s. `
    + `Did the process die? Is another instance running with profile ${profile}?`,
  );
}

const stats = (values) => {
  const sort = [...values].sort((a, b) => a - b);
  const q = (p) => sort[Math.min(sort.length - 1, Math.floor(p * sort.length))];
  return `p50 ${q(0.5).toFixed(1)} ms · p95 ${q(0.95).toFixed(1)} ms · max ${sort[sort.length - 1].toFixed(1)} ms`;
};

try {
  // We wait for the PAGE, not the port (lesson from launch.mjs): CDP
  // responds before the window has created its document — looking up
  // the page just once is a race, lost as soon as startup is cold.
  let page = null;
  for (let attempt = 0; attempt < 300 && !page; attempt++) {
    page = browser
      .contexts()
      .flatMap((context) => context.pages())
      .find((candidate) => candidate.url().includes('tauri.localhost'));
    if (!page) await new Promise((resolve) => setTimeout(resolve, 100));
  }
  if (!page) throw new Error('Tauri window not found after 30 s — did the process start?');
  await page.locator('[data-testid="row"]').first().waitFor({ timeout: 60000 });
  const startup = performance.now() - t0;
  await page.waitForFunction(() => document.getElementById('perf').dataset.startup);
  console.log(`startup    : ${startup.toFixed(0)} ms (spawn -> first row, wall clock)`);
  console.log('internal   :', await page.locator('#perf').textContent());

  // The total is ASYNCHRONOUS since PLAN-DEFILEMENT-PROFOND (rows
  // first, count settles at rest of the pump): the 300 jumps must
  // cover the real depth, not the floor of the first rows.
  await page.waitForFunction(() => window.__mesure.state().exactTotal);
  const state = await page.evaluate(() => window.__mesure.state());
  console.log(`decor      : ${state.total} rows · template ${state.h1} px`);

  // MESURE_SANS_ACTIVITE=1: weigh RAM AT REST, ADR 0002 methodology —
  // the same posture as the v1 bench, without which the comparison
  // weighs a marathon runner against a sleeper.
  const atRest = process.env.MESURE_SANS_ACTIVITE === '1';

  if (!atRest) {
  // Pages: 300 jumps spread over the depth, deterministic LCG.
  const pages_ms = await page.evaluate(async () => {
    let seed = 42;
    const random = () => ((seed = (seed * 1103515245 + 12345) % 2147483648) / 2147483648);
    const total = window.__mesure.state().total;
    const measurements = [];
    for (let n = 0; n < 300; n++) {
      const index = Math.floor(random() * Math.max(1, total - 40));
      measurements.push(await window.__mesure.page(index));
    }
    return measurements;
  });
  console.log(`page       : ${stats(pages_ms)} (IPC service + render + reflow)`);

  // Theme: 60 hot switches across the shipped themes (read from the product).
  const themes_ms = await page.evaluate(() => {
    const names = window.__mesure.themes;
    const measurements = [];
    for (let n = 0; n < 60; n++) measurements.push(window.__mesure.theme(names[n % names.length]));
    window.__mesure.theme('elements');
    return measurements;
  });
  console.log(`theme      : ${stats(themes_ms)}`);

  // Open: 20 messages among the 400 most recent (seeded bodies).
  const opens_ms = await page.evaluate(async () => {
    const measurements = [];
    for (let n = 0; n < 20; n++) measurements.push(await window.__mesure.open(n * 20));
    return measurements;
  });
  console.log(`open       : ${stats(opens_ms)}`);
  }

  console.log(`stabilizing 30 s before the RAM measurement${atRest ? ' (rest, no activity)' : ''}…`);
  await new Promise((resolve) => setTimeout(resolve, 30000));
  const ram = execSync(
    `powershell -NoProfile -ExecutionPolicy Bypass -File "${path.join(import.meta.dirname, 'measure-ram.ps1')}"`
    + ` -AppPid ${app.pid} -Profil "${profile}"`,
  ).toString();
  console.log('RAM        :', ram.trim());
} finally {
  if (browser) await browser.close();
  app.kill();
}
