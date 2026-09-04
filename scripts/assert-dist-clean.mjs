// assert-dist-clean.mjs -- the release poka-yoke in ONE copy
// (PLAN-AUDIT-V3 E7, D-52 item 8; extracted at PLAN-MACOS review: it
// lived twice, in PowerShell and in bash, and two copies of a
// shipping guard drift). Exits non-zero if any __e2e seam survives in
// the built dist -- `cargo tauri build` embeds WHATEVER dist sits on
// disk, and a gate run leaves a seam-flavored one behind.
//
//   node scripts/assert-dist-clean.mjs [distDir]

import { readdirSync, readFileSync } from 'node:fs';
import { join, extname, dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const dist = resolve(process.argv[2] ?? join(here, '..', 'apps', 'desktop', 'ui-v2', 'dist'));
const extensions = new Set(['.js', '.mjs', '.css', '.html', '.map']);

const leaks = [];
const walk = (folder) => {
  for (const entry of readdirSync(folder, { withFileTypes: true })) {
    const path = join(folder, entry.name);
    if (entry.isDirectory()) walk(path);
    else if (extensions.has(extname(entry.name)) && readFileSync(path, 'utf8').includes('__e2e')) {
      leaks.push(entry.name);
    }
  }
};
walk(dist);

if (leaks.length > 0) {
  console.error(`e2e seams found in the release bundle (${leaks.join(', ')}) -- release interrupted, NOTHING is published.`);
  process.exit(1);
}
console.log('dist rebuilt clean: no __e2e in the bundle');
