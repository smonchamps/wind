// Screen 01 of the redesign (PLAN-UI-V2 §P4, D4): at ZERO accounts,
// the app welcomes — since PLAN-RETOURS-8 (A75), it's the four-step
// ONBOARDING FLOW that opens on a blank base (the `wind-accueil-fait`
// key absent); step 1 carries A11's desk unchanged — the door flows
// (Microsoft, generic IMAP, IPC contract) play inside it as-is.
// Separate launch: the zero-account state cannot be played on the
// Clarity decor.
import { test, expect } from '@playwright/test';
import { launchAppV2, closeApp, purgeLocals } from '../launch.mjs';

let app;
let browser;
let page;

test.describe.configure({ mode: 'serial' });

test.beforeAll(async () => {
  ({ app, browser, page } = await launchAppV2({ fresh: true }));
  // The WebView2 profile is shared: a previous suite may have set
  // the onboarding markers — clear them to play the TRUE first
  // launch.
  await purgeLocals(page, ['wind-accueil-fait', 'wind-accueil-commence']);
  await page.reload();
});

test.afterAll(async () => {
  await purgeLocals(page, ['wind-accueil-fait', 'wind-accueil-commence']);
  await closeApp({ app, browser });
});

test('at zero accounts, the onboarding flow welcomes — step 1, the desk', async () => {
  await expect(page.locator('[data-testid="onboarding"]')).toBeVisible();
  // Field 2026-08-22 (finding 1): "Bienvenue dans Wind", then
  // "Étape 1/5", then the add prompt.
  await expect(page.locator('[data-testid="onboarding"]')).toContainText(
    'Bienvenue dans Wind',
  );
  await expect(page.locator('[data-testid="onboarding-progress"]')).toHaveText(
    'Étape 1/5',
  );
  await expect(page.locator('[data-testid="onboarding"]')).toContainText(
    'Pour commencer, ajoutez une adresse email.',
  );
  // 2nd field pass (finding 2): the "server detected" note has left
  // the onboarding screen; (finding 1): with no account, "Ajouter"
  // is THE gesture — primary.
  await expect(page.locator('[data-testid="onboarding"]')).not.toContainText(
    'Le serveur est détecté automatiquement.',
  );
});

test('with no account added, Continue is ABSENT (D4, 3rd field pass)', async () => {
  // Never a grayed-out button: as long as no account exists, the step
  // does not show Continue — "Ajouter" is the primary gesture.
  await expect(page.locator('[data-testid="onboarding-continue"]')).toHaveCount(0);
});

test('at rest, the progress line says everything is up to date', async () => {
  // The blank base is the ONLY decor truly at rest: zero accounts, so
  // no failed sync (the dummy accounts of the other decors), no
  // pending send, no catch-up. This is the state the v1 test kept —
  // "no banner when every body is there".
  await expect(page.locator('[data-testid="progress"]')).toHaveText(
    'Tous les messages sont à jour',
  );
});

test('an invalid entry is rejected on the spot', async () => {
  await page.locator('[data-testid="onboarding-address"]').fill('pas-une-adresse');
  await page.locator('[data-testid="desk-continue"]').click();
  await expect(page.locator('[data-testid="onboarding-error"]')).toContainText(
    'adresse e-mail complète',
  );
});

// BEFORE the "unknown domain" flow: the desk is stateful in serial
// mode — once the IMAP fields are revealed, they stay revealed.
test('a Microsoft address takes the OAuth route, never the IMAP desk (D4)', async () => {
  await page.locator('[data-testid="onboarding-address"]').fill('paul@outlook.com');
  await page.locator('[data-testid="desk-continue"]').click();
  // The route is the test: the failure comes from the OAuth
  // configuration (MICROSOFT_CLIENT_ID removed by the harness — fast
  // failure, no browser), NOT from a generic desk that would have
  // revealed itself.
  await expect(page.locator('[data-testid="onboarding-error"]')).toContainText(
    'Connexion impossible',
  );
  await expect(page.locator('#ob-imap')).toHaveCount(0);
  await page.locator('[data-testid="onboarding-address"]').fill('');
});

test('an unknown domain reveals the IMAP/SMTP desk, servers proposed', async () => {
  await page.locator('[data-testid="onboarding-address"]').fill('paul@exemple.fr');
  await page.locator('[data-testid="desk-continue"]').click();
  await expect(page.locator('#ob-imap')).toHaveValue('imap.exemple.fr');
  await expect(page.locator('#ob-smtp')).toHaveValue('smtp.exemple.fr');
  // Nothing was sent: no connection error, the form waits.
  await expect(page.locator('[data-testid="onboarding-error"]')).toHaveCount(0);
});

// Ported from compte-generique.spec.js (R2): the generic form's IPC
// contract. The original bug — fields sent flat instead of the
// `input` struct — made IMAP setup impossible WITHOUT any test
// seeing it. We target a host that never resolves (`.test`, a
// reserved TLD): the failure MUST come from the connection, never
// from deserialization.
test('generic account: the form reaches the connection (IPC contract)', async () => {
  await page.locator('#ob-mdp').fill('mot-de-passe-factice');
  await page.locator('#ob-imap').fill('imap.invalide.test');
  await page.locator('#ob-smtp').fill('smtp.invalide.test');
  await page.locator('[data-testid="desk-continue"]').click();

  const error = page.locator('[data-testid="onboarding-error"]');
  await expect(error).toContainText('connexion IMAP impossible', { timeout: 30_000 });
  // The original regression, named: it must never come back.
  await expect(error).not.toContainText('invalid args');
  await expect(error).not.toContainText('missing required key');
});

// Field 2026-08-22 (finding 3): the revealed generic desk carries a
// "Retour" that COLLAPSES the server fields — nothing is sent, the
// address stays.
test('the generic desk collapses via "Retour"', async () => {
  await expect(page.locator('#ob-imap')).toHaveCount(1);
  await page.locator('[data-testid="desk-back"]').click();
  await expect(page.locator('#ob-imap')).toHaveCount(0);
  await expect(page.locator('[data-testid="onboarding-address"]')).toHaveValue(
    'paul@exemple.fr',
  );
});

// LAST (the reload resets the desk to zero, the earlier tests are
// stateful): the second regime of screen 01 — a machine already
// onboarded that fell back to zero accounts finds the desk ALONE, no
// steps (A75).
test('already onboarded, zero accounts: the desk alone, no flow', async () => {
  await page.evaluate(() => localStorage.setItem('wind-accueil-fait', '1'));
  await page.reload();
  await expect(page.locator('[data-testid="onboarding"]')).toBeVisible();
  await expect(page.locator('[data-testid="onboarding"]')).toContainText(
    'Bienvenue dans Wind',
  );
  await expect(page.locator('[data-testid="onboarding-progress"]')).toHaveCount(0);
  await expect(page.locator('[data-testid="onboarding-continue"]')).toHaveCount(0);
});
