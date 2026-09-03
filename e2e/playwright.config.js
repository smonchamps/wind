import { defineConfig } from '@playwright/test';

export default defineConfig({
  testDir: './tests',
  workers: 1,
  // PLAN-KAIZEN E3, confirmed at PLAN-AUDIT-V2 D4: a flaky test doesn't
  // turn the run red — it gets COUNTED (JSON report read by gate.ps1:
  // "flaky: N"). The figure now exists; the
  // failOnFlakyTests decision will be made on it.
  retries: 1,
  timeout: 180_000,
  expect: { timeout: 15_000 },
  reporter: [['list'], ['json', { outputFile: 'test-results/report.json' }]],
  // The examples build (seeders) lives HERE, outside a spec's
  // 180 s timeout (PLAN-AUDIT-V2 E9); the teardown restores the
  // machine's theme if the "OS follow" test was killed mid-flight.
  globalSetup: './global-setup.mjs',
  globalTeardown: './global-teardown.mjs',
});
