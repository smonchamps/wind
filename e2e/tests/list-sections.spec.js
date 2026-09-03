// Field STOP 2 PLAN-AUDIT-V2, pass 2 (2026-09-02): "ALREADY READ"
// overlapped a row, an empty gap one row below. The section
// band is positioned from the height MODEL (h1/h2 probes)
// ; rows stack at their REAL height. The probes rendered
// neither the mailbox block nor the ⋯ of organized mode: 6 px
// less per row, a whole row's worth after twenty. The net targets
// what the user SEES: the band is set on its gap, and the
// first "Already read" row starts right under it.
import { test, expect } from '@playwright/test';
import { launchAppV2, closeApp } from '../launch.mjs';

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

test('the "Already read" band is set on its gap, to the pixel — unified mailbox, organized mode', async () => {
  await expect(page.locator('[data-testid="row"]').first()).toBeVisible();
  await page.locator('[data-testid="organized-mode"]').click();
  await expect(page.locator('[data-testid="organized-mode"]')).toHaveAttribute('aria-checked', 'true');
  await page.locator('[data-testid="nav-folder"][data-category="inbox"]').click();
  const band = page.locator('[data-testid="section"]', { hasText: 'Déjà consulté' });
  await expect(band).toBeVisible();
  // The probes bind via ResizeObserver: we leave a frame or two.
  // 2 px tolerance: the gap falls on a sub-pixel (480.83 px measured).
  await page.waitForTimeout(500);
  const geo = await page.evaluate(() => {
    const band = [...document.querySelectorAll('[data-testid="section"]')]
      .find((e) => e.textContent.includes('Déjà consulté'));
    const empty = document.querySelector('.header-space');
    const rows = [...document.querySelectorAll('.window [data-testid="row"]')];
    const firstRead = rows.find((l) => !l.classList.contains('unread'));
    return {
      band: band.getBoundingClientRect().top,
      empty: empty?.getBoundingClientRect().top ?? null,
      emptyBottom: empty?.getBoundingClientRect().bottom ?? null,
      firstRead: firstRead?.getBoundingClientRect().top ?? null,
    };
  });
  expect(geo.empty).not.toBeNull();
  expect(Math.abs(geo.band - geo.empty)).toBeLessThanOrEqual(2);
  expect(Math.abs(geo.firstRead - geo.emptyBottom)).toBeLessThanOrEqual(2);
});
