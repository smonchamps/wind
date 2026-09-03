// E2E launcher: builds the application, seeds an ISOLATED database, starts
// the Tauri window with the test hooks, attaches to it via CDP.
//
// Determinism by construction:
// - disposable test database (WIND_DB_PATH) — never the real one;
// - dummy account with an invalid token (WIND_E2E_ACCOUNT) — offline
//   guaranteed, the outbox logs without ever sending anything;
// - OAuth configuration stripped from the environment — no test can
//   touch the real account, even by accident.
//
// Two lessons from the first CI run:
// - **diagnosability**: the application's output is CAPTURED and
//   spat back out on failure. Without that, a startup panic or a missing
//   WebView2 look like a silent timeout, undiagnosable
//   remotely;
// - **wait for the PAGE, not the port**: CDP answers before the window
//   has created its document. Settling for the open port creates a race
//   that shows up as soon as the startup is cold.
import { spawn, execSync } from 'node:child_process';
import { copyFileSync, mkdirSync, renameSync, rmSync, statSync } from 'node:fs';
import { createHash } from 'node:crypto';
import path from 'node:path';
import { chromium } from '@playwright/test';
import { sweepZombies, buildV2, purgeHttpCache } from './rebuild-v2.mjs';
import { purgeOAuth } from './isolation.mjs';
import { allocateCdpPort } from './port-cdp.mjs';
import { browserArgs } from './browser-args.mjs';

const root = path.resolve(import.meta.dirname, '..');
// A first WebView2 startup on a cold machine (CI: no cache,
// antivirus active) largely exceeds the original 30 s.
const READY_TIMEOUT_MS = 90_000;
const POLL_MS = 500;
// Memo of the suite's port (allocated on first launch, see attach).
let suitePort = null;

// The app embeds ui-v2 (the only UI since B2, PLAN-RETRAIT-V1);
// the default decor is the Clarity test data (seed_clarity). The
// rebuild pitfalls (stale dist, zombie, cache) live in
// `rebuild-v2.mjs`, in one place.
// Seeds by TEMPLATE (PLAN-KAIZEN-CLAUDE wave 2, E6): the same
// seed recipe used to be replayed via `cargo run --example` on EVERY spec
// (~14 runs per suite). The template is built once — key =
// recipe + fingerprint of the seeder's exe (a modified seeder invalidates
// it) — then each spec receives a file COPY (the database stays disposable
// and isolated, STANDARD §7.1 upheld). The examples are compiled once
// per suite process; the exe is then run directly.
let examplesBuilt = false;

function seed(db, steps) {
  if (!examplesBuilt) {
    execSync('cargo build -p mail-core --examples', { cwd: root, stdio: 'inherit' });
    examplesBuilt = true;
  }
  const exe = (example) => path.join(root, 'target', 'debug', 'examples', `${example}.exe`);
  const hash = createHash('sha1');
  for (const name of [...new Set(steps.map((step) => step.example))].sort()) {
    const stat = statSync(exe(name));
    hash.update(`${name}|${stat.size}|${stat.mtimeMs}\0`);
  }
  hash.update(JSON.stringify(steps));
  const template = path.join(root, 'target', 'e2e', 'gabarits', `${hash.digest('hex')}.db`);
  // The seeders freeze the clock AT BUILD TIME — relative days
  // ("today", "yesterday") but also `derniere_synchro` set to "2 min
  // ago": a template from an hour ago would make the status bar say
  // "1 hour ago" (PAID red on the push gate,
  // 2026-08-23 — a day-granularity key wasn't enough). Freshness by
  // TTL: past 30 min, we rebuild (~1-4 s), the decor stays
  // within the minute of its wording. AND by calendar day (PAID
  // red on the 2026-08-28→29 pre-push): a template built at 11:50 PM
  // stays "fresh" at 00:15, but its "Today, 09:12" has
  // become "Yesterday" — midnight expires it regardless of the TTL.
  let fresh = false;
  try {
    const built = statSync(template).mtimeMs;
    fresh = Date.now() - built < 30 * 60 * 1000
      && new Date(built).toDateString() === new Date().toDateString();
  } catch {
    /* no template: needs building */
  }
  if (!fresh) {
    mkdirSync(path.dirname(template), { recursive: true });
    // Build alongside then rename: an interrupted seed never leaves
    // a half-full template under the final key — nor its WAL
    // sidecars (orphan frames replayed onto the rebuilt database
    // would poison the cache without changing the key).
    const job = `${template}.chantier`;
    for (const suffix of ['', '-wal', '-shm']) rmSync(`${job}${suffix}`, { force: true });
    for (const step of steps) {
      execSync(`"${exe(step.example)}" "${job}" ${step.args}`.trim(), {
        cwd: root,
        stdio: 'inherit',
      });
    }
    renameSync(job, template);
  }
  copyFileSync(template, db);
}

