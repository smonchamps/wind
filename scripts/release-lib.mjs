// release-lib.mjs -- the release chain's DECISIONS, in one testable file
// (audit lot 4, E12a/E12b: B29 build identity, B30 draft-then-Latest).
// The PowerShell and bash scripts do the I/O (git, gh, cargo); this
// module says what a release must carry and when it may be promoted, so
// the same rule is proven by `node --test` on every push and applied on
// release day.
//
//   node scripts/release-lib.mjs attest <out.json> <platform> <version> \
//        <commit> <branch> <cargoLock> <distDir> <artifact>...
//   node scripts/release-lib.mjs publishable <assetsDir> <version> <commit> [--proven=<key>]... [--windows-only]
//   node scripts/release-lib.mjs channels <version> [--windows-only]     (JSON: the matrix, for the .ps1 scripts)
//
// An attestation binds the announced version to the exact inputs of the
// build: the commit, the branch, the lockfile digest, the dist digest and
// the digest of every artifact and signature. `publishable` is the gate of
// publish-release.ps1: the named matrix, the manifest keys, the pairwise
// distinct signatures and both attestations at the tag's commit -- the
// cryptographic proof itself is release-verify's, run by the script.

import { createHash } from 'node:crypto';
import { existsSync, readdirSync, readFileSync, statSync, writeFileSync } from 'node:fs';
import { basename, join } from 'node:path';

export const sha256 = (bytes) => createHash('sha256').update(bytes).digest('hex');

// Text inputs are digested with LF line endings: a Windows checkout under
// autocrlf carries CRLF where the Mac carries LF, and the two platforms'
// attestations must agree on the SAME lockfile (review 2026-09-07).
const TEXT = new Set(['.html', '.js', '.mjs', '.css', '.map', '.json', '.svg', '.txt', '.lock', '.toml']);
export const textDigest = (bytes) => sha256(Buffer.from(bytes.toString('utf8').replace(/\r\n/g, '\n')));
const fileDigest = (path) => {
  const bytes = readFileSync(path);
  const dot = path.lastIndexOf('.');
  const ext = dot >= 0 ? path.slice(dot).toLowerCase() : '';
  return TEXT.has(ext) || path.endsWith('Cargo.lock') ? textDigest(bytes) : sha256(bytes);
};

// The dist digest covers names AND contents, in a stable order.
export function distDigest(distDir) {
  const files = [];
  const walk = (folder, prefix) => {
    for (const entry of readdirSync(folder, { withFileTypes: true }).sort((a, b) => a.name.localeCompare(b.name))) {
      const path = join(folder, entry.name);
      const name = prefix ? `${prefix}/${entry.name}` : entry.name;
      if (entry.isDirectory()) walk(path, name);
      else files.push(`${name}:${fileDigest(path)}`);
    }
  };
  walk(distDir, '');
  return sha256(files.join('\n'));
}

export function attestation({ platform, version, commit, branch, cargoLockPath, distDir, artifacts }) {
  if (!/^[0-9]+\.[0-9]+\.[0-9]+$/.test(version)) throw new Error(`version '${version}' is not MAJOR.MINOR.PATCH`);
  if (!/^[0-9a-f]{40}$/.test(commit)) throw new Error(`commit '${commit}' is not a full SHA-1`);
  return {
    schema: 'wind-release-attestation/1',
    platform,
    version,
    tag: version,
    commit,
    branch,
    cargo_lock_sha256: fileDigest(cargoLockPath),
    dist_sha256: distDigest(distDir),
    artifacts: artifacts.map((path) => ({
      name: basename(path),
      bytes: statSync(path).size,
      sha256: sha256(readFileSync(path)),
    })),
    built_at: new Date().toISOString(),
  };
}

// The channels a complete release serves (STANDARD 2.10: eleven assets,
// four keys). `exe` names the updater artifact of the channel.
export function expectedMatrix(version, { windowsOnly = false } = {}) {
  const windows = [
    { key: 'windows-aarch64', exe: `Wind_${version}_arm64-setup.exe`, attestation: 'attestation-windows.json' },
    { key: 'windows-x86_64', exe: `Wind_${version}_x64-setup.exe`, attestation: 'attestation-windows.json' },
  ];
  const mac = [
    { key: 'darwin-x86_64', exe: `Wind_${version}_x64.app.tar.gz`, dmg: `Wind_${version}_x64.dmg`, attestation: 'attestation-macos.json' },
    { key: 'darwin-aarch64', exe: `Wind_${version}_aarch64.app.tar.gz`, dmg: `Wind_${version}_aarch64.dmg`, attestation: 'attestation-macos.json' },
  ];
  const channels = windowsOnly ? windows : [...windows, ...mac];
  const assets = ['latest.json', ...new Set(channels.map((c) => c.attestation))];
  for (const c of channels) {
    assets.push(c.exe, `${c.exe}.sig`);
    if (c.dmg) assets.push(c.dmg);
  }
  return { channels, assets };
}

