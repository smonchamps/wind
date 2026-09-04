// PLAN-MODE-ORGANISE E1 — the foundation of Organized mode (2026-08-29).
//
// The "Organized" toggle lives to the right of search (form settled
// at the prototype, six Chief Engineer passes) ; the state lives in
// SQLite prefs (D2 amended: the core must read it — the Non rules of
// E3 will die with it), so it survives a reload WITHOUT localStorage.
// In organized mode, the nav gains Feed and Paper trail — views of
// the unified flow filtered by sender routing (routage_unified_scoped,
// PK probe proven at spike S2). Classic mode stays today's app: the
// "zero diff" guard is the first test.
import { test, expect } from '@playwright/test';
import { launchAppV2, closeApp, injectArrival, purgeLocals } from '../launch.mjs';

let app;
let browser;
let page;

test.describe.configure({ mode: 'serial' });

test.beforeAll(async () => {

  ({ app, browser, page } = await launchAppV2({
    accounts: [{ email: 'principal@exemple.fr', messages: 6 }],
  }));
  // RETOURS-15: the suite asserts the PANE at three panes — a
  // `wind-volets` left behind by an interrupted redesign-panes run
  // (shared WebView2 profile, the launch.mjs trap) would silently run
  // it at two. Purge, then reload so the app re-reads the default.
  await purgeLocals(page, ['wind-volets', 'wind-largeurs']);
  await page.reload();
});

test.afterAll(async () => {
  await purgeLocals(page, ['wind-volets', 'wind-largeurs']);
  await closeApp({ app, browser });
});

test('classic mode is intact: toggle off, nav at the six folders', async () => {
  await expect(page.locator('[data-testid="row"]').first()).toBeVisible();
  const toggle = page.locator('[data-testid="organized-mode"]');
  await expect(toggle).toBeVisible();
  await expect(toggle).toHaveAttribute('aria-checked', 'false');
  // The "classic unchanged" guard: exactly the six canonical folders,
  // neither Feed nor Paper trail.
  await expect(page.locator('[data-testid="nav-folder"]')).toHaveCount(6);
  await expect(page.locator('[data-testid="nav-folder"][data-category="feed"]')).toHaveCount(0);
  // R3/R12 (RETOURS-13): in classic, the long label and no net.
  await expect(page.locator('[data-testid="nav-folder"][data-category="inbox"]'))
    .toContainText('Inbox');
  await expect(page.locator('[data-testid="nav-separator"]')).toHaveCount(0);
});

