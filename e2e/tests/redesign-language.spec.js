// The interface language (PLAN-LANGUES, E1): catalog audit
// (same keys in fr and en — the fallback must never be used in
// production), immediate switch from Settings > Display, sweep
// of the major screens in English, REAL round trip (reloading rereads the
// preference from the database, not a component state), then a return to
// French — the canonical language of the other flows (L-6).
import { test, expect } from '@playwright/test';
import { launchAppV2, closeApp, purgeLocals } from '../launch.mjs';
import { FR } from '../../apps/desktop/ui-v2/src/lib/catalog.fr.js';
import { EN } from '../../apps/desktop/ui-v2/src/lib/catalog.en.js';

let app;
let browser;
let page;

test.describe.configure({ mode: 'serial' });

test.beforeAll(async () => {
  ({ app, browser, page } = await launchAppV2({ lang: 'fr' }));
});

test.afterAll(async () => {
  await closeApp({ app, browser });
});

test('the fr and en catalogs carry exactly the same keys', () => {
  expect(Object.keys(EN).sort()).toEqual(Object.keys(FR).sort());
});

// D4 (E5b): English is the default; the suite pins the WebView to `--lang=fr`
// (launch.mjs), so the first launch DETECTS French — the default itself is
// tested by e2e/language.test.mjs.
test('the pinned French system language is detected at the first launch', async () => {
  await expect(page.locator('[data-testid="row"]').first()).toBeVisible();
  await expect(page.locator('[data-testid="write"]')).toContainText('Écrire'); // lang:fr
  await expect(page.locator('html')).toHaveAttribute('lang', 'fr');
});

test('the switch to English is immediate, no restart', async () => {
  await page.locator('[data-testid="settings"]').click();
  await page.locator('[data-testid="settings-group"][data-group="affichage"]').click();
  const choice = page.locator('[data-testid="display-language"]');
  await expect(choice).toHaveValue('fr');
  await choice.selectOption('en');
  // The overlay itself has already switched…
  await expect(page.locator('[data-testid="settings-done"]')).toHaveText('Done');
  await page.locator('[data-testid="settings-done"]').click();
  // …and the major screens too: header, nav, tabs, pane, document
  // language (screen readers).
  await expect(page.locator('[data-testid="write"]')).toContainText('Compose');
  await expect(page.locator('[data-testid="nav-folder"][data-category="inbox"]')).toContainText('Inbox');
  await expect(page.locator('[data-testid="nav-folder"][data-category="trash"]')).toContainText('Trash');
  await expect(page.locator('[data-testid="tab"][data-tab="nonlus"]')).toContainText('Unread');
  await expect(page.locator('[data-testid="reading-pane"]')).toContainText('Select a message to read it.');
  await expect(page.locator('html')).toHaveAttribute('lang', 'en');
});

test('the round trip is real: reloading rereads the preference from the database', async () => {
  await page.reload();
  await expect(page.locator('[data-testid="row"]').first()).toBeVisible();
  await expect(page.locator('[data-testid="write"]')).toContainText('Compose');
  await expect(page.locator('html')).toHaveAttribute('lang', 'en');
});

test('reverting to French restores the exact forms of the prototype', async () => {
  await page.locator('[data-testid="settings"]').click();
  await page.locator('[data-testid="settings-group"][data-group="affichage"]').click();
  await page.locator('[data-testid="display-language"]').selectOption('fr');
  await expect(page.locator('[data-testid="settings-done"]')).toHaveText('Terminé'); // lang:fr
  await page.locator('[data-testid="settings-done"]').click();
  await expect(page.locator('[data-testid="write"]')).toContainText('Écrire'); // lang:fr
  await expect(page.locator('[data-testid="reading-pane"]')).toContainText('Sélectionnez un message pour le lire.'); // lang:fr
  await expect(page.locator('html')).toHaveAttribute('lang', 'fr');
});

