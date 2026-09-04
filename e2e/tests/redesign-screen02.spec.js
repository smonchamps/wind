// Screen 02 of the redesign (PLAN-UI-V2 §P2), played on the Clarity
// decor: real nav, tabs filtered on the core side, reading pane, real
// action. The file is named to run AFTER the v1 journeys (alphabetical
// order): a single asset rebuild per gate.
import { mkdirSync, rmSync, writeFileSync } from 'node:fs';
import { execSync } from 'node:child_process';
import path from 'node:path';
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

const folder = (category) =>
  page.locator(`[data-testid="nav-folder"][data-category="${category}"]`);

test('the nav carries the unread badges of the Clarity decor (A29, W2-D4)', async () => {
  await expect(page.locator('[data-testid="row"]').first()).toBeVisible();
  // Since A29 the nav says ONLY the unread count, in a filled badge —
  // the totals ("4 / 18") left the nav, the status bar says them. We
  // target the badge element — the count alone, never the whole row
  // (V8: the icons are SVGs, no more ligature at all).
  const badge = (category) => folder(category).locator('.badge');
  await expect(badge('inbox')).toHaveText('4');
  await expect(folder('inbox')).not.toContainText('/');
  await expect(badge('sent')).toHaveCount(0);
  await expect(badge('drafts')).toHaveCount(0);
  await expect(badge('junk')).toHaveText('2');
  await expect(badge('archive')).toHaveCount(0);
  await expect(badge('trash')).toHaveCount(0);
  // V4: the nav counter is a bare NUMBER in tabular accent figures —
  // the filled pill is dead (transparent background), and the unread
  // mark of a list row carries its 9 px --brand disk.
  const badgeBg = await badge('inbox').evaluate(
    (el) => getComputedStyle(el).backgroundColor,
  );
  expect(['rgba(0, 0, 0, 0)', 'transparent']).toContain(badgeBg);
  await expect(page.locator('[data-testid="row"].unread .disk').first()).toBeVisible();
  await expect(page.locator('[data-testid="row"]:not(.unread) .disk')).toHaveCount(0);
  // Mailboxes: the aggregate + one row per REAL account; the current
  // mailbox (All, at startup) is the tile — identity alone, no
  // counter (A36, field E3).
  await expect(page.locator('[data-testid="nav-mailbox"]')).toHaveCount(3);
  await expect(page.locator('[data-testid="nav-mailbox"]').first()).toContainText('All inboxes');
  await expect(page.locator('[data-testid="nav-mailbox"]').first()).not.toContainText('unread');
});

// PLAN-RETOURS-10 R4: the nav glyph aligns on the label's baseline
// THEN drops 2 px — optical alignment C, chosen by the Chief Engineer
// on the board (field session of 2026-08-27). The expected gap is
// therefore 2 px EXACTLY; the 0.5 px tolerance distinguishes the three
// candidates on the board (centered ≈ +2.6, pure baseline = 0 — both
// RED here). The baseline is measured with a zero-size inline-block
// probe slipped into the label — its bottom edge IS the baseline (the
// CSS definition of an empty inline-block); getBoundingClientRect sees
// the transform, so the drop is measured.
test('the nav glyphs hold optical alignment C — baseline + 2 px (RETOURS-10)', async () => {
  const gap = (loc) =>
    loc.evaluate((el) => {
      const svg = el.querySelector('svg.ic').getBoundingClientRect();
      const label = el.querySelector('.label, .title-tile');
      const probe = document.createElement('span');
      probe.style.cssText =
        'display:inline-block;width:0;height:0;padding:0;margin:0;border:0';
      label.appendChild(probe);
      const baseline = probe.getBoundingClientRect().bottom;
      probe.remove();
      return svg.bottom - baseline;
    });
  // The three carriers: folder row, mailbox row, tile of the current
  // mailbox ("All mailboxes" at startup).
  expect(Math.abs((await gap(folder('inbox'))) - 2)).toBeLessThanOrEqual(0.5);
  expect(Math.abs((await gap(folder('trash'))) - 2)).toBeLessThanOrEqual(0.5);
  expect(
    Math.abs((await gap(page.locator('[data-testid="nav-mailbox"]').first())) - 2),
  ).toBeLessThanOrEqual(0.5);
  expect(
    Math.abs((await gap(page.locator('[data-testid="nav-mailbox"]').nth(1))) - 2),
  ).toBeLessThanOrEqual(0.5);
});

test('the list pane carries its title banner — the mailbox name, no button (UI v3, E1)', async () => {
  // Chief Engineer verdict of 2026-08-16 (ANNOTATIONS-V3 §3): the
  // banner of the Classic mockup enters, WITHOUT "Mark all read" — the
  // title alone.
  const title = page.locator('[data-testid="list-title"]');
  await expect(title).toHaveText('Inbox');
  await expect(title.locator('button')).toHaveCount(0);
  // PLAN-RETOURS-V3 R2: the top banner in the SAME visual format as
  // the bottom filter banner — same height (52 px), same background
  // (--bg since V3 — --panel is dead), a rule separates it from the
  // list like the bottom rule.
  const template = (loc) =>
    loc.evaluate((el) => {
      const s = getComputedStyle(el);
      return { h: el.offsetHeight, bg: s.backgroundColor };
    });
  const top = await template(title);
  const bottom = await template(page.locator('[data-testid="tabs"]'));
  expect(top.h).toBe(bottom.h);
  expect(top.bg).toBe(bottom.bg);
  // The computed value is rounded to the MACHINE pixel (0.666667px at
  // 150% scale): we assert the rule exists, not its exact size.
  const border = await title.evaluate(
    (el) => parseFloat(getComputedStyle(el).borderBottomWidth),
  );
  expect(border).toBeGreaterThan(0);
  // The banner follows the current mailbox.
  await folder('archive').click();
  await expect(title).toHaveText('Archives');
  // Back to the starting state: the suite is serial.
  await folder('inbox').click();
  await expect(title).toHaveText('Inbox');
  await expect(page.locator('[data-testid="row"]').first()).toBeVisible();
});

// (A81 — PLAN-REPERE-LIGNE: the test "the list row carries the
// initials avatar" died with its object — the tile left the LIST. It
// still lives in the thread (thread assertions further below) and in
// the Drafts folder, where it says the recipient: this second use is
// held by row-marker.spec.js, "the Drafts folder keeps its tile (D9)
// and its time at the right edge".)

test('reloading keeps the served rows — never a skeleton (PLAN-REACTIVITE E1)', async () => {
  // The reload that the cycle and gestures trigger in a burst must
  // NEVER go back through the waiting rows: the transport is HELD
  // (the __e2eHold seam), the reload starts, and the screen must show
  // the SAME rows — zero "…" — until the fresh version arrives.
  // Before E1, `reload()` discarded the pages: this test showed N
  // skeletons, deterministically.
  const rows = page.locator('[data-testid="row"]');
  const before = await rows.count();
  expect(before).toBeGreaterThan(0);
  try {
    await page.evaluate(() => {
      window.__e2eHold = new Promise((release) => {
        window.__e2eRelease = release;
      });
      window.__mesure.reload();
    });
    // The flight is open (transport held), the DOM has re-rendered:
    // the rows hold, no waiting state.
    await expect(page.locator('[data-testid="row-pending"]')).toHaveCount(0);
    await expect(rows).toHaveCount(before);
  } finally {
    // Release NO MATTER WHAT: the suite is serial — a hold that
    // survived the test would freeze every one that follows.
    await page.evaluate(() => {
      window.__e2eRelease?.();
      delete window.__e2eHold;
      delete window.__e2eRelease;
    });
  }
  // The fresh version replaced without flicker.
  await expect(rows.first()).toBeVisible();
  await expect(page.locator('[data-testid="row-pending"]')).toHaveCount(0);
});

test("the status bar dates the last poll — even on failure", async () => {
  // The decor accounts have no server: the STABLE state here is a
  // poll failure, and that is exactly what must say since when we are
  // living off the stock (PLAN-SYNCHRO E1, state 6 mockup). The
  // Clarity decor sets `derniere_synchro` 2 minutes ago — the minute
  // shown can drift with the launch duration, not the shape. (The
  // rest state "All messages are up to date" stays covered by the
  // onboarding spec, without a timestamp: mailbox never polled.)
  await expect(page.locator('[data-testid="progress"]')).toContainText(
    /Sync failed · will retry automatically · last synced \d+ minutes? ago/,
  );
});

test('offline is an alert state — the red disc precedes the text (field 2026-09-04)', async () => {
  // The OS event is enough: the UI listens to window online/offline
  // (P0-bis). Offline, the bar must not merely say it — it must ALERT
  // it, with the same red disc as a failed sync (PLAN-AUDIT-V3
  // STOP 2, Chief-Engineer finding at the E5 field pass).
  await page.evaluate(() => window.dispatchEvent(new Event('offline')));
  await expect(page.locator('[data-testid="progress"]')).toContainText(/Offline/);
  await expect(page.locator('.alert-dot')).toHaveCount(1);
  await page.evaluate(() => window.dispatchEvent(new Event('online')));
  await expect(page.locator('.alert-dot')).toHaveCount(1); // the decor's poll failure keeps its own dot afterwards
});

test('the poll button lives in the bar — "Retry" on failure (E3)', async () => {
  // Same decor: the poll fails, and the button becomes the lever
  // closest to the fault (S-D1, state 6 mockup). The click triggers
  // the REAL light pass — the decor accounts have no server, the
  // failure must stay said after the gesture, and the button must
  // re-arm.
  const button = page.locator('[data-testid="btn-poll"]');
  await expect(button).toBeVisible();
  await expect(button).toBeEnabled();
  await expect(button).toContainText('Try again');
  await button.click();
  await expect(page.locator('[data-testid="progress"]')).toContainText(
    /Sync failed/,
  );
  await expect(button).toBeEnabled();
});

