// Spike D-53 (PLAN-AUDIT-2026-09-LOT5 § 3.2) — the Feed RAM protocol,
// driven through the e2e launcher (same seed, same window, same
// profile as e2e/tests/bench-ram-feed.spec.js), ONE instance isolated
// by PID + profile (measure-ram.ps1 -AppPid -Profil).
//
//   node spikes/feed-memory/bench.mjs <option> <run> [ko]
//
// The option label only tags the output: the Feed.svelte applied in
// the worktree is what gets measured (the launcher rebuilds the dist
// and re-embeds it — rebuild-v2.mjs fingerprint). Raw JSON lines go
// to spikes/feed-memory/raw/<option>-<ko>k-run<run>.jsonl.
import { execSync } from 'node:child_process';
import { appendFileSync, mkdirSync } from 'node:fs';
import path from 'node:path';
import { launchAppV2, closeApp } from '../../e2e/launch.mjs';

const [option = 'A', run = '1', koArg = '100'] = process.argv.slice(2);
const ko = Number(koArg);
const root = path.resolve(import.meta.dirname, '..', '..');
const rawDir = path.join(import.meta.dirname, 'raw');
mkdirSync(rawDir, { recursive: true });
const out = path.join(rawDir, `${option}-${ko}k-run${run}.jsonl`);
const record = (obj) => {
  const line = JSON.stringify({ option, run: Number(run), ko, t: new Date().toISOString(), ...obj });
  console.log(line);
  appendFileSync(out, `${line}\n`);
};
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

const { app, browser, page } = await launchAppV2({
  accounts: [{ email: 'principal@exemple.fr', messages: 200, ko }],
});
const profile = path.join(root, 'target', 'e2e', 'webview2');
const ps = (script, args) => execSync(
  `powershell -NoProfile -ExecutionPolicy Bypass -File "${script}" ${args}`,
  { encoding: 'utf8' },
).trim();
const ram = (phase) => {
  const total = ps(path.join(root, 'e2e', 'measure-ram.ps1'), `-AppPid ${app.pid} -Profil "${profile}"`);
  const detail = JSON.parse(ps(path.join(import.meta.dirname, 'ram-detail.ps1'), `-AppPid ${app.pid} -Profil "${profile}"`));
  const mb = Number(total.replace(/,/g, '.').match(/^([\d.]+) Mo/)?.[1] ?? NaN);
  record({ phase, mb, raw: total, detail });
};

try {
  await page.locator('[data-testid="row"]').first().waitFor({ timeout: 60_000 });
  await sleep(8000);
  ram('rest');
  await page.locator('[data-testid="organized-mode"]').click();
  await page.locator('[data-testid="organized-mode"][aria-checked="true"]').waitFor();
  await page.evaluate(async () => {
    const invoke = window.__TAURI__.core.invoke;
    for (let n = 0; n < 16; n += 1) {
      await invoke('route_sender', { address: `expediteur${n}@exemple.fr`, destination: 'feed', rule: null });
    }
  });
  const t0 = performance.now();
  await page.locator('[data-testid="nav-folder"][data-category="feed"]').click();
  const cards = page.locator('[data-testid="feed-card"]');
  await cards.first().waitFor();
  const firstPaint = performance.now() - t0;
  await sleep(5000);
  const state = () => page.evaluate(() => ({
    cards: document.querySelectorAll('[data-testid="feed-card"]').length,
    iframes: document.querySelectorAll('iframe.body').length,
    dormant: document.querySelectorAll('[data-testid="feed-dormant-body"]').length,
  }));
  record({ phase: 'feed-page-1', ...(await state()), firstCardMs: Math.round(firstPaint) });
  ram('feed-page-1');

  // Frame timing in the page during the scroll: a rAF loop records
  // every frame interval; long frames (> 50 ms) are the visible cost.
  await page.evaluate(() => {
    window.__frames = [];
    let last = performance.now();
    const tick = (now) => { window.__frames.push(now - last); last = now; if (!window.__stopFrames) requestAnimationFrame(tick); };
    requestAnimationFrame(tick);
  });
  // Ten pages: scroll the last card into view until 200 cards, timing
  // each page's arrival (scroll -> card count grows).
  const pages = [];
  let count = await cards.count();
  const scrollStart = performance.now();
  for (let i = 0; i < 40 && count < 200; i += 1) {
    const before = count;
    const s = performance.now();
    await cards.last().scrollIntoViewIfNeeded();
    for (let w = 0; w < 100 && count === before; w += 1) {
      await sleep(100);
      count = await cards.count();
    }
    pages.push({ from: before, to: count, ms: Math.round(performance.now() - s) });
    await sleep(700);
  }
  const scrollMs = Math.round(performance.now() - scrollStart);
  const frames = await page.evaluate(() => { window.__stopFrames = true; return window.__frames; });
  const long = frames.filter((f) => f > 50);
  record({
    phase: 'scroll', pages, scrollMs, frames: frames.length,
    longFrames: long.length, longFramesMs: Math.round(long.reduce((a, b) => a + b, 0)),
    maxFrameMs: Math.round(Math.max(...frames)),
  });
  await sleep(8000);
  record({ phase: 'feed-10-pages', ...(await state()) });
  ram('feed-10-pages');

  await page.locator('[data-testid="nav-folder"][data-category="inbox"]').click();
  await page.locator('[data-testid="row"]').first().waitFor();
  await sleep(8000);
  ram('back-inbox');
  await sleep(25000);
  ram('back-inbox-25s');
} finally {
  await closeApp({ app, browser });
}
