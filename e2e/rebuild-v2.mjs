// Rebuild the application — the SINGLE spot that knows the three
// MEASURED traps of the redesign bench. Each trap cost a session of
// ghosts; they live here, once.
//
// Since B2 (PLAN-RETRAIT-V1), ui-v2 is the ONLY interface: no more
// dist swapping — all that remains is the parity bench's window size.
import { execSync } from 'node:child_process';
import {
  mkdirSync,
  readdirSync,
  readFileSync,
  rmSync,
  utimesSync,
  writeFileSync,
} from 'node:fs';
import { createHash } from 'node:crypto';
import path from 'node:path';

// Fingerprint of the embedded dist + the Tauri config: relative paths
// AND contents (Vite assets are hashed into their NAME — a rename
// alone must be enough to invalidate). Deterministic: sorted walk.
export function distFingerprint(distDir, conf) {
  const hash = createHash('sha1');
  const walk = (dir) => {
    for (const entry of readdirSync(dir, { withFileTypes: true }).sort((a, b) =>
      a.name.localeCompare(b.name),
    )) {
      const abs = path.join(dir, entry.name);
      if (entry.isDirectory()) {
        walk(abs);
      } else {
        hash.update(path.relative(distDir, abs).replaceAll('\\', '/'));
        hash.update('\0');
        hash.update(readFileSync(abs));
        hash.update('\0');
      }
    }
  };
  walk(distDir);
  hash.update(conf);
  return hash.digest('hex');
}

// Kill the surviving bench instances — ONLY the ones spawned from THIS
// target/: never the user's installed application, and never the
// suite of ANOTHER worktree (the original '*\target\*' pattern shot
// down the other suite's application mid-flight — Stop-Process -Force
// = code 0xFFFFFFFF with no output, finding of 2026-08-15,
// PLAN-ISOLATION-E2E). Exported: now that the build is memoized, the
// launcher must sweep itself before resuming a previous spec's
// database — a zombie holding wind.db would produce an unreadable
// EBUSY instead of a clean flake.
export function sweepZombies(root) {
  const sweep =
    'Get-Process wind-desktop -ErrorAction SilentlyContinue | '
    + `Where-Object { $_.Path -like '${path.join(root, 'target')}\\*' } | Stop-Process -Force`;
  try {
    execSync(`powershell -NoProfile -Command "${sweep}"`, { stdio: 'ignore' });
  } catch {
    /* nothing to kill */
  }
}

function construire(root, { release, windowOverride }) {
  // 1. `generate_context!` only embeds the dist when main.rs is
  //    COMPILED: a change to assets ALONE recompiles nothing, and the
  //    binary would keep a stale dist (observed: CSS rule present on
  //    disk, absent from the loaded stylesheets). The mtime bump
  //    forces re-expansion — but only if the dist or the config have
  //    REALLY changed since the last build: bumping on every launch
  //    used to pay for a full link per spec, even when nothing changed
  //    (PLAN-KAIZEN-CLAUDE wave 2, E1 — ~74 s per spec, dominated by
  //    the rebuild).
  const fingerprint = distFingerprint(
    path.join(root, 'apps', 'desktop', 'ui-v2', 'dist'),
    readFileSync(path.join(root, 'apps', 'desktop', 'tauri.conf.json'), 'utf8') +
      JSON.stringify(windowOverride),
  );
  const storedFile = path.join(
    root,
    'target',
    'e2e',
    `empreinte-rebuild-${release ? 'release' : 'debug'}.txt`,
  );
  let stored = null;
  try {
    stored = readFileSync(storedFile, 'utf8');
  } catch {
    /* first build: no fingerprint yet */
  }
  if (stored !== fingerprint) {
    utimesSync(path.join(root, 'apps', 'desktop', 'src', 'main.rs'), new Date(), new Date());
  }
  // 2. A zombie bench locks the exe: the LINK fails ("access denied")
  //    and the old binary would silently be replayed (observed).
  sweepZombies(root);
  // 3. Any config swap (parity-bench window size): RESTORED even on
  //    failure — the repository is never left dirty.
  const conf = path.join(root, 'apps', 'desktop', 'tauri.conf.json');
  const command = `cargo build -p wind-desktop${release ? ' --release' : ''}`;
  if (!windowOverride) {
    execSync(command, { cwd: root, stdio: 'inherit' });
  } else {
    const original = readFileSync(conf, 'utf8');
    const modified = JSON.parse(original);
    Object.assign(modified.app.windows[0], windowOverride);
    try {
      writeFileSync(conf, JSON.stringify(modified, null, 2));
      execSync(command, { cwd: root, stdio: 'inherit' });
    } finally {
      writeFileSync(conf, original);
    }
  }
  // The fingerprint is only written AFTER a successful build: an
  // interrupted build must not make the next launch believe the
  // binary is good.
  mkdirSync(path.dirname(storedFile), { recursive: true });
  writeFileSync(storedFile, fingerprint);
}

// Memoized per suite PROCESS: Playwright reuses its worker from one
// spec file to the next (workers: 1) — the first spec pays for the
// build, the following ones pay nothing at all, not even Vite's `npm
// run build` (~3 s) nor the cargo no-op. A worker restarted (after a
// failure) comes back through here: the Vite build replays, and the
// on-disk fingerprint then avoids the bump — cargo only checks.
const alreadyBuilt = new Set();

export function buildV2(root, { release = true, windowOverride = null, seams = false } = {}) {
  const key = `${root}|${release}|${seams}|${JSON.stringify(windowOverride)}`;
  if (alreadyBuilt.has(key)) return;
  // The seam flavor (PLAN-AUDIT-V3 E7, D-52 item 8): the e2e build
  // compiles the `__e2e*` seams IN (VITE_E2E=1), every other build
  // compiles them OUT. Vite folds the flag at build time; the release
  // path (make-release.ps1) additionally ASSERTS their absence in the
  // bundle before cargo tauri build embeds it.
  execSync('npm run build', {
    cwd: path.join(root, 'apps', 'desktop', 'ui-v2'),
    stdio: 'inherit',
    env: { ...process.env, VITE_E2E: seams ? '1' : '0' },
  });
  construire(root, { release, windowOverride });
  alreadyBuilt.add(key);
}

// The WebView2 profile's HTTP cache survives rebuilds and can serve a
// stale index.html with its old hashed assets — ghost styles, CSP from
// another era (observed). We purge the CACHE, not the profile: the GPU
// cache that makes startups warm stays.
export function purgeHttpCache(profile) {
  for (const folder of ['Cache', 'Code Cache']) {
    rmSync(path.join(profile, 'EBWebView', 'Default', folder), {
      recursive: true,
      force: true,
    });
  }
}