test('the toggle recomposes the nav, the Feed serves the routed senders, and the mode PERSISTS', async () => {
  await page.locator('[data-testid="organized-mode"]').click();
  await expect(page.locator('[data-testid="organized-mode"]')).toHaveAttribute('aria-checked', 'true');
  await expect(page.locator('[data-testid="nav-folder"]')).toHaveCount(10);
  // R3/R12 (RETOURS-13): in organized mode the Inbox is called
  // "Inbox" (the English catalogue names both the same; the French short form R3 is proven by redesign-language.spec.js), and a net separates the 5 organized folders from the rest.
  const inboxRank = page.locator('[data-testid="nav-folder"][data-category="inbox"]');
  await expect(inboxRank).toContainText('Inbox');
  await expect(page.locator('[data-testid="nav-separator"]')).toHaveCount(1);

  // The Feed before any routing: nothing — the filter is real, not a
  // fixture (the Paper trail re-proves it below after routing). E5bis:
  // the Feed is a scene of CARDS, no longer a list.
  await page.locator('[data-testid="nav-folder"][data-category="feed"]').click();
  await expect(page.locator('[data-testid="feed-empty"]')).toBeVisible();
  await expect(page.locator('[data-testid="feed-card"]')).toHaveCount(0);

  // Route the senders of the test set to the Feed, through THE
  // product command (the "Move to…" gesture arrives later in E1 —
  // the service, itself, is already the real one).
  await page.evaluate(async () => {
    const invoke = window.__TAURI__.core.invoke;
    for (let n = 0; n < 12; n += 1) {
      await invoke('route_sender', {
        address: `expediteur${n}@exemple.fr`,
        destination: 'feed',
        rule: null,
      });
    }
  });

  // Persistence is in the DATABASE (SQLite prefs): a full reload
  // re-reads the mode from the core — never from localStorage.
  await page.reload();
  await expect(page.locator('[data-testid="organized-mode"]')).toHaveAttribute('aria-checked', 'true');
  await expect(page.locator('[data-testid="nav-folder"]')).toHaveCount(10);

  // The Feed now shows the mail of the routed senders, in cards
  // ALREADY OPEN: the body reads without a click (E5bis — the proof
  // of the D5/S3 preloading, in the sanitized S1 iframe).
  await page.locator('[data-testid="nav-folder"][data-category="feed"]').click();
  await expect(page.locator('[data-testid="feed-card"]').first()).toBeVisible();
  // R11 (RETOURS-13): the header at the Screener's format — glyph +
  // title + two Chief Engineer sentences, on the left; all new: the
  // "Unread" section.
  await expect(page.locator('[data-testid="feed-title"] svg')).toHaveCount(1);
  await expect(page.locator('[data-testid="feed"]'))
    .toContainText('All your informational emails are gathered here.');
  await expect(page.locator('[data-testid="feed"]'))
    .toContainText('Just scroll through them to read.');
  await expect(page.locator('[data-testid="feed-section-unread"]')).toBeVisible();
  await expect(
    page.frameLocator('[data-testid="feed-card"] iframe').first().locator('body'),
  ).toContainText('contenu de démonstration'); // lang:fr
  // The fold (Chief Engineer finding): folding replaces the body with
  // the preview, unfolding renders it again.
  const first = page.locator('[data-testid="feed-card"]').first();
  await first.locator('[data-testid="feed-fold"]').click();
  await expect(first.locator('iframe')).toHaveCount(0);
  await first.locator('[data-testid="feed-fold"]').click();
  await expect(first.locator('iframe')).toHaveCount(1);
  // …the Paper trail stays empty (the destination really filters)…
  await page.locator('[data-testid="nav-folder"][data-category="paper_trail"]').click();
  await expect(page.locator('[data-testid="status"]')).toContainText('Paper trail');
  await expect(page.locator('[data-testid="row"]')).toHaveCount(0);
  // …and the ORGANIZED Inbox no longer shows them (E2: a thread
  // routed elsewhere lives in ITS OWN view — the shared exclusion of
  // the flow; everything is in the Feed here, so the organized Inbox
  // is empty).
  await page.locator('[data-testid="nav-folder"][data-category="inbox"]').click();
  // The empty state is ASSERTED ("Aucun message ici.") — never a
  // zero count while the page is still loading.
  await expect(page.locator('[data-testid="list"]')).toContainText('No messages here.');
  await expect(page.locator('[data-testid="row"]')).toHaveCount(0);
});

