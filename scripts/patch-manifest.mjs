// patch-manifest.mjs -- adds one darwin platform key to a published
// latest.json (PLAN-MACOS, called by release-macos.sh step 5). A
// separate, checked file -- not an inline `node -e` heredoc -- so the
// gate's `node --check` net covers it and the decision is testable
// (review 2026-09-04: shell-interpolated JS is invisible to every net).
//
//   node scripts/patch-manifest.mjs <manifest> <version> <sigfile> <url> [platform]
//
// Refuses: a manifest of another version, a signature identical to an
// existing channel's (the make-release.ps1 trap 3, crossing), an empty
// signature. Writes in place, 2-space indent, no BOM.

import { readFileSync, writeFileSync } from 'node:fs';

const [manifestPath, version, sigPath, url, platform = 'darwin-x86_64'] = process.argv.slice(2);
if (!manifestPath || !version || !sigPath || !url) {
  console.error('usage: patch-manifest.mjs <manifest> <version> <sigfile> <url> [platform]');
  process.exit(1);
}
const manifest = JSON.parse(readFileSync(manifestPath, 'utf8'));
if (manifest.version !== version) {
  console.error(`latest.json says ${manifest.version}, not ${version}`);
  process.exit(1);
}
const signature = readFileSync(sigPath, 'utf8').trim();
if (!signature) {
  console.error(`empty signature in ${sigPath} -- the updater would refuse the package`);
  process.exit(1);
}
for (const [key, entry] of Object.entries(manifest.platforms ?? {})) {
  if (key !== platform && entry.signature === signature) {
    console.error(`signature identical to ${key} -- crossing, interrupted`);
    process.exit(1);
  }
}
manifest.platforms[platform] = { signature, url };
writeFileSync(manifestPath, JSON.stringify(manifest, null, 2));
console.log(`${platform} added to ${manifestPath}`);