// The promotion rule. `assetsDir` holds every asset of the draft, downloaded.
// Returns the list of failures; empty means publishable. Pure on its inputs:
// the cryptographic proof of each signature is reported by the caller in
// `proofs` (channel key -> true when release-verify said VALID).
export function publishable({ assetsDir, version, commit, proofs = {}, windowsOnly = false }) {
  const failures = [];
  const { channels, assets } = expectedMatrix(version, { windowsOnly });
  const present = new Set(existsSync(assetsDir) ? readdirSync(assetsDir) : []);
  for (const name of assets) if (!present.has(name)) failures.push(`asset missing: ${name}`);
  const extra = [...present].filter((name) => !assets.includes(name));
  for (const name of extra) failures.push(`unexpected asset: ${name}`);

  let manifest = null;
  const manifestPath = join(assetsDir, 'latest.json');
  if (present.has('latest.json')) {
    const bytes = readFileSync(manifestPath);
    if (bytes[0] === 0xef && bytes[1] === 0xbb && bytes[2] === 0xbf) failures.push('latest.json carries a BOM');
    try {
      manifest = JSON.parse(bytes.toString('utf8'));
    } catch (err) {
      failures.push(`latest.json unreadable: ${err.message}`);
    }
  }
  if (manifest) {
    if (manifest.version !== version) failures.push(`manifest version '${manifest.version}' is not '${version}'`);
    const keys = Object.keys(manifest.platforms ?? {});
    for (const c of channels) {
      const entry = manifest.platforms?.[c.key];
      if (!entry) {
        failures.push(`manifest key missing: ${c.key}`);
        continue;
      }
      const expectedUrl = `https://github.com/smonchamps/wind/releases/download/${version}/${c.exe}`;
      if (entry.url !== expectedUrl) failures.push(`${c.key}: url '${entry.url}' is not '${expectedUrl}'`);
      const sigPath = join(assetsDir, `${c.exe}.sig`);
      if (existsSync(sigPath) && readFileSync(sigPath, 'utf8').trim() !== entry.signature) {
        failures.push(`${c.key}: manifest signature differs from ${c.exe}.sig`);
      }
      if (proofs[c.key] !== true) failures.push(`${c.key}: no cryptographic proof of the signature`);
    }
    for (const key of keys) {
      if (!channels.some((c) => c.key === key)) failures.push(`manifest key unexpected: ${key}`);
    }
    const signatures = channels.map((c) => manifest.platforms?.[c.key]?.signature).filter(Boolean);
    if (new Set(signatures).size !== signatures.length) failures.push('two channels share one signature (crossing)');
  }

  for (const name of new Set(channels.map((c) => c.attestation))) {
    const path = join(assetsDir, name);
    if (!present.has(name)) continue;
    let att;
    try {
      att = JSON.parse(readFileSync(path, 'utf8'));
    } catch (err) {
      failures.push(`${name} unreadable: ${err.message}`);
      continue;
    }
    if (att.version !== version) failures.push(`${name}: version '${att.version}' is not '${version}'`);
    if (att.commit !== commit) failures.push(`${name}: built from ${att.commit}, the tag points at ${commit}`);
    if (att.branch !== 'main') failures.push(`${name}: built from branch '${att.branch}'`);
    for (const artifact of att.artifacts ?? []) {
      const artifactPath = join(assetsDir, artifact.name);
      if (!existsSync(artifactPath)) continue;
      if (sha256(readFileSync(artifactPath)) !== artifact.sha256) {
        failures.push(`${artifact.name}: the uploaded bytes differ from the attested build`);
      }
    }
  }
  const attestations = [...new Set(channels.map((c) => c.attestation))]
    .filter((name) => present.has(name))
    .map((name) => JSON.parse(readFileSync(join(assetsDir, name), 'utf8')));
  const locks = new Set(attestations.map((a) => a.cargo_lock_sha256));
  if (locks.size > 1) failures.push('the platforms were built from different lockfiles');
  return failures;
}

// Each platform builds its own UI on its own machine; byte-identical output
// across operating systems is not a promise Vite makes. A difference is
// therefore SAID, never a refusal (review 2026-09-07).
export function distMismatch(assetsDir) {
  const digests = ['attestation-windows.json', 'attestation-macos.json']
    .map((name) => join(assetsDir, name))
    .filter((path) => existsSync(path))
    .map((path) => JSON.parse(readFileSync(path, 'utf8')).dist_sha256);
  return new Set(digests).size > 1;
}

const isMain = process.argv[1] && basename(process.argv[1]) === 'release-lib.mjs';
if (isMain) {
  const [command, ...rest] = process.argv.slice(2);
  try {
    if (command === 'attest') {
      const [out, platform, version, commit, branch, cargoLockPath, distDir, ...artifacts] = rest;
      const att = attestation({ platform, version, commit, branch, cargoLockPath, distDir, artifacts });
      writeFileSync(out, JSON.stringify(att, null, 2));
      console.log(`${platform} attestation written: ${out} (commit ${commit.slice(0, 7)}, dist ${att.dist_sha256.slice(0, 12)})`);
    } else if (command === 'publishable') {
      const [assetsDir, version, commit, ...flags] = rest;
      const proofs = {};
      for (const flag of flags) {
        const proven = flag.match(/^--proven=(.+)$/);
        if (proven) proofs[proven[1]] = true;
      }
      const failures = publishable({ assetsDir, version, commit, proofs, windowsOnly: flags.includes('--windows-only') });
      for (const failure of failures) console.error(`FAIL  ${failure}`);
      if (failures.length) process.exit(1);
      if (distMismatch(assetsDir)) console.log('WARN  the two platforms embed different UI builds (each built its own dist); said, not refused');
      console.log(`publishable: ${version} at ${commit.slice(0, 7)}, every check passes`);
    } else if (command === 'channels') {
      const [version, ...flags] = rest;
      if (!/^[0-9]+\.[0-9]+\.[0-9]+$/.test(version ?? '')) throw new Error(`version '${version}' is not MAJOR.MINOR.PATCH`);
      console.log(JSON.stringify(expectedMatrix(version, { windowsOnly: flags.includes('--windows-only') })));
    } else {
      console.error('usage: release-lib.mjs attest <out> <platform> <version> <commit> <branch> <Cargo.lock> <distDir> <artifact>... | publishable <assetsDir> <version> <commit> [--proven=<key>]... [--windows-only] | channels <version> [--windows-only]');
      process.exit(1);
    }
  } catch (err) {
    console.error(`FAIL  ${err.message}`);
    process.exit(1);
  }
}