test('"Move to…" routes the WHOLE sender — the ⋯ of the cards and the thread bar', async () => {
  // Everything is in the Feed (previous test) ; the ⋯ of a card sends
  // its sender to the Paper trail — what the user SEES: the menu,
  // the toast, then the mail in the Paper trail (a list, itself).
  await page.locator('[data-testid="nav-folder"][data-category="feed"]').click();
  // RETOURS-13 R10: already-read cards may have grouped by sender —
  // unfold everything to catch the first card.
  for (const g of await page.locator('[data-testid="feed-group"]').all()) await g.click();
  const card = page.locator('[data-testid="feed-card"]').first();
  await card.hover();
  await card.locator('[data-testid="feed-gestures"]').click();
  await page.locator('[data-testid="feed-to-paper_trail"]').click();
  await expect(page.locator('[data-testid="toast"]')).toContainText('Paper trail');
  await page.locator('[data-testid="nav-folder"][data-category="paper_trail"]').click();
  // RETOURS-14 R6 (D7): the Paper trail is GROUPED by sender — the
  // group unfolds, the thread opens from its rows.
  await expect(page.locator('[data-testid="paper-trail-group"]').first()).toBeVisible();
  await page.locator('[data-testid="paper-trail-group"]').first().click();
  await expect(page.locator('[data-testid="paper-trail-message"]').first()).toBeVisible();
  // The thread bar, from the Paper trail: the Move to… menu.
  await page.locator('[data-testid="paper-trail-message"]').first().click();
  await page.locator('[data-testid="move-to"]').click();
  await expect(page.locator('[data-testid="move-feed"]')).toBeVisible();
  await page.keyboard.press('Escape');
  // The gesture only exists in organized mode: the classic guard.
  await page.locator('[data-testid="organized-mode"]').click();
  await page.locator('[data-testid="row"]').first().click();
  await expect(page.locator('[data-testid="move-to"]')).toHaveCount(0);
  await page.locator('[data-testid="organized-mode"]').click();
});

// ------- RETOURS-13 R10 — the Feed in Unread / Read sections -------
test('cards read down to the bottom group by sender — "Read previously"', async () => {
  await page.locator('[data-testid="nav-folder"][data-category="feed"]').click();
  const scene = page.locator('[data-testid="feed"]');
  // Unfold the already-read groups, then walk the WHOLE scene: the
  // bottom of each elevation crosses the screen — the definition of "read".
  for (const g of await page.locator('[data-testid="feed-group"]').all()) await g.click();
  await scene.evaluate(async (el) => {
    for (let y = 0; y <= el.scrollHeight; y += 150) {
      el.scrollTop = y;
      await new Promise((r) => setTimeout(r, 40));
    }
  });
  // Give the marks time to be written (observer + IPC).
  await page.waitForTimeout(600);
  // The sectioning happens AT THE SERVICE of the page (a card never
  // jumps mid-read): a folder round trip.
  await page.locator('[data-testid="nav-folder"][data-category="paper_trail"]').click();
  await page.locator('[data-testid="nav-folder"][data-category="feed"]').click();
  await expect(page.locator('[data-testid="feed-section-read"]')).toBeVisible();
  // Field finding C5: the "Unread" title STAYS, the check mark says all read.
  await expect(page.locator('[data-testid="feed-section-unread"]')).toBeVisible();
  await expect(page.locator('[data-testid="feed-all-read"]'))
    .toContainText('You have read all the new stories in your Feed.');
  // Folded by default: no card on screen, groups by sender sorted
  // alphabetically.
  await expect(page.locator('[data-testid="feed-card"]')).toHaveCount(0);
  const names = await page.locator('[data-testid="feed-group-name"]').allTextContents();
  expect(names.length).toBeGreaterThan(1);
  expect(names).toEqual(
    [...names].sort((a, b) => a.localeCompare(b, 'fr', { sensitivity: 'base' })),
  );
  // The click unfolds the group: its cards, folded onto the subject
  // line — unfoldable one by one.
  await page.locator('[data-testid="feed-group"]').first().click();
  const card = page.locator('[data-testid="feed-card"]').first();
  await expect(card).toBeVisible();
  await expect(card.locator('iframe')).toHaveCount(0);
  await card.locator('[data-testid="feed-fold"]').click();
  await expect(card.locator('iframe')).toHaveCount(1);
});

