// PLAN-SWEEP-2026-09 — backlog 102 (beta): "the moment I open a
// letter, it turns read — I cannot actually read the flow". The read
// witness fired the instant a card's foot entered the scene: a short
// card fits whole, so it marked itself on arrival. The witness now
// requires a DWELL (2 s in production; the __e2eFeedDwell seam makes
// both directions provable here — the net is proven by breaking it).
import { test, expect } from '@playwright/test';
import { launchAppV2, closeApp, injectArrival } from '../launch.mjs';

let app;
let browser;
let page;

test.describe.configure({ mode: 'serial' });

test.beforeAll(async () => {
  ({ app, browser, page } = await launchAppV2({
    accounts: [{ email: 'principal@exemple.fr', messages: 3 }],
  }));
  await expect(page.locator('[data-testid="row"]').first()).toBeVisible();
  await page.locator('[data-testid="organized-mode"]').click();
  await expect(page.locator('[data-testid="organized-mode"]')).toHaveAttribute('aria-checked', 'true');
  injectArrival({
    email: 'principal@exemple.fr', sender: 'lettre@exemple.fr',
    name: 'La Lettre', subject: 'Edition courte', n: 1, // lang:fr
  });
  await page.reload();
  await expect(page.locator('[data-testid="row"]').first()).toBeVisible();
  await page.evaluate(async () => {
    await window.__TAURI__.core.invoke('route_sender', {
      address: 'lettre@exemple.fr',
      destination: 'feed',
      rule: null,
    });
  });
});

test.afterAll(async () => {
  await closeApp({ app, browser });
});

test('a short card fully in view does NOT turn read on arrival (backlog 102)', async () => {
  // A dwell the test never reaches: any mark fired on arrival is the bug.
  await page.evaluate(() => { window.__e2eFeedDwell = 600000; });
  await page.locator('[data-testid="nav-folder"][data-category="feed"]').click();
  await expect(page.locator('[data-testid="feed-card"]').first()).toBeVisible();
  // The instant the beta tester described: the card just appeared.
  await page.waitForTimeout(700);
  await page.reload();
  await expect(page.locator('[data-testid="row"]').first()).toBeVisible();
  await page.evaluate(() => { window.__e2eFeedDwell = 600000; });
  await page.locator('[data-testid="nav-folder"][data-category="feed"]').click();
  // Still UNREAD: the card sits in the unread section, no read groups.
  await expect(page.locator('[data-testid="feed-section-unread"]')).toBeVisible();
  await expect(page.locator('[data-testid="feed-group"]')).toHaveCount(0);
});

test('the foot held in view past the dwell DOES mark the card read', async () => {
  await page.evaluate(() => { window.__e2eFeedDwell = 100; });
  await page.reload();
  await expect(page.locator('[data-testid="row"]').first()).toBeVisible();
  await page.evaluate(() => { window.__e2eFeedDwell = 100; });
  await page.locator('[data-testid="nav-folder"][data-category="feed"]').click();
  await expect(page.locator('[data-testid="feed-card"]').first()).toBeVisible();
  await page.waitForTimeout(600);
  await page.reload();
  await expect(page.locator('[data-testid="row"]').first()).toBeVisible();
  await page.locator('[data-testid="nav-folder"][data-category="feed"]').click();
  await expect(page.locator('[data-testid="feed-group"]')).toHaveCount(1);
});
