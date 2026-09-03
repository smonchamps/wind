// PLAN-RETOURS-10 R1: the list's multi-select — Ctrl-click,
// Shift-click, hover checkbox — and the grouped-action bar (D1-D4,
// D6, field verdicts from 2026-08-27).
//
// The net targets what the user SEES (lesson from PLAN-ESPACEMENT: a
// net is proven by breaking it): the transformed bar and its count,
// the checked box, the nav's unread badge, the rows that leave the
// mailbox. ORDER THOUGHT THROUGH (serial suite, decor isolated per
// spec): read/unread marking plays FIRST — since field finding
// R1-1, Ctrl-click moves the reading focus and so MARKS READ, and
// any test that plays it before would skew the badge; destructive
// gestures play LAST.
import { test, expect } from '@playwright/test';
import { launchAppV2, closeApp } from '../launch.mjs';

let app;
let browser;
let page;

test.describe.configure({ mode: 'serial' });

test.beforeAll(async () => {
  ({ app, browser, page } = await launchAppV2());
  await expect(page.locator('[data-testid="row"]').first()).toBeVisible();
});

test.afterAll(async () => {
  await closeApp({ app, browser });
});

const rows = () => page.locator('[data-testid="row"]');
const bar = () => page.locator('[data-testid="bar-selection"]');
const checked = () =>
  page.locator('[data-testid="row-checkbox"][aria-checked="true"]');
const checkboxOf = (i) => rows().nth(i).locator('[data-testid="row-checkbox"]');
const folder = (category) =>
  page.locator(`[data-testid="nav-folder"][data-category="${category}"]`);
const toast = () => page.locator('[data-testid="toast"]');

// The resolved color of --sel, to compare computed backgrounds.
const selHue = () =>
  page.evaluate(() => {
    const d = document.createElement('div');
    d.style.background = 'var(--sel)';
    document.body.appendChild(d);
    const v = getComputedStyle(d).backgroundColor;
    d.remove();
    return v;
  });

test('grouped read marking via checkboxes: the badge drops — then unread raises it again', async () => {
  // The Clarity decor carries 4 unread in the Inbox (redesign-screen02).
  const badge = folder('inbox').locator('.badge');
  await expect(badge).toHaveText('4');
  // We check via the CHECKBOX (it does not select, so it marks nothing
  // in passing), and via the REMAINING unchecked box — never by index
  // on a live locator: rows are keyed by index and one reserved mid-
  // loop would target (and UN-check) another row — the exact local
  // flake profile (reviewed).
  const toCheck = page.locator(
    '[data-testid="row"].unread [data-testid="row-checkbox"][aria-checked="false"]',
  );
  while ((await toCheck.count()) > 0) {
    await toCheck.first().click();
  }
  await page.locator('[data-testid="bar-read"]').click();
  await expect(toast()).toContainText('marquées lues');
  // The completed gesture clears the selection, and the nav states the new count.
  await expect(bar()).toHaveCount(0);
  await expect(badge).toHaveCount(0);
  // Grouped unread on a row WITHOUT a thread (a thread re-marked unread
  // would count all its messages — D6): the badge climbs back to 1.
  const simple = rows()
    .filter({ hasNot: page.locator('.chip', { hasText: /message/ }) })
    .first();
  await simple.locator('[data-testid="row-checkbox"]').click();
  await page.locator('[data-testid="bar-unread"]').click();
  await expect(badge).toHaveText('1');
});

