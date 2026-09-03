// PLAN-ESPACEMENT (A83): three spacing levels between messages —
// "Low" (the existing one, pixel for pixel), "Medium", "High".
//
// This file is the SAFETY NET of the project, and it targets one precise
// bug class: the h1/h2 height templates are MEASURED at render,
// and all the windowing depends on them. A height frozen on the old level
// would make the scrollbar lie by 13.6% to 27.3% and could
// position the window 12,000 px off — a blank screen.
//
// FIRST VERSION REWRITTEN (2026-08-25 review): the previous net
// was partly DECORATIVE, and the review proved it —
//  · "the top row doesn't move" read the internal `first` state,
//    which nothing recomputes when h1 changes outside a pinned section:
//    the assertion passed even with all re-anchoring removed;
//  · "the bar states the true height" compared two members drawn from the
//    SAME h1 — an arithmetic identity, unkillable;
//  · the decor had neither a pin (the path of the real bug) nor a
//    carrier row (h2 never checked), and the window stayed too tall
//    for a phantom bar to even be visible.
// Every test below has been verified capable of FAILING.
import { test, expect } from '@playwright/test';
import { launchAppV2, closeApp, purgeLocals } from '../launch.mjs';

let app;
let browser;
let page;

test.describe.configure({ mode: 'serial' });


// The values from the Chief Engineer's D1 decision. The delta is
// arithmetic: +6 px of padding = +12 px of row, on BOTH templates.
const LEVELS = [
  { id: 'low', pad: 13, h1: 88, h2: 115 },
  { id: 'medium', pad: 19, h1: 100, h2: 127 },
  { id: 'high', pad: 25, h1: 112, h2: 139 },
];

// A DEEP Inbox: there must be enough to scroll far (the geometry
// lie is invisible over ten rows) AND the ability to pin, which
// only the Inbox allows (D4 of A73). The seeder builds threads:
// the decor therefore also carries CARRIER rows, without which h2 would
// never be exercised.
test.beforeAll(async () => {
  ({ app, browser, page } = await launchAppV2({
    accounts: [{ email: 'un@exemple.fr', messages: 400 }],
  }));
  await purgeLocals(page, ['wind-espacement']);
  await page.reload();
  await expect(page.locator('[data-testid="row"]').first()).toBeVisible();
});

test.afterAll(async () => {
  await purgeLocals(page, ['wind-espacement']);
  await closeApp({ app, browser });
});

const setLevel = async (level) => {
  await page.locator('[data-testid="settings"]').click();
  await page.locator('[data-testid="settings-group"][data-group="affichage"]').click();
  await page.locator('[data-testid="display-spacing"]').selectOption(level);
  await page.locator('[data-testid="settings-done"]').click();
  // The switch goes through a render, a layout and a ResizeObserver.
  await page.waitForTimeout(200);
};

const geometry = () => page.evaluate(() => window.__mesure.state());

// WHAT THE USER SEES, not what the component believes: the
// subject of the row actually sitting at the top of the frame. It is the
// only reading a broken re-anchor cannot accidentally satisfy.
const subjectAtTop = () => page.evaluate(() => {
  const frame = document.querySelector('.frame');
  const top = frame.getBoundingClientRect().top;
  let winner = null;
  let gap = Infinity;
  for (const row of frame.querySelectorAll('[data-testid="row"]')) {
    const d = Math.abs(row.getBoundingClientRect().top - top);
    if (d < gap) { gap = d; winner = row; }
  }
  return winner?.querySelector('.subject')?.textContent?.trim() ?? null;
});

test('the three levels give the two expected templates (D1)', async () => {
  for (const level of LEVELS) {
    await setLevel(level.id);
    await expect(page.locator('[data-testid="row"]').first()).toBeVisible();
    const pad = await page.evaluate(() => {
      const l = document.querySelector('[data-testid="row"]');
      return l ? getComputedStyle(l).paddingTop : null;
    });
    expect(pad).toBe(`${level.pad}px`);
    // The PROBED templates follow — they are what the windowing uses.
    // h2 matters as much as h1: `extraChip = h2 - h1` carries the whole
    // calculation for carrier rows.
    const { h1, h2 } = await geometry();
    expect(h1).toBe(level.h1);
    expect(h2).toBe(level.h2);
  }
  await setLevel('low');
});

