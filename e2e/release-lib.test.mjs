// The release chain's decisions (audit lot 4, E12a/E12b), proven without
// git, gh or a build: an attestation binds a build to its inputs, and the
// promotion rule refuses every incomplete or inconsistent draft.
import test from 'node:test';
import assert from 'node:assert/strict';
import { mkdtempSync, mkdirSync, writeFileSync, readFileSync, rmSync } from 'node:fs';
import { join } from 'node:path';
import { tmpdir } from 'node:os';
import { attestation, distMismatch, expectedMatrix, publishable, sha256, textDigest } from '../scripts/release-lib.mjs';

const VERSION = '0.20.0';
const COMMIT = 'a'.repeat(40);
const OTHER_COMMIT = 'b'.repeat(40);

const scratch = () => mkdtempSync(join(tmpdir(), 'wind-release-'));

// A complete, consistent draft: eleven assets, four keys, two attestations.
function completeDraft(dir, { commit = COMMIT, lockA = 'lock', lockB = 'lock', distA = 'dist', distB = 'dist' } = {}) {
  const { channels } = expectedMatrix(VERSION);
  const platforms = {};
  const perPlatform = { 'attestation-windows.json': [], 'attestation-macos.json': [] };
  for (const c of channels) {
    writeFileSync(join(dir, c.exe), `bytes of ${c.exe}`);
    const sig = Buffer.from(`sig of ${c.key}`).toString('base64');
    writeFileSync(join(dir, `${c.exe}.sig`), sig);
    if (c.dmg) writeFileSync(join(dir, c.dmg), `dmg ${c.dmg}`);
    platforms[c.key] = { signature: sig, url: `https://github.com/smonchamps/wind/releases/download/${VERSION}/${c.exe}` };
    perPlatform[c.attestation].push(c.exe, `${c.exe}.sig`);
    if (c.dmg) perPlatform[c.attestation].push(c.dmg);
  }
  writeFileSync(join(dir, 'latest.json'), JSON.stringify({ version: VERSION, platforms }));
  const write = (name, platform, lock, dist) => writeFileSync(join(dir, name), JSON.stringify({
    schema: 'wind-release-attestation/1', platform, version: VERSION, tag: VERSION, commit, branch: 'main',
    cargo_lock_sha256: sha256(lock), dist_sha256: sha256(dist),
    artifacts: perPlatform[name].map((n) => ({ name: n, sha256: sha256(readFileSync(join(dir, n))) })),
  }));
  write('attestation-windows.json', 'windows', lockA, distA);
  write('attestation-macos.json', 'macos', lockB, distB);
  return Object.fromEntries(channels.map((c) => [c.key, true]));
}

test('the matrix names eleven assets and four keys, five and two when Windows-only', () => {
  const full = expectedMatrix(VERSION);
  assert.equal(full.assets.length, 11 + 2, 'eleven release assets plus the two attestations');
  assert.equal(full.channels.length, 4);
  const windows = expectedMatrix(VERSION, { windowsOnly: true });
  assert.equal(windows.assets.length, 5 + 1);
  assert.equal(windows.channels.length, 2);
});

