// Tests for the CDP port allocation helper (PLAN-ISOLATION-E2E, E1).
// Deliberately outside `tests/`: that folder belongs to Playwright
// (a single driven window, workers: 1) — these tests are pure
// node:test, run by `node --test` before the suite.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { createServer } from 'node:net';
import { allocateCdpPort } from './port-cdp.mjs';

const listen = (port) =>
  new Promise((resolve, reject) => {
    const server = createServer();
    server.once('error', reject);
    server.listen(port, '127.0.0.1', () => resolve(server));
  });

const close = (server) => new Promise((resolve) => server.close(resolve));

test('the port returned is free: you can listen on it right away', async () => {
  const port = await allocateCdpPort();
  assert.ok(Number.isInteger(port) && port > 0 && port < 65536, `invalid port: ${port}`);
  const server = await listen(port);
  await close(server);
});

test('a port held by another process is never returned', async () => {
  // We hold a port, then allocate several times: the OS must
  // never return the held port — that's the whole promise against
  // concurrent e2e suites.
  const held = await listen(0);
  const heldPort = held.address().port;
  try {
    for (let n = 0; n < 20; n++) {
      assert.notEqual(await allocateCdpPort(), heldPort);
    }
  } finally {
    await close(held);
  }
});