test("during a cycle, the ring replaces the status bar's disk (V2)", async () => {
  // V2 (PLAN-ELEMENTS): the hitofude stroke is DEAD — the disk / ring
  // pair replaces it. The 9 px --brand filled disk says rest; the
  // hollow ring of the same diameter (2 px wall, open top quarter,
  // CSS rotation — no more SMIL nor <mask>, lesson A40 no longer
  // applies) says an action is running. A52 holds and strengthens:
  // the percentage lives in the TEXT, never in the signature. The
  // decor's cycle is short (accounts without a server): we catch the
  // ring within the window.
  const button = page.locator('[data-testid="btn-poll"]');
  await expect(button).toBeEnabled();
  await button.click();
  await expect(
    page.locator('[data-testid="status"] .ring'),
  ).toBeAttached({ timeout: 8000 });
  // The calligraphic signature never comes back (A28/A36/A40 fallen).
  await expect(page.locator('[data-testid="status"] path.boucle')).toHaveCount(0);
});

test('selecting opens the pane, reads the body, and the unread mark falls', async () => {
  await page.locator('[data-testid="row"]').first().click();
  await expect(page.locator('[data-testid="reading-pane"] [data-testid="thread-subject"]')).toHaveText(
    'Relecture du contrat Vantis', // lang:fr
  );
  // The body lives in the sandbox iframe — invariant S1.
  await expect(
    page.frameLocator('[data-testid="reading-pane"] iframe').locator('body'),
  ).toContainText('Bonjour Paul');
  // mark_seen is REAL: the inbox hero count falls.
  await expect(folder('inbox')).toContainText('3');
});

test('a link in the body opens in the system browser — the body does not move', async () => {
  // Field finding 2026-08-15: the click was navigating the sandbox
  // iframe to the site, refused (X-Frame-Options / CSP) — WebView2
  // replaced the body with its "This content was blocked" page.
  // Since then, the click is intercepted (lib/liens.js) and goes to
  // open_link; the `__e2eLinks` seam captures the URL instead of
  // opening a real browser — the whole upstream path (iframe
  // allow-same-origin, interception, scheme filter) is the real path.
  await page.evaluate(() => {
    window.__e2eLinks = [];
  });
  const frame = page.frameLocator('[data-testid="reading-pane"] iframe');
  try {
    await frame.locator('a[href="https://espace.exemple/vantis"]').click();
    await expect
      .poll(() => page.evaluate(() => window.__e2eLinks))
      .toEqual(['https://espace.exemple/vantis']);
  } finally {
    await page.evaluate(() => {
      delete window.__e2eLinks;
    });
  }
  // The body is still there — never a "content blocked" page.
  await expect(frame.locator('body')).toContainText('Bonjour Paul');
});

test('the Unread tab filters on the core side', async () => {
  await page.locator('[data-tab="nonlus"]').click();
  await expect(page.locator('[data-testid="row"]')).toHaveCount(3);
  await page.locator('[data-tab="tous"]').click(); // lang:fr — the tab id is a VALUE the UI still names in French (D16 leftovers, D-55)
  await expect(page.locator('[data-testid="row"]').nth(4)).toBeVisible();
});

test('the canonical folders serve their lists', async () => {
  await folder('archive').click();
  await expect(page.locator('[data-testid="status"]')).toContainText(
    'Archives · 64 items',
  );
  await expect(page.locator('[data-testid="row"]').first()).toBeVisible();
  await folder('trash').click();
  await expect(page.locator('[data-testid="status"]')).toContainText(
    'Trash · 3 items',
  );
  await folder('inbox').click();
  await expect(page.locator('[data-testid="row"]').first()).toBeVisible();
});

test("an account's mailbox bounds the list", async () => {
  await page.locator('[data-testid="nav-mailbox"]').nth(2).click();
  // 3 since RETOURS-11: the decor gains a second Registrar message
  // (the sender rule's fixture).
  await expect(page.locator('[data-testid="row"]')).toHaveCount(3);
  await page.locator('[data-testid="nav-mailbox"]').first().click();
  await expect(page.locator('[data-testid="row"]').nth(4)).toBeVisible();
});

test('archiving acts on the core and confirms via the toast', async () => {
  await page.locator('[data-testid="row"]').nth(1).click();
  await page.locator('[data-testid="archive"]').click();
  await expect(page.locator('[data-testid="toast"]')).toContainText(
    'Conversation archived.',
  );
  // The total left the nav (A29, W2-D4): the core's proof reads at
  // the Archive folder — the status bar counts its elements.
  await folder('archive').click();
  await expect(page.locator('[data-testid="status"]')).toContainText('Archives · 65');
  await folder('inbox').click();
  await expect(page.locator('[data-testid="row"]').first()).toBeVisible();
});

test('the reading pane shows the THREAD as cards — old ones collapsed, last one expanded (UI v3, E3)', async () => {
  // Chief Engineer verdict of 2026-08-16 (ANNOTATIONS-V3 §6, decision
  // D4): the pane and screen 03 are two frames of the SAME object
  // (Thread) — here the pane frame: title, collapsed cards one line
  // each, last one expanded in its own sandbox iframe (S1 intact).
  await page.locator('[data-testid="row"]').first().click();
  const pane = page.locator('[data-testid="reading-pane"]');
  await expect(pane.locator('[data-testid="thread-subject"]')).toHaveText(
    'Relecture du contrat Vantis', // lang:fr
  );
  await expect(pane.locator('[data-testid="message-collapsed"]')).toHaveCount(2);
  await expect(pane.locator('[data-testid="message-expanded"]')).toHaveCount(1);
  await expect(
    pane.frameLocator('[data-testid="message-expanded"] iframe').locator('body'),
  ).toContainText('Bonjour Paul');
  // The last message's attachments, in the pane.
  await expect(pane.locator('[data-testid="message-expanded"]')).toContainText(
    'Contrat_Vantis_v4.pdf',
  );
});

test('the thread matches the mockup exactly — avatars, two-line header, long time (A45/A92)', async () => {
  // Chief Engineer feedback of 2026-08-16 (captures of the Classic
  // prototype's pane, ANNOTATIONS-V3 §6): inventory chips on the
  // left — n messages ALWAYS said, files SUMMED over the thread —,
  // bare buttons on the right, avatar cards, expanded header on two
  // lines "Name <address>" then "To: …" (A92), long time; the
  // From/To/Subject block is gone.
  await page.locator('[data-testid="row"]').first().click();
  const pane = page.locator('[data-testid="reading-pane"]');
  const chips = pane.locator('[data-testid="thread-chips"]');
  // 3 messages, and 3 files = the thread's sum on THIS decor (PDF +
  // Camille's XLSX, post-scan, + Sofia's XLSX) — the row alone
  // carried only one. The sum stabilizes once message_body has served
  // the post-scan count of the last message (2, not 1): asserting 2
  // used to catch the pre-scan value, by race.
  await expect(chips).toContainText('3 messages');
  await expect(chips).toContainText('3 files');
  // The right-hand buttons are BARE (button, no border or background).
  for (const testid of ['see-conversation', 'all-expand']) {
    const button = pane.locator(`[data-testid="${testid}"]`);
    await expect(button).toHaveClass(/bare/);
    expect(await button.evaluate((el) => el.tagName)).toBe('BUTTON');
  }
  // The cards carry the initials avatar, like the list (E2).
  const replies = pane.locator('[data-testid="message-collapsed"]');
  await expect(replies.nth(0).locator('.avatar')).toHaveText('PM');
  await expect(replies.nth(1).locator('.avatar')).toHaveText('SN');
  const expanded = pane.locator('[data-testid="message-expanded"]');
  await expect(expanded.locator('.avatar')).toHaveText('CR');
  // The expanded header (PLAN-RETOURS-12 R5): "Name <address>" then
  // "To: …" — the account name comes from our own copy in the thread
  // (Sent) — and the long time.
  await expect(expanded.locator('.addr-sender')).toHaveText('<c.rousseau@atelier-nord.fr>');
  await expect(expanded.locator('[data-testid="row-to"]')).toHaveText(
    'To: Paul Mérand <paul.merand@atelier-nord.fr>', // lang:fr
  );
  await expect(expanded.locator('.message-head .when')).toHaveText(/^Today, 09:12$/);
  await expect(replies.nth(0).locator('.when')).toHaveText(/, 18:20$/);
  await expect(replies.nth(1).locator('.when')).toHaveText(/, 11:05$/);
  // The From/To/Subject block no longer exists (the header says it all).
  await expect(expanded.locator('dl')).toHaveCount(0);
});

test('a single-message thread says "1 message" — and opens on "Collapse all" (fields A45/A47)', async () => {
  // The Chief Engineer's second capture: a one-message thread keeps
  // the full rank. The "archiving" test moved Planning to Archive —
  // we follow it there.
  await folder('archive').click();
  await page
    .locator('[data-testid="row"]', { hasText: 'Planning de la semaine 33' }) // lang:fr
    .first()
    .click();
  const pane = page.locator('[data-testid="reading-pane"]');
  const chips = pane.locator('[data-testid="thread-chips"]');
  await expect(chips).toContainText('1 message');
  await expect(chips).not.toContainText('file');
  // A47: a lone message opens EXPANDED — the toggle, derived from the
  // state, therefore says "Collapse all" right from opening.
  await expect(pane.locator('[data-testid="all-collapse"]')).toBeVisible();
  const expanded = pane.locator('[data-testid="message-expanded"]');
  await expect(expanded.locator('.avatar')).toHaveText('YB');
  // With no copy of ours in the thread, the recipient is the bare
  // account address — the honest fact, the core does not know our name.
  await expect(expanded.locator('.addr-sender')).toHaveText('<y.belkacem@atelier-nord.fr>');
  await expect(expanded.locator('[data-testid="row-to"]')).toHaveText(
    'To: paul.merand@atelier-nord.fr',
  );
  await expect(expanded.locator('.message-head .when')).toHaveText(/^Today, 08:40$/);
  await folder('inbox').click();
  await page.locator('[data-testid="row"]').first().click();
});

