// PLAN-RETOURS-14 R2 (D2/D3): the organized Inbox loses the generic
// banner and the tabs, takes the mode views' normalized header
// (Feed/Screener pattern, .header-view classes), and the current
// section's name stays visible while scrolling (a sticky band).
import { test, expect } from '@playwright/test';
import { launchAppV2, closeApp, injectArrival } from '../launch.mjs';

let app;
let browser;
let page;

test.describe.configure({ mode: 'serial' });

test.beforeAll(async () => {
  ({ app, browser, page } = await launchAppV2({
    accounts: [{ email: 'principal@exemple.fr', messages: 40 }],
  }));
});

test.afterAll(async () => {
  await closeApp({ app, browser });
});

test("the organized Inbox: normalized header, neither generic banner nor tabs", async () => {
  await expect(page.locator('[data-testid="row"]').first()).toBeVisible();
  // In classic: generic banner AND tabs — the starting guard.
  await expect(page.locator('[data-testid="tabs"]')).toBeVisible();
  await expect(page.locator('[data-testid="inbox-title"]')).toHaveCount(0);

  await page.locator('[data-testid="organized-mode"]').click();
  await expect(page.locator('[data-testid="organized-mode"]')).toHaveAttribute('aria-checked', 'true');

  // The header at the mode views' format: glyph + "Inbox" in
  // display, NOT the banner's h1; the footer disappears (D3).
  const title = page.locator('[data-testid="inbox-title"]');
  await expect(title).toBeVisible();
  await expect(title).toContainText('Inbox');
  await expect(title.locator('svg')).toHaveCount(1);
  await expect(page.locator('[data-testid="tabs"]')).toHaveCount(0);

  // The other views keep their shape: the Archive stays at the
  // classic banner with tabs.
  await page.locator('[data-testid="nav-folder"][data-category="archive"]').click();
  await expect(page.locator('[data-testid="tabs"]')).toBeVisible();
  await expect(page.locator('[data-testid="inbox-title"]')).toHaveCount(0);
  await page.locator('[data-testid="nav-folder"][data-category="inbox"]').click();
});

test('the section name stays visible while scrolling, and leaves again at the top', async () => {
  const frame = page.locator('[data-testid="list"] .frame');
  await expect(page.locator('[data-testid="section"]').first()).toBeVisible();
  // At the top of the list: no sticky band — the real band suffices.
  await expect(page.locator('[data-testid="stuck-section"]')).toHaveCount(0);

  // Scroll into the flow: the real band leaves, the sticky one takes
  // over. The sticky container is 0 px tall (outside the windowing
  // geometry): it's the inner label that shows.
  await frame.evaluate((el) => { el.scrollTop = 800; });
  const label = page.locator('[data-testid="stuck-section"] .header-frame');
  await expect(label).toBeVisible();
  await expect(label).toContainText('New for you');

  // And it really sticks: at the top of the frame, down to the
  // geometry.
  const mailboxFrame = await frame.boundingBox();
  const band = await page.locator('[data-testid="stuck-section"] .header-frame').boundingBox();
  expect(band.y - mailboxFrame.y).toBeGreaterThanOrEqual(0);
  expect(band.y - mailboxFrame.y).toBeLessThan(8);

  // Back to the top: the sticky band retracts.
  await frame.evaluate((el) => { el.scrollTop = 0; });
  await expect(page.locator('[data-testid="stuck-section"]')).toHaveCount(0);
});

// RETOURS-14 R7 (D8): the Feed's nav badges (cards never opened — the
// exact semantics are proven core-side, mail-core test
// `the_feed_badge_counts_never_opened_cards`) and the Paper trail's
// (IMAP unread). Here: the display path.
test("the Feed's and Paper trail's nav badges say the work remaining", async () => {
  await page.evaluate(async () => {
    const invoke = window.__TAURI__.core.invoke;
    for (let n = 0; n < 4; n += 1) {
      await invoke('route_sender', {
        address: `expediteur${n}@exemple.fr`, destination: 'feed', rule: null,
      });
    }
    for (let n = 4; n < 10; n += 1) {
      await invoke('route_sender', {
        address: `expediteur${n}@exemple.fr`, destination: 'paper_trail', rule: null,
      });
    }
  });
  await page.reload();
  const badge = (cat) =>
    page.locator(`[data-testid="nav-folder"][data-category="${cat}"] .badge`);
  await expect(badge('feed')).toBeVisible();
  await expect(badge('feed')).toHaveText(/^[1-9]\d*$/);
  await expect(badge('paper_trail')).toBeVisible();
  await expect(badge('paper_trail')).toHaveText(/^[1-9]\d*$/);
  // The Screener keeps its own (pre-existing) — nothing was broken.
  await expect(page.locator('[data-testid="nav-folder"][data-category="screener"]')).toBeVisible();
});

