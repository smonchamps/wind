// The display panes (PLAN-VOLETS E1): the Layout is chosen in
// Settings > Display — three panes (default, screen 02's grid) or two
// panes (full-width list, full-screen opening via screen 03, message
// alone included). Played on the Clarity decor.
//
// Hygiene: the WebView2 profile is SHARED between suites and gates —
// the `wind-volets` preference is cleared BEFORE AND AFTER, so that
// neither an interrupted run nor this suite pushes other screens out
// of the default.
import { test, expect } from '@playwright/test';
import { launchAppV2, closeApp, purgeLocals } from '../launch.mjs';

let app;
let browser;
let page;

test.describe.configure({ mode: 'serial' });

test.beforeAll(async () => {
  ({ app, browser, page } = await launchAppV2());
  // A previous interrupted run may have left a mode behind: return to
  // the default BEFORE any assertion.
  await purgeLocals(page, ['wind-volets', 'wind-largeurs']);
  await page.reload();
  await expect(page.locator('[data-testid="row"]').first()).toBeVisible();
});

test.afterAll(async () => {
  await purgeLocals(page, ['wind-volets', 'wind-largeurs']);
  await closeApp({ app, browser });
});

const folder = (category) =>
  page.locator(`[data-testid="nav-folder"][data-category="${category}"]`);

test('the default is three panes — the reading pane is there, nothing to set', async () => {
  await expect(page.locator('[data-testid="reading-pane"]')).toBeVisible();
  await page.locator('[data-testid="settings"]').click();
  await page
    .locator('[data-testid="settings-group"][data-group="affichage"]')
    .click();
  await expect(page.locator('[data-testid="display-panes"]')).toHaveValue('3');
  await page.locator('[data-testid="settings-done"]').click();
});

test('switching to two panes is immediate — reading leaves the grid', async () => {
  await page.locator('[data-testid="settings"]').click();
  await page
    .locator('[data-testid="settings-group"][data-group="affichage"]')
    .click();
  await page.locator('[data-testid="display-panes"]').selectOption('2');
  // Immediate application, no confirmation (the theme's gesture): the
  // pane disappears WHILE the overlay is still open.
  await expect(page.locator('[data-testid="reading-pane"]')).toHaveCount(0);
  await page.locator('[data-testid="settings-done"]').click();
  await expect(page.locator('[data-testid="row"]').first()).toBeVisible();
});

test("in two panes, opening is screen 03 — Escape leaves the list intact", async () => {
  const rows = page.locator('[data-testid="row"]');
  const before = await rows.count();
  await rows.filter({ hasText: 'Relecture du contrat Vantis' }).first().click();
  // Full screen: the conversation, NOT the pane.
  await expect(page.locator('[data-testid="thread-subject"]')).toContainText(
    'Relecture du contrat Vantis',
  );
  await expect(page.locator('[data-testid="reading-pane"]')).toHaveCount(0);
  await page.keyboard.press('Escape');
  await expect(page.locator('[data-testid="conversation"]')).toHaveCount(0);
  // The list is INTACT: same rows, the selection holds.
  await expect(rows).toHaveCount(before);
  await expect(
    rows.filter({ hasText: 'Relecture du contrat Vantis' }).first(),
  ).toHaveClass(/chosen/);
});

test("a message WITHOUT a thread opens full screen — the single-message fallback (V-D2)", async () => {
  // "Compte rendu du 4 août": a standalone message from the decor (no
  // conversation to open — Annex A's test asserts it at the pane).
  await page
    .locator('[data-testid="row"]', { hasText: 'Compte rendu du 4 août' })
    .click();
  await expect(page.locator('[data-testid="thread-subject"]')).toContainText(
    'Compte rendu du 4 août',
  );
  // The thread served is the row itself: ONE message, expanded, with
  // its real files (message_attachments). R2: name + size in one chip.
  await expect(page.locator('[data-testid="message-expanded"]')).toHaveCount(1);
  const convAttachment = page.locator('[data-testid="conversation"] [data-testid="attachment"]');
  await expect(convAttachment).toContainText('CR_04-08.pdf');
  await expect(convAttachment).toContainText('220 Ko');
  await page.locator('[data-testid="back-to-mailbox"]').click();
  await expect(page.locator('[data-testid="conversation"]')).toHaveCount(0);
});

test("a local echo opens full screen — local body, deferred gesture stated (V-D2)", async () => {
  // The offline contract from E3 (PLAN-REACTIVITE), replayed in two
  // panes: delete → echo in Trash → opening goes through screen 03 and
  // serves echo_body.
  await page
    .locator('[data-testid="row"]', { hasText: 'Facture 2026-0841' })
    .first()
    .click();
  await page.locator('[data-testid="delete"]').click();
  await expect(page.locator('[data-testid="conversation"]')).toHaveCount(0);
  await folder('trash').click();
  const echo = page.locator('[data-testid="row"]', { hasText: 'Facture 2026-0841' });
  await echo.click();
  await expect(page.locator('[data-testid="thread-subject"]')).toContainText(
    'Facture 2026-0841',
  );
  await expect(page.locator('[data-testid="message-expanded"]')).toHaveCount(1);
  // A gesture on the echo awaits reconciliation — and states it, here
  // too; the return to the mailbox is played by the existing wiring.
  await page.locator('[data-testid="delete"]').click();
  await expect(page.locator('[data-testid="toast"]')).toContainText(
    'Copie en cours de synchronisation',
  );
  await expect(page.locator('[data-testid="conversation"]')).toHaveCount(0);
  await folder('inbox').click();
});