test('the pane is flat, "Open" and "Expand" get their own glyph (field A46)', async () => {
  // Chief Engineer feedback of 2026-08-16: the pane no longer locks
  // itself into an elevation — it scrolls in a single flow, the
  // thread head with no rule (.voletLecture drawing of the
  // prototype); "See the conversation" becomes "Open" (open_in_full —
  // one icon, one meaning, A3); the toggle labels are "Expand
  // all"/"Collapse all" (A47).
  await page.locator('[data-testid="row"]').first().click();
  const pane = page.locator('[data-testid="reading-pane"]');
  const openButton = pane.locator('[data-testid="see-conversation"]');
  await expect(openButton).toContainText('Open');
  await expect(openButton.locator('.ic')).toHaveAttribute('data-name', 'open_in_full');
  const expandButton = pane.locator('[data-testid="all-expand"]');
  await expect(expandButton).toContainText('Expand all');
  await expect(expandButton.locator('.ic')).toHaveAttribute('data-name', 'unfold_more');
  // Flat: the pane itself scrolls, the head carries no rule at all.
  expect(await pane.evaluate((el) => getComputedStyle(el).overflowY)).toBe('auto');
  expect(
    await pane.locator('.head').evaluate((el) => getComputedStyle(el).borderBottomWidth),
  ).toBe('0px');
});

test('the "Expand all"/"Collapse all" toggle FOLLOWS the real expand state (field A47)', async () => {
  const pane = page.locator('[data-testid="reading-pane"]');
  await pane.locator('[data-testid="all-expand"]').click();
  await expect(pane.locator('[data-testid="message-expanded"]')).toHaveCount(3);
  await expect(pane.locator('[data-testid="all-expand"]')).toHaveCount(0);
  const collapseButton = pane.locator('[data-testid="all-collapse"]');
  await expect(collapseButton).toContainText('Collapse all');
  await expect(collapseButton.locator('.ic')).toHaveAttribute('data-name', 'unfold_less');
  // Derived from the state (A47, reverses the "lone gesture" of A46):
  // collapsing a message by HAND falls back to "Expand all"…
  await pane.locator('[data-testid="message-expanded"]').first().locator('.message-head').click();
  await expect(pane.locator('[data-testid="message-collapsed"]')).toHaveCount(1);
  await expect(pane.locator('[data-testid="all-expand"]')).toBeVisible();
  // …and re-expanding it by hand puts it back on "Collapse all".
  await pane.locator('[data-testid="message-collapsed"]').click();
  await expect(collapseButton).toBeVisible();
  // "Collapse all" closes EVERYTHING — the last one included.
  await collapseButton.click();
  await expect(pane.locator('[data-testid="message-expanded"]')).toHaveCount(0);
  await expect(pane.locator('[data-testid="message-collapsed"]')).toHaveCount(3);
  await expect(pane.locator('[data-testid="all-expand"]')).toBeVisible();
  // Put the thread back to its opening state: the last one expanded.
  await pane.locator('[data-testid="message-collapsed"]').last().click();
  await expect(pane.locator('[data-testid="message-expanded"]')).toHaveCount(1);
});

test('the body height follows the content — never a fixed template (field A47)', async () => {
  // Camille's body is short: the iframe hugs its document (the old
  // floor used to fix 220 px), to within a rule's thickness.
  const pane = page.locator('[data-testid="reading-pane"]');
  const body = pane.locator('[data-testid="message-expanded"] iframe');
  await expect(
    pane.frameLocator('[data-testid="message-expanded"] iframe').locator('body'),
  ).toContainText('Bonjour Paul');
  // The NON-circular proof: the content is measured with the iframe
  // at zero height (scrollHeight ≥ set height otherwise), then
  // compared to the set height — they coincide, to within a rule.
  const measure = () =>
    body.evaluate((el) => {
      const placed = el.offsetHeight;
      el.style.height = '0';
      const raw = el.contentDocument.documentElement.scrollHeight;
      el.style.height = `${placed}px`;
      return { placed, raw };
    });
  await expect
    .poll(async () => {
      const { placed, raw } = await measure();
      return placed > 60 && Math.abs(placed - raw) <= 2;
    })
    .toBe(true);
});

test('the compose header no longer repeats the subject, "From" hugs the header (field A46)', async () => {
  // Resuming the Vantis thread's draft: the window opens as before —
  // but the header no longer carries the subject reminder (the
  // Subject field says it below), and the header → "From" gap is that
  // of the prototype's composer (6 px).
  await page.locator('[data-testid="conv-draft"]').click();
  const compose = page.locator('[data-testid="compose"]');
  await expect(compose).toBeVisible();
  await expect(compose.locator('[data-testid="compose-kicker"]')).toBeVisible();
  // The draft's subject only lives in ITS field (input value, outside
  // textContent) — no text reminder anywhere in the window.
  await expect(compose).not.toContainText('Relecture du contrat Vantis'); // lang:fr
  expect(
    await compose
      .locator('[data-testid="compose-from"]')
      .evaluate((el) => getComputedStyle(el.closest('.fields')).paddingTop),
  ).toBe('6px');
  await page.locator('[data-testid="compose-cancel"]').click();
  await expect(compose).toHaveCount(0);
});

// ——— Screen 03: the full-screen conversation (P3) ————————————————————

test('seeing the conversation opens the thread full screen, last message expanded', async () => {
  await page.locator('[data-testid="row"]').first().click();
  await page.locator('[data-testid="see-conversation"]').click();
  await expect(page.locator('[data-testid="conversation"] [data-testid="thread-subject"]')).toHaveText(
    'Relecture du contrat Vantis', // lang:fr
  );
  // Frame exclusivity (D4, v3 review): a SINGLE Thread mounted.
  await expect(page.locator('[data-testid="thread-subject"]')).toHaveCount(1);
  await expect(page.locator('[data-testid="message-collapsed"]')).toHaveCount(2);
  await expect(page.locator('[data-testid="message-expanded"]')).toHaveCount(1);
  // The expanded body lives in ITS OWN sandbox iframe (S1).
  await expect(
    page.frameLocator('[data-testid="message-expanded"] iframe').locator('body'),
  ).toContainText('Bonjour Paul');
  // The message's real attachments.
  await expect(page.locator('[data-testid="message-expanded"]')).toContainText(
    'Contrat_Vantis_v4.pdf',
  );
});

test("expand all expands the thread, a message's header collapses it", async () => {
  await page.locator('[data-testid="all-expand"]').click();
  await expect(page.locator('[data-testid="message-expanded"]')).toHaveCount(3);
  await page.locator('[data-testid="message-expanded"]').first().locator('.message-head').click();
  await expect(page.locator('[data-testid="message-collapsed"]')).toHaveCount(1);
});

test('going back leaves the mailbox intact, selection included', async () => {
  await page.locator('[data-testid="back-to-mailbox"]').click();
  await expect(page.locator('[data-testid="conversation"]')).toHaveCount(0);
  await expect(page.locator('[data-testid="row"]').first()).toBeVisible();
  await expect(page.locator('[data-testid="thread-subject"]')).toHaveText(
    'Relecture du contrat Vantis', // lang:fr
  );
});

// ——— Screen 04 + Settings: composing and themes (P4) ————————————————

test('writing opens the composer; cancelling an empty one leaves nothing', async () => {
  await page.locator('[data-testid="write"]').click();
  await expect(page.locator('[data-testid="compose-kicker"]')).toHaveText(
    'New message',
  );
  // The sending account IS CHOSEN (A10): two accounts on the decor,
  // the first by default, the other selectable.
  const from = page.locator('[data-testid="compose-from"]');
  await expect(from).toHaveValue('paul.merand@atelier-nord.fr');
  await expect(from.locator('option')).toHaveCount(2);
  await from.selectOption('paul@merand.fr');
  await expect(from).toHaveValue('paul@merand.fr');
  await page.locator('[data-testid="compose-cancel"]').click();
  await expect(page.locator('[data-testid="compose"]')).toHaveCount(0);
  await expect(page.locator('[data-testid="toast"]')).toHaveCount(0);
});

test('"Reply all" sits between Reply and Forward, per message (A14, R4/D4)', async () => {
  // R4 (PLAN-RETOURS-3, D4): reply gestures live at the BOTTOM of each
  // message — A14 still holds, "Reply all" between Reply and Forward;
  // "Delete" joined them, PER message (PLAN-INVITATIONS, field R8' of
  // 2026-08-23). The THREAD bar only keeps SORT (D5) and "Report as
  // spam" (R2/D2). No click: offline guaranteed.
  const messageBar = await page
    .locator('[data-testid="reading-pane"] [data-testid="actions-message"]')
    .last()
    .locator('button')
    .evaluateAll((buttons) => buttons.map((button) => button.dataset.testid));
  expect(messageBar).toEqual(['reply', 'reply-all', 'forward', 'delete']);
  const threadBar = await page
    .locator('[data-testid="reading-pane"] .actions button')
    .evaluateAll((buttons) => buttons.map((button) => button.dataset.testid));
  // "Pin" joined the thread bar in the Inbox (RETOURS-7, D3).
  expect(threadBar).toEqual(['archive', 'report-spam', 'pin']);

  await page.locator('[data-testid="see-conversation"]').click();
  const messageBarConv = await page
    .locator('[data-testid="conversation"] [data-testid="actions-message"]')
    .last()
    .locator('button')
    .evaluateAll((buttons) => buttons.map((button) => button.dataset.testid));
  expect(messageBarConv).toEqual(['reply', 'reply-all', 'forward', 'delete']);
  const threadBarConv = await page
    .locator('[data-testid="conversation"] .actions button')
    .evaluateAll((buttons) => buttons.map((button) => button.dataset.testid));
  expect(threadBarConv).toEqual(['archive', 'report-spam', 'pin']);

  await page.locator('[data-testid="back-to-mailbox"]').click();
  await expect(page.locator('[data-testid="thread-subject"]')).toHaveText(
    'Relecture du contrat Vantis', // lang:fr
  );
});