test('Ctrl-click checks AND moves the reading focus (field finding R1-1); Cancel clears', async () => {
  const subject0 = (await rows().nth(0).locator('.subject').textContent()).trim();
  await rows().nth(0).click({ modifiers: ['Control'] });
  // The list bar transforms (D3): the count + the actions.
  await expect(bar()).toBeVisible();
  await expect(bar()).toContainText('1 sélectionné');
  await expect(checked()).toHaveCount(1);
  // Field finding R1-1: the border AND the pane follow the Ctrl-clicked row.
  await expect(rows().nth(0)).toHaveClass(/chosen/);
  await expect(
    page.locator('[data-testid="reading-pane"] [data-testid="thread-subject"]'),
  ).toContainText(subject0);
  // Ctrl-click elsewhere adds — and the focus still follows; on a
  // checked one, it removes (toggle).
  await rows().nth(2).click({ modifiers: ['Control'] });
  await expect(bar()).toContainText('2 sélectionnés');
  await expect(rows().nth(2)).toHaveClass(/chosen/);
  await expect(rows().nth(0)).not.toHaveClass(/chosen/);
  await rows().nth(2).click({ modifiers: ['Control'] });
  await expect(bar()).toContainText('1 sélectionné');
  // Cancel returns the list to rest: title bar, zero boxes.
  await page.locator('[data-testid="bar-cancel"]').click();
  await expect(bar()).toHaveCount(0);
  await expect(page.locator('[data-testid="list-title"]')).toBeVisible();
  await expect(checked()).toHaveCount(0);
});

test('Shift-click extends from the selected row (field finding R1-2)', async () => {
  // A BARE click selects the first row — it is the anchor: the exact
  // scenario from the finding (the first message selected by
  // default, then Shift-click further down → the whole range checks).
  await rows().nth(0).click();
  await rows().nth(3).click({ modifiers: ['Shift'] });
  await expect(bar()).toContainText('4 sélectionnés');
  await expect(checked()).toHaveCount(4);
  await page.locator('[data-testid="bar-cancel"]').click();
  await expect(bar()).toHaveCount(0);
});

test("the checkbox lives on hover, checks without selecting, and the content moves aside (D4, field finding R1-3)", async () => {
  // At rest the checkbox is invisible (opacity 0 — it stays in the DOM);
  // hover reveals it AND pushes the content aside (padding 16 → 34 px,
  // the height does not move).
  const opacity = (loc) => loc.evaluate((el) => getComputedStyle(el).opacity);
  const leftPad = (i) =>
    rows().nth(i).evaluate((el) => getComputedStyle(el).paddingLeft);
  expect(await opacity(checkboxOf(1))).toBe('0');
  expect(await leftPad(1)).toBe('16px');
  await rows().nth(1).hover();
  await expect.poll(async () => opacity(checkboxOf(1))).toBe('1');
  await expect.poll(() => leftPad(1)).toBe('34px');
  await checkboxOf(1).click();
  await expect(bar()).toContainText('1 sélectionné');
  // The checkbox does not select: the border has not moved on this row.
  await expect(rows().nth(1)).not.toHaveClass(/chosen/);
  // As soon as a selection exists, ALL checkboxes show and ALL rows
  // move aside as a block (D4) — measured on a row neither hovered nor checked.
  await expect.poll(() => opacity(checkboxOf(3))).toBe('1');
  await expect.poll(() => leftPad(3)).toBe('34px');
  await page.locator('[data-testid="bar-cancel"]').click();
});

test('selection clears on folder change', async () => {
  await checkboxOf(0).click();
  await expect(bar()).toBeVisible();
  await folder('archive').click();
  await expect(bar()).toHaveCount(0);
  await folder('inbox').click();
  await expect(rows().first()).toBeVisible();
  await expect(bar()).toHaveCount(0);
});

test('a checked pinned row tints like the others (field finding R1-7)', async () => {
  // Pin the first row via the thread bar, then check it in its
  // section: its background must be the selection hue — A73's --tile
  // floor yields to the check (field verdict).
  await rows().nth(0).click();
  await page.locator('[data-testid="pin"]').click();
  const pinned = page.locator('[data-testid="pins"] [data-testid="row"]').first();
  await expect(pinned).toBeVisible();
  await pinned.locator('[data-testid="row-checkbox"]').click();
  await expect(bar()).toContainText('1 sélectionné');
  expect(await pinned.evaluate((el) => getComputedStyle(el).backgroundColor)).toBe(
    await selHue(),
  );
  await page.locator('[data-testid="bar-cancel"]').click();
  // Reset: unpin (the row is still the reading selection, the thread
  // bar is open on it).
  await page.locator('[data-testid="pin"]').click();
  await expect(page.locator('[data-testid="pins"]')).toHaveCount(0);
});

