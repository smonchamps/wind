// Spike A02 (PLAN-AUDIT-2026-09 lot 5 §3.1): does the global command lock
// delay interactive gestures while heavy work runs?
//
//   node harness.mjs --variant A --exe <wind-desktop.exe> --db <run.db>
//                    --big-uid 199696 --body-uids <json> --attach <file>
//                    --out <raw.json> [--reps 30] [--loads idle,sanitize,attach,both]
//
// Launches the REAL window on the fixture copy (WIND_DB_PATH), attaches
// over CDP (same recipe as e2e/measure-v2.mjs), then drives everything
// from the page through `window.__TAURI__.core.invoke` — the exact
// transport the UI uses (transport.js). Every gesture is timed from the
// page (performance.now around the invoke): IPC + pump scheduling +
// lock wait + work. The heavy work is a page-side loop that re-invokes
// the heavy command as soon as the previous one returns.
//
// Gestures (30 reps each, interleaved open/page/pin so the three see the
// same load pattern):
//   open : thread_messages(row) then message_body(row)  (one figure = sum)
//   page : list_category(reception, offset (n+1)*200, limit 200)
//   pin  : toggle_pin(row)
// Loads:
//   idle     : nothing else running (reference)
//   sanitize : message_body on the ~10 MB body, in a loop (the D-1 path,
//              sanitize under the lock) — SUBSTITUTE for the body backfill
//   attach   : attach_files of the 25 MiB file on a fresh draft, in a loop
//   both     : the two loops together
import { spawn } from 'node:child_process';
import { mkdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import { createServer } from 'node:net';
import { chromium } from '@playwright/test';

const args = Object.fromEntries(
  process.argv.slice(2).reduce((acc, cur, i, all) => {
    if (cur.startsWith('--')) acc.push([cur.slice(2), all[i + 1]]);
    return acc;
  }, []),
);
const root = path.resolve(import.meta.dirname, '..', '..');
const reps = Number(args.reps ?? 30);
const loads = (args.loads ?? 'idle,sanitize,attach,both').split(',');
const bodyUids = JSON.parse(readFileSync(args['body-uids'], 'utf8')).body_uids;
const bigUid = Number(args['big-uid']);

function allocatePort() {
  return new Promise((resolve, reject) => {
    const probe = createServer();
    probe.once('error', reject);
    probe.listen(0, '127.0.0.1', () => {
      const { port } = probe.address();
      probe.close((e) => (e ? reject(e) : resolve(port)));
    });
  });
}

const conf = JSON.parse(readFileSync(path.join(root, 'apps', 'desktop', 'tauri.conf.json'), 'utf8'));
const port = await allocatePort();
const profile = path.join(import.meta.dirname, 'profile');
mkdirSync(profile, { recursive: true });
for (const folder of ['Cache', 'Code Cache']) {
  rmSync(path.join(profile, 'EBWebView', 'Default', folder), { recursive: true, force: true });
}
const env = {
  ...process.env,
  WIND_DB_PATH: args.db,
  WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: [
    conf.app.windows[0].additionalBrowserArgs ?? '',
    `--remote-debugging-port=${port}`,
    '--lang=en',
  ].filter(Boolean).join(' '),
  WEBVIEW2_USER_DATA_FOLDER: profile,
};
for (const v of ['GOOGLE_CLIENT_ID', 'GOOGLE_CLIENT_SECRET', 'MICROSOFT_CLIENT_ID', 'MICROSOFT_CLIENT_SECRET']) delete env[v];

const app = spawn(args.exe, [], { env, stdio: 'ignore' });
let browser = null;
for (let i = 0; i < 300 && !browser; i++) {
  try { browser = await chromium.connectOverCDP(`http://127.0.0.1:${port}`); }
  catch { await new Promise((r) => setTimeout(r, 100)); }
}
if (!browser) { app.kill(); throw new Error('CDP unreachable'); }

const stats = (v) => {
  const s = [...v].sort((a, b) => a - b);
  const q = (p) => s[Math.min(s.length - 1, Math.floor(p * s.length))];
  return { n: s.length, p50: q(0.5), p95: q(0.95), max: s[s.length - 1], mean: s.reduce((a, b) => a + b, 0) / s.length };
};
const fmt = (x) => `p50 ${x.p50.toFixed(1)} · p95 ${x.p95.toFixed(1)} · max ${x.max.toFixed(1)} ms (n=${x.n})`;

const result = { variant: args.variant, exe: args.exe, db: args.db, reps, started: new Date().toISOString(), loads: {} };
try {
  let page = null;
  for (let i = 0; i < 300 && !page; i++) {
    page = browser.contexts().flatMap((c) => c.pages()).find((p) => p.url().includes('tauri.localhost'));
    if (!page) await new Promise((r) => setTimeout(r, 100));
  }
  if (!page) throw new Error('window not found');
  await page.locator('[data-testid="row"]').first().waitFor({ timeout: 90000 });
  // Let the startup settle (count, nav probes) before measuring.
  await page.waitForTimeout(3000);

  // Page-side toolkit, installed once.
  await page.evaluate(({ bodyUids, bigUid, attach }) => {
    const invoke = window.__TAURI__.core.invoke;
    const set = new Set(bodyUids);
    window.__spike = {
      invoke,
      rows: null,
      loadsRunning: {},
      loadTimes: { sanitize: [], attach: [] },
      loadErrors: [],
      async prepare() {
        const first = await invoke('list_category', { category: 'reception', accountId: null, unread: false, offset: 0, limit: 200 });
        // Rows with a cached body, excluding the big one (its own load).
        this.rows = first.rows.filter((r) => set.has(r.uid) && r.uid !== bigUid);
        this.big = first.rows.find((r) => r.uid === bigUid) || null;
        if (!this.big) {
          // The big body's row may not be on page 0: build a synthetic
          // identity from a sibling (same mailbox/version).
          const s = first.rows[0];
          this.big = { account_id: s.account_id, mailbox: s.mailbox, uid: bigUid, version: s.version };
        }
        return { rows: this.rows.length, big: !!this.big };
      },
      async time(fn) { const t0 = performance.now(); await fn(); return performance.now() - t0; },
      async open(row) {
        return this.time(async () => {
          await invoke('thread_messages', { threadId: row.thread_id, accountId: row.account_id, mailbox: row.mailbox, uid: row.uid, version: row.version });
          await invoke('message_body', { accountId: row.account_id, mailbox: row.mailbox, uid: row.uid, version: row.version, showImages: false });
        });
      },
      async page(n) {
        return this.time(() => invoke('list_category', { category: 'reception', accountId: null, unread: false, offset: (n + 1) * 200, limit: 200 }));
      },
      async pin(row) {
        return this.time(() => invoke('toggle_pin', { accountId: row.account_id, mailbox: row.mailbox, uid: row.uid }));
      },
      startLoad(kind) {
        this.loadsRunning[kind] = true;
        const b = this.big;
        const loop = async () => {
          while (this.loadsRunning[kind]) {
            const t0 = performance.now();
            try {
              if (kind === 'sanitize') {
                await invoke('message_body', { accountId: b.account_id, mailbox: b.mailbox, uid: b.uid, version: b.version, showImages: false });
              } else {
                const report = await invoke('attach_files', { accountId: b.account_id, draftId: null, paths: [attach], editToken: null });
                // Reclaim the 25 MiB draft at once (delete_draft is itself
                // under the lock, ~ms): the database stays bounded.
                if (report && report.draft_id != null) await invoke('delete_draft', { id: report.draft_id });
              }
            } catch (e) { this.loadErrors.push(`${kind}: ${String(e?.message ?? e)}`); await new Promise((r) => setTimeout(r, 50)); }
            this.loadTimes[kind].push(performance.now() - t0);
          }
        };
        loop();
      },
      async stopLoads() {
        for (const k of Object.keys(this.loadsRunning)) this.loadsRunning[k] = false;
        // Wait for the in-flight heavy command to return.
        await new Promise((r) => setTimeout(r, 6000));
        const out = { times: this.loadTimes, errors: this.loadErrors };
        this.loadTimes = { sanitize: [], attach: [] };
        this.loadErrors = [];
        return out;
      },
    };
  }, { bodyUids, bigUid, attach: args.attach });
  const prep = await page.evaluate(() => window.__spike.prepare());
  console.log('prepared:', JSON.stringify(prep));
  if (prep.rows < 5) throw new Error('not enough rows with a cached body on page 0');

  for (const load of loads) {
    console.log(`\n== load: ${load} (variant ${args.variant})`);
    if (load === 'sanitize' || load === 'both') await page.evaluate(() => window.__spike.startLoad('sanitize'));
    if (load === 'attach' || load === 'both') await page.evaluate(() => window.__spike.startLoad('attach'));
    if (load !== 'idle') await page.waitForTimeout(2000); // the loop is well under way
    const t = await page.evaluate(async (reps) => {
      const s = window.__spike;
      const out = { open: [], page: [], pin: [] };
      for (let n = 0; n < reps; n++) {
        const row = s.rows[n % s.rows.length];
        out.open.push(await s.open(row));
        out.page.push(await s.page(n));
        out.pin.push(await s.pin(row));
      }
      return out;
    }, reps);
    const loadInfo = await page.evaluate(() => window.__spike.stopLoads());
    result.loads[load] = { gestures: t, load: loadInfo };
    for (const g of ['open', 'page', 'pin']) console.log(`  ${g.padEnd(5)}: ${fmt(stats(t[g]))}`);
    for (const k of ['sanitize', 'attach']) if (loadInfo.times[k].length) console.log(`  load ${k}: ${fmt(stats(loadInfo.times[k]))}`);
    if (loadInfo.errors.length) console.log(`  load errors: ${loadInfo.errors.length} — first: ${loadInfo.errors[0]}`);
    await page.waitForTimeout(1500);
  }
  result.finished = new Date().toISOString();
  writeFileSync(args.out, JSON.stringify(result, null, 1));
  console.log(`\nraw figures -> ${args.out}`);
} finally {
  if (browser) await browser.close().catch(() => {});
  app.kill();
}
