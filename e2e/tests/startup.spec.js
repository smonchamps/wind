// The order of the startup burst (PLAN-DEMARRAGE, E2).
//
// Field finding of 2026-08-26, COLD run on the real database (12.84 GB,
// 251,524 envelopes, 64 mailboxes): `list_category` was EMITTED at 89.6 ms
// and SERVED at 440.6 ms — 350 ms, for a clean workload of 28 ms. Between
// the two, seven startup probes were holding the global lock in turn.
// The only command the user is waiting on the result of was the
// last one served.
//
// The cause was not a badly written order: it's that `ready = true`
// (App.svelte) does NOT flush right away — Svelte schedules rendering as a
// microtask. The ten calls that followed were therefore firing BEFORE
// `<List>` was mounted, and the first page arrived tenth. An
// `await tick()` hands control back to the flush: the list requests its page
// FIRST, alone, with no contender for the lock.
//
// WHAT THIS TEST PROVES — and nothing more: the EMISSION order of
// commands at startup. It does NOT prove the SERVICE order:
// `off_pump` takes a `std::sync::Mutex` from a `spawn_blocking`, and
// a mutex is not fair. The opposable proof of the gain remains the
// plateau measured on the bench against a database of real mass.
//
// WHAT IT DOES NOT OBSERVE: the launcher attaches to an already-live page,
// so the assertion covers a RELOAD — warm WebView2 cache, database
// already adopted, accounts already connected. Cold startup is measured on
// the bench (`spikes/demarrage`), never here.
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

// The startup probes that take the global lock. FROZEN list, and
// hardcoded on purpose: a formula like "no periodic probe"
// would be true by construction (the intervals only fire
// at 5 s at the earliest) and could never fail.
const PROBES = [
  'ui_state',
  'markers_get',
  'names_get',
  'list_drafts',
  'telemetry_pending',
  'telemetry_consent_get',
];

test('the first page of the list is requested before the startup probes', async () => {
  // The log must exist BEFORE the first line of application JS:
  // `call()` reads it on every call, and a log placed after
  // startup would be EMPTY of everything we want to observe — the test
  // would then pass vacuously. Hence `addInitScript` + `reload`.
  await page.addInitScript(() => {
    window.__e2eLog = [];
  });
  await page.reload();
  await page.locator('[data-testid="row"]').first().waitFor({ timeout: 30000 });

  const log = await page.evaluate(() =>
    window.__e2eLog.map((entry) => entry.command),
  );

  // --- Anti-vacuity guards, BEFORE any order assertion -------------
  // Without them, an empty or truncated log would make everything else green.
  expect(log.length).toBeGreaterThan(5);
  // `lang_get` fires from main.js, BEFORE App is mounted: if it's missing,
  // the injection didn't get ahead of the entry module and the
  // log doesn't see the startup.
  expect(log[0]).toBe('lang_get');
  // The `ready` boundary: nothing fires before the migration probe
  // has answered (A41).
  expect(log).toContain('migration_check');
  expect(log).toContain('list_category');

  // --- The assertion: the list ahead of every probe ---------------------
  // On the INDEX, never on `start`/`end`: on this decor everything settles
  // within a few milliseconds and a time comparison would be
  // either flaky, or green by noise. The array order is the only
  // contract `log.push` guarantees.
  const listIndex = log.indexOf('list_category');
  // PLAN-AUDIT-V2 E9: the net cannot be EMPTY — eight probes
  // renamed on the app side made this test green without an assertion. Three
  // short-circuit under e2e, the other five must be there.
  const present = PROBES.filter((probe) => log.includes(probe));
  expect(
    present.length,
    `probes seen at startup: ${present.join(', ')} — did a rename empty the net?`,
  ).toBeGreaterThanOrEqual(3);
  for (const probe of PROBES) {
    const probeIndex = log.indexOf(probe);
    // Three commands short-circuit under e2e: absent = nothing to say.
    if (probeIndex === -1) continue;
    expect(
      probeIndex,
      `${probe} is emitted BEFORE the first page of the list `
        + `(${probeIndex} < ${listIndex}) — the list is queuing back behind the probes`,
    ).toBeGreaterThan(listIndex);
  }
});