// `fresh: true`: NEW database and no dummy account — the "zero
// account" state that must show screen 01 (onboarding).
// `accounts: [{email, messages}]`: the seed_inbox decor — the journeys
// carried over from v1 (R2) replay the EXACT seeds of the original specs.
// The local keys the suites touch — the WebView2 profile is
// SHARED between suites, an interrupted run leaves its state behind: we purge
// BEFORE AND AFTER (PLAN-AUDIT-V2 E9: five specs were each copying their own
// list). An already-dead window is not an error.
export const LOCAL_KEYS = [
  'wind-accueil-fait',
  'wind-accueil-commence',
  'wind-volets',
  'wind-largeurs',
  'wind-theme',
  'wind-theme-auto',
  'wind-espacement',
];

export async function purgeLocals(page, keys = LOCAL_KEYS) {
  await page
    .evaluate((list) => {
      for (const key of list) localStorage.removeItem(key);
    }, keys)
    .catch(() => { /* window already dead */ });
}

export async function launchAppV2({ fresh = false, accounts = null, lang = 'en' } = {}) {
  buildV2(root, { release: false });

  const db = path.join(
    root,
    'target',
    'e2e',
    fresh ? 'parcours-v2-vierge.db' : accounts ? 'parcours-v2-inbox.db' : 'parcours-v2.db',
  );
  // A zombie from a previous spec still holding this database
  // would cause an unreadable EBUSY on rmSync — since the build (and its
  // sweep) is memoized, the launcher sweeps itself.
  sweepZombies(root);
  // The database AND its sidecars: a -wal orphaned from a previous run stuck
  // to a freshly copied database would be a lie about the state.
  for (const suffix of ['', '-wal', '-shm']) rmSync(`${db}${suffix}`, { force: true });
  mkdirSync(path.dirname(db), { recursive: true });
  if (fresh) {
    return attach(db, [], lang);
  }
  if (accounts) {
    const steps = [];
    for (const account of accounts) {
      // `ko: N`: each message carries a synthetic body of N KB —
      // the decor for the Feed's RAM measurements (STOP 2 field test,
      // PLAN-AUDIT-V2: 249 MB after ten pages of real letters).
      const heavy = account.ko ? ` ${account.messages} ${account.ko}` : '';
      steps.push({ example: 'seed_inbox', args: `${account.messages} ${account.email}${heavy}` });
      // `archives: N`: an Archives mailbox of N messages, without a body —
      // the decor for deep scrolling (PLAN-DEFILEMENT-PROFOND). The
      // seeder registers the mailbox in the `folders` cache (the canonical
      // archives resolves), the other accounts' Archived/Invoices folders
      // stay intact.
      if (account.archives) {
        steps.push({ example: 'seed_inbox', args: `${account.archives} ${account.email} 0 0 Archives` });
      }
    }
    seed(db, steps);
    // `disconnected: true`: the account lives in the REGISTRY (seeded above)
    // but does not receive a session — the "dead token" state of the real
    // thing, the one Settings can now repair (field test 2026-08-20).
    return attach(
      db,
      accounts.filter((account) => !account.disconnected).map((account) => account.email),
      lang,
    );
  }
  seed(db, [{ example: 'seed_clarity', args: '' }]);
  return attach(db, ['paul.merand@atelier-nord.fr', 'paul@merand.fr'], lang);
}

// Mail ARRIVING mid-spec (PLAN-MODE-ORGANISE E2): envelopes dated NOW
// enter through the production path
// (`upsert_envelopes` — the Screener's arrival decision lives there), into
// the LIVE database of the spec (WAL: the app is running, like a sync).
// The examples are already compiled by `seed`; the caller reloads the
// page to see the new state.
// ⚠️ default `db` = the database of the `accounts` decor (parcours-v2-inbox) —
// a spec launched under ANOTHER decor (Clarity, blank) must pass its own
// database, otherwise the arrival lands in a file the app doesn't read (the
// seeder would come back green, the assertion would go red with no clue).
export function injectArrival({ email, sender, n = 1, name = null, subject = null, replyTo = null, body = null, db = null }) {
  db ??= path.join(root, 'target', 'e2e', 'parcours-v2-inbox.db');
  statSync(db); // the database MUST EXIST — never an arrival into the void
  const exe = path.join(root, 'target', 'debug', 'examples', 'seed_arrival.exe');
  const args = [`"${db}"`, email, sender, String(n)];
  // The arguments are POSITIONAL: `reponseA` (RETOURS-14 R4, the
  // decor for the interleaved thread) requires name and subject ahead of it.
  if (name || subject || replyTo) args.push(`"${name ?? sender}"`);
  if (subject || replyTo) args.push(`"${subject ?? 'Premier contact'}"`);
  if (replyTo || body) args.push(`"${replyTo ?? '-'}"`);
  // `corps: 'images'` (STOP 2 PLAN-AUDIT-V2 field test): a remote-image body
  // per arrival — the decor for the Feed's image guard.
  if (body) args.push(body);
  execSync(`"${exe}" ${args.join(' ')}`, { cwd: root, stdio: 'inherit' });
}

