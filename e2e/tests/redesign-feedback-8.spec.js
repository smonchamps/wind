// PLAN-RETOURS-8: R1 — the account marker (icon + hue, Settings >
// Accounts → nav → list badge, D3: unified mailbox only) and R2 — the
// five-step first-run onboarding journey (accounts, panes,
// theme, beta, end — A91). Decor: two real accounts — the badge only
// makes sense in multi-account.
//
// Hygiene: the WebView2 profile is SHARED between suites — the
// localStorage keys touched (onboarding, panes, widths, theme) are
// removed BEFORE and AFTER. The full journey is forced via the
// `__e2eOnboarding` seam (a seeded decor is otherwise deemed "already
// onboarded" — that is the intended production behavior for updates).
import { test, expect } from '@playwright/test';
import { launchAppV2, closeApp, purgeLocals } from '../launch.mjs';

let app;
let browser;
let page;

test.describe.configure({ mode: 'serial' });


test.beforeAll(async () => {
  ({ app, browser, page } = await launchAppV2({
    accounts: [
      { email: 'un@exemple.fr', messages: 6 },
      { email: 'deux@exemple.fr', messages: 4 },
    ],
  }));
  await purgeLocals(page);
  await page.reload();
  await expect(page.locator('[data-testid="row"]').first()).toBeVisible();
});

test.afterAll(async () => {
  await purgeLocals(page);
  await closeApp({ app, browser });
});

const navMailbox = (label) =>
  page.locator('[data-testid="nav-mailbox"]', { hasText: label });

// ---------------------------------------------------------------- R2 --
// The PRODUCTION path of an update: accounts already there, the
// key absent — the installation is deemed onboarded, the key gets set,
// no journey (this is the state left by beforeAll: purge + reload).

test('an existing installation is deemed onboarded — never a journey', async () => {
  await expect(page.locator('[data-testid="onboarding"]')).toHaveCount(0);
  expect(
    await page.evaluate(() => localStorage.getItem('wind-accueil-fait')),
  ).toBe('1');
});

// ---------------------------------------------------------------- R1 --

test('setting a marker from Settings > Accounts shows it in the nav', async () => {
  // Without a marker: the account mailboxes carry the neutral glyph, no
  // badge anywhere.
  await expect(page.locator('[data-testid="nav-marker"]')).toHaveCount(0);

  await page.locator('[data-testid="settings"]').click();
  await page.locator('[data-testid="account-marker"]').first().click();
  await expect(page.locator('[data-testid="settings-marker"]')).toBeVisible();

  // A marker only exists WHOLE: the icon alone sets nothing.
  await page.locator('[data-testid="marker-icon"][data-icon="home"]').click();
  await page.locator('[data-testid="marker-hue"][data-color="blue"]').click();
  // The row reflects the persisted state (the badge replaces `person`).
  await expect(
    page.locator('[data-testid="account-marker"] .marker').first(),
  ).toHaveAttribute('data-hue', 'blue');
  await page.locator('[data-testid="settings-done"]').click();

  // The nav: the account's mailbox carries the marker's TRACE (A82 — bare
  // glyph in the hue, never again a solid badge), the other account
  // stays at the neutral glyph.
  await expect(page.locator('[data-testid="nav-marker"]')).toHaveCount(1);
  await expect(page.locator('[data-testid="nav-marker"]')).toHaveClass(/bare-marker/);
  await expect(page.locator('[data-testid="nav-marker"]')).toHaveAttribute(
    'data-hue',
    'blue',
  );
  await expect(page.locator('[data-testid="nav-marker"] .ic')).toHaveAttribute('data-name', 'home');
});

