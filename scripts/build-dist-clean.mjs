// build-dist-clean.mjs -- the release dist, built ONCE and CLEAN of the
// e2e seams, then asserted (PLAN-AUDIT-2026-09 lot 4, E12d; A09/D-33).
//
// `cargo tauri build` and `cargo tauri dev` run it through
// `build.beforeBuildCommand` / `build.beforeDevCommand` of
// apps/desktop/tauri.conf.json, so the dependency "the dist is rebuilt
// before the Rust build" is declared where the build reads it, not
// re-typed in each release script (they used to carry the same
// sequence twice, in PowerShell and in bash).
//
//   node scripts/build-dist-clean.mjs
//
// Env: VITE_E2E is FORCED to 0 here -- a release or a dev launch never
// embeds a seam; the e2e harness builds its own flavour through
// e2e/rebuild-v2.mjs and `cargo build`, which does not run this hook.

import { execSync } from 'node:child_process';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const ui = join(here, '..', 'apps', 'desktop', 'ui-v2');

execSync('npm run build', {
  cwd: ui,
  stdio: 'inherit',
  env: { ...process.env, VITE_E2E: '0' },
});
execSync(`node "${join(here, 'assert-dist-clean.mjs')}"`, { stdio: 'inherit' });
