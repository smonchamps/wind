// The interface language (PLAN-LANGUES, E1): catalog audit
// (same keys in fr and en — the fallback must never be used in
// production), immediate switch from Settings > Display, sweep
// of the major screens in English, REAL round trip (reloading rereads the
// preference from the database, not a component state), then a return to
// French — the canonical language of the other flows (L-6).
import { test, expect } from '@playwright/test';
import { launchAppV2, closeApp } from '../launch.mjs';
import { FR } from '../../apps/desktop/ui-v2/src/lib/catalog.fr.js';
import { EN } from '../../apps/desktop/ui-v2/src/lib/catalog.en.js';

let app;
let browser;
let page;

test.describe.configure({ mode: 'serial' });

test.beforeAll(async () => {
  ({ app, browser, page } = await launchAppV2());
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
  await expect(page.locator('[data-testid="write"]')).toContainText('Écrire');
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
  await expect(page.locator('[data-testid="settings-done"]')).toHaveText('Terminé');
  await page.locator('[data-testid="settings-done"]').click();
  await expect(page.locator('[data-testid="write"]')).toContainText('Écrire');
  await expect(page.locator('[data-testid="reading-pane"]')).toContainText('Sélectionnez un message pour le lire.');
  await expect(page.locator('html')).toHaveAttribute('lang', 'fr');
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
