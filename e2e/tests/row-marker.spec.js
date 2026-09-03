// PLAN-REPERE-LIGNE (A80-A82): the mailbox is spelled out in full, on
// the sender's line — "Sender on ▣ Label". The block lives where
// accounts mix (D7), it does NOT require a marker (D8: the word is
// enough), it repeats in the reading pane (D5), and the truncation
// protects the time and the name (D4, ceiling at the measured third).
// Clarity decor: two real accounts, a thread of three messages — no
// marker set, this is the D8 case (the marker itself is covered by
// redesign-feedback-8).
import { test, expect } from '@playwright/test';
import { launchAppV2, closeApp, purgeLocals } from '../launch.mjs';

let app;
let browser;
let page;

test.describe.configure({ mode: 'serial' });

// The WebView2 profile is SHARED between suites: the list width
// touched by the truncation test is purged before AND after.

test.beforeAll(async () => {
  ({ app, browser, page } = await launchAppV2());
  await purgeLocals(page, ['wind-largeurs']);
  await page.reload();
  await expect(page.locator('[data-testid="row"]').first()).toBeVisible();
});

test.afterAll(async () => {
  await purgeLocals(page, ['wind-largeurs']);
  await closeApp({ app, browser });
});

const navMailbox = (label) =>
  page.locator('[data-testid="nav-mailbox"]', { hasText: label });

test('the row spells out the mailbox in full — on every row (A80, D8)', async () => {
  // Unified mailbox (default): every row carries the "on
  // <label>" block; no account has a custom name or a marker on
  // this decor — the label is the address (D8), no marker.
  const blocks = page.locator('[data-testid="row-mailbox"]');
  await expect(blocks.first()).toBeVisible();
  const nRows = await page.locator('[data-testid="row"]').count();
  await expect(blocks).toHaveCount(nRows);
  await expect(blocks.first().locator('.word')).toHaveText('in');
  await expect(blocks.first().locator('.lbl')).toContainText('@');
  // D8: account without a marker — the block is there, the marker isn't.
  await expect(page.locator('[data-testid="row-mailbox"] .bare-marker')).toHaveCount(0);
  // The tooltip gives the full label even when truncated (D4). No
  // account in the decor has a custom name: label AND address are the
  // same string, and it is said only ONCE — "address — address" would
  // be a stutter (review of 2026-08-25). The "name — address" form of
  // the named case is covered by feedback-9-account-name.
  const title = await blocks.first().getAttribute('title');
  expect(title).toMatch(/^[^ ]+@[^ ]+$/);
  expect(title).toBe((await blocks.first().locator('.lbl').innerText()).trim());
});

test('the single-account view says nothing (D7) — list AND reading pane', async () => {
  await navMailbox('paul.merand@atelier-nord.fr').click();
  await expect(page.locator('[data-testid="row"]').first()).toBeVisible();
  await expect(page.locator('[data-testid="row-mailbox"]')).toHaveCount(0);

  // Field verdict of 2026-08-25 (point 12): the PANE follows the list.
  // D5 said "the same scheme in the pane"; the field showed the
  // asymmetry — the list fell silent, the pane still said the mailbox.
  await page.locator('[data-testid="row"]').first().click();
  const pane = page.locator('[data-testid="reading-pane"]');
  await expect(pane.locator('[data-testid="message-expanded"]')).toBeVisible();
  await expect(pane.locator('.mailbox')).toHaveCount(0);

  // Back to the unified mailbox for the rest of the suite.
  await navMailbox('All inboxes').click();
  await expect(page.locator('[data-testid="row"]').first()).toBeVisible();
});

test('search says the mailbox even from a single-account view (D7 exception)', async () => {
  // Search CROSSES accounts: it's the only view where the block shows
  // while the nav is scoped to one account. Without this guard,
  // inverting the condition of `mailboxOf` would leave everything green.
  await navMailbox('paul.merand@atelier-nord.fr').click();
  await expect(page.locator('[data-testid="row"]').first()).toBeVisible();
  await expect(page.locator('[data-testid="row-mailbox"]')).toHaveCount(0);

  await page.locator('[data-testid="search-field"]').fill('Vantis');
  await expect(page.locator('[data-testid="results"]')).toBeVisible();
  const results = page.locator('[data-testid="results"] [data-testid="row"]');
  await expect(results.first()).toBeVisible();
  const nResults = await results.count();
  await expect(
    page.locator('[data-testid="results"] [data-testid="row-mailbox"]'),
  ).toHaveCount(nResults);

  // Back to the starting state: the suite is serial.
  await page.locator('[data-testid="search-field"]').press('Escape');
  await expect(page.locator('[data-testid="results"]')).toHaveCount(0);
  await navMailbox('All inboxes').click();
  await expect(page.locator('[data-testid="row"]').first()).toBeVisible();
});