// RETOURS-14 R5 (D6): Settings > Screener — the EXHAUSTIVE list of
// decisions (the Screener page's history shows only the screened-out
// ones), alphabetical, filterable, with "Reinstate".
test('Settings > Screener: all decisions, alphabetical, search and reinstatement', async () => {
  await page.evaluate(async () => {
    await window.__TAURI__.core.invoke('route_sender', {
      address: 'zeta@exemple.fr', destination: 'screened_out', rule: 'spam',
    });
  });
  await page.locator('[data-testid="settings"]').click();
  await page.locator('[data-testid="settings-group"][data-group="screener"]').click();

  const rows = page.locator('[data-testid="screener-decision"]');
  // 4 Feed + 6 Paper trail (previous test) + 1 screened out = 11, ALL
  // destinations combined.
  await expect(rows).toHaveCount(11);
  // Alphabetical, not chronological: expediteur0 first, zeta last.
  await expect(rows.first()).toContainText('expediteur0@exemple.fr');
  await expect(rows.first()).toContainText('The Feed');
  await expect(rows.last()).toContainText('zeta@exemple.fr');
  await expect(rows.last()).toContainText('marked as junk');

  // The search filters, and the "nothing" is said.
  await page.locator('[data-testid="screener-search"]').fill('zeta');
  await expect(rows).toHaveCount(1);
  await page.locator('[data-testid="screener-search"]').fill('introuvable');
  await expect(rows).toHaveCount(0);
  await expect(page.locator('[data-testid="screener-decisions-empty"]')).toBeVisible();

  // R10 (field): "Edit" offers ALL the rules again — a Yes replaces
  // the screened-out verdict, the displayed verdict follows.
  await page.locator('[data-testid="screener-search"]').fill('zeta');
  await page.locator('[data-testid="decision-edit"]').click();
  await expect(page.locator('[data-testid="decision-menu"]')).toBeVisible();
  await page.locator('[data-testid="decision-to-feed"]').click();
  await expect(page.locator('[data-testid="toast"]')).toContainText('go to the Feed');
  await expect(rows.first()).toContainText('The Feed');
  // "Send back to screener" — the former Reinstate: the verdict dies.
  await page.locator('[data-testid="decision-edit"]').click();
  await page.locator('[data-testid="decision-resend"]').click();
  await expect(rows).toHaveCount(0);
  await page.locator('[data-testid="screener-search"]').fill('');
  await expect(rows).toHaveCount(10);
  await page.locator('[data-testid="settings-done"]').click();
});

// RETOURS-14 R6 (D7): the Paper trail grouped by sender — recency at
// the top (the exact order is proven core-side, mail-core test
// `the_paper_trail_groups_by_sender_by_recency`). Here: the view,
// the unfold, the thread opening.
test('the grouped Paper trail: one rank per sender, the thread opens from the group', async () => {
  await page.locator('[data-testid="nav-folder"][data-category="paper_trail"]').click();
  await expect(page.locator('[data-testid="paper-trail-title"]')).toContainText('Paper trail');
  const groups = page.locator('[data-testid="paper-trail-group"]');
  // Six addresses routed to the Paper trail (badges test) but the
  // test set only has 8 senders (4 to 7 actually here), and the
  // group key is the thread's HEAD sender — a mixed thread (the
  // fixture makes one message in five reply to the previous one)
  // gives its head to another sender: 5 ranks, never a flat list.
  await expect(groups).toHaveCount(5);
  await expect(page.locator('[data-testid="row"]')).toHaveCount(0);

  // Unfold: the threads of the group's single sender.
  await groups.first().click();
  const messages = page.locator('[data-testid="paper-trail-message"]');
  await expect(messages.first()).toBeVisible();

  // Open: the reading pane stays the Paper trail's reader.
  await messages.first().click();
  await expect(page.locator('[data-testid="reading-pane"] [data-testid="thread-subject"]')).toBeVisible();

  // Fold: the rows retract.
  await groups.first().click();
  await expect(messages).toHaveCount(0);

  // R9 (field, 2nd pass): the button opens a MENU of the four sorts,
  // each entry with its glyph; the ranks' order follows the choice.
  const sort = page.locator('[data-testid="paper-trail"] [data-testid="sort-section"]');
  await expect(sort).toContainText('Newest');
  const byDate = await groups.evaluateAll((els) => els.map((e) => e.dataset.address));
  await sort.click();
  const sortMenu = page.locator('[data-testid="sort-menu"]');
  await expect(sortMenu).toBeVisible();
  // Four entries, each with its own glyph (sort_*, A104, A112).
  await expect(sortMenu.locator('[role="menuitemradio"]')).toHaveCount(4);
  await expect(sortMenu.locator('svg[data-name^="sort_"]')).toHaveCount(4);
  await sortMenu.locator('[data-testid="sort-date-asc"]').click();
  await expect(sort).toContainText('Oldest');
  await expect
    .poll(async () => groups.evaluateAll((els) => els.map((e) => e.dataset.address)))
    .toEqual([...byDate].reverse());
  // The alphabet is based on the sender's DISPLAYED NAME (what the
  // rank shows), not the address.
  await sort.click();
  await page.locator('[data-testid="sort-alpha-az"]').click();
  await expect(sort).toContainText('A → Z');
  const names = await groups.evaluateAll((els) => els.map((e) => e.querySelector('.sender').textContent));
  expect(names).toEqual([...names].sort((a, b) => a.localeCompare(b, 'fr', { sensitivity: 'base' })));
  await sort.click();
  await page.locator('[data-testid="sort-alpha-za"]').click();
  await expect(sort).toContainText('Z → A');
  await expect
    .poll(async () => groups.evaluateAll((els) => els.map((e) => e.querySelector('.sender').textContent)))
    .toEqual([...names].reverse());
  await sort.click();
  await page.locator('[data-testid="sort-date-desc"]').click();
  await expect(sort).toContainText('Newest');

  // Review: the sender gestures survive the grouped view — the ⋯ of
  // a group routes the WHOLE sender (Move to…, Screen out).
  await groups.first().hover();
  await groups.first().locator('[data-testid="paper-trail-gestures"]').click();
  await expect(page.locator('[data-testid="paper-trail-menu"]')).toBeVisible();
  await expect(page.locator('[data-testid="paper-trail-screen-out"]')).toBeVisible();
  const address = await groups.first().getAttribute('data-address');
  await page.locator('[data-testid="paper-trail-to-inbox"]').click();
  await expect(page.locator('[data-testid="toast"]')).toContainText('Sender moved');
  // The verdict is SET (the core's door) — the rank count, itself,
  // may not move: a mixed thread routed by ANOTHER sender stays in
  // the Paper trail with the same head (golden rule).
  await expect
    .poll(async () => page.evaluate(async (a) => {
      const routings = await window.__TAURI__.core.invoke('routings');
      return routings.find((r) => r.address === a)?.destination;
    }, address))
    .toBe('inbox');
});