test('replying prefills from the core: address, Re:, lead-in, quote — without the original attachments', async () => {
  // R4: reply is PER message; the last expanded message of the Vantis
  // thread is Camille Rousseau's (`.last()`).
  await page.locator('[data-testid="reply"]').last().click();
  await expect(page.locator('[data-testid="compose-kicker"]')).toHaveText('Reply');
  await expect(page.locator('[data-testid="compose-to"]')).toHaveValue(
    'c.rousseau@atelier-nord.fr',
  );
  await expect(page.locator('[data-testid="compose-subject"]')).toHaveValue(
    'Re: Relecture du contrat Vantis', // lang:fr
  );
  const body = await page.locator('[data-testid="compose-body"]').innerText();
  // The GAP lead-in → quote is part of the contract (one blank line,
  // not four): the assertion measures both line breaks, not just the
  // lead-in.
  expect(body.startsWith('Hello Camille,\n\n')).toBe(true);
  expect(body).toContain('a écrit :'); // lang:fr
  // E3 (PJ-D4): a reply does NOT carry the original attachments — the
  // prototype's chip promised a send that never existed, it fell with
  // the fiction.
  await expect(page.locator('[data-testid="compose"]')).not.toContainText(
    'Contrat_Vantis_v4.pdf',
  );
  await expect(page.locator('[data-testid="compose-attachments"]')).toHaveCount(0);
});

test('saving the draft keeps it and confirms', async () => {
  await page.locator('[data-testid="compose-draft"]').click();
  await expect(page.locator('[data-testid="compose"]')).toHaveCount(0);
  await expect(page.locator('[data-testid="toast"]')).toContainText(
    'Draft saved.',
  );
});

test('sending logs into the outbox and confirms', async () => {
  // R4: reply is PER message; the last expanded message of the Vantis
  // thread is Camille Rousseau's (`.last()`).
  await page.locator('[data-testid="reply"]').last().click();
  await expect(page.locator('[data-testid="compose-to"]')).toHaveValue(
    'c.rousseau@atelier-nord.fr',
  );
  await page.locator('[data-testid="compose-send"]').click();
  await expect(page.locator('[data-testid="compose"]')).toHaveCount(0);
  await expect(page.locator('[data-testid="toast"]')).toContainText('Message sent.');
});

// ——— P5: search, image guard, feedback slot, progress ———————

test('search serves its results in the prototype rows (D1)', async () => {
  await page.locator('[data-testid="search-field"]').fill('Vantis');
  await expect(page.locator('[data-testid="results"]')).toBeVisible();
  // Search spans mailboxes: the Vantis thread comes out as several
  // messages (inbox, sent…) — we require its presence, not its rank.
  await expect(
    page.locator('[data-testid="results"] [data-testid="row"]',
      { hasText: 'Relecture du contrat Vantis' }).first(), // lang:fr
  ).toBeVisible();
  await expect(page.locator('[data-testid="progress"]')).toContainText('Search ·');
  // Escape in the field: the mailbox comes back as it was.
  await page.locator('[data-testid="search-field"]').press('Escape');
  await expect(page.locator('[data-testid="results"]')).toHaveCount(0);
  await expect(page.locator('[data-testid="row"]').first()).toBeVisible();
});

test('the preview decodes HTML entities — never an &eacute; residue', async () => {
  // The decor's body carries &eacute; and &nbsp;: the visible text
  // must be the prototype's, without a single ampersand entity.
  const row = page.locator('[data-testid="row"]', { hasText: 'renouvellement du domaine' }); // lang:fr
  await expect(row).toContainText('pour éviter toute interruption de service.'); // lang:fr
  await expect(row).not.toContainText('&');
});

test('attachments are taken from the PANE — a lone message has no conversation (Annex A)', async () => {
  // "Compte rendu du 4 août": SOLE message, one attachment. // lang:fr
  await page.locator('[data-testid="row"]', { hasText: 'Compte rendu du 4 août' }).click(); // lang:fr
  // R2 (PLAN-RETOURS-4, D4): name AND weight in the SAME clickable
  // chip — a single chip per attachment, carrying both pieces of
  // information.
  const chip = page.locator('[data-testid="reading-files"] [data-testid="attachment"]');
  await expect(chip).toHaveCount(1);
  await expect(chip).toContainText('CR_04-08.pdf');
  await expect(chip).toContainText('220 Ko');
  await expect(chip).toBeEnabled();
});

test('the cross clears the search in one click (field verdict)', async () => {
  await page.locator('[data-testid="search-field"]').fill('Vantis');
  await expect(page.locator('[data-testid="results"]')).toBeVisible();
  await page.locator('[data-testid="clear-search"]').click();
  await expect(page.locator('[data-testid="search-field"]')).toHaveValue('');
  await expect(page.locator('[data-testid="results"]')).toHaveCount(0);
});

test('R3: the body stays on a light slate even under a dark theme (PLAN-RETOURS-4, D3)', async () => {
  // A42's dark slate made sender-colored text illegible (field
  // finding 2026-08-18). The body now ALWAYS bakes a light slate
  // (mail-render Palette::default — white background, dark ink),
  // whatever the theme: the front no longer sends a palette. We force
  // a -nuit theme BEFORE opening the message (opening the thread
  // clears the body cache → a fresh fetch under this theme); the old
  // code would have baked a dark background here — reintroducing a
  // theme palette would break this test.
  // It runs BEFORE the image-memory tests (RETOURS-11): it waits for
  // the visible guard, so a message nothing is yet written to the
  // database for.
  await page.evaluate(() => { document.documentElement.dataset.theme = 'elements-nuit'; });
  await page.locator('[data-testid="row"]', { hasText: 'renouvellement du domaine' }).click(); // lang:fr
  await expect(page.locator('[data-testid="images-guard"]')).toBeVisible();
  const srcdoc = await page.locator('iframe.body').first().getAttribute('srcdoc');
  expect(srcdoc).toContain('background:#ffffff');
  expect(srcdoc).toContain('color:#222222');
  expect(srcdoc).not.toContain('color-scheme:dark');
  await page.evaluate(() => { delete document.documentElement.dataset.theme; });
});

test('remote images stay blocked; "Show images" SURVIVES the selection (RETOURS-11, D1-D2)', async () => {
  await page.locator('[data-testid="row"]', { hasText: 'renouvellement du domaine' }).click(); // lang:fr
  await expect(page.locator('[data-testid="images-guard"]')).toContainText(
    '1 remote image blocked',
  );
  await page.locator('[data-testid="show-images"]').click();
  await expect(page.locator('[data-testid="images-guard"]')).toHaveCount(0);
  // Coming back to the message: the guard does NOT return — the
  // choice is written to the database PER message (D1 reverses
  // invariant A43; D2: envelope key). The anchor is the srcdoc: the
  // REAL image URL is only found there if the render granted images
  // (blocked means the neutral pixel) — never a count at 0 read
  // before the paint.
  await page.locator('[data-testid="row"]').first().click();
  await page.locator('[data-testid="row"]', { hasText: 'renouvellement du domaine' }).click(); // lang:fr
  await expect(page.locator('iframe.body').first()).toHaveAttribute(
    'srcdoc',
    /registrar\.exemple\/logo\.png/,
  );
  await expect(page.locator('[data-testid="images-guard"]')).toHaveCount(0);
});

test('"Always show": the sender rule is set from the banner and revoked in Settings (RETOURS-11, D3-D4)', async () => {
  // The OTHER Registrar message: the PER-MESSAGE choice from the
  // previous test does not bleed onto it (D2) — its guard is there.
  // Via the personal account's mailbox: 21 days old, this message
  // sorts at the bottom of the Inbox, where the windowed list does
  // not necessarily materialize its row (depending on window height)
  // — the 3-row mailbox, though, always shows it.
  await page.locator('[data-testid="nav-mailbox"]').nth(2).click();
  await page.locator('[data-testid="row"]', { hasText: 'domaine renouvelé' }).click(); // lang:fr
  await expect(page.locator('[data-testid="images-guard"]')).toBeVisible();
  await page.locator('[data-testid="always-show-images"]').click();
  await expect(page.locator('iframe.body').first()).toHaveAttribute(
    'srcdoc',
    /registrar\.exemple\/logo\.png/,
  );
  await expect(page.locator('[data-testid="images-guard"]')).toHaveCount(0);
  // Settings > Display: the rule is LISTED, and can be removed (D4).
  await page.locator('[data-testid="settings"]').click();
  await page.locator('[data-testid="settings-group"][data-group="affichage"]').click();
  const rule = page.locator('[data-testid="sender-images"]', {
    hasText: 'no-reply@registrar.fr',
  });
  await expect(rule).toBeVisible();
  await rule.locator('[data-testid="remove-image-sender"]').click();
  await expect(page.locator('[data-testid="sender-images"]')).toHaveCount(0);
  await page.locator('[data-testid="settings-done"]').click();
  // Revoked, the guard COMES BACK on this message: proof that the
  // grant came from the sender rule ALONE ("Always" does not write a
  // per-message choice) — the net is non-vacuous by construction.
  await page.locator('[data-testid="row"]').first().click();
  await page.locator('[data-testid="row"]', { hasText: 'domaine renouvelé' }).click(); // lang:fr
  await expect(page.locator('[data-testid="images-guard"]')).toBeVisible();
  // Give the Inbox back to the following tests.
  await page.locator('[data-testid="nav-mailbox"]').first().click();
  await expect(page.locator('[data-testid="row"]').nth(4)).toBeVisible();
});

