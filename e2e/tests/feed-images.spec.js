// Field STOP 2 PLAN-AUDIT-V2 (2026-09-02): "Always show
// images" had no effect in the Feed after ten pages scrolled. The guard
// did call the core, then `load(0)` — which only re-serves
// page 0; the merge by key (E10) kept a card past that
// point unchanged. The net targets what the user SEES: the guard of a
// page-2 card disappears on click.
import { test, expect } from '@playwright/test';
import { launchAppV2, closeApp, injectArrival } from '../launch.mjs';

let app;
let browser;
let page;

test.describe.configure({ mode: 'serial' });

test.beforeAll(async () => {
  ({ app, browser, page } = await launchAppV2({
    accounts: [{ email: 'principal@exemple.fr', messages: 6 }],
  }));
});

test.afterAll(async () => {
  await closeApp({ app, browser });
});

test('"Always show images" lifts the guard of a card past page 0', async () => {
  // 25 letters from the same sender, each with a remote image:
  // more than one Feed page (20).
  injectArrival({
    email: 'principal@exemple.fr', sender: 'lettre@exemple.fr',
    name: 'La Lettre', subject: 'Edition', n: 25, body: 'images',
  });
  await page.reload();
  await expect(page.locator('[data-testid="row"]').first()).toBeVisible();
  await page.locator('[data-testid="organized-mode"]').click();
  await expect(page.locator('[data-testid="organized-mode"]')).toHaveAttribute('aria-checked', 'true');
  // Routing to the Feed sets the images rule (RETOURS-14, "Yes = images
  // rule"): we REVOKE it — the field case of letters routed
  // before this rule, or whose rule was removed in Settings.
  await page.evaluate(async () => {
    const invoke = window.__TAURI__.core.invoke;
    await invoke('route_sender', {
      address: 'lettre@exemple.fr',
      destination: 'feed',
      rule: null,
    });
    await invoke('revoke_images_sender', { address: 'lettre@exemple.fr' });
  });
  await page.locator('[data-testid="nav-folder"][data-category="feed"]').click();
  const cards = page.locator('[data-testid="feed-card"]');
  await expect(cards).toHaveCount(20);
  // Page 2 loads on approaching the bottom.
  await cards.last().scrollIntoViewIfNeeded();
  await expect(cards).toHaveCount(25);
  const last = cards.last();
  await last.scrollIntoViewIfNeeded();
  const guard = last.locator('[data-testid="feed-images-guard"]');
  await expect(guard).toBeVisible();
  await guard.getByRole('button', { name: 'Always show images from this sender' }).click();
  // What the user sees: THIS card's guard goes away, the
  // letter's images render (the iframe carries the real URL).
  await expect(guard).toHaveCount(0);
  await expect(last.locator('iframe.body')).toHaveAttribute('srcdoc', /images\.exemple\/lettre-/);
  // And the rule holds for its sisters already served: the first card
  // (page 0) no longer has a guard either.
  await expect(cards.first().locator('[data-testid="feed-images-guard"]')).toHaveCount(0);
});
