// The release scripts against FAKE git/gh/cargo/rustup (audit lot 4, E12a/b):
// the guards refuse before any build, the release commit precedes the
// builds, the tag is pushed and the Release is created as a DRAFT with its
// attestation. Windows only (the scripts are PowerShell); nothing real is
// pushed or uploaded -- every tool on the PATH is a stub that logs its call.
import test from 'node:test';
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { cpSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { tmpdir } from 'node:os';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const root = join(here, '..');
// WIND_RELEASE_SCRIPTS points the harness at another copy of the scripts:
// the net was proven by running it against the pre-lot-4 make-release.ps1
// (2026-09-07), which fails both guards.
const scriptsDir = process.env.WIND_RELEASE_SCRIPTS ?? join(root, 'scripts');
const VERSION = '0.20.0';
const COMMIT = 'c'.repeat(40);
const windows = process.platform === 'win32';

// One stub for every tool: `<tool>.cmd` -> node fake.mjs <tool> <args>.
// Behaviour comes from the environment; every call is appended to FAKE_LOG.
const FAKE = `
import { appendFileSync, existsSync, mkdirSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
const [tool, ...args] = process.argv.slice(2);
appendFileSync(process.env.FAKE_LOG, tool + ' ' + args.join(' ') + '\\n');
const say = (s) => process.stdout.write(s + '\\n');
if (tool === 'git') {
  if (args[0] === 'branch') say(process.env.FAKE_BRANCH ?? 'main');
  else if (args[0] === 'status') {
    if (process.env.FAKE_DIRTY) say(process.env.FAKE_DIRTY);
    // A build that dirtied the tree: the cargo stub leaves a marker file.
    if (existsSync(join(process.env.FAKE_REPO, 'dirty-after-build'))) say(' M Cargo.lock');
  }
  else if (args[0] === 'rev-parse') say(process.env.FAKE_COMMIT);
  else if (args[0] === 'diff') process.exit(args.includes('--cached') ? (process.env.FAKE_STAGED ? 1 : 0) : 0);
  process.exit(0);
}
if (tool === 'rustup') { say('aarch64-pc-windows-msvc'); say('x86_64-pc-windows-msvc'); process.exit(0); }
if (tool === 'cargo') {
  if (args[0] === 'tauri' && args[1] === 'build') {
    const triple = args[args.indexOf('--target') + 1];
    const arch = triple.startsWith('aarch64') ? 'arm64' : 'x64';
    const dir = join(process.env.FAKE_REPO, 'target', triple, 'release', 'bundle', 'nsis');
    mkdirSync(dir, { recursive: true });
    const exe = join(dir, 'Wind_' + process.env.FAKE_VERSION + '_' + arch + '-setup.exe');
    writeFileSync(exe, 'installer ' + arch);
    writeFileSync(exe + '.sig', Buffer.from('signature ' + arch).toString('base64'));
    if (process.env.FAKE_BUILD_DIRTIES) writeFileSync(join(process.env.FAKE_REPO, 'dirty-after-build'), '1');
  }
  process.exit(0);
}
if (tool === 'gh') { if (args[0] === 'api') say(process.env.FAKE_COMMIT); process.exit(0); }
process.exit(0);
`;

function fixture() {
  const dir = mkdtempSync(join(tmpdir(), 'wind-release-scripts-'));
  const repo = join(dir, 'repo');
  mkdirSync(join(repo, 'scripts'), { recursive: true });
  cpSync(join(scriptsDir, 'make-release.ps1'), join(repo, 'scripts', 'make-release.ps1'));
  cpSync(join(root, 'scripts', 'release-lib.mjs'), join(repo, 'scripts', 'release-lib.mjs'));
  mkdirSync(join(repo, 'apps', 'desktop', 'ui-v2', 'dist'), { recursive: true });
  writeFileSync(join(repo, 'apps', 'desktop', 'ui-v2', 'dist', 'index.html'), '<html>fixture</html>');
  writeFileSync(join(repo, 'apps', 'desktop', 'tauri.conf.json'), JSON.stringify({ version: '0.19.0', plugins: { updater: { pubkey: 'x' } } }, null, 2));
  writeFileSync(join(repo, 'Cargo.toml'), '[workspace.package]\nversion = "0.19.0"\n');
  writeFileSync(join(repo, 'Cargo.lock'), '# lock\n');
  writeFileSync(join(repo, 'CHANGELOG.md'), `# Changelog\n\n## [${VERSION}] - 2026-09-07\n\n- fixture\n`);
  const bin = join(dir, 'bin');
  mkdirSync(bin);
  writeFileSync(join(bin, 'fake.mjs'), FAKE);
  for (const tool of ['git', 'gh', 'cargo', 'rustup']) {
    writeFileSync(join(bin, `${tool}.cmd`), `@echo off\r\nnode "%~dp0fake.mjs" ${tool} %*\r\n`);
  }
  return { dir, repo, bin, log: join(dir, 'calls.log') };
}

function run(fx, env = {}) {
  writeFileSync(fx.log, '');
  const result = spawnSync('powershell', ['-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', join(fx.repo, 'scripts', 'make-release.ps1'), VERSION, '-Yes'], {
    cwd: fx.repo,
    encoding: 'utf8',
    env: {
      ...process.env,
      PATH: `${fx.bin};${process.env.PATH}`,
      FAKE_LOG: fx.log, FAKE_REPO: fx.repo, FAKE_VERSION: VERSION, FAKE_COMMIT: COMMIT,
      GOOGLE_CLIENT_ID: 'g', GOOGLE_CLIENT_SECRET: 's', MICROSOFT_CLIENT_ID: 'm',
      ...env,
    },
  });
  const calls = readFileSync(fx.log, 'utf8').split('\n').filter(Boolean);
  return { status: result.status, out: result.stdout + result.stderr, calls };
}

test('a dirty tree or a foreign branch is refused before any bump or build', { skip: !windows && 'PowerShell scripts' }, () => {
  const fx = fixture();
  try {
    let r = run(fx, { FAKE_DIRTY: ' M crates/mail-core/src/lib.rs' });
    assert.notEqual(r.status, 0);
    assert.match(r.out, /not clean/);
    assert.ok(!r.calls.some((c) => c.startsWith('cargo tauri build')), r.calls.join('\n'));
    assert.equal(readFileSync(join(fx.repo, 'apps', 'desktop', 'tauri.conf.json'), 'utf8').includes('0.19.0'), true, 'no bump on refusal');
    r = run(fx, { FAKE_BRANCH: 'feature' });
    assert.notEqual(r.status, 0);
    assert.match(r.out, /from main/);
  } finally {
    rmSync(fx.dir, { recursive: true, force: true });
  }
});

test('the release commit precedes the builds, the tag is pushed and the Release is a draft with its attestation', { skip: !windows && 'PowerShell scripts' }, () => {
  const fx = fixture();
  try {
    const r = run(fx, { FAKE_STAGED: '1' });
    assert.equal(r.status, 0, r.out);
    const index = (prefix) => r.calls.findIndex((c) => c.startsWith(prefix));
    assert.ok(index('git commit') >= 0, r.calls.join('\n'));
    assert.ok(index('git commit') < index('cargo tauri build'), 'commit before the first build');
    assert.ok(index('git push origin ' + VERSION) > index('cargo tauri build'), 'tag pushed after the builds');
    const create = r.calls.find((c) => c.startsWith('gh release create'));
    assert.ok(create, 'a Release is created');
    assert.match(create, /--draft/);
    assert.doesNotMatch(create, /--latest/);
    assert.match(create, new RegExp(`--target ${COMMIT}`));
    assert.match(create, /attestation-windows\.json/);
    assert.ok(index('git push origin ' + VERSION) < index('gh release create'), 'the tag exists before the draft');
    const attestation = JSON.parse(readFileSync(join(fx.repo, 'target', 'aarch64-pc-windows-msvc', 'release', 'bundle', 'nsis', 'attestation-windows.json'), 'utf8'));
    assert.equal(attestation.commit, COMMIT);
    assert.equal(attestation.version, VERSION);
    assert.equal(attestation.artifacts.length, 4, 'two installers and two signatures');
    assert.equal(readFileSync(join(fx.repo, 'apps', 'desktop', 'tauri.conf.json'), 'utf8').includes(`"${VERSION}"`), true, 'bumped');
    const manifest = JSON.parse(readFileSync(join(fx.repo, 'target', 'aarch64-pc-windows-msvc', 'release', 'bundle', 'nsis', 'latest.json'), 'utf8'));
    assert.deepEqual(Object.keys(manifest.platforms).sort(), ['windows-aarch64', 'windows-x86_64']);
  } finally {
    rmSync(fx.dir, { recursive: true, force: true });
  }
});

test('a build that modifies the tree stops the release before any push or draft', { skip: !windows && 'PowerShell scripts' }, () => {
  const fx = fixture();
  try {
    const r = run(fx, { FAKE_STAGED: '1', FAKE_BUILD_DIRTIES: '1' });
    assert.notEqual(r.status, 0);
    assert.match(r.out, /modified the tree/);
    assert.ok(!r.calls.some((c) => c.startsWith('git push')), r.calls.join('\n'));
    assert.ok(!r.calls.some((c) => c.startsWith('gh release create')), r.calls.join('\n'));
  } finally {
    rmSync(fx.dir, { recursive: true, force: true });
  }
});

test('the matrix the scripts consume is the tested one', () => {
  const r = spawnSync('node', [join(root, 'scripts', 'release-lib.mjs'), 'channels', VERSION], { encoding: 'utf8' });
  assert.equal(r.status, 0, r.stderr);
  const matrix = JSON.parse(r.stdout);
  assert.equal(matrix.channels.length, 4);
  assert.equal(matrix.assets.length, 13);
  const win = JSON.parse(spawnSync('node', [join(root, 'scripts', 'release-lib.mjs'), 'channels', VERSION, '--windows-only'], { encoding: 'utf8' }).stdout);
  assert.equal(win.channels.length, 2);
});

test('publish-release refuses to promote a draft whose proof is missing (release-lib rule)', () => {
  // The PowerShell wrapper delegates the decision to release-lib.mjs; the
  // CLI form is what it calls, proven here without gh.
  const dir = mkdtempSync(join(tmpdir(), 'wind-publish-'));
  try {
    writeFileSync(join(dir, 'latest.json'), JSON.stringify({ version: VERSION, platforms: {} }));
    const r = spawnSync('node', [join(root, 'scripts', 'release-lib.mjs'), 'publishable', dir, VERSION, COMMIT], { encoding: 'utf8' });
    assert.notEqual(r.status, 0);
    assert.match(r.stderr, /asset missing/);
    assert.match(r.stderr, /manifest key missing: darwin-aarch64/);
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});