test('the draft lives in the list: a mention on the thread, resumed in the folder, mute feedback slot', async () => {
  // PLAN-BROUILLONS: the slot no longer carries drafts — the Inbox
  // mention (variant B) and the Drafts folder do. The P4 journey's
  // draft replies to the Vantis thread: it is the thread's most
  // recent item, its body takes the preview.
  await expect(page.locator('[data-testid="slot-notice"]')).toHaveCount(0);
  const thread = page
    .locator('[data-testid="row"]', { hasText: 'Relecture du contrat Vantis' }) // lang:fr
    .first();
  await expect(thread.locator('[data-testid="mention-draft"]')).toHaveText('Draft: ');
  await expect(thread).toContainText('Hello Camille,');

  // The folder: the LOCAL drafts (2 from the decor + the P4 one),
  // most recent to oldest; the status bar counts like the other
  // categories; the click RESUMES — never a read.
  await folder('drafts').click();
  await expect(page.locator('[data-testid="folder-drafts"]')).toBeVisible();
  await expect(page.locator('[data-testid="row-draft"]')).toHaveCount(3);
  await expect(page.locator('[data-testid="progress"]')).toContainText(
    'Drafts · 3 items',
  );
  await page.locator('[data-testid="row-draft"]').first().click();
  await expect(page.locator('[data-testid="compose-subject"]')).toHaveValue(
    'Re: Relecture du contrat Vantis', // lang:fr — the English `Re:` of the app before the French fixture subject
  );
  await expect(page.locator('[data-testid="compose-to"]')).toHaveValue(
    'c.rousseau@atelier-nord.fr',
  );

  // Emptying then closing: the only case where closing deletes — the
  // row leaves the folder WITHOUT waiting for the probe (onbrouillon).
  await page.locator('[data-testid="compose-to"]').fill('');
  await page.locator('[data-testid="compose-subject"]').fill('');
  // `fill('')` on a contenteditable is a Chromium no-op: we empty it
  // like the user — select all, delete.
  await page.locator('[data-testid="compose-body"]').click();
  await page.keyboard.press('Control+a');
  await page.keyboard.press('Delete');
  await page.locator('[data-testid="compose-cancel"]').click();
  await expect(page.locator('[data-testid="compose"]')).toHaveCount(0);
  await expect(page.locator('[data-testid="row-draft"]')).toHaveCount(2);

  // Back in the Inbox: the Vantis thread keeps its mention — the
  // DECOR's draft targets it too, and it is its body that takes the
  // preview.
  await folder('inbox').click();
  const still = page
    .locator('[data-testid="row"]', { hasText: 'Relecture du contrat Vantis' }) // lang:fr
    .first();
  await expect(still.locator('[data-testid="mention-draft"]')).toBeVisible();
  await expect(still).toContainText('Merci pour la v4'); // lang:fr
});

test('the conversation carries the draft in last position, the click resumes it (E3)', async () => {
  // The list promised a "last email": screen 03 holds it (B-D4-b) —
  // dotted block at the end of the thread, draft body, click =
  // resume, the conversation stays mounted under the composer.
  await page
    .locator('[data-testid="row"]', { hasText: 'Relecture du contrat Vantis' }) // lang:fr
    .first()
    .click();
  await page.locator('[data-testid="see-conversation"]').click();
  const block = page.locator('[data-testid="conv-draft"]');
  await expect(block).toContainText('Draft');
  await expect(block).toContainText('Merci pour la v4'); // lang:fr
  await expect(block).toContainText('Resume');
  await block.click();
  await expect(page.locator('[data-testid="compose-subject"]')).toHaveValue(
    'Re : Relecture du contrat Vantis', // lang:fr — the SEEDED draft's subject (seed_clarity.rs), fixture
  );
  await expect(page.locator('[data-testid="compose-body"]')).toContainText('Merci pour la v4'); // lang:fr
  // Closing keeps it: the block remains, the conversation has not moved.
  await page.locator('[data-testid="compose-cancel"]').click();
  await expect(page.locator('[data-testid="compose"]')).toHaveCount(0);
  await expect(page.locator('[data-testid="conversation"]')).toBeVisible();
  await expect(block).toBeVisible();
  // Back to the mailbox: the serial chain resumes from the Inbox.
  await page.locator('[data-testid="back-to-mailbox"]').click();
  await expect(page.locator('[data-testid="conversation"]')).toHaveCount(0);
});

test("the progress line carries the outbox's non-faulty wait", async () => {
  // The P4 journey's send is still pending (account offline by
  // construction): a NON-faulty wait — the line, not the slot.
  await expect(page.locator('[data-testid="progress"]')).toContainText(
    "Outbox · 1 message waiting",
  );
});

test('the Feedback button opens the form, and the reply goes through the outbox (RETOURS-11, beta field)', async () => {
  // AFTER the "1 send pending" test: the reply sent here makes a
  // second one — the order is the file's idiom (carried state, serial).
  await page.locator('[data-testid="feedback"]').click();
  const card = page.locator('[data-testid="back-card"]');
  await expect(card).toBeVisible();
  // "Send" ABSENT as long as the field is empty — never greyed out
  // (the house rule from the onboarding journey, D4/RETOURS-8).
  await expect(card.locator('[data-testid="back-send"]')).toHaveCount(0);
  await card
    .locator('[data-testid="back-text"]')
    .fill('La liste défile mal sur mon poste.'); // lang:fr
  await card.locator('[data-testid="back-send"]').click();
  await expect(page.locator('[data-testid="back-card"]')).toHaveCount(0);
  await expect(page.locator('[data-testid="toast"]')).toContainText('Thank you');
  // The decor accounts have no server: the reply stays LOGGED in the
  // outbox (queue_send, the golden rule "never a lost send") — the
  // progress line moves to TWO sends.
  await expect(page.locator('[data-testid="progress"]')).toContainText(
    "Outbox · 2 messages waiting",
  );
});

test('the shortcuts serve the keyboard (D3)', async () => {
  // c: write; Escape first leaves the field (the letters become
  // letters again there), the second one closes — empty, nothing is kept.
  await page.keyboard.press('c');
  await expect(page.locator('[data-testid="compose-kicker"]')).toHaveText(
    'New message',
  );
  await page.keyboard.press('Escape');
  await page.keyboard.press('Escape');
  await expect(page.locator('[data-testid="compose"]')).toHaveCount(0);
  // e: archive the selection.
  await page.locator('[data-testid="row"]').first().click();
  await page.keyboard.press('e');
  await expect(page.locator('[data-testid="toast"]')).toContainText(
    'Conversation archived.',
  );
});

test('the keyboard activates what the click activates (A8): nav, row, tab', async () => {
  // A nav row is not a <button> (the prototype's geometry): it must
  // still respond to Enter.
  await page.locator('[data-testid="nav-folder"][data-category="archive"]').focus();
  await page.keyboard.press('Enter');
  await expect(page.locator('[data-testid="status"]')).toContainText('Archives ·');
  // A list row, on Space.
  await page.locator('[data-testid="row"]').first().focus();
  await page.keyboard.press(' ');
  await expect(page.locator('[data-testid="reading-pane"] [data-testid="thread-subject"]')).not.toBeEmpty();
  // Back to the inbox via the keyboard.
  await page.locator('[data-testid="nav-folder"][data-category="inbox"]').focus();
  await page.keyboard.press('Enter');
  await expect(page.locator('[data-testid="row"]').first()).toBeVisible();
});

test('settings apply and persist the theme', async () => {
  await page.locator('[data-testid="settings"]').click();
  // A13: themes live in their group, chosen on the rail.
  await page.locator('[data-testid="settings-group"][data-group="themes"]').click();
  // V7 amended (A94): four cards — Elements, Elements · nuit,
  // Innamoramento, Innamoramento · nuit ("Mona" renamed, A95).
  await expect(page.locator('[data-testid="theme"]')).toHaveCount(4);
  // Innamoramento applies and displays (A94) — the new card is not
  // decorative, it sets the attribute like the two original ones.
  await page.locator('[data-theme-id="innamoramento"]').click();
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'innamoramento');
  await page.locator('[data-theme-id="elements-nuit"]').click();
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'elements-nuit');
  await page.locator('[data-testid="settings-done"]').click();
  await expect(page.locator('[data-testid="settings-modal"]')).toHaveCount(0);
  // Persistence: the choice survives in localStorage (reloaded at mount).
  expect(await page.evaluate(() => localStorage.getItem('wind-theme'))).toBe('elements-nuit');
  // The check mark follows the choice on reopening; back to `elements`
  // to avoid tinting other journeys.
  await page.locator('[data-testid="settings"]').click();
  await page.locator('[data-testid="settings-group"][data-group="themes"]').click();
  await expect(page.locator('[data-theme-id="elements-nuit"] .check')).toBeVisible();
  await page.locator('[data-theme-id="elements"]').click();
  await page.locator('[data-testid="settings-done"]').click();
});

test('the two-pane settings navigate by click AND by keyboard (A13)', async () => {
  await page.locator('[data-testid="settings"]').click();
  // The rail carries the seven groups (Signature entered with
  // RETOURS-6); Accounts is the opening group.
  // RETOURS-13 field C4: the Screener group is there regardless of
  // mode — 8 groups.
  await expect(page.locator('[data-testid="settings-group"]')).toHaveCount(8);
  await expect(page.locator('[data-testid="settings-accounts"]')).toBeVisible();
  // By click: Shortcuts — the D3 table as reference, read-only.
  await page.locator('[data-testid="settings-group"][data-group="raccourcis"]').click();
  await expect(page.locator('[data-testid="settings-shortcuts"]')).toContainText('Del');
  await expect(page.locator('[data-testid="settings-shortcuts"] kbd')).toHaveCount(7);
  // By keyboard (A8): Enter activates the group like the click.
  await page.locator('[data-testid="settings-group"][data-group="apropos"]').focus();
  await page.keyboard.press('Enter');
  // About: the application's REAL version, not a hardcoded text.
  await expect(page.locator('[data-testid="about-version"]')).toHaveText(/^\d+\.\d+\.\d+/);
  await expect(page.locator('[data-testid="settings-about"]')).toContainText('Apache 2.0');
  // R2 (PLAN-RETOURS-11, D5): the origin mention — the EU flag
  // (dedicated SVG, fixed colors) and "Made in EU" as is.
  await expect(page.locator('[data-testid="about-origin"]')).toContainText('Made in EU');
  await expect(
    page.locator('[data-testid="about-origin"] svg[data-name="drapeau-ue"]'),
  ).toBeVisible();
  // "Check for updates" goes through update_check for real; in E2E
  // the command replies "up to date" (no network, handover §7.5).
  await page.locator('[data-testid="about-check"]').click();
  await expect(page.locator('[data-testid="settings-about"]')).toContainText(
    'You are up to date.',
  );
  await page.locator('[data-testid="settings-done"]').click();
  await expect(page.locator('[data-testid="settings-modal"]')).toHaveCount(0);
});

test('the Accounts section lists the real accounts and opens the add desk (A11)', async () => {
  await page.locator('[data-testid="settings"]').click();
  const section = page.locator('[data-testid="settings-accounts"]');
  await expect(section).toContainText('paul.merand@atelier-nord.fr');
  await expect(section).toContainText('paul@merand.fr');
  // "Add an account" expands THE desk from screen 01 — same
  // implementation: address, domain-based routing, generic fields.
  await page.locator('[data-testid="settings-add"]').click();
  await page.locator('[data-testid="onboarding-address"]').fill('paul@exemple.fr');
  await page.locator('[data-testid="desk-continue"]').click();
  await expect(page.locator('#ob-imap')).toHaveValue('imap.exemple.fr');
  // Nothing sent; Done closes it, the desk unmounts cleanly.
  await page.locator('[data-testid="settings-done"]').click();
  await expect(page.locator('[data-testid="settings-modal"]')).toHaveCount(0);
});