// ------------------------- E2 — the Screener -------------------------
// The retention (D3 "arrivals only") is proven by what the user
// SEES: an unknown who writes AFTER activation does NOT appear in
// the organized Inbox — it waits at the Screener, with its badge; a
// known sender (mail from before the era) arrives normally. The
// arrival goes through the production path (`injectArrival` →
// upsert_envelopes), never a fixture.
test("an unknown who writes waits at the Screener — the organized Inbox doesn't show it", async () => {
  // Neutral fixture: the verdicts set by the E1 tests are cleared —
  // the desk is proven on a workstation with no prior routing.
  await page.evaluate(async () => {
    const invoke = window.__TAURI__.core.invoke;
    const routings = await invoke('routings');
    for (const r of routings) await invoke('remove_routing', { address: r.address });
  });
  injectArrival({
    email: 'principal@exemple.fr', sender: 'inconnue@exemple.fr',
    name: 'Nouvelle Venue', subject: 'Premiere fois',
  });
  injectArrival({
    email: 'principal@exemple.fr', sender: 'expediteur2@exemple.fr',
    name: 'Alice Martin', subject: 'Suite du dossier', // lang:fr
  });
  await page.reload();
  await expect(page.locator('[data-testid="organized-mode"]')).toHaveAttribute('aria-checked', 'true');
  // The known sender arrives in the Inbox; the unknown one is NOT there.
  await expect(page.locator('[data-testid="row"]', { hasText: 'Suite du dossier' })).toHaveCount(1); // lang:fr
  await expect(page.locator('[data-testid="row"]', { hasText: 'Premiere fois' })).toHaveCount(0);
  // The Screener's badge counts ITS message.
  const screenerRank = page.locator('[data-testid="nav-folder"][data-category="screener"]');
  await expect(screenerRank).toContainText('Screener');
  await expect(screenerRank).toContainText('1');
  // The desk: a rank at the rows' format, the address in plain text.
  await screenerRank.click();
  await expect(page.locator('[data-testid="screener"]')).toContainText('Do you want to receive their messages?');
  // R4/R7 (RETOURS-13): the screener glyph tops the title, the
  // subtitle carries the three Chief Engineer sentences word for word.
  await expect(page.locator('[data-testid="screener-title"] svg')).toHaveCount(1);
  await expect(page.locator('[data-testid="screener"]'))
    .toContainText('Do you allow them to contact you?');
  await expect(page.locator('[data-testid="screener"]'))
    .toContainText('Senders will never be told of your decision.');
  const rank = page.locator('[data-testid="screener-rank"]');
  await expect(rank).toHaveCount(1);
  await expect(rank).toContainText('Nouvelle Venue');
  await expect(rank).toContainText('<inconnue@exemple.fr>');
  await expect(rank).toContainText('Premiere fois');
});

test("the bare Yes returns the sender to the Inbox, the desk empties", async () => {
  await page.locator('[data-testid="screener-yes"]').click();
  await expect(page.locator('[data-testid="toast"]')).toContainText('can write to you');
  await expect(page.locator('[data-testid="screener-empty"]')).toBeVisible();
  // R6 (RETOURS-13): the empty history, the Chief Engineer text word for word.
  await expect(page.locator('[data-testid="screener"]'))
    .toContainText('You have not screened out any senders yet.');
  await page.locator('[data-testid="nav-folder"][data-category="inbox"]').click();
  await expect(page.locator('[data-testid="row"]', { hasText: 'Premiere fois' })).toHaveCount(1);
});

test('the No with a rule screens out, the history says so, "Reinstate" returns to the Screener', async () => {
  injectArrival({
    email: 'principal@exemple.fr', sender: 'promo@exemple.fr',
    name: 'Promo Eclair', subject: 'Offre eclair', n: 2,
  });
  await page.reload();
  await expect(page.locator('[data-testid="row"]', { hasText: 'Offre eclair' })).toHaveCount(0);
  await page.locator('[data-testid="nav-folder"][data-category="screener"]').click();
  await expect(page.locator('[data-testid="screener-rank"]')).toHaveCount(1);
  // The No's mini ⋯ sets the rule — "Archived automatically".
  await page.locator('[data-testid="screener-mini-no"]').click();
  await page.locator('[data-testid="screener-rule-archive"]').click();
  await expect(page.locator('[data-testid="toast"]')).toContainText('will be archived automatically');
  await expect(page.locator('[data-testid="screener-empty"]')).toBeVisible();
  const history = page.locator('[data-testid="screener-history"]');
  await expect(history).toHaveCount(1);
  await expect(history).toContainText('promo@exemple.fr');
  await expect(history).toContainText('archived automatically');
  // "Reinstate" undoes the verdict: the unknown sender RE-waits at the desk.
  await page.locator('[data-testid="screener-reinstate"]').click();
  await expect(page.locator('[data-testid="toast"]')).toContainText('reinstated');
  await expect(page.locator('[data-testid="screener-rank"]')).toHaveCount(1);
  await expect(page.locator('[data-testid="screener-rank"]')).toContainText('promo@exemple.fr');
});

