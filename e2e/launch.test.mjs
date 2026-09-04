// Tests for the startup decision of the launcher (field finding of
// 2026-09-04). The rule: a window that is UP but carries no frontend is
// a BUILD failure, and it must say so — not leave every selector to
// time out and report the specs as red.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { frontendFailure } from './launch.mjs';

test('the Tauri asset error is named as a build failure, with its remedy', () => {
  const message = frontendFailure('asset not found: index.html');
  assert.ok(message, 'the error page must be recognized');
  assert.match(message, /does not carry its frontend/);
  assert.match(message, /cargo build -p wind-desktop/);
  // The path must reach the reader WHOLE: a backslash inside a JS
  // string ate `target\debug\` on the first write of this message.
  assert.match(message, /target\/debug\/wind-desktop\.exe/);
});

test('the surrounding whitespace of the error page changes nothing', () => {
  assert.ok(frontendFailure('\n  asset not found: index.html  \n'));
});

test('a body that merely SPEAKS of a missing asset is not a build failure', () => {
  // A mail body, a test fixture, a journal line: the error page is the
  // WHOLE document, never a sentence inside a rendered application.
  assert.equal(
    frontendFailure('Sync failed: asset not found: logo.png — will retry'),
    null,
  );
});

test('the application still painting nothing is not a failure (it is early)', () => {
  // attach() finds the page before the app mounts: an empty body is the
  // normal state of a healthy start, and must never abort the launch.
  assert.equal(frontendFailure(''), null);
  assert.equal(frontendFailure(null), null);
  assert.equal(frontendFailure(undefined), null);
});

test('a mounted application is not a failure', () => {
  assert.equal(frontendFailure('Inbox\nNew for you\nPaper trail'), null);
});
