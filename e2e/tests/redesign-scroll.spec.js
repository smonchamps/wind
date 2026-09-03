// Deep scrolling (PLAN-DEFILEMENT-PROFOND): a held drag of the
// scrollbar must neither saturate the core nor make the empty screen
// lie.
//
// Field finding of 2026-08-20, measured on the measure-scroll.mjs
// bench: a 2 s drag triggered ~161 `list_category` (one page per
// position crossed, never canceled), the serialized queue of
// `off_pump` drained over minutes on the real base, and during that
// time ALL folders said "Aucun message ici." — the source change
// reset `total = 0` without invalidating the empty-state display
// guard.
//
// Dedicated decor: 6,000 messages in Archives — enough pages for a
// fast drag to cross dozens of them.
import { test, expect } from '@playwright/test';
import { launchAppV2, closeApp } from '../launch.mjs';
import { holdBar } from '../scroll-gesture.mjs';

let app;
let browser;
let page;

test.describe.configure({ mode: 'serial' });

test.beforeAll(async () => {
  ({ app, browser, page } = await launchAppV2({
    accounts: [{ email: 'defil@exemple.fr', messages: 300, archives: 6000 }],
  }));
});

test.afterAll(async () => {
  await closeApp({ app, browser });
});

const folder = (category) =>
  page.locator(`[data-testid="nav-folder"][data-category="${category}"]`);

test('rows never wait on the count — page first, total at rest (field 2026-08-20)', async () => {
  // Counting a category (a NOT EXISTS probe per row over the full
  // set, ~240 ms on 200 k — much more cold) used to delay every
  // FIRST display: it now lives in `category_total`, requested once
  // the page pump is at rest — never ahead of the rows.
  // Startup settled first (inbox, probes): the log must only carry
  // the observed gesture.
  await expect(page.locator('[data-testid="row"]').first()).toBeVisible();
  await new Promise((resolve) => setTimeout(resolve, 1500));
  await page.evaluate(() => {
    window.__e2eLog = [];
  });
  await folder('archive').click();
  await expect(page.locator('[data-testid="row"]').first()).toBeVisible();
  // 6,000 messages: page 0 is FULL, the real total can only come
  // from the count — and it lands in the status, after the rows.
  await expect(page.locator('[data-testid="status"]')).toContainText('Archives · 6000');
  const order = await page.evaluate(() => {
    const log = window.__e2eLog;
    const page0 = log.find((a) => a.command === 'list_category');
    const account = log.find((a) => a.command === 'category_total');
    delete window.__e2eLog;
    return {
      page0Arrival: page0?.arrival ?? null,
      startAccount: account?.start ?? null,
    };
  });
  expect(order.page0Arrival).not.toBeNull();
  expect(order.startAccount).not.toBeNull();
  expect(order.startAccount).toBeGreaterThan(order.page0Arrival);
});

test('a held drag never keeps more than two pages in flight (E1)', async () => {
  await expect(page.locator('[data-testid="row"]').first()).toBeVisible();
  await folder('archive').click();
  await expect(page.locator('[data-testid="list-title"]')).toHaveText('Archives');
  await expect(page.locator('[data-testid="row"]').first()).toBeVisible();
  // Quiescence proven BEFORE the hold: not one pending row on screen,
  // so not one open flight — otherwise the burst assertion could pass
  // vacuously (two leftover flights would already occupy the gauge).
  await expect(page.locator('[data-testid="row-pending"]')).toHaveCount(0);

  // Transport HELD for the whole gesture: the core does not answer —
  // exactly the field saturation, made DETERMINISTIC (on a small, fast
  // decor the queue would not form; on the real base it lasted
  // minutes). The log (the __e2eLog seam) counts what the list ASKS
  // FOR during this silence.
  try {
    await page.evaluate(() => {
      window.__e2eLog = [];
      window.__e2eHold = new Promise((release) => {
        window.__e2eRelease = release;
      });
    });
    // The scrollbar held by click down to 1/3 of the list (about ten
    // pages crossed) — the gesture shared with the bench.
    await holdBar(page, { step: 60 });
    // The invariant that kills the field bug: core silent, the list
    // asks for AT LEAST one page (the gesture moved the window) and
    // NEVER more than 2 — not one per position crossed. Pages passed
    // over are not sent; the next ones wait for a free flight, the
    // core's queue does not grow.
    const requested = await page.evaluate(
      () => window.__e2eLog.filter((a) => a.command === 'list_category').length,
    );
    expect(requested).toBeGreaterThanOrEqual(1);
    expect(requested).toBeLessThanOrEqual(2);
  } finally {
    // Release and clean up NO MATTER WHAT: the suite is serial — a
    // hold that survived would freeze every following test, a log
    // that survived would record every call for the rest of the
    // suite.
    await page.evaluate(() => {
      window.__e2eRelease?.();
      delete window.__e2eHold;
      delete window.__e2eRelease;
      delete window.__e2eLog;
    });
  }

  // The core answers: the CURRENT window is served in a single pair
  // of round trips — rows visible, no more waiting, without first
  // draining a queue of pages that turned invisible.
  await expect(page.locator('[data-testid="row"]').first()).toBeVisible({ timeout: 5000 });
  await expect(page.locator('[data-testid="row-pending"]')).toHaveCount(0, { timeout: 5000 });
});

test('the empty screen only asserts itself after proof — never "No messages here." on a full mailbox (E2)', async () => {
  // Transport HELD: page 0 of the folder being opened does not
  // answer. The screen must SHOW the wait — not assert an emptiness
  // it has not proven (the field bug's lie: "No messages here." in
  // every folder while the queue drained).
  const list = page.locator('[data-testid="list"]');
  try {
    await page.evaluate(() => {
      window.__e2eHold = new Promise((release) => {
        window.__e2eRelease = release;
      });
    });
    await folder('inbox').click();
    await expect(page.locator('[data-testid="list-title"]')).toHaveText('Inbox');
    // During the flight: never the empty message, the wait shows.
    await expect(page.locator('[data-testid="row-pending"]').first()).toBeVisible();
    await expect(list).not.toContainText('No messages here.');
  } finally {
    // Release NO MATTER WHAT: the suite is serial — a hold that
    // survived the test would freeze every following one.
    await page.evaluate(() => {
      window.__e2eRelease?.();
      delete window.__e2eHold;
      delete window.__e2eRelease;
    });
  }
  // Page 0 arrives: the rows take the wait's place.
  await expect(page.locator('[data-testid="row"]').first()).toBeVisible();
  await expect(page.locator('[data-testid="row-pending"]')).toHaveCount(0);
});
