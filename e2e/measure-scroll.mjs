// Deep-scroll bench (defilement-archives project, Phase 0): reproduces
// the field finding of 2026-08-20 — a fast drag of the scrollbar in
// Archives leaves ".." blocks, and switching to any folder afterward
// says "No messages here." on full mailboxes, for several minutes in
// the field.
//
// Hypothesis to test (never a guess without a measurement): every
// intermediate drag position triggers its own `list_category` pages
// (O(offset) outside the inbox), nothing cancels the pages that became
// invisible, and the global `off_pump` lock serializes everything —
// the queue drains over minutes, EVERY command waits behind it.
//
// Counting: the `window.__e2eLog` seam in transport.js (laid down by
// PLAN-DEFILEMENT-PROFOND E1) — one {command, start, arrival} record
// per call to the core. (Two approaches ruled out, as observed:
// wrapping `__TAURI__.core.invoke` loses the race against the Tauri
// injection — a late defineProperty replaces it without a setter;
// wrapping `__TAURI_INTERNALS__.invoke` after the fact counts nothing,
// transport holds the original reference.)
//
// Protocol:
// - seeded database: 2,000 INBOX + 120,000 Archives (one account);
// - 3 s of background noise (periodic probes) to calibrate;
// - simulated drag: scrollTop pushed in 120 steps over ~2 s (≈60
//   events/s, the density of a scrollbar held by the mouse) up to 1/3
//   of the list;
// - 500 ms sampling: calls sent/settled, rows, placeholders, "No
//   messages here.";
// - at T+5 s: switch to Inbox then back to Archives (the second part
//   of the finding); sample until recovery.
//
//   node measure-scroll.mjs
//   MEASURE_REUSE=1 to keep the database in place.
import { spawn, execSync } from 'node:child_process';
import { existsSync, mkdirSync, rmSync } from 'node:fs';
import path from 'node:path';
import { chromium } from '@playwright/test';
import { buildV2, purgeHttpCache } from './rebuild-v2.mjs';
import { purgeOAuth } from './isolation.mjs';
import { allocateCdpPort } from './port-cdp.mjs';
import { browserArgs } from './browser-args.mjs';
import { holdBar } from './scroll-gesture.mjs';

const root = path.resolve(import.meta.dirname, '..');
const ARCHIVES = 120_000;
const INBOX = 2_000;

buildV2(root, { seams: true });

// --- Seeded database ---------------------------------------------------
const db = path.join(root, 'target', 'e2e', 'measure-scroll.db');
if (process.env.MEASURE_REUSE && existsSync(db)) {
  console.log(`reused database: ${db}`);
} else {
  rmSync(db, { force: true });
  mkdirSync(path.dirname(db), { recursive: true });
  execSync(
    `cargo run -p mail-core --example seed_inbox --release -- "${db}" ${INBOX} mesure@exemple.fr`,
    { cwd: root, stdio: 'inherit' },
  );
  // The seeder registers the "Archives" mailbox in the `folders` cache
  // itself: the archive canonical resolves without an SQL patch.
  execSync(
    `cargo run -p mail-core --example seed_inbox --release -- "${db}" ${ARCHIVES} mesure@exemple.fr 500 0 Archives`,
    { cwd: root, stdio: 'inherit' },
  );
}

// --- Launch (mirrors measure-v2.mjs) -----------------------------------
const profile = path.join(root, 'target', 'e2e', 'webview2-mesure-defilement');
mkdirSync(profile, { recursive: true });
purgeHttpCache(profile);
const port = await allocateCdpPort();
const env = {
  ...process.env,
  WIND_DB_PATH: db,
  WIND_E2E_ACCOUNT: 'mesure@exemple.fr',
  WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: browserArgs(root, port),
  WEBVIEW2_USER_DATA_FOLDER: profile,
};
purgeOAuth(env);

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
  throw new Error(`CDP unreachable on port ${port} after 30 s`);
}