test('in one pane, the nav leaves the grid and lives in a drawer (E2)', async () => {
  await page.locator('[data-testid="settings"]').click();
  await page
    .locator('[data-testid="settings-group"][data-group="affichage"]')
    .click();
  await page.locator('[data-testid="display-panes"]').selectOption('1');
  await page.locator('[data-testid="settings-done"]').click();
  // The nav is no longer in the grid; the drawer button is there.
  await expect(page.locator('[data-testid="nav"]')).toHaveCount(0);
  await expect(page.locator('[data-testid="btn-drawer"]')).toBeVisible();
  // Open: the Nav is THE SAME — real folders, real counts.
  await page.locator('[data-testid="btn-drawer"]').click();
  await expect(page.locator('[data-testid="drawer"]')).toBeVisible();
  await expect(folder('trash')).toBeVisible();
  // Choosing closes AND filters — the completed gesture no longer
  // needs the panel.
  await folder('trash').click();
  await expect(page.locator('[data-testid="drawer"]')).toHaveCount(0);
  await expect(page.locator('[data-testid="status"]')).toContainText('Corbeille');
});

test("Escape closes the drawer; leaving one-pane mode carries it away", async () => {
  await page.locator('[data-testid="btn-drawer"]').click();
  await expect(page.locator('[data-testid="drawer"]')).toBeVisible();
  await page.keyboard.press('Escape');
  await expect(page.locator('[data-testid="drawer"]')).toHaveCount(0);
  // Back to the inbox via the drawer, then to two-pane mode — the
  // drawer button fades with the mode, the nav returns to the grid
  // (the suite continues in mode 2, which the persistence test expects).
  await page.locator('[data-testid="btn-drawer"]').click();
  await folder('inbox').click();
  await page.locator('[data-testid="settings"]').click();
  await page
    .locator('[data-testid="settings-group"][data-group="affichage"]')
    .click();
  await page.locator('[data-testid="display-panes"]').selectOption('2');
  await page.locator('[data-testid="settings-done"]').click();
  await expect(page.locator('[data-testid="btn-drawer"]')).toHaveCount(0);
  await expect(page.locator('[data-testid="nav"]')).toBeVisible();
});

test('the preference survives a relaunch — and returning to three panes restores the pane', async () => {
  // Persistence: the reloaded page restores mode 2 BEFORE the first
  // render (no grid flash).
  await page.reload();
  await expect(page.locator('[data-testid="row"]').first()).toBeVisible();
  await expect(page.locator('[data-testid="reading-pane"]')).toHaveCount(0);
  // Return to three panes: the pane comes back, the value is re-read.
  await page.locator('[data-testid="settings"]').click();
  await page
    .locator('[data-testid="settings-group"][data-group="affichage"]')
    .click();
  await expect(page.locator('[data-testid="display-panes"]')).toHaveValue('2');
  await page.locator('[data-testid="display-panes"]').selectOption('3');
  await expect(page.locator('[data-testid="reading-pane"]')).toBeVisible();
  await page.locator('[data-testid="settings-done"]').click();
  // In three panes, the click opens IN the pane — no full screen.
  await page
    .locator('[data-testid="row"]', { hasText: 'Relecture du contrat Vantis' })
    .first()
    .click();
  await expect(page.locator('[data-testid="thread-subject"]')).toContainText(
    'Relecture du contrat Vantis',
  );
  await expect(page.locator('[data-testid="conversation"]')).toHaveCount(0);
});

test('panes resize with the mouse — bounds, persistence, double-click (PLAN-RETOURS-V3 R3)', async () => {
  // Chief Engineer verdict 2026-08-16 (D3): handles on BOTH borders in
  // three panes; nav bounds 180-400, list 300-640; widths persisted;
  // double-click = return to default.
  const width = (testid) =>
    page
      .locator(`[data-testid="${testid}"]`)
      .evaluate((el) => Math.round(el.getBoundingClientRect().width));
  const drag = async (testid, dx) => {
    const mailbox = await page.locator(`[data-testid="${testid}"]`).boundingBox();
    const x = mailbox.x + mailbox.width / 2;
    const y = mailbox.y + mailbox.height / 2;
    await page.mouse.move(x, y);
    await page.mouse.down();
    await page.mouse.move(x + dx, y, { steps: 4 });
    await page.mouse.up();
  };

  await expect.poll(() => width('list')).toBe(400);
  // The nav first, bounded down to 180 — it frees up the list's
  // ceiling (window 1000: 1000 - 180 - 120 thread reserve = 700).
  await drag('handle-nav', -500);
  await expect.poll(() => width('nav')).toBe(180);
  await drag('handle-list', 120);
  await expect.poll(() => width('list')).toBe(520);
  // The upper bound holds the handle back: 640, never beyond.
  await drag('handle-list', 500);
  await expect.poll(() => width('list')).toBe(640);
  // The window's CEILING holds the other border back (reviewed
  // 2026-08-16): nav max 400 would crush the thread under its
  // reserve — 1000 - 640 - 120 = 240, never beyond, the list handle
  // stays grabbable on screen.
  await drag('handle-nav', 500);
  await expect.poll(() => width('nav')).toBe(240);

  // Persistence: the reloaded page restores the widths BEFORE the
  // first render — written on RELEASE, never on pointermove.
  await page.reload();
  await expect(page.locator('[data-testid="row"]').first()).toBeVisible();
  await expect.poll(() => width('list')).toBe(640);
  await expect.poll(() => width('nav')).toBe(240);

  // Double-click: each border returns to its default.
  await page.locator('[data-testid="handle-list"]').dblclick();
  await expect.poll(() => width('list')).toBe(400);
  await page.locator('[data-testid="handle-nav"]').dblclick();
  await expect.poll(() => width('nav')).toBe(248);
});