test('the "e" shortcut archives the checked BATCH (field finding R1-8)', async () => {
  const subjects = await rows().locator('.subject').allTextContents();
  const departing = [subjects[2].trim(), subjects[3].trim()];
  await checkboxOf(2).click();
  await checkboxOf(3).click();
  await page.keyboard.press('e');
  await expect(toast()).toContainText('2 conversations archivées');
  await expect(bar()).toHaveCount(0);
  await expect
    .poll(async () => {
      const remaining = (await rows().locator('.subject').allTextContents()).map((s) => s.trim());
      return departing.filter((s) => remaining.includes(s)).length;
    })
    .toBe(0);
});

// D6 (Chief Engineer, 2026-08-27): a batch gesture carries the WHOLE
// THREAD — row 0 of the decor is a 3-message thread (Vantis): it is
// THE case that made the first version of this test fail (the thread
// came back, minus a message) — the net is proven non-decorative by
// this story, do not re-filter it down to "simple" rows.
test('grouped archive: one toast, threads leave WHOLE (D6)', async () => {
  const subjects = await rows().locator('.subject').allTextContents();
  const departing = [subjects[0].trim(), subjects[1].trim()];
  await checkboxOf(0).click();
  await checkboxOf(1).click();
  // PLAN-AUDIT-V2 E6: ONE core call for the batch — instead of N × k
  // one-off commands in series (250 + 50 IPC for 50 conversations).
  await page.evaluate(() => {
    window.__e2eLog = [];
  });
  await page.locator('[data-testid="bar-archive"]').click();
  await expect(toast()).toContainText('2 conversations archivées');
  const gestures = await page.evaluate(() => {
    const commands = window.__e2eLog.map((entry) => entry.command);
    delete window.__e2eLog;
    return commands;
  });
  expect(gestures.filter((c) => c === 'act_on_group')).toHaveLength(1);
  expect(gestures).not.toContain('archive_message');
  expect(gestures).not.toContain('thread_messages');
  await expect(bar()).toHaveCount(0);
  // The two subjects have left the Inbox…
  await expect(rows().first()).toBeVisible();
  await expect
    .poll(async () => {
      const remaining = (await rows().locator('.subject').allTextContents()).map((s) => s.trim());
      return departing.filter((s) => remaining.includes(s)).length;
    })
    .toBe(0);
  // …and end up in Archives.
  await folder('archive').click();
  await expect(rows().first()).toBeVisible();
  await expect
    .poll(async () => {
      const archived = (await rows().locator('.subject').allTextContents()).map((s) => s.trim());
      return departing.filter((s) => archived.includes(s)).length;
    })
    .toBe(2);
  await folder('inbox').click();
  await expect(rows().first()).toBeVisible();
});

test('grouped delete: the rows join the trash', async () => {
  const subjects = await rows().locator('.subject').allTextContents();
  const departing = subjects[0].trim();
  await checkboxOf(0).click();
  await page.locator('[data-testid="bar-delete"]').click();
  await expect(toast()).toContainText('supprimé');
  await expect
    .poll(async () => {
      const remaining = (await rows().locator('.subject').allTextContents()).map((s) => s.trim());
      return remaining.includes(departing);
    })
    .toBe(false);
  await folder('trash').click();
  await expect(rows().first()).toBeVisible();
  await expect
    .poll(async () => {
      const trash = (await rows().locator('.subject').allTextContents()).map((s) => s.trim());
      return trash.includes(departing);
    })
    .toBe(true);
});