try {
  let page = null;
  for (let attempt = 0; attempt < 300 && !page; attempt++) {
    page = browser
      .contexts()
      .flatMap((context) => context.pages())
      .find((candidate) => candidate.url().includes('tauri.localhost'));
    if (!page) await new Promise((resolve) => setTimeout(resolve, 100));
  }
  if (!page) throw new Error('Tauri window not found after 30 s');

  page.on('console', (message) => {
    if (message.type() === 'error') console.log(`[console] ${message.text()}`);
  });
  await page.locator('[data-testid="row"]').first().waitFor({ timeout: 60000 });

  await page.evaluate(() => {
    window.__e2eLog = [];
  });

  const screenState = () => page.evaluate(() => {
    const frame = document.querySelector('[data-testid="list"] .frame');
    const log = window.__e2eLog.filter((a) => a.command === 'list_category');
    const settled = log.filter((a) => a.arrival !== null).length;
    return {
      t: Math.round(performance.now()),
      calls: log.length,
      settled,
      inFlight: log.length - settled,
      rows: document.querySelectorAll('[data-testid="row"]').length,
      pending: document.querySelectorAll('[data-testid="row-pending"]').length,
      emptyText: document.querySelector('[data-testid="list"] .empty')?.textContent?.trim() ?? null,
      scrollTop: frame ? Math.round(frame.scrollTop) : null,
    };
  });

  // --- Background noise: 3 s of periodic probes, no gesture -----------
  const beforeNoise = await screenState();
  await new Promise((resolve) => setTimeout(resolve, 3000));
  const afterNoise = await screenState();
  const noisePerSecond = (afterNoise.calls - beforeNoise.calls) / 3;
  console.log(`background noise: ${noisePerSecond.toFixed(1)} call(s)/s outside scrolling`);

  // --- Archives, then the drag -----------------------------------------
  await page.locator('[data-testid="nav-folder"][data-category="archive"]').click();
  await page.locator('[data-testid="row"]').first().waitFor({ timeout: 30000 });
  // The total is asynchronous (rows first, count settles at rest):
  // the drag targets 1/3 of the REAL height, not the provisional floor.
  await page.waitForFunction(() => window.__mesure.state().exactTotal, null, { timeout: 30000 });
  await new Promise((resolve) => setTimeout(resolve, 1000));
  const beforeDrag = await screenState();
  console.log('before drag:', JSON.stringify(beforeDrag));

  // A HELD drag: ~60 events/s for 2 s — the gesture shared with the
  // spec (scroll-gesture.mjs).
  await holdBar(page, { step: 120 });
  const dragEnd = Date.now();
  const afterDrag = await screenState();
  console.log('drag done  :', JSON.stringify(afterDrag));
  console.log(`drag burst: ~${afterDrag.calls - beforeDrag.calls} calls in ${((afterDrag.t - beforeDrag.t) / 1000).toFixed(1)} s`);

  // --- Sampling + folder switch at T+5 s -------------------------------
  let switched = false;
  let recovered = null;
  const samplingStart = Date.now();
  while (Date.now() - samplingStart < 180_000) {
    await new Promise((resolve) => setTimeout(resolve, 500));
    const snapshot = await screenState();
    const sinceDrag = ((Date.now() - dragEnd) / 1000).toFixed(1);
    console.log(`T+${sinceDrag}s :`, JSON.stringify(snapshot));

    if (!switched && Date.now() - dragEnd > 5000) {
      switched = true;
      console.log('--- switch to Inbox ---');
      await page.locator('[data-testid="nav-folder"][data-category="inbox"]').click();
      await new Promise((resolve) => setTimeout(resolve, 1500));
      console.log('inbox      :', JSON.stringify(await screenState()));
      console.log('--- back to Archives ---');
      await page.locator('[data-testid="nav-folder"][data-category="archive"]').click();
      await new Promise((resolve) => setTimeout(resolve, 1500));
      console.log('archives   :', JSON.stringify(await screenState()));
    }

    // Recovered: nothing in flight, rows visible, no pending state.
    if (switched && snapshot.inFlight === 0 && snapshot.rows > 0
        && snapshot.pending === 0 && snapshot.emptyText === null) {
      recovered = sinceDrag;
      break;
    }
  }

  // --- Wrap-up -----------------------------------------------------------
  const log = (await page.evaluate(() => window.__e2eLog))
    .filter((a) => a.command === 'list_category');
  const settled = log.filter((a) => a.arrival !== null);
  const durations = settled.map((a) => a.arrival - a.start);
  // The ceiling on simultaneous in-flight calls — the E1 invariant (≤ 2 after the fix).
  const bounds = [];
  for (const a of log) {
    bounds.push([a.start, +1]);
    bounds.push([a.arrival ?? Number.MAX_SAFE_INTEGER, -1]);
  }
  bounds.sort((x, y) => x[0] - y[0] || x[1] - y[1]);
  let current = 0;
  let maxInFlight = 0;
  for (const [, delta] of bounds) {
    current += delta;
    if (current > maxInFlight) maxInFlight = current;
  }
  const stats = (values) => {
    if (values.length === 0) return 'no value';
    const sort = [...values].sort((a, b) => a - b);
    const q = (p) => sort[Math.min(sort.length - 1, Math.floor(p * sort.length))];
    return `p50 ${q(0.5).toFixed(0)} ms · p95 ${q(0.95).toFixed(0)} ms · max ${sort[sort.length - 1].toFixed(0)} ms`;
  };
  console.log('\n--- summary ---');
  console.log(`list_category calls: ${log.length} total, ${settled.length} settled`);
  console.log(`simultaneous in-flight (max): ${maxInFlight}`);
  console.log(`end-to-end duration (start -> arrival, queue wait included): ${stats(durations)}`);
  console.log(recovered !== null
    ? `recovery: T+${recovered}s after the end of the drag`
    : 'NO recovery within 180 s');
} finally {
  await browser.close().catch(() => {});
  app.kill();
}