// R3 (RETOURS-13): the organized Inbox keeps the SHORT name. Only the
// French catalogue can prove it — both English values read "Inbox", so
// organized-mode.spec.js cannot (E6b, Chief Engineer decision D22).
test('in French, the organized Inbox is named by its short form (R3)', async () => {
  await page.locator('[data-testid="organized-mode"]').click();
  await expect(page.locator('[data-testid="organized-mode"]')).toHaveAttribute('aria-checked', 'true');
  const inbox = page.locator('[data-testid="nav-folder"][data-category="inbox"]');
  await expect(inbox).toContainText('Réception'); // lang:fr
  await expect(inbox).not.toContainText('Boîte de réception'); // lang:fr
  await page.locator('[data-testid="organized-mode"]').click();
});

// The French plural of the catalogue (`list.nSelection`, two forms):
// the English value has one form, so multi-select.spec.js cannot prove the
// French one any more (E6b review).
test('in French, the selection count agrees in number', async () => {
  const checkbox = (i) => page.locator('[data-testid="row"]').nth(i).locator('[data-testid="row-checkbox"]');
  await checkbox(0).click();
  await expect(page.locator('[data-testid="bar-selection"]')).toContainText('1 sélectionné'); // lang:fr
  await checkbox(1).click();
  await expect(page.locator('[data-testid="bar-selection"]')).toContainText('2 sélectionnés'); // lang:fr
  await checkbox(1).click();
  await checkbox(0).click();
  await expect(page.locator('[data-testid="bar-selection"]')).toHaveCount(0);
});

// The French sweep of D28 (Chief Engineer decision of 2026-09-03): the
// forms only the French catalogue renders, proven here since the suite
// runs in English — the relative date of the expanded header, the
// cleanup title, and the onboarding step counter (below, fresh launch).
test('in French, the expanded header dates the message in French', async () => {
  await page.locator('[data-testid="row"]').first().click();
  const expanded = page.locator('[data-testid="reading-pane"] [data-testid="message-expanded"]');
  await expect(expanded.locator('.message-head .when')).toHaveText(/^Aujourd'hui, 09:12$/); // lang:fr
});

test('in French, the cleanup screen carries its French title', async () => {
  // the Cleanup rank exists in organized mode only (cleanup.spec.js)
  await page.locator('[data-testid="organized-mode"]').click();
  const rank = page.locator('[data-testid="nav-folder"][data-category="cleanup"]');
  await expect(rank).toContainText('Nettoyage de printemps'); // lang:fr
  await rank.click();
  await expect(page.locator('[data-testid="cleanup"]')).toContainText('En lançant un nettoyage de printemps'); // lang:fr
  await page.locator('[data-testid="nav-folder"][data-category="inbox"]').click();
  await page.locator('[data-testid="organized-mode"]').click();
});

// D4 (E5b, 2026-09-03): English is the default of a REAL first launch —
// an empty database, a WebView whose locale is neither French nor
// English. The pure decision is unit-tested; this is the wiring
// (`lang_get` → `detectLanguage` → `<html lang>` → the onboarding text).
test('a first launch on a non-French system speaks English (D4)', async () => {
  await closeApp({ app, browser });
  ({ app, browser, page } = await launchAppV2({ fresh: true, lang: 'de-DE' }));
  // The suite's WebView2 profile already carries the onboarding-done flag
  // (localStorage), so the empty database opens on the header, not on the
  // onboarding: the header is the witness.
  await expect(page.locator('[data-testid="search-field"]')).toHaveAttribute('placeholder', 'Search messages, people, files');
  await expect(page.locator('html')).toHaveAttribute('lang', 'en');
});

// D28: a REAL French first launch (empty database, French locale, the
// onboarding markers purged) speaks the French onboarding.
test('a first launch on a French system shows the French onboarding steps', async () => {
  await closeApp({ app, browser });
  ({ app, browser, page } = await launchAppV2({ fresh: true, lang: 'fr' }));
  await purgeLocals(page, ['wind-accueil-fait', 'wind-accueil-commence']);
  await page.reload();
  await expect(page.locator('[data-testid="onboarding"]')).toContainText('Étape 1/5'); // lang:fr
  await expect(page.locator('html')).toHaveAttribute('lang', 'fr');
  await purgeLocals(page, ['wind-accueil-fait', 'wind-accueil-commence']);
});