test('the mailbox block only lives in the unified mailbox (D3/D7) — and on ALL rows (D8)', async () => {
  // Unified mailbox (default): EVERY row states its mailbox in full
  // (A80/D8 — an account without a marker is no longer indistinguishable);
  // the trace, on the other hand, only appears on the rows of the account
  // with the marker.
  const blocks = page.locator('[data-testid="row-mailbox"]');
  await expect(blocks.first()).toBeVisible();
  const rowCount = await page.locator('[data-testid="row"]').count();
  await expect(blocks).toHaveCount(rowCount);
  const traces = page.locator('[data-testid="row-mailbox"] .bare-marker');
  await expect(traces.first()).toHaveAttribute('data-hue', 'blue');
  const traceCount = await traces.count();
  expect(traceCount).toBeGreaterThan(0);
  expect(traceCount).toBeLessThan(rowCount);

  // The reading pane carries the SAME object (D5/A82): the marker's
  // trace, not a badge — it is the only surface where the thread's
  // trace is verified.
  const atMarker = page
    .locator('[data-testid="row"]')
    .filter({ has: page.locator('[data-testid="row-mailbox"] .bare-marker') })
    .first();
  await atMarker.click();
  const pane = page.locator('[data-testid="reading-pane"]');
  await expect(pane.locator('.mailbox .bare-marker').first()).toBeVisible();
  await expect(pane.locator('.mailbox .bare-marker').first()).toHaveAttribute(
    'data-hue',
    'blue',
  );

  // Single-account view: the block has nothing left to say — none (D7).
  await navMailbox('un@exemple.fr').click();
  await expect(page.locator('[data-testid="row"]').first()).toBeVisible();
  await expect(page.locator('[data-testid="row-mailbox"]')).toHaveCount(0);

  // Back to the unified mailbox for the rest.
  await navMailbox('Toutes les boîtes').click();
  await expect(page.locator('[data-testid="row"]').first()).toBeVisible();
});

test('removing the marker returns the neutral glyph — the block stays, without a trace', async () => {
  await page.locator('[data-testid="settings"]').click();
  await page.locator('[data-testid="account-marker"]').first().click();
  await page.locator('[data-testid="marker-remove"]').click();
  await expect(page.locator('[data-testid="marker-remove"]')).toHaveCount(0);
  await page.locator('[data-testid="settings-done"]').click();

  await expect(page.locator('[data-testid="nav-marker"]')).toHaveCount(0);
  // A80/D8: removing the marker removes the TRACE, never the block — the
  // mailbox is stated in full, marker or not.
  const rowCount = await page.locator('[data-testid="row"]').count();
  await expect(page.locator('[data-testid="row-mailbox"]')).toHaveCount(rowCount);
  await expect(page.locator('[data-testid="row-mailbox"] .bare-marker')).toHaveCount(0);
});

// ---------------------------------------------------------------- R2 --