test('the Drafts folder keeps its tile (D9) and its time on the right edge', async () => {
  // A81 only removes the tile from the LIST row: in the Drafts
  // folder it says the recipient, so the row keeps its leading
  // column (`tiled` class). Nothing covered this before this guard —
  // removing the tile left the whole gate green.
  await page.locator('[data-testid="nav-folder"][data-category="drafts"]').click();
  const row = page.locator('[data-testid="row-draft"]').first();
  await expect(row).toBeVisible();
  await expect(row.locator('.avatar')).toBeVisible();
  await expect(row).toHaveClass(/tiled/);
  // The folder does not mix accounts in the display: no block.
  await expect(row.locator('[data-testid="row-mailbox"]')).toHaveCount(0);

  // And its time holds the RIGHT edge: `.exp` no longer grows since
  // A80, it's the flex that pushes — the draft row must have it like
  // the others, otherwise the time sticks to the recipient.
  const frame = await row.boundingBox();
  const time = await row.locator('.time').boundingBox();
  expect(frame.x + frame.width - (time.x + time.width)).toBeLessThan(24);

  await page.locator('[data-testid="nav-folder"][data-category="inbox"]').click();
  await expect(page.locator('[data-testid="row"]').first()).toBeVisible();
});

test('the reading pane says the mailbox, expanded card and collapsed row (D5)', async () => {
  // The decor's first thread (3 messages: 1 expanded, 2 collapsed).
  await page.locator('[data-testid="row"]').first().click();
  const pane = page.locator('[data-testid="reading-pane"]');
  const expanded = pane.locator('[data-testid="message-expanded"] .mailbox');
  await expect(expanded).toBeVisible();
  await expect(expanded.locator('.word')).toHaveText('in');
  await expect(expanded.locator('.lbl')).toHaveText('paul.merand@atelier-nord.fr');
  // The collapsed rows say it too — behind the name.
  await expect(pane.locator('[data-testid="message-collapsed"] .mailbox')).toHaveCount(2);
});

test('truncation protects the time and the name at the lower bound (D4, 300 px)', async () => {
  // The handle all the way to the left: list at 300 px (lower bound of
  // BOUNDS.list), set by the persisted preference — the real channel.
  await page.evaluate(() => {
    localStorage.setItem('wind-largeurs', JSON.stringify({ nav: 248, list: 300 }));
  });
  await page.reload();
  const first = page.locator('[data-testid="row"]').first();
  await expect(first).toBeVisible();
  // The long label ellipses (CSS only: the full text stays in the
  // DOM, read by assistive technologies).
  const label = page
    .locator('[data-testid="row-mailbox"] .lbl', { hasText: 'atelier-nord' })
    .first();
  await expect(label).toBeVisible();
  expect(
    await label.evaluate((el) => el.scrollWidth > el.clientWidth),
  ).toBe(true);
  // The time never yields: visible, whole, inside the column.
  const time = first.locator('.time');
  await expect(time).toBeVisible();
  const column = await page.locator('[data-testid="list"]').boundingBox();
  const mailbox = await time.boundingBox();
  expect(mailbox.x + mailbox.width).toBeLessThanOrEqual(column.x + column.width + 1);

  // The third ceiling is the figure from the design (§1.3): the block
  // never takes more than a third of its row, whatever the pane's
  // width.
  const l1 = await first.locator('.l1').boundingBox();
  const block = await first.locator('[data-testid="row-mailbox"]').boundingBox();
  expect(block.width).toBeLessThanOrEqual(l1.width / 3 + 1);

  // AND nothing paints OVER the time. This is the guard for the bug
  // found in review: with a min-width:0 on the block, the "in" word and the
  // marker (both flex:none) overflowed a block crushed to 0 px and
  // covered the time. The Sent folder is the worst case in the decor —
  // its column says "To: <address>", much longer than a name.
  await page.locator('[data-testid="nav-folder"][data-category="sent"]').click();
  const send = page.locator('[data-testid="row"]').first();
  await expect(send).toBeVisible();
  const sendBlock = await send.locator('[data-testid="row-mailbox"]').boundingBox();
  const sendTime = await send.locator('.time').boundingBox();
  expect(sendBlock.x + sendBlock.width).toBeLessThanOrEqual(sendTime.x + 1);
  await page.locator('[data-testid="nav-folder"][data-category="inbox"]').click();
  await expect(page.locator('[data-testid="row"]').first()).toBeVisible();
  // Back to default for the following suites.
  await purgeLocals(page, ['wind-largeurs']);
  await page.reload();
  await expect(page.locator('[data-testid="row"]').first()).toBeVisible();
});
