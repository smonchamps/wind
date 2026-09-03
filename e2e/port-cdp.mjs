// Allocation of a free CDP port (PLAN-ISOLATION-E2E).
//
// The hardcoded port 9222 was the ONLY state shared between two e2e
// suites played at the same time from two worktrees (finding 2026-08-15:
// applications dead on startup, cross failures — `connectOverCDP`
// recognizes "its" window by the sole criterion `tauri.localhost`, true for
// any Wind window). Root-cause remedy: no more shared
// port at all — the OS picks a free port on every launch.
//
// TOCTOU window accepted: between the probe closing and WebView2's
// bind, a third party can take the port. The failure is then loud
// (CDP unreachable, log spat back out) and the retry picks another
// port — that's a theoretical flake, not a stable state.
import { createServer } from 'node:net';

export function allocateCdpPort() {
  return new Promise((resolve, reject) => {
    const probe = createServer();
    probe.once('error', reject);
    probe.listen(0, '127.0.0.1', () => {
      const { port } = probe.address();
      probe.close((error) => (error ? reject(error) : resolve(port)));
    });
  });
}