test('the scrollbar states the REAL height of the rows', async () => {
  // Not tautological: the reference height is MEASURED in the DOM
  // (getBoundingClientRect), not read back from the template that was used
  // to calculate it. A template frozen on the old level would therefore be
  // caught.
  for (const level of LEVELS) {
    await setLevel(level.id);
    const { total } = await geometry();
    // The two templates are measured SEPARATELY in the DOM: the
    // decor's first row is a carrier (it has its chip rank),
    // confusing it with a bare row would say 115 where 88 is expected.
    const measure = await page.evaluate(() => {
      const frame = document.querySelector('.frame');
      const rows = [...frame.querySelectorAll('[data-testid="row"]')];
      const bareRow = rows.find((row) => !row.querySelector('.chips'));
      const carrierRow = rows.find((row) => row.querySelector('.chips'));
      return {
        scrollHeight: frame.scrollHeight,
        hBare: bareRow ? bareRow.getBoundingClientRect().height : null,
        hCarrier: carrierRow ? carrierRow.getBoundingClientRect().height : null,
      };
    });
    expect(measure.hBare).not.toBeNull();
    expect(Math.round(measure.hBare)).toBe(level.h1);
    if (measure.hCarrier !== null) {
      expect(Math.round(measure.hCarrier)).toBe(level.h2);
    }
    // The bar covers at least all the rows at their real height,
    // and not much more (carrier rows add their chip rank).
    expect(measure.scrollHeight).toBeGreaterThanOrEqual(total * level.h1);
    expect(measure.scrollHeight).toBeLessThan(total * (level.h1 + 30));
  }
  await setLevel('low');
});

test('hot switch deep in the list: the row IN VIEW does not move', async () => {
  await setLevel('low');
  await page.evaluate(() => window.__mesure.page(200));
  await page.waitForTimeout(200);

  const indexBefore = (await geometry()).first;
  const subjectBefore = await subjectAtTop();
  expect(indexBefore).toBeGreaterThan(150);
  expect(subjectBefore).toBeTruthy();

  await setLevel('high');

  expect((await geometry()).h1).toBe(112);
  // The assertion that matters: the SAME message is at the top of the screen.
  expect(await subjectAtTop()).toBe(subjectBefore);
  expect(Math.abs((await geometry()).first - indexBefore)).toBeLessThanOrEqual(1);

  await setLevel('low');
});

test("hot switch WITH a pinned conversation — the bug's path", async () => {
  // The bug the review measured only shows up THERE: pinned
  // rows grow with the level, their
  // ResizeObserver wakes the effect that recomputes the position — and it
  // is scheduled BEFORE the re-anchor. Without a pin, this path stays
  // dormant.
  await setLevel('low');
  await page.locator('[data-testid="row"]').first().click();
  const pin = page.locator('[data-testid="reading-pane"] [data-testid="pin"]');
  await pin.click();
  await expect(page.locator('[data-testid="pins"]')).toBeVisible();

  await page.evaluate(() => window.__mesure.page(200));
  await page.waitForTimeout(200);
  const indexBefore = (await geometry()).first;
  const subjectBefore = await subjectAtTop();
  expect(indexBefore).toBeGreaterThan(150);

  await setLevel('high');

  expect(await subjectAtTop()).toBe(subjectBefore);
  expect(Math.abs((await geometry()).first - indexBefore)).toBeLessThanOrEqual(1);

  // Unpin to return the decor for what follows.
  await setLevel('low');
  await page.locator('[data-testid="pins"] [data-testid="row"]').first().click();
  await pin.click();
  await expect(page.locator('[data-testid="pins"]')).toHaveCount(0);
});