test('classic mode ALWAYS shows everything — retention is an organized-mode matter', async () => {
  await page.locator('[data-testid="organized-mode"]').click();
  await expect(page.locator('[data-testid="organized-mode"]')).toHaveAttribute('aria-checked', 'false');
  // The sender still waiting (promo) is VISIBLE in classic.
  await expect(page.locator('[data-testid="row"]', { hasText: 'Offre eclair' }).first()).toBeVisible();
  await expect(page.locator('[data-testid="row"]', { hasText: 'Premiere fois' })).toHaveCount(1);
  // Returning to organized mode, WITHOUT navigating: the displayed
  // list refreshes itself (E2 review — the toggle reloaded the nav
  // but not the Inbox, the screen kept the other mode's page).
  await page.locator('[data-testid="organized-mode"]').click();
  await expect(page.locator('[data-testid="organized-mode"]')).toHaveAttribute('aria-checked', 'true');
  await expect(page.locator('[data-testid="row"]', { hasText: 'Offre eclair' })).toHaveCount(0);
  await expect(page.locator('[data-testid="row"]', { hasText: 'Premiere fois' })).toHaveCount(1);
});

// ------------------- E3 — the No rules executed -------------------
test("the No rule runs on arrival — and never touches mail earlier than the verdict", async () => {
  // promo@ is re-waiting at the desk (previous test): the No with the
  // rule "Moved automatically to the trash" (trash in the core, D4).
  await page.locator('[data-testid="nav-folder"][data-category="screener"]').click();
  await page.locator('[data-testid="screener-mini-no"]').click();
  await page.locator('[data-testid="screener-rule-trash"]').click();
  await expect(page.locator('[data-testid="toast"]')).toContainText('will go to the Trash');
  // The verdict is timestamped to the SECOND: an arrival within the
  // same second counts as earlier ("> verdict", an accepted limit) —
  // we let the boundary pass before injecting.
  await page.waitForTimeout(1500);
  // Its NEXT message arrives: the rule handles it — action log +
  // local disappearance, it appears NOWHERE, not even in classic. Its
  // mail from BEFORE the verdict, though, does not move.
  injectArrival({
    email: 'principal@exemple.fr', sender: 'promo@exemple.fr',
    name: 'Promo Eclair', subject: 'Relance finale',
  });
  // The WITNESS (E3 review): a second arrival, from an unknown sender
  // — its presence proves that the injection and its processing did
  // take place; without it, "Relance finale absent" would be equally
  // true if nothing had arrived at all (vacant net).
  injectArrival({
    email: 'principal@exemple.fr', sender: 'temoin@exemple.fr',
    name: 'Temoin', subject: 'Temoin de synchro',
  });
  await page.reload();
  await expect(page.locator('[data-testid="organized-mode"]')).toHaveAttribute('aria-checked', 'true');
  await page.locator('[data-testid="nav-folder"][data-category="screener"]').click();
  await expect(page.locator('[data-testid="screener-rank"]')).toContainText('temoin@exemple.fr');
  await page.locator('[data-testid="nav-folder"][data-category="inbox"]').click();
  await expect(page.locator('[data-testid="row"]', { hasText: 'Relance finale' })).toHaveCount(0);
  await page.locator('[data-testid="organized-mode"]').click();
  await expect(page.locator('[data-testid="organized-mode"]')).toHaveAttribute('aria-checked', 'false');
  await expect(page.locator('[data-testid="row"]', { hasText: 'Temoin de synchro' })).toHaveCount(1);
  await expect(page.locator('[data-testid="row"]', { hasText: 'Offre eclair' }).first()).toBeVisible();
  await expect(page.locator('[data-testid="row"]', { hasText: 'Relance finale' })).toHaveCount(0);
  await page.locator('[data-testid="organized-mode"]').click();
  await expect(page.locator('[data-testid="organized-mode"]')).toHaveAttribute('aria-checked', 'true');
});

