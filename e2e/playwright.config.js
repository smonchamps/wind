import { defineConfig } from '@playwright/test';

export default defineConfig({
  testDir: './tests',
  workers: 1,
  // PLAN-KAIZEN E3, confirme a PLAN-AUDIT-V2 D4 : un flaky ne rend pas
  // le run rouge — il se COMPTE (rapport JSON lu par gate.ps1 :
  // « flaky : N »). Le chiffre existe desormais ; la decision
  // failOnFlakyTests se prendra sur lui.
  retries: 1,
  timeout: 180_000,
  expect: { timeout: 15_000 },
  reporter: [['list'], ['json', { outputFile: 'test-results/rapport.json' }]],
  // La compilation des exemples (seeders) vit ICI, hors du timeout de
  // 180 s d'une spec (PLAN-AUDIT-V2 E9) ; le teardown rend a la machine
  // son theme si l'epreuve « suivi OS » a ete tuee en plein vol.
  globalSetup: './global-setup.mjs',
  globalTeardown: './global-teardown.mjs',
});
