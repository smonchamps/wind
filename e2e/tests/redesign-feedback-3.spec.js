// PLAN-RETOURS-3 — the field findings played on the Clarity decor, in
// ISOLATION (their own instance): R4 (reply BY message, to a specific
// message of the thread) and R2 (report junk / the reverse).
// Named to run after redesign-screen02 (alphabetical order) — a single asset
// rebuild per gate.
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

test('R4/D4: replying targets the CHOSEN message of the thread, not the last', async () => {
  // The Vantis thread: m1 (our send, Paul) <- m2 (Sofia Nardi) <- m3
  // (Camille Rousseau). The per-message gesture must target THIS
  // message — replying to Sofia's message composes to Sofia, not to
  // Camille (the last one). This is the precision the finding asks
  // for.
  await expect(page.locator('[data-testid="row"]').first()).toBeVisible();
  await page
    .locator('[data-testid="row"]', { hasText: 'Relecture du contrat Vantis' }) // lang:fr
    .first()
    .click();
  await expect(
    page.locator('[data-testid="reading-pane"] [data-testid="thread-subject"]'),
  ).toHaveText('Relecture du contrat Vantis'); // lang:fr

  // Expand everything: the three messages, each with its own reply
  // bar AT THE BOTTOM.
  await page.locator('[data-testid="reading-pane"] [data-testid="all-expand"]').click();
  const messages = page.locator('[data-testid="reading-pane"] [data-testid="message-expanded"]');
  await expect(messages).toHaveCount(3);
  // Chronological order (oldest first): m2 (Sofia) in the middle.
  await expect(messages.nth(1)).toContainText('Sofia Nardi');

  // Reply TO Sofia's message (the middle one) — her bar, not the last
  // one's.
  await messages.nth(1).locator('[data-testid="reply"]').click();
  await expect(page.locator('[data-testid="compose-kicker"]')).toHaveText('Reply');
  await expect(page.locator('[data-testid="compose-to"]')).toHaveValue(
    's.nardi@atelier-nord.fr',
  );
  await page.locator('[data-testid="compose-cancel"]').click();
  await expect(page.locator('[data-testid="compose"]')).toHaveCount(0);

  // Field finding (2026-08-18): OUR OWN message (m1, Paul, first)
  // ALSO carries the three gestures, and replying to it targets the
  // original recipients (Camille in To), never oneself (Paul).
  await expect(messages.nth(0)).toContainText('Paul Mérand'); // lang:fr
  await expect(
    messages.nth(0).locator('[data-testid="reply"]'),
  ).toBeVisible();
  await expect(
    messages.nth(0).locator('[data-testid="reply-all"]'),
  ).toBeVisible();
  await messages.nth(0).locator('[data-testid="reply"]').click();
  await expect(page.locator('[data-testid="compose-kicker"]')).toHaveText('Reply');
  await expect(page.locator('[data-testid="compose-to"]')).toHaveValue(
    'c.rousseau@atelier-nord.fr',
  );
  await page.locator('[data-testid="compose-cancel"]').click();
  await expect(page.locator('[data-testid="compose"]')).toHaveCount(0);

  // Reply-all on our own message: the original To (Camille), the
  // original Cc STAYS in Cc (Sofia) — never oneself.
  await messages.nth(0).locator('[data-testid="reply-all"]').click();
  await expect(page.locator('[data-testid="compose-to"]')).toHaveValue(
    'c.rousseau@atelier-nord.fr',
  );
  await expect(page.locator('[data-testid="compose-cc"]')).toHaveValue(
    's.nardi@atelier-nord.fr',
  );
  await page.locator('[data-testid="compose-cancel"]').click();
  await expect(page.locator('[data-testid="compose"]')).toHaveCount(0);
});

test('R2/D2: reporting a message as junk removes it from the Inbox', async () => {
  await folder('inbox').click();
  // "Atelier de septembre" (Sofia, work account — which has a Spam
  // folder): a standalone message, simple case.
  const row = page.locator('[data-testid="row"]', { hasText: 'Atelier de septembre' });
  await expect(row.first()).toBeVisible();
  await row.first().click();
  await expect(
    page.locator('[data-testid="reading-pane"] [data-testid="thread-subject"]'),
  ).toHaveText('Atelier de septembre');

  // The thread bar carries "Report as spam" (Inbox view).
  await page.locator('[data-testid="reading-pane"] [data-testid="report-spam"]').click();
  await expect(page.locator('[data-testid="toast"]')).toContainText(
    'Reported as spam',
  );
  // Optimistic disappearance: the Inbox no longer carries a trace of it.
  await expect(
    page.locator('[data-testid="row"]', { hasText: 'Atelier de septembre' }),
  ).toHaveCount(0);
});

test('R2/D2: "Not spam" brings a message back to the Inbox', async () => {
  await folder('junk').click();
  const row = page.locator('[data-testid="row"]', { hasText: 'Vous avez gagné' }); // lang:fr
  await expect(row.first()).toBeVisible();
  await row.first().click();
  await expect(
    page.locator('[data-testid="reading-pane"] [data-testid="thread-subject"]'),
  ).toHaveText('Vous avez gagné'); // lang:fr

  // In the Junk view, the bar switches to "Not spam" —
  // and "Report as spam" is no longer there.
  await expect(
    page.locator('[data-testid="reading-pane"] [data-testid="report-spam"]'),
  ).toHaveCount(0);
  await page.locator('[data-testid="reading-pane"] [data-testid="not-spam"]').click();
  await expect(page.locator('[data-testid="toast"]')).toContainText('Moved back to Inbox');
  await expect(
    page.locator('[data-testid="row"]', { hasText: 'Vous avez gagné' }), // lang:fr
  ).toHaveCount(0);
});