// RETOURS-14 R4 (D5): the "mixed thread" — an UNKNOWN sender replies
// in a known sender's thread. The golden rule leaves the whole
// thread in the Inbox (never lose mail); the unknown sender waits at
// the Screener while their message is read — and the thread SAYS SO
// (badge "Awaiting the Screener").
test('mixed thread: the unknown sender who replies in a known thread is flagged, and waits at the Screener', async () => {
  await page.locator('[data-testid="nav-folder"][data-category="inbox"]').click();
  // The test set only has 8 senders and the previous tests have
  // routed them ALL — we reinstate one: expediteur0 becomes a known
  // sender again, NOT routed, its thread lives in the Inbox.
  await page.evaluate(async () => {
    await window.__TAURI__.core.invoke('remove_routing', { address: 'expediteur0@exemple.fr' });
  });
  // The intruder replies in known sender expediteur0's thread (uid
  // 16, a one-message thread) — through THE production path
  // (upsert_envelopes).
  injectArrival({
    email: 'principal@exemple.fr', sender: 'intrus@exemple.fr', n: 1,
    name: 'Un Intrus', subject: 'Je rejoins le fil', // lang:fr
    replyTo: '<seed-INBOX-16@exemple.fr>',
  });
  await page.reload();

  // The mixed thread STAYS in the Inbox, headed by the intruder's message.
  const row = page.locator('[data-testid="row"]', { hasText: 'Je rejoins le fil' }).first(); // lang:fr
  await expect(row).toBeVisible();
  await row.click();

  // The organized Inbox is a scene without a pane: the thread opens
  // at screen 03. The badge says the wait — on the intruder's message.
  await expect(page.locator('[data-testid="conversation"] [data-testid="screener-pending"]').first())
    .toContainText('Awaiting the Screener');
  await page.locator('[data-testid="back-to-mailbox"]').click();

  // And the intruder REALLY waits at the desk.
  await page.locator('[data-testid="nav-folder"][data-category="screener"]').click();
  await expect(page.locator('[data-testid="screener-rank"]', { hasText: 'intrus@exemple.fr' }))
    .toBeVisible();
  await page.locator('[data-testid="nav-folder"][data-category="inbox"]').click();
});

// RETOURS-14 R8 (field 2026-08-31): a YES at the Screener is worth
// trust — the verdict ALSO sets the rule "always show this sender's
// images", revocable at Settings > Display.
// (The exact semantics are proven core-side, mail-core test
// `a_yes_to_the_screener_allows_the_senders_images`.)
test("approving a sender at the Screener allows their images — a visible, revocable rule", async () => {
  // The intruder from the previous test waits at the desk: Yes.
  await page.locator('[data-testid="nav-folder"][data-category="screener"]').click();
  await page.locator('[data-testid="screener-rank"]', { hasText: 'intrus@exemple.fr' })
    .locator('[data-testid="screener-yes"]').click();
  await expect(page.locator('[data-testid="toast"]')).toContainText('can write to you');

  // The image rule is set — Settings > Display shows it, and its
  // existing exit door removes it.
  await page.locator('[data-testid="settings"]').click();
  await page.locator('[data-testid="settings-group"][data-group="affichage"]').click();
  const rule = page.locator('[data-testid="sender-images"]', { hasText: 'intrus@exemple.fr' });
  await expect(rule).toBeVisible();
  await rule.locator('[data-testid="remove-image-sender"]').click();
  await expect(rule).toHaveCount(0);
  await page.locator('[data-testid="settings-done"]').click();
});