// ——— E2 of Settings: the decision groups (R-D1, R-D2) —————————————

test('Themes: following the dark OS suffixes the chosen theme with -nuit (D6, A42/V7, R1 RETOURS-13)', async () => {
  // R1 (PLAN-RETOURS-13): the toggle now lives at the TOP of the
  // Themes section — it governs the theme, not the display.
  await page.locator('[data-testid="settings"]').click();
  await page.locator('[data-testid="settings-group"][data-group="themes"]').click();
  const toggle = page.locator('[data-testid="display-auto"]');
  await expect(toggle).toHaveAttribute('aria-checked', 'false');
  await toggle.click();
  // Dark OS: the night variant of the chosen theme (elements) is
  // shown; the persisted choice stays the BASE theme — the suffix is
  // a derived state, never saved (A42, mechanics kept by V7/A94
  // across the whole table).
  await page.emulateMedia({ colorScheme: 'dark' });
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'elements-nuit');
  expect(await page.evaluate(() => localStorage.getItem('wind-theme'))).not.toBe('elements-nuit');
  // A -nuit theme chosen by hand stays at peace: already dark…
  await page.locator('[data-testid="settings-group"][data-group="themes"]').click();
  await page.locator('[data-theme-id="elements-nuit"]').click();
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'elements-nuit');
  expect(await page.evaluate(() => localStorage.getItem('wind-theme'))).toBe('elements-nuit');
  // …including when the OS switches back to light — the explicit
  // choice wins (A42 review: this direction was not yet asserted).
  await page.emulateMedia({ colorScheme: 'light' });
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'elements-nuit');
  await page.emulateMedia({ colorScheme: 'dark' });
  // The check mark follows the DISPLAYED card (A42 review): elements
  // chosen under a dark OS shows as elements-nuit — so does the check
  // mark, otherwise a "correction" click on the -nuit card would lock
  // into permanent dark mode.
  await page.locator('[data-theme-id="elements"]').click();
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'elements-nuit');
  await expect(page.locator('[data-theme-id="elements-nuit"] .check')).toBeVisible();
  // Light OS: the choice comes back as is — the attribute DROPS
  // (elements), and the check mark returns to the light card. Full
  // assertion: not "something other than elements-nuit", the absence
  // of the attribute (A42 review).
  await page.emulateMedia({ colorScheme: 'light' });
  await expect(page.locator('html')).not.toHaveAttribute('data-theme');
  await expect(page.locator('[data-theme-id="elements"] .check')).toBeVisible();
  // Persistence: the boolean survives like the theme.
  expect(await page.evaluate(() => localStorage.getItem('wind-theme-auto'))).toBe('1');
  // The rail stayed on Themes since the explicit choice — the toggle
  // is there, at the top of the section (R1 RETOURS-13).
  await toggle.click();
  await page.emulateMedia({ colorScheme: null });
  await page.locator('[data-testid="settings-done"]').click();
});

test('following the OS reads the Tauri API: a real Windows toggle suffixes it and reverts (field A42)', async () => {
  // Field finding of 2026-08-16: prefers-color-scheme is DEAD in
  // Tauri's WebView2 (never dark, zero events) — the D6 test above,
  // played via emulateMedia, only exercises the fallback. Here the
  // toggle is REAL: registry + WM_SETTINGCHANGE broadcast
  // (dark-toggle.ps1, the Windows Settings gesture), and it is the
  // Tauri theme()/onThemeChanged channel that must reflect it.
  test.skip(process.platform !== 'win32', 'bascule AppsUseLightTheme — Windows seulement');
  const key = String.raw`HKCU:\SOFTWARE\Microsoft\Windows\CurrentVersion\Themes\Personalize`;
  const initial = Number(execSync(
    `powershell -NoProfile -c "(Get-ItemProperty '${key}' -Name AppsUseLightTheme).AppsUseLightTheme"`,
  ).toString().trim());
  const script = path.resolve(import.meta.dirname, '..', 'dark-toggle.ps1');
  const toggleTheme = (v) => execSync(
    `powershell -NoProfile -ExecutionPolicy Bypass -File "${script}" -v ${v}`,
  );
  // Witness file for globalTeardown (PLAN-AUDIT-V2 E9, D7): if the
  // runner dies between the toggle and the finally, the machine gets
  // its theme back.
  const witness = path.resolve(import.meta.dirname, '..', 'test-results', 'theme-initial.txt');
  mkdirSync(path.dirname(witness), { recursive: true });
  writeFileSync(witness, String(initial));
  try {
    await page.locator('[data-testid="settings"]').click();
    await page.locator('[data-testid="settings-group"][data-group="themes"]').click();
    const toggle = page.locator('[data-testid="display-auto"]');
    await toggle.click();
    await expect(toggle).toHaveAttribute('aria-checked', 'true');
    // Light OS first (the reference state), then dark: the night
    // variant of the chosen theme (elements) must set itself WITHOUT
    // emulateMedia — delivering the Tauri event takes ~1 s.
    toggleTheme(1);
    await expect(page.locator('html')).not.toHaveAttribute('data-theme', /nuit/, { timeout: 10_000 });
    toggleTheme(0);
    await expect(page.locator('html')).toHaveAttribute('data-theme', 'elements-nuit', { timeout: 10_000 });
    // And the RETURN — the exact direction of the field finding (KO point 4).
    toggleTheme(1);
    await expect(page.locator('html')).not.toHaveAttribute('data-theme', /nuit/, { timeout: 10_000 });
    await toggle.click();
    await expect(toggle).toHaveAttribute('aria-checked', 'false');
    await page.locator('[data-testid="settings-done"]').click();
  } finally {
    // The machine gets its setting back, whatever happens to the test.
    toggleTheme(initial);
    rmSync(witness, { force: true });
  }
});

test('old choices migrate: any -nuit to elements-nuit, everything else to the default (V7)', async () => {
  // A profile from before A42 carries `nuit`: the POLARITY survives
  // the migration (the pattern from the Discovery → Wind migration,
  // PLAN-WIND E3, replayed by V7 across the whole Wada table).
  await page.evaluate(() => localStorage.setItem('wind-theme', 'nuit'));
  await page.reload();
  await expect(page.locator('[data-testid="row"]').first()).toBeVisible();
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'elements-nuit');
  expect(await page.evaluate(() => localStorage.getItem('wind-theme'))).toBe('elements-nuit');
  // A Wada -nuit theme keeps its night — the polarity choice is the
  // only thing that survives V7, and it is WRITTEN (not a silent fallback).
  await page.evaluate(() => localStorage.setItem('wind-theme', 'bruyere-nuit'));
  await page.reload();
  await expect(page.locator('[data-testid="row"]').first()).toBeVisible();
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'elements-nuit');
  expect(await page.evaluate(() => localStorage.getItem('wind-theme'))).toBe('elements-nuit');
  // A light REMOVED theme (safran) falls back to the default, silently.
  await page.evaluate(() => localStorage.setItem('wind-theme', 'safran'));
  await page.reload();
  await expect(page.locator('[data-testid="row"]').first()).toBeVisible();
  await expect(page.locator('html')).not.toHaveAttribute('data-theme');
  // A94: a VALID -nuit-suffixed choice is NOT a relic — the migration
  // guard knows Innamoramento, `innamoramento-nuit` survives startup
  // as is (without the id in THEMES, it used to be rewritten to
  // elements-nuit — proven red by removing it).
  await page.evaluate(() => localStorage.setItem('wind-theme', 'innamoramento-nuit'));
  await page.reload();
  await expect(page.locator('[data-testid="row"]').first()).toBeVisible();
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'innamoramento-nuit');
  expect(await page.evaluate(() => localStorage.getItem('wind-theme'))).toBe('innamoramento-nuit');
  // A95: the theme "Mona" is RENAMED "Innamoramento" (Chief Engineer,
  // 2026-08-29, never published in a release) — a choice persisted
  // under the old id follows the rename, in both polarities, and the
  // migration is WRITTEN (not a silent fallback to the default).
  await page.evaluate(() => localStorage.setItem('wind-theme', 'mona-nuit'));
  await page.reload();
  await expect(page.locator('[data-testid="row"]').first()).toBeVisible();
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'innamoramento-nuit');
  expect(await page.evaluate(() => localStorage.getItem('wind-theme'))).toBe('innamoramento-nuit');
  await page.evaluate(() => localStorage.setItem('wind-theme', 'mona'));
  await page.reload();
  await expect(page.locator('[data-testid="row"]').first()).toBeVisible();
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'innamoramento');
  expect(await page.evaluate(() => localStorage.getItem('wind-theme'))).toBe('innamoramento');
  // Back to the default so as not to tint other journeys — and a
  // reload: removing the key does not un-set the attribute, the page
  // would stay DISPLAYED in night mode for the next test (review).
  await page.evaluate(() => localStorage.removeItem('wind-theme'));
  await page.reload();
  await expect(page.locator('html')).not.toHaveAttribute('data-theme');
});

test('Notifications: arrival bubbles turn off and the preference persists in the database (R-D2)', async () => {
  await page.locator('[data-testid="settings"]').click();
  await page.locator('[data-testid="settings-group"][data-group="notifications"]').click();
  const toggle = page.locator('[data-testid="notif-bubbles"]');
  // The default protects the announcement: on as long as nothing is set.
  await expect(toggle).toHaveAttribute('aria-checked', 'true');
  await toggle.click();
  await expect(toggle).toHaveAttribute('aria-checked', 'false');
  await page.locator('[data-testid="settings-done"]').click();
  // The REAL round trip: reloading the application re-reads the
  // preference from the database — not from a component's state.
  await page.reload();
  await expect(page.locator('[data-testid="row"]').first()).toBeVisible();
  await page.locator('[data-testid="settings"]').click();
  await page.locator('[data-testid="settings-group"][data-group="notifications"]').click();
  await expect(toggle).toHaveAttribute('aria-checked', 'false');
  // Back to the default so as not to tint other journeys.
  await toggle.click();
  await expect(toggle).toHaveAttribute('aria-checked', 'true');
  await page.locator('[data-testid="settings-done"]').click();
});