// ---------- RETOURS-13 R5/R9 — the Screener buttons' defaults ----------
test("the bare No sends to the Trash — the shipped default, said by the toast and the history", async () => {
  // temoin@ waits at the Screener (end of the E3 test).
  await page.locator('[data-testid="nav-folder"][data-category="screener"]').click();
  await expect(page.locator('[data-testid="screener-rank"]')).toContainText('temoin@exemple.fr');
  await page.locator('[data-testid="screener-no"]').click();
  await expect(page.locator('[data-testid="toast"]')).toContainText('will go to the Trash');
  await expect(page.locator('[data-testid="screener-history"]', { hasText: 'temoin@exemple.fr' }))
    .toContainText('deleted automatically');
  // We undo it: temoin re-waits, the serial chain's state is restored.
  await page.locator('[data-testid="screener-history"]', { hasText: 'temoin@exemple.fr' })
    .locator('[data-testid="screener-reinstate"]').click();
  await expect(page.locator('[data-testid="screener-rank"]')).toContainText('temoin@exemple.fr');
  await page.locator('[data-testid="nav-folder"][data-category="inbox"]').click();
});

test('Settings > Screener sets the defaults — the bare click obeys, persistence is in the database', async () => {
  await page.locator('[data-testid="settings"]').click();
  await page.locator('[data-testid="settings-group"][data-group="screener"]').click();
  const yes = page.locator('[data-testid="screener-default-yes"]');
  const no = page.locator('[data-testid="screener-default-no"]');
  await expect(yes).toHaveValue('inbox');
  await expect(no).toHaveValue('trash');
  await yes.selectOption('feed');
  await no.selectOption('archive');
  await page.locator('[data-testid="settings-done"]').click();
  // The bare Yes click follows the set default: temoin goes to the Feed.
  await page.locator('[data-testid="nav-folder"][data-category="screener"]').click();
  await page.locator('[data-testid="screener-yes"]').click();
  await expect(page.locator('[data-testid="toast"]')).toContainText('go to the Feed');
  // Persistence is in the DATABASE: a full reload re-reads the
  // defaults from the core.
  await page.reload();
  await page.locator('[data-testid="settings"]').click();
  await page.locator('[data-testid="settings-group"][data-group="screener"]').click();
  await expect(page.locator('[data-testid="screener-default-yes"]')).toHaveValue('feed');
  await expect(page.locator('[data-testid="screener-default-no"]')).toHaveValue('archive');
  // Back to the shipped defaults, temoin re-waits: the state is restored.
  await page.locator('[data-testid="screener-default-yes"]').selectOption('inbox');
  await page.locator('[data-testid="screener-default-no"]').selectOption('trash');
  await page.locator('[data-testid="settings-done"]').click();
  await page.evaluate(async () => {
    await window.__TAURI__.core.invoke('remove_routing', { address: 'temoin@exemple.fr' });
  });
  await page.locator('[data-testid="nav-folder"][data-category="inbox"]').click();
});