test('outside the windowed flow, changing the level does not move the list', async () => {
  // The Drafts folder and search results are not
  // windowed: their position has nothing to do with the flow's geometry.
  // Applying re-anchoring there would scroll the list back to the top on
  // every level change (in Drafts, `total` is 0, so go(0)).
  await page.locator('[data-testid="search-field"]').fill('message');
  await expect(page.locator('[data-testid="results"]')).toBeVisible();
  const results = page.locator('[data-testid="results"] [data-testid="row"]');
  await expect(results.first()).toBeVisible();

  await page.evaluate(() => { document.querySelector('.frame').scrollTop = 300; });
  await page.waitForTimeout(100);
  const before = await page.evaluate(() => document.querySelector('.frame').scrollTop);
  expect(before).toBeGreaterThan(0);

  await setLevel('medium');
  const after = await page.evaluate(() => document.querySelector('.frame').scrollTop);
  // The list may have moved by a few pixels (the rows grew),
  // but it was not SENT BACK to the top nor thrown against the stop.
  expect(after).toBeGreaterThan(0);

  await setLevel('low');
  await page.locator('[data-testid="search-field"]').press('Escape');
  await expect(page.locator('[data-testid="results"]')).toHaveCount(0);
});

test('the level survives relaunch; a garbled value falls back to the default', async () => {
  await setLevel('medium');
  await page.reload();
  await expect(page.locator('[data-testid="row"]').first()).toBeVisible();
  expect((await geometry()).h1).toBe(100);

  // A corrupted preference does not break the list — including a key
  // from the PROTOTYPE: `'toString' in LEVELS` is true, and the guard
  // must go through the list of levels, not the `in` operator.
  for (const corrupted of ['gigantesque', 'toString', 'constructor']) {
    await page.evaluate((v) => localStorage.setItem('wind-espacement', v), corrupted);
    await page.reload();
    await expect(page.locator('[data-testid="row"]').first()).toBeVisible();
    expect((await geometry()).h1).toBe(88);
    const pad = await page.evaluate(() => {
      const l = document.querySelector('[data-testid="row"]');
      return getComputedStyle(l).paddingTop;
    });
    expect(pad).toBe('13px');
  }
});

test('SHORT window: the probes leave no phantom bar', async () => {
  // The probe stack measures ~203 px. The phantom can only be seen
  // if the frame is shorter — at the decor's size it would stay
  // invisible, and removing `position:relative` from the cage would pass.
  // We therefore shrink the frame below the stack, in an
  // empty folder where any excess is necessarily a phantom.
  await page.locator('[data-testid="nav-folder"][data-category="drafts"]').click();
  await expect(page.locator('[data-testid="folder-drafts"]')).toBeVisible();
  // `.frame` is `flex:1` in a column: setting `height` does not
  // constrain it, its growth must be removed instead.
  const measure = await page.evaluate(() => {
    const frame = document.querySelector('.frame');
    const before = frame.style.flex;
    frame.style.flex = '0 0 150px';
    void frame.offsetHeight;
    const r = { scrollHeight: frame.scrollHeight, clientHeight: frame.clientHeight };
    frame.style.flex = before;
    return r;
  });
  expect(measure.clientHeight).toBeLessThanOrEqual(160);
  expect(measure.scrollHeight).toBeLessThanOrEqual(measure.clientHeight);
  await page.locator('[data-testid="nav-folder"][data-category="inbox"]').click();
  await expect(page.locator('[data-testid="row"]').first()).toBeVisible();
});

test('the window recalculates when the frame grows (Chief Engineer decision D3)', async () => {
  // PRE-EXISTING bug fixed by this project: the frame's height was
  // read via `clientHeight`, which is not a signal — enlarging the
  // window left an empty band at the bottom until the next
  // scroll. We change the frame's height, which triggers the same
  // ResizeObserver as a window resize.
  const countRows = () => page.locator('[data-testid="row"]').count();

  await page.evaluate(() => { document.querySelector('.frame').style.flex = '0 0 300px'; });
  await page.waitForTimeout(200);
  const short = await countRows();

  await page.evaluate(() => { document.querySelector('.frame').style.flex = '0 0 1400px'; });
  await page.waitForTimeout(300);
  const long = await countRows();

  // Without the fix, the number of rendered rows would stay that of the
  // short frame: the bottom band would be empty.
  expect(long).toBeGreaterThan(short);

  await page.evaluate(() => { document.querySelector('.frame').style.flex = ''; });
  await page.waitForTimeout(200);
});