// ——— Attachments (PLAN-PIECES-JOINTES E2) ——————————————————————————
// The native dialog cannot be driven: the `window.__e2eAttachments`
// seam (transport.js) injects the fixture paths — the picker never
// opens, everything else along the path is real.

const fixtures = path.resolve(import.meta.dirname, '..', '..', 'target', 'e2e', 'fixtures');

test('attaching is real: name + size chips, total weight, removal per chip', async () => {
  mkdirSync(fixtures, { recursive: true });
  const quote = path.join(fixtures, 'devis.pdf');
  const photo = path.join(fixtures, 'photo.jpg');
  writeFileSync(quote, Buffer.alloc(812 * 1024, 1));
  writeFileSync(photo, Buffer.alloc(2 * 1024 * 1024, 2));

  await page.locator('[data-testid="write"]').click();
  await page.evaluate((filePaths) => {
    window.__e2eAttachments = filePaths;
  }, [quote, photo]);
  await page.locator('[data-testid="compose-attach"]').click();

  await expect(page.locator('[data-testid="attachment-compose"]')).toHaveCount(2);
  await expect(page.locator('[data-testid="compose-attachments"]')).toContainText('devis.pdf');
  await expect(page.locator('[data-testid="compose-attachments"]')).toContainText('photo.jpg');
  // 812 Ko + 2 Mo — the same shape as the chips (the core's decimal point).
  await expect(page.locator('[data-testid="compose-weight"]')).toContainText('2.8 Mo / 25 MB'); // the total is composed by the shell in French (D17, debt D-56), the limit by the English catalogue

  await page.locator('[data-testid="attachment-remove"]').first().click();
  await expect(page.locator('[data-testid="attachment-compose"]')).toHaveCount(1);
  await expect(page.locator('[data-testid="compose-weight"]')).toContainText('2.0 Mo / 25 MB'); // idem D-56
});

test('closing keeps the attachments, resuming restores them (PJ-D1)', async () => {
  await page.locator('[data-testid="compose-body"]').fill('Corps avec pièce E2'); // lang:fr
  await page.locator('[data-testid="compose-cancel"]').click();
  await expect(page.locator('[data-testid="compose"]')).toHaveCount(0);
  await expect(page.locator('[data-testid="toast"]')).toContainText('Draft saved.');

  await folder('drafts').click();
  await expect(page.locator('[data-testid="folder-drafts"]')).toBeVisible();
  await page
    .locator('[data-testid="row-draft"]', { hasText: 'Corps avec pièce E2' }) // lang:fr
    .click();
  await expect(page.locator('[data-testid="compose"]')).toBeVisible();
  await expect(page.locator('[data-testid="attachment-compose"]')).toHaveCount(1);
  await expect(page.locator('[data-testid="compose-attachments"]')).toContainText('photo.jpg');
});

test('sending carries the attachment: the log holds it (PJ-D2)', async () => {
  await page.locator('[data-testid="compose-to"]').fill('dest@exemple.fr');
  await page.locator('[data-testid="compose-subject"]').fill('Envoi avec pièce E2'); // lang:fr
  await page.locator('[data-testid="compose-send"]').click();
  await expect(page.locator('[data-testid="compose"]')).toHaveCount(0);
  await expect(page.locator('[data-testid="toast"]')).toContainText('Message sent.');

  // The decor accounts have no server: the send stays logged in the
  // queue — and the log must carry the attachment (assertion PJ-D2).
  const status = await page.evaluate(() => window.__TAURI__.core.invoke('outbox_status'));
  const entry = status.entries.find((e) => e.subject === 'Envoi avec pièce E2'); // lang:fr
  expect(entry).toBeTruthy();
  expect(entry.attachments).toBe(1);
});

test('past the cap: the refusal is said, nothing gets attached (PJ-D3)', async () => {
  const huge = path.join(fixtures, 'enorme.bin');
  writeFileSync(huge, Buffer.alloc(26 * 1024 * 1024));

  await page.locator('[data-testid="write"]').click();
  await page.evaluate((filePath) => {
    window.__e2eAttachments = [filePath];
  }, huge);
  await page.locator('[data-testid="compose-attach"]').click();

  await expect(page.locator('[data-testid="compose-refusal"]')).toContainText('enorme.bin');
  await expect(page.locator('[data-testid="compose-refusal"]')).toContainText(
    'exceeds the remaining space',
  );
  await expect(page.locator('[data-testid="attachment-compose"]')).toHaveCount(0);

  await page.evaluate(() => {
    delete window.__e2eAttachments;
  });
  await page.locator('[data-testid="compose-cancel"]').click();
  await expect(page.locator('[data-testid="compose"]')).toHaveCount(0);
});

test('forwarding fetches for real — offline: failure said, "Retry", send held back (PJ-D4)', async () => {
  // The previous journey lived in the Drafts folder: back to the
  // Inbox, where the Vantis row exists.
  await folder('inbox').click();
  await page
    .locator('[data-testid="row"]', { hasText: 'Relecture du contrat Vantis' }) // lang:fr
    .click();
  // R4: forward PER message; the last message of the Vantis thread
  // carries the pricing annex (`.last()`).
  await page.locator('[data-testid="forward"]').last().click();
  await expect(page.locator('[data-testid="compose-kicker"]')).toHaveText('Forward');
  // Field finding STOP 2 PLAN-AUDIT-V2 (2026-09-02): "a word typed
  // AFTER the block vanished on send" — the cursor placed at the end
  // of the body was landing INSIDE the marked block, which the send
  // replaces. The word typed at the end lives OUTSIDE the block (the
  // editable blank line that follows it).
  const body = page.locator('[data-testid="compose-body"]');
  await body.click();
  await page.keyboard.press('Control+End');
  await page.keyboard.type('APRES-LE-BLOC');
  await expect(body).toContainText('APRES-LE-BLOC');
  await expect(body.locator('[data-wind-transfert]')).not.toContainText('APRES-LE-BLOC');
  // The decor accounts have no server: every fetch ends in failure —
  // name in alert, "Retry" — never a filled chip, and never an
  // attachment silently missing.
  await expect(page.locator('[data-testid="attachment-failure"]').first()).toBeVisible();
  // The last message of the Vantis thread carries the pricing annex.
  await expect(page.locator('[data-testid="compose-attachments"]')).toContainText(
    'Annexe_tarifs.xlsx',
  );
  await expect(page.locator('[data-testid="attachment-compose"]')).toHaveCount(0);
  await expect(page.locator('[data-testid="attachment-retry"]').first()).toBeVisible();

  // Sending is BLOCKED as long as attachments are missing.
  await page.locator('[data-testid="compose-to"]').fill('dest@exemple.fr');
  await page.locator('[data-testid="compose-send"]').click();
  await expect(page.locator('[data-testid="toast"]')).toContainText(
    'Some forwarded files are still missing',
  );
  await expect(page.locator('[data-testid="compose"]')).toBeVisible();

  // Giving up (the cross) is the EXPLICIT gesture that frees the send.
  const failures = page.locator('[data-testid="attachment-failure"]');
  const remaining = await failures.count();
  for (let i = 0; i < remaining; i += 1) {
    await page.locator('[data-testid="attachment-give-up"]').first().click();
  }
  await expect(failures).toHaveCount(0);
  await page.locator('[data-testid="compose-send"]').click();
  await expect(page.locator('[data-testid="compose"]')).toHaveCount(0);
  await expect(page.locator('[data-testid="toast"]')).toContainText('Message sent.');
});

// P0-bis (PLAN-SYNCHRO): a network drop is SAID instantly, without
// waiting for a cycle to stall on the socket timeout. We drive the
// event the OS would emit (navigator.onLine itself is not
// scriptable): the event → bar wiring is what matters.
test('offline: the bar says it instantly, coming back restores it (P0-bis)', async () => {
  await folder('inbox').click();
  const progress = page.locator('[data-testid="progress"]');
  await expect(progress).not.toContainText('Offline');

  await page.evaluate(() => window.dispatchEvent(new Event('offline')));
  await expect(progress).toContainText('Offline');

  await page.evaluate(() => window.dispatchEvent(new Event('online')));
  await expect(progress).not.toContainText('Offline');
});

// E3 (PLAN-REACTIVITE, R-D1 "< 1 s"): a gesture's outcome shows from
// the local database — the decor accounts have NO server, so this
// journey is exactly the offline contract: deletion → echo visible in
// Trash right away, counter in agreement with the list, body openable
// locally; a gesture on an echo is deferred and SAYS SO; the echo
// survives (the action is still pending — the sweep never removes a
// pending intention).
test("deleting shows in Trash instantly — offline included (E3)", async () => {
  await folder('inbox').click();

  await page
    .locator('[data-testid="row"]', { hasText: 'Facture 2026-0841' })
    .first()
    .click();
  await page.locator('[data-testid="delete"]').click();
  await expect(page.locator('[data-testid="toast"]')).toContainText(
    'Conversation deleted.',
  );
  // The counter left the nav (A29, W2-D4): Trash itself says
  // "3 + the echo" — the status bar counts its elements.
  await folder('trash').click();
  await expect(page.locator('[data-testid="status"]')).toContainText(
    'Trash · 4 items',
  );
  const echo = page.locator('[data-testid="row"]', { hasText: 'Facture 2026-0841' });
  await expect(echo).toBeVisible();

  // The echo opens LOCALLY (echo_body) — the pane carries the subject.
  await echo.click();
  await expect(page.locator('[data-testid="reading-pane"] [data-testid="thread-subject"]')).toContainText(
    'Facture 2026-0841',
  );
  // A gesture on the echo waits for reconciliation — and says so.
  await page.locator('[data-testid="delete"]').click();
  await expect(page.locator('[data-testid="toast"]')).toContainText(
    'Copy still syncing',
  );
  // The echo still lives: its intention (the logged action) is
  // waiting on the server — offline, nothing sweeps it away.
  await expect(echo).toBeVisible();
  await folder('inbox').click();
});