// ---------------- E4 — the organized Inbox (sections) ----------------
test('the organized Inbox has its sections and, at three panes, reads in the PANE', async () => {
  // Mode ON, on the Inbox (end of the E3 test). The two sections
  // frame ONE flow: unread first, the seam is the COUNT. RETOURS-15
  // D1 (2026-09-04, beta feedback — reverses the E4/A99 rule): the
  // 3-pane setting is honored here — the reading pane exists and a
  // click opens IN the pane, never screen 03.
  const sections = page.locator('[data-testid="section"]');
  await expect(sections).toHaveCount(2);
  await expect(sections.first()).toContainText('New for you ·');
  await expect(sections.last()).toContainText('Previously seen');
  await expect(page.locator('[data-testid="reading-pane"]')).toBeVisible();

  const label = await sections.first().textContent();
  const n = Number(label.match(/(\d+)/)[1]);
  await page.locator('[data-testid="row"]').first().click();
  await expect(
    page.locator('[data-testid="reading-pane"] [data-testid="thread-subject"]'),
  ).toBeVisible();
  await expect(page.locator('[data-testid="conversation"]')).toHaveCount(0);
  // D2 (2026-09-04): the row never jumps under the open reading — the
  // sections hold until the list is re-served.
  await expect(sections.first()).toContainText(`New for you · ${n}`);
  // The product gesture that re-serves (a folder round trip): the
  // READ thread has left "New for you".
  await page.locator('[data-testid="nav-folder"][data-category="feed"]').click();
  await page.locator('[data-testid="nav-folder"][data-category="inbox"]').click();
  await expect(sections.first()).toContainText(`New for you · ${n - 1}`);
});

test('at two panes the organized Inbox still opens screen 03 (V-D2 unchanged)', async () => {
  await page.locator('[data-testid="settings"]').click();
  await page.locator('[data-testid="settings-group"][data-group="affichage"]').click();
  await page.locator('[data-testid="display-panes"]').selectOption('2');
  await page.locator('[data-testid="settings-done"]').click();
  await expect(page.locator('[data-testid="reading-pane"]')).toHaveCount(0);
  await page.locator('[data-testid="row"]').first().click();
  await expect(page.locator('[data-testid="conversation"]')).toBeVisible();
  await page.keyboard.press('Escape');
  await expect(page.locator('[data-testid="conversation"]')).toHaveCount(0);
  // Back to three panes: the suite's default state is restored.
  await page.locator('[data-testid="settings"]').click();
  await page.locator('[data-testid="settings-group"][data-group="affichage"]').click();
  await page.locator('[data-testid="display-panes"]').selectOption('3');
  await page.locator('[data-testid="settings-done"]').click();
  await expect(page.locator('[data-testid="reading-pane"]')).toBeVisible();
});

test("a row's ⋯ moves the sender — left of the time, without shifting the geometry", async () => {
  const rank = page.locator('[data-testid="row"]', { hasText: 'Suite du dossier' }); // lang:fr
  await expect(rank).toHaveCount(1);
  await rank.locator('[data-testid="row-gestures"]').click();
  await page.locator('[data-testid="gestures-feed"]').click();
  await expect(page.locator('[data-testid="toast"]')).toContainText('Feed');
  await expect(page.locator('[data-testid="row"]', { hasText: 'Suite du dossier' })).toHaveCount(0); // lang:fr
  await page.locator('[data-testid="nav-folder"][data-category="feed"]').click();
  await expect(page.locator('[data-testid="feed-card"]', { hasText: 'Suite du dossier' })).toHaveCount(1); // lang:fr
  await page.locator('[data-testid="nav-folder"][data-category="inbox"]').click();
  // Fixture restored (E5 review): the verdict set by THIS test is
  // cleared — the following tests inherit a full Inbox, never a Feed
  // populated by accident.
  await page.evaluate(async () => {
    await window.__TAURI__.core.invoke('remove_routing', { address: 'expediteur2@exemple.fr' });
  });
  // The list does not follow an external write (it only reloads on a
  // poll generation's beat): we refresh it through the product
  // gesture — a folder round trip. Before RETOURS-13 this step went
  // through a LUCKY reload of the probe (a lucky net).
  await page.locator('[data-testid="nav-folder"][data-category="feed"]').click();
  await page.locator('[data-testid="nav-folder"][data-category="inbox"]').click();
  await expect(page.locator('[data-testid="row"]', { hasText: 'Suite du dossier' })).toHaveCount(1); // lang:fr
});

