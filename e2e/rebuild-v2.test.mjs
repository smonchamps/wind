// Tests for the rebuild decision (PLAN-KAIZEN-CLAUDE wave 2, E1).
// The rule: the mtime bump of main.rs (which forces the re-expansion of
// `generate_context!`, hence recompile + link) is only paid if the dist
// or tauri.conf.json have REALLY changed since the last build.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { mkdtempSync, mkdirSync, writeFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { distFingerprint } from './rebuild-v2.mjs';

function testDist(files) {
  const dir = mkdtempSync(path.join(tmpdir(), 'wind-dist-'));
  for (const [rel, content] of Object.entries(files)) {
    const abs = path.join(dir, rel);
    mkdirSync(path.dirname(abs), { recursive: true });
    writeFileSync(abs, content);
  }
  return dir;
}

test('the same tree gives the same fingerprint (determinism)', () => {
  const a = testDist({ 'index.html': '<html>', 'assets/app.js': 'x=1' });
  const b = testDist({ 'index.html': '<html>', 'assets/app.js': 'x=1' });
  try {
    assert.equal(distFingerprint(a, 'conf'), distFingerprint(b, 'conf'));
  } finally {
    rmSync(a, { recursive: true, force: true });
    rmSync(b, { recursive: true, force: true });
  }
});

test('changed content changes the fingerprint', () => {
  const a = testDist({ 'index.html': '<html>' });
  const b = testDist({ 'index.html': '<html>!' });
  try {
    assert.notEqual(distFingerprint(a, 'conf'), distFingerprint(b, 'conf'));
  } finally {
    rmSync(a, { recursive: true, force: true });
    rmSync(b, { recursive: true, force: true });
  }
});

test('a changed file name changes the fingerprint (Vite hashed assets)', () => {
  const a = testDist({ 'assets/app-abc.js': 'x=1' });
  const b = testDist({ 'assets/app-def.js': 'x=1' });
  try {
    assert.notEqual(distFingerprint(a, 'conf'), distFingerprint(b, 'conf'));
  } finally {
    rmSync(a, { recursive: true, force: true });
    rmSync(b, { recursive: true, force: true });
  }
});

test('the Tauri conf is part of the fingerprint (bench window size)', () => {
  const a = testDist({ 'index.html': '<html>' });
  try {
    assert.notEqual(distFingerprint(a, 'conf-1'), distFingerprint(a, 'conf-2'));
  } finally {
    rmSync(a, { recursive: true, force: true });
  }
});