test('keyboard triage advances: e/Delete select the row below (A38)', async () => {
  // Played AFTER the E3 journey: its Trash counts "3 + the echo" — a
  // Delete here would add one echo too many before the assertion.
  // Starting from a fresh source (nav round trip), and working on
  // rows with NO role in the rest of the suite ("Atelier de
  // septembre", then the row below it): the Vantis thread (PJ-D4
  // forward) stays intact.
  await page.locator('[data-testid="nav-folder"][data-category="archive"]').click();
  await page.locator('[data-testid="nav-folder"][data-category="inbox"]').click();
  const rows = page.locator('[data-testid="row"]');
  await expect(rows.first()).toBeVisible();
  // The row below is captured BEFORE the gesture — afterwards it has
  // shifted up one rank.
  let subjects = await rows.locator('.subject').allTextContents();
  const departure = subjects.indexOf('Atelier de septembre');
  expect(departure).toBeGreaterThan(-1);
  const below = subjects[departure + 1];
  await rows.nth(departure).click();
  // The mouse click does NOT leave focus on the row: no later
  // keystroke (shortcut or not) can light up the :focus-visible ring
  // on a node recycled by index.
  expect(
    await page.evaluate(() => document.activeElement === document.body),
  ).toBe(true);
  await page.keyboard.press('e');
  await expect(page.locator('[data-testid="toast"]')).toContainText(
    'Conversation archived.',
  );
  // The shortcut removes focus from the clicked row: the
  // :focus-visible ring never appears on a recycled node (rows are
  // keyed by index — it would show ANOTHER conversation).
  expect(
    await page.evaluate(() => document.activeElement === document.body),
  ).toBe(true);
  // The selection has advanced: the row below carries the border AND
  // its pane is open (three panes — like on click).
  const selected = page.locator('[data-testid="row"].chosen');
  await expect(selected).toHaveCount(1);
  await expect(selected.locator('.subject')).toHaveText(below);
  await expect(page.locator('[data-testid="reading-pane"] [data-testid="thread-subject"]')).toHaveText(below);
  // The FRESH list first (stale-while-revalidate: served rows stay
  // shown for a moment) — once the archived row is gone, capturing
  // the next row below is safe.
  await expect(
    page.locator('[data-testid="row"]', { hasText: 'Atelier de septembre' }),
  ).toHaveCount(0);
  subjects = await rows.locator('.subject').allTextContents();
  const next = subjects[subjects.indexOf(below) + 1];
  // The gesture chains without picking the mouse back up: Delete acts
  // on the advanced selection, and advances further.
  await page.keyboard.press('Delete');
  await expect(page.locator('[data-testid="toast"]')).toContainText(
    'Conversation deleted.',
  );
  await expect(selected.locator('.subject')).toHaveText(next);
  await expect(page.locator('[data-testid="reading-pane"] [data-testid="thread-subject"]')).toHaveText(next);
});

// ——— The "empty then close" race (field finding of 2026-08-15) ——
// A deferred save started BEFORE the emptying still carries content:
// without serialization, its result used to resurrect the draft that
// closing had just deleted (a ghost in the folder — seen twice by the
// suite under load). Holding the transport makes the race
// deterministic: the write is IN FLIGHT when the gesture decides.
test('emptying then closing never resurrects the draft — the in-flight save lands first', async () => {
  await page.keyboard.press('c');
  await page.locator('[data-testid="compose-subject"]').fill('Course E2E');
  await page.locator('[data-testid="compose-body"]').fill('Premier contenu.');
  // First COMPLETE save: the draft has an id.
  await page.waitForTimeout(2600);
  // Second write, then hold: the save starts and gets BLOCKED.
  await page.locator('[data-testid="compose-body"]').fill('Contenu condamné.'); // lang:fr
  await page.evaluate(() => {
    window.__e2eHold = new Promise((release) => {
      window.__e2eRelease = release;
    });
  });
  await page.waitForTimeout(2300);
  // The emptying and the gesture, during the flight. (`fill('')` does
  // not empty a contenteditable: Ctrl+A + Delete, like the user.)
  await page.locator('[data-testid="compose-subject"]').fill('');
  await page.locator('[data-testid="compose-body"]').click();
  await page.keyboard.press('Control+a');
  await page.keyboard.press('Delete');
  await page.locator('[data-testid="compose-cancel"]').click();
  await page.evaluate(() => {
    window.__e2eRelease?.();
    delete window.__e2eHold;
    delete window.__e2eRelease;
  });
  // Closing waited for the flight, then deleted: no ghost.
  await expect(page.locator('[data-testid="compose"]')).toHaveCount(0);
  await page.locator('[data-testid="nav-folder"][data-category="drafts"]').click();
  await expect(page.locator('[data-testid="folder-drafts"]')).toBeVisible();
  await expect(
    page.locator('[data-testid="row-draft"]', { hasText: 'Course E2E' }),
  ).toHaveCount(0);
  await page.locator('[data-testid="nav-folder"][data-category="inbox"]').click();
  await expect(page.locator('[data-testid="row"]').first()).toBeVisible();
});

// PLAN-RETOURS-5 (field 2026-08-21): during the reconciliation window,
// the temporary Sent entry (the send echo) says its REAL recipients —
// never "To: sent" — and its attachment shows in metadata (name +
// weight), an INERT chip: the bytes went out with the send, nothing
// gets saved until the server copy has arrived (D2 — the echo dies at
// reconciliation, proven at the core by `the_real_row_kills_the_echo`).
test('the send echo says its recipients and its attachment — never "To: sent" (RETOURS-5)', async () => {
  await folder('sent').click();
  const row = page.locator('[data-testid="row"]', { hasText: 'Bordereau signé' }); // lang:fr
  await expect(row).toBeVisible();
  await expect(row).toContainText('c.rousseau@atelier-nord.fr');
  await expect(row).not.toContainText('sent');

  await row.click();
  const pane = page.locator('[data-testid="reading-pane"]');
  await expect(pane.locator('[data-testid="thread-subject"]')).toContainText('Bordereau signé'); // lang:fr
  // The expanded message's head: the "To:" line (A92) says the real
  // recipient.
  await expect(pane.locator('[data-testid="row-to"]').first()).toContainText('c.rousseau@atelier-nord.fr');
  // The send log's attachment: name + weight in the chip, inert.
  const attachment = pane.locator('[data-testid="attachment"]');
  await expect(attachment).toHaveCount(1);
  await expect(attachment).toContainText('Bordereau-signe.pdf');
  await expect(attachment).toContainText('20 Ko');
  await expect(attachment).toBeDisabled();
  await folder('inbox').click();
});

// PLAN-RETOURS-5 (D3-D4): address autocompletion — the contacts
// directory (learned from the decor's mail) suggests on the prefix,
// the menu SHOWS the display name, insertion sets the BARE address.
// By keyboard (Enter) as by click, in Cc as in To.
test('autocompletion suggests known addresses — name shown, bare address inserted (RETOURS-5)', async () => {
  await page.locator('[data-testid="write"]').click();
  const toField = page.locator('[data-testid="compose-to"]');
  await toField.fill('rousseau');
  const menu = page.locator('[data-testid="compose-suggestions"]');
  await expect(menu).toBeVisible();
  const first = page.locator('[data-testid="address-suggestion"]').first();
  await expect(first).toContainText('Camille Rousseau');
  await expect(first).toContainText('c.rousseau@atelier-nord.fr');
  await toField.press('Enter');
  await expect(toField).toHaveValue('c.rousseau@atelier-nord.fr');
  await expect(menu).toHaveCount(0);

  // Cc, by CLICK — a recipient known by their address.
  await page.locator('[data-testid="compose-cc-button"]').click();
  const ccField = page.locator('[data-testid="compose-cc"]');
  await ccField.fill('s.nar');
  await expect(menu).toBeVisible();
  await page.locator('[data-testid="address-suggestion"]').first().click();
  await expect(ccField).toHaveValue('s.nardi@atelier-nord.fr');
  await expect(menu).toHaveCount(0);

  // Cleanup: fields emptied — the draft born from typing is discarded
  // on close ("a draft emptied of its text is discarded").
  await toField.fill('');
  await ccField.fill('');
  await page.locator('[data-testid="compose-cancel"]').click();
  await expect(page.locator('[data-testid="compose"]')).toHaveCount(0);
});

// ——— v3 review: frame exclusivity (played at the end of the chain — it archives) ———

test('archiving via the shortcut from screen 03 closes the frame — never a ghost full screen (v3 review)', async () => {
  // v3 review: three hand-reconciled booleans used to leave `visible`
  // armed when `e` archived from full screen — the next list click
  // would reopen screen 03 unrequested, with TWO Threads mounted.
  // Since then, exclusivity lives in the store (fil.cadre).
  await page.locator('[data-testid="row"]').first().click();
  await page.locator('[data-testid="see-conversation"]').click();
  await expect(page.locator('[data-testid="conversation"]')).toBeVisible();
  await page.keyboard.press('e');
  await expect(page.locator('[data-testid="toast"]')).toContainText('Conversation archived.');
  // The full-screen frame fell with the thread.
  await expect(page.locator('[data-testid="conversation"]')).toHaveCount(0);
  // The next click opens the PANE, never a resurrected screen 03 —
  // and the object stays unique.
  await page.locator('[data-testid="row"]').first().click();
  await expect(page.locator('[data-testid="conversation"]')).toHaveCount(0);
  await expect(page.locator('[data-testid="thread-subject"]')).toHaveCount(1);
  await expect(page.locator('[data-testid="reading-pane"] [data-testid="thread-subject"]')).not.toBeEmpty();
  // Keyboard triage (A38) is ALIVE afterwards: e advances further.
  const subject = await page.locator('[data-testid="reading-pane"] [data-testid="thread-subject"]').innerText();
  await page.keyboard.press('e');
  await expect(page.locator('[data-testid="toast"]')).toContainText('Conversation archived.');
  await expect(page.locator('[data-testid="reading-pane"] [data-testid="thread-subject"]')).not.toHaveText(subject);
});