// ------------------------- E5 — Set aside -------------------------
test('set aside: the thread leaves the list, lives in the pile, and "Done" returns it', async () => {
  const rank = page.locator('[data-testid="row"]', { hasText: 'Premiere fois' });
  await expect(rank).toHaveCount(1);
  await rank.locator('[data-testid="row-gestures"]').click();
  await page.locator('[data-testid="gestures-aside"]').click();
  await expect(page.locator('[data-testid="toast"]')).toContainText('Set aside');
  await expect(page.locator('[data-testid="row"]', { hasText: 'Premiere fois' })).toHaveCount(0);
  // The pile, at the bottom right: the count, the fan, the board.
  const pile = page.locator('[data-testid="pile-button"]');
  await expect(pile).toContainText('1');
  await pile.click();
  const card = page.locator('[data-testid="pile-card"]');
  await expect(card).toHaveCount(1);
  await expect(card).toContainText('Premiere fois');
  await page.locator('[data-testid="pile-see-board"]').click();
  await expect(page.locator('[data-testid="pile-board"]')).toBeVisible();
  await expect(page.locator('[data-testid="pile-board-card"]')).toContainText('Premiere fois');
  // "Done" sends the message back where it came from — the pile empties.
  await page.locator('[data-testid="pile-finish"]').click();
  await expect(page.locator('[data-testid="toast"]')).toContainText('Brought back');
  await expect(page.locator('[data-testid="pile-board"]')).toHaveCount(0);
  await expect(page.locator('[data-testid="row"]', { hasText: 'Premiere fois' })).toHaveCount(1);
  await expect(page.locator('[data-testid="pile-button"]')).toHaveCount(0);
});

test('the thread bar toggles "Set aside" / "Resume"', async () => {
  await page.locator('[data-testid="row"]', { hasText: 'Premiere fois' }).click();
  // RETOURS-15 D1: at three panes the thread opens in the PANE — its
  // bar carries the same toggle as screen 03.
  await expect(
    page.locator('[data-testid="reading-pane"] [data-testid="thread-subject"]'),
  ).toBeVisible();
  const toggle = page.locator('[data-testid="put-aside"]');
  await expect(toggle).toContainText('Set aside');
  await toggle.click();
  // The thread has just left its view: its row is gone from the list.
  await expect(page.locator('[data-testid="row"]', { hasText: 'Premiere fois' })).toHaveCount(0);
  // Resuming from the fan: the card opens screen 03, the bar says
  // "Bring back", the gesture returns the thread to the Inbox.
  await page.locator('[data-testid="pile-button"]').click();
  await page.locator('[data-testid="pile-card"]').click();
  await expect(page.locator('[data-testid="conversation"]')).toBeVisible();
  await expect(page.locator('[data-testid="put-aside"]')).toContainText('Bring back');
  await page.locator('[data-testid="put-aside"]').click();
  await expect(page.locator('[data-testid="toast"]')).toContainText('Brought back');
  await page.locator('[data-testid="back-to-mailbox"]').click();
  await expect(page.locator('[data-testid="row"]', { hasText: 'Premiere fois' })).toHaveCount(1);
});

test('leaving the mode from an organized view returns the Inbox and the classic nav', async () => {
  await page.locator('[data-testid="nav-folder"][data-category="feed"]').click();
  await page.locator('[data-testid="organized-mode"]').click();
  await expect(page.locator('[data-testid="organized-mode"]')).toHaveAttribute('aria-checked', 'false');
  await expect(page.locator('[data-testid="nav-folder"]')).toHaveCount(6);
  // Never an orphaned view: the category falls back to the Inbox.
  await expect(page.locator('[data-testid="row"]').first()).toBeVisible();
  // The cleanup returns the workstation to classic for the other specs.
  await page.evaluate(async () => {
    const invoke = window.__TAURI__.core.invoke;
    const routings = await invoke('routings');
    for (const r of routings) await invoke('remove_routing', { address: r.address });
  });
});