test('the first-run onboarding journey: five steps, including back', async () => {
  // The seam forces the journey on this seeded decor — and under it,
  // NOTHING is written to the profile (accueil.js): the real path of the
  // key is proven by the "existing installation" test above and
  // by the resume test below.
  await page.addInitScript(() => {
    window.__e2eOnboarding = true;
  });
  // The key set by the beforeAll's decision is removed: the final
  // "still null" assertion then proves that the seam writes NOTHING.
  await page.evaluate(() => localStorage.removeItem('wind-accueil-fait'));
  await page.reload();

  // Step 1: the existing accounts are listed, Continue active (D4:
  // at least one account — there are two).
  const onboarding = page.locator('[data-testid="onboarding"]');
  await expect(onboarding).toBeVisible();
  await expect(page.locator('[data-testid="onboarding-progress"]')).toHaveText(
    'Étape 1/5',
  );
  await expect(page.locator('[data-testid="onboarding-accounts"]')).toContainText(
    'un@exemple.fr',
  );
  // Finding 2 (field visit 2026-08-22): addresses already exist — the
  // add bar is collapsed behind "Ajouter une autre adresse email",
  // and clicking it reopens it.
  await expect(page.locator('[data-testid="onboarding-address"]')).toHaveCount(0);
  await page.locator('[data-testid="onboarding-add-other"]').click();
  await expect(page.locator('[data-testid="onboarding-address"]')).toBeVisible();
  await page.locator('[data-testid="onboarding-continue"]').click();

  // Step 2: the three pane previews. Back first: step 1
  // returns, accounts still there — progress is not lost.
  await expect(page.locator('[data-testid="onboarding-progress"]')).toHaveText(
    'Étape 2/5',
  );
  await page.locator('[data-testid="onboarding-back"]').click();
  await expect(page.locator('[data-testid="onboarding-progress"]')).toHaveText(
    'Étape 1/5',
  );
  await expect(page.locator('[data-testid="onboarding-accounts"]')).toContainText(
    'deux@exemple.fr',
  );
  await page.locator('[data-testid="onboarding-continue"]').click();

  // Choosing "deux volets" applies immediately (appliquerVolets)
  // and the SINGLE preview (2nd pass, finding 3) follows the choice.
  await expect(page.locator('[data-testid="onboarding-pane"]')).toHaveCount(3);
  await page.locator('[data-testid="onboarding-pane"][data-panes="2"]').click();
  await expect(
    page.locator('[data-testid="onboarding-pane"][data-panes="2"]'),
  ).toHaveAttribute('aria-pressed', 'true');
  await expect(page.locator('[data-testid="onboarding-preview"]')).toHaveAttribute(
    'data-panes',
    '2',
  );
  await page.locator('[data-testid="onboarding-continue"]').click();

  // Step 3: the four preview cards (V7 amended, A94); choosing
  // "Elements · nuit" sets the theme instantly (data-theme on the
  // root).
  await expect(page.locator('[data-testid="onboarding-progress"]')).toHaveText(
    'Étape 3/5',
  );
  await expect(page.locator('[data-testid="onboarding-theme"]')).toHaveCount(4);
  await page.locator('[data-testid="onboarding-theme"][data-theme-id="elements-nuit"]').click();
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'elements-nuit');
  await page.locator('[data-testid="onboarding-continue"]').click();

  // Step 4 (RETOURS-11, beta field test): Wind is in beta — the step
  // presents the header's Feedback button and what it does.
  await expect(page.locator('[data-testid="onboarding-progress"]')).toHaveText(
    'Étape 4/5',
  );
  await expect(page.locator('[data-testid="onboarding-beta"]')).toContainText('Feedback');
  await page.locator('[data-testid="onboarding-continue"]').click();

  // Step 5: the recap (finding 8) — the three choices, each
  // links back to its step. Clicking "Disposition" GOES BACK there, then
  // the journey resumes.
  await expect(page.locator('[data-testid="onboarding-progress"]')).toHaveText(
    'Étape 5/5',
  );
  const recap = page.locator('[data-testid="onboarding-recap"]');
  await expect(recap).toContainText('un@exemple.fr');
  await expect(page.locator('[data-testid="recap-panes"]')).toContainText(
    'Deux volets',
  );
  await expect(page.locator('[data-testid="recap-theme"]')).toContainText(
    'Elements · nuit',
  );
  await page.locator('[data-testid="recap-panes"]').click();
  await expect(page.locator('[data-testid="onboarding-progress"]')).toHaveText(
    'Étape 2/5',
  );
  await page.locator('[data-testid="onboarding-continue"]').click();
  await page.locator('[data-testid="onboarding-continue"]').click();
  await page.locator('[data-testid="onboarding-continue"]').click();
  await expect(page.locator('[data-testid="onboarding-progress"]')).toHaveText(
    'Étape 5/5',
  );
  // Finish opens the standard window — in TWO panes (the step 2
  // choice held): no reading pane in the grid. Under the
  // seam, the key is NOT set (no pollution of the profile).
  await page.locator('[data-testid="onboarding-finish"]').click();
  await expect(onboarding).toHaveCount(0);
  await expect(page.locator('[data-testid="row"]').first()).toBeVisible();
  await expect(page.locator('[data-testid="reading-pane"]')).toHaveCount(0);
  expect(
    await page.evaluate(() => localStorage.getItem('wind-accueil-fait')),
  ).toBe(null);
});

test('a journey abandoned midway RESUMES — never deemed onboarded', async () => {
  // The real path, WITHOUT the seam: the "started" mark is there (the
  // journey had displayed), the "done" key absent (Finish was never
  // clicked), accounts exist (added at step 1 before
  // leaving). On the next launch, the journey resumes — the heuristic
  // "accounts ⇒ already onboarded" does not swallow it (2026-08-22 review).
  await page.addInitScript(() => {
    delete window.__e2eOnboarding;
  });
  await page.evaluate(() => {
    localStorage.removeItem('wind-accueil-fait');
    localStorage.setItem('wind-accueil-commence', '1');
  });
  await page.reload();
  await expect(page.locator('[data-testid="onboarding"]')).toBeVisible();
  await expect(page.locator('[data-testid="onboarding-progress"]')).toHaveText(
    'Étape 1/5',
  );
  // Finish cleanly: the key gets set (the REAL write path),
  // the app returns for the following suites.
  await page.locator('[data-testid="onboarding-continue"]').click();
  await page.locator('[data-testid="onboarding-continue"]').click();
  await page.locator('[data-testid="onboarding-continue"]').click();
  await page.locator('[data-testid="onboarding-continue"]').click();
  await page.locator('[data-testid="onboarding-finish"]').click();
  await expect(page.locator('[data-testid="onboarding"]')).toHaveCount(0);
  expect(
    await page.evaluate(() => localStorage.getItem('wind-accueil-fait')),
  ).toBe('1');
});