async function attach(db, emails, lang = 'en') {
  // Explicit, writable WebView2 profile: on a CI runner,
  // the default location can be refused. Stable from one launch to
  // the next — a fresh profile every time would make every startup
  // cold, hence slow, for nothing.
  const profile = path.join(root, 'target', 'e2e', 'webview2');
  mkdirSync(profile, { recursive: true });
  purgeHttpCache(profile);

  // Free CDP port, chosen by the OS — one port per SUITE, not per
  // launch: WebView2 shares its browser process per profile, and
  // two launches of the same gate (same profile) must carry IDENTICAL
  // browser arguments — a lingering process with different options
  // would make the environment creation fail. Across
  // worktrees, each suite is a separate Node process: separate
  // ports, no shared state at all (finding 2026-08-15, port-cdp.mjs).
  const port = (suitePort ??= await allocateCdpPort());

  const env = {
    ...process.env,
    WIND_DB_PATH: db,
    // `--lang=<lang>`: language detection on first launch
    // (navigator.language, PLAN-LANGUES) reads the WebView locale — without
    // this pin, the suite would depend on the machine's language. English
    // is the language of the journeys since PLAN-BASCULE-ANGLAIS E6b (the
    // product's default, D4 — Chief Engineer decision D22 of 2026-09-03);
    // redesign-language.spec.js pins `fr` for the French round trip, and
    // a non-French locale proves the D4 default.
    // The PRODUCTION arguments (tauri.conf.json) + the CDP port + the
    // pinned language, composed by browser-args.mjs — the variable
    // OVERRIDES the config at the WebView2 loader level, so it must
    // carry it forward for e2e to see the shipped browser (review
    // 2026-08-16). WIND_E2E_ARGS_EXTRA: pass-through for the measurement
    // benches (E4, measure-scrollbar.mjs) — a flag set in the
    // parent's environment would never reach WebView2 without it.
    WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: browserArgs(
      root,
      port,
      process.env.WIND_E2E_ARGS_EXTRA ?? '',
      lang,
    ),
    WEBVIEW2_USER_DATA_FOLDER: profile,
  };
  if (emails.length > 0) env.WIND_E2E_ACCOUNT = emails.join(',');
  else delete env.WIND_E2E_ACCOUNT;
  purgeOAuth(env);

  const app = spawn(path.join(root, 'target', 'debug', 'wind-desktop.exe'), [], {
    env,
    stdio: ['ignore', 'pipe', 'pipe'],
  });

  // The application's log is our only window into a startup
  // failure: we collect it from the very first line.
  let log = '';
  app.stdout.on('data', (chunk) => {
    log += chunk;
  });
  app.stderr.on('data', (chunk) => {
    log += chunk;
  });
  let exited = null;
  app.on('exit', (code, signal) => {
    exited = { code, signal };
  });

  // We wait for the application's PAGE to be there. We stop dead if the
  // process dies: no point waiting 90 s for a CDP that will never come.
  let browser = null;
  let page = null;
  const deadline = Date.now() + READY_TIMEOUT_MS;
  while (!page && !exited && Date.now() < deadline) {
    try {
      browser ??= await chromium.connectOverCDP(`http://127.0.0.1:${port}`);
      page =
        browser
          .contexts()
          .flatMap((context) => context.pages())
          .find((candidate) => candidate.url().includes('tauri.localhost')) ?? null;
    } catch {
      // Neither the port nor the page is ready: we go around again.
    }
    if (!page) await new Promise((resolve) => setTimeout(resolve, POLL_MS));
  }

  if (!page) {
    if (browser) await browser.close().catch(() => {});
    app.kill();
    throw new Error(startupFailure(exited, browser !== null, log, port));
  }
  return { app, browser, page };
}

/// Failure message that SAYS why: dead process (with its code), silent
/// port, or page never created — and in every case the application's
/// actual output.
function startupFailure(exited, connected, log, port) {
  let cause;
  if (exited) {
    cause = `the application stopped at startup (code ${exited.code}, signal ${exited.signal})`;
  } else if (connected) {
    cause = `CDP reachable on port ${port}, but no "tauri.localhost" page after ${READY_TIMEOUT_MS / 1000} s`;
  } else {
    cause = `CDP unreachable on port ${port} after ${READY_TIMEOUT_MS / 1000} s`;
  }
  const output = log.trim();
  return output
    ? `${cause}\n--- application output ---\n${output}\n--- end ---`
    : `${cause}\n(the application wrote nothing to its output)`;
}

export async function closeApp({ app, browser }) {
  if (browser) await browser.close().catch(() => {});
  if (app) {
    // Wait for the ACTUAL exit: a second launch in the same gate
    // (screen 01 on a blank database) reuses the suite's port and the
    // WebView2 profile — reclaiming them from a still-live process is
    // a race.
    const finished = new Promise((resolve) => {
      if (app.exitCode !== null) resolve();
      else app.once('exit', resolve);
    });
    app.kill();
    await finished;
  }
}