test('an attestation binds version, commit, lockfile, dist and every artifact', () => {
  const dir = scratch();
  try {
    mkdirSync(join(dir, 'dist', 'assets'), { recursive: true });
    writeFileSync(join(dir, 'dist', 'index.html'), '<html>');
    writeFileSync(join(dir, 'dist', 'assets', 'index.js'), 'js');
    writeFileSync(join(dir, 'Cargo.lock'), 'lock');
    writeFileSync(join(dir, 'Wind_0.20.0_x64-setup.exe'), 'exe');
    const att = attestation({
      platform: 'windows', version: VERSION, commit: COMMIT, branch: 'main',
      cargoLockPath: join(dir, 'Cargo.lock'), distDir: join(dir, 'dist'),
      artifacts: [join(dir, 'Wind_0.20.0_x64-setup.exe')],
    });
    assert.equal(att.commit, COMMIT);
    assert.equal(att.cargo_lock_sha256, sha256('lock'));
    // A Windows checkout under autocrlf must attest the SAME lockfile as the Mac.
    writeFileSync(join(dir, 'Cargo.lock'), 'lock\r\nline two\r\n');
    const crlf = attestation({ platform: 'windows', version: VERSION, commit: COMMIT, branch: 'main', cargoLockPath: join(dir, 'Cargo.lock'), distDir: join(dir, 'dist'), artifacts: [] });
    assert.equal(crlf.cargo_lock_sha256, textDigest(Buffer.from('lock\nline two\n')));
    assert.equal(att.artifacts[0].name, 'Wind_0.20.0_x64-setup.exe');
    assert.equal(att.artifacts[0].sha256, sha256('exe'));
    const before = att.dist_sha256;
    writeFileSync(join(dir, 'dist', 'assets', 'index.js'), 'js changed');
    const after = attestation({
      platform: 'windows', version: VERSION, commit: COMMIT, branch: 'main',
      cargoLockPath: join(dir, 'Cargo.lock'), distDir: join(dir, 'dist'), artifacts: [],
    }).dist_sha256;
    assert.notEqual(before, after, 'a changed dist changes the digest');
    assert.throws(() => attestation({ platform: 'windows', version: 'v1', commit: COMMIT, branch: 'main', cargoLockPath: join(dir, 'Cargo.lock'), distDir: join(dir, 'dist'), artifacts: [] }));
    assert.throws(() => attestation({ platform: 'windows', version: VERSION, commit: 'abc', branch: 'main', cargoLockPath: join(dir, 'Cargo.lock'), distDir: join(dir, 'dist'), artifacts: [] }));
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});

test('a complete, proven, consistent draft is publishable', () => {
  const dir = scratch();
  try {
    const proofs = completeDraft(dir);
    assert.deepEqual(publishable({ assetsDir: dir, version: VERSION, commit: COMMIT, proofs }), []);
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});

test('an incomplete matrix, a missing proof, a crossed signature or a foreign commit refuse promotion', () => {
  const dir = scratch();
  try {
    const proofs = completeDraft(dir);
    // The mac half missing (B30): never Latest.
    rmSync(join(dir, `Wind_${VERSION}_aarch64.app.tar.gz`));
    let failures = publishable({ assetsDir: dir, version: VERSION, commit: COMMIT, proofs });
    assert.ok(failures.some((f) => f.includes('asset missing')), failures.join('\n'));
    completeDraft(dir);
    // One proof absent (B31): a failure, never "not proven".
    const { 'darwin-x86_64': _, ...partial } = proofs;
    failures = publishable({ assetsDir: dir, version: VERSION, commit: COMMIT, proofs: partial });
    assert.ok(failures.some((f) => f.includes('no cryptographic proof')), failures.join('\n'));
    // A crossed signature.
    const manifest = JSON.parse(readFileSync(join(dir, 'latest.json'), 'utf8'));
    manifest.platforms['windows-x86_64'].signature = manifest.platforms['windows-aarch64'].signature;
    writeFileSync(join(dir, 'latest.json'), JSON.stringify(manifest));
    failures = publishable({ assetsDir: dir, version: VERSION, commit: COMMIT, proofs });
    assert.ok(failures.some((f) => f.includes('crossing')), failures.join('\n'));
    // Built from another commit than the tag (B29).
    completeDraft(dir, { commit: OTHER_COMMIT });
    failures = publishable({ assetsDir: dir, version: VERSION, commit: COMMIT, proofs });
    assert.ok(failures.some((f) => f.includes('the tag points at')), failures.join('\n'));
    // Two platforms built from different lockfiles or UI builds.
    completeDraft(dir, { lockB: 'other lock' });
    failures = publishable({ assetsDir: dir, version: VERSION, commit: COMMIT, proofs });
    assert.ok(failures.some((f) => f.includes('different lockfiles')), failures.join('\n'));
    // Each platform builds its own dist: a difference is said, not refused.
    completeDraft(dir, { distB: 'other dist' });
    failures = publishable({ assetsDir: dir, version: VERSION, commit: COMMIT, proofs });
    assert.deepEqual(failures, []);
    assert.equal(distMismatch(dir), true);
    // Uploaded bytes differing from the attested build.
    completeDraft(dir);
    writeFileSync(join(dir, `Wind_${VERSION}_x64-setup.exe`), 'replaced after the build');
    failures = publishable({ assetsDir: dir, version: VERSION, commit: COMMIT, proofs });
    assert.ok(failures.some((f) => f.includes('differ from the attested build')), failures.join('\n'));
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});

test('the explicit Windows-only exception accepts five assets and two keys, and nothing more', () => {
  const dir = scratch();
  try {
    completeDraft(dir);
    const { channels } = expectedMatrix(VERSION);
    for (const c of channels.filter((c) => c.key.startsWith('darwin'))) {
      rmSync(join(dir, c.exe));
      rmSync(join(dir, `${c.exe}.sig`));
      rmSync(join(dir, c.dmg));
    }
    rmSync(join(dir, 'attestation-macos.json'));
    const manifest = JSON.parse(readFileSync(join(dir, 'latest.json'), 'utf8'));
    delete manifest.platforms['darwin-x86_64'];
    delete manifest.platforms['darwin-aarch64'];
    writeFileSync(join(dir, 'latest.json'), JSON.stringify(manifest));
    const proofs = { 'windows-aarch64': true, 'windows-x86_64': true };
    assert.ok(publishable({ assetsDir: dir, version: VERSION, commit: COMMIT, proofs }).length > 0, 'refused by default');
    assert.deepEqual(publishable({ assetsDir: dir, version: VERSION, commit: COMMIT, proofs, windowsOnly: true }), []);
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});
