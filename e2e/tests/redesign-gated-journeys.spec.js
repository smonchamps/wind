// R2 (PLAN-RETRAIT-V1): the journeys from the old interface ported
// to v2, with the EXACT seed of the original specs (seed_inbox,
// 200 messages: one thread in five, one attachment in ten). This file
// REPLACED parcours-critiques / recherche / multi-comptes at B2
// (2026-08-15) — they have run on v2 since R2, only the name still
// recalled v1, hence the rename.
//
// Deliberate drops (PASSATION §2.6):
// - "star (s)" and "move (v)" fall with D2 — cut at the switchover,
//   core commands kept, reversible by a short spec;
// - auto-advance after archiving (v1 opened the next message) was
//   first an accepted gap A6 (the prototype closes the pane), then
//   RESTORED by A38 (2026-08-15) to the shortcut alone: e/Del advance
//   the selection to the row below — covered by the redesign-screen02 spec;
// - "two drafts with the same subject distinct in the body": covered
//   by nature since PLAN-BROUILLONS — the Drafts folder shows LOCAL
//   drafts and their preview is distinguished in the body, without
//   network.
import { test, expect } from '@playwright/test';
import { launchAppV2, closeApp } from '../launch.mjs';

test.describe.configure({ mode: 'serial' });

test.describe('v1 decor: one account, 200 messages', () => {
  let app;
  let browser;
  let page;

  test.beforeAll(async () => {
    ({ app, browser, page } = await launchAppV2({
      accounts: [{ email: 'e2e@exemple.fr', messages: 200 }],
    }));
  });

  test.afterAll(async () => {
    await closeApp({ app, browser });
  });

  test("read: the list is displayed, most recent first, the body opens in an iframe", async () => {
    await expect(page.locator('[data-testid="row"]').first()).toBeVisible();
    await expect(page.locator('[data-testid="row"]').first()).toContainText('n°200');
    // 200 messages, one thread in five: 160 conversations. The total has
    // left the nav (A29, W2-D4) — it is read on the perf line.
    await expect(page.locator('[data-testid="perf"]')).toContainText('160 conversations');
    // No stray notice at launch (update/telemetry neutralized §7.5).
    await expect(page.locator('[data-testid="slot-notice"]')).toHaveCount(0);

    await page.locator('[data-testid="row"]').first().click();
    await expect(page.locator('[data-testid="reading-pane"] [data-testid="thread-subject"]')).toContainText('n°200');
    await expect(
      page.frameLocator('[data-testid="reading-pane"] iframe').locator('body'),
    ).toContainText('Corps du message n°200'); // lang:fr
  });

  test('sort: "e" archives the selection, the list follows', async () => {
    await page.keyboard.press('e');
    await expect(page.locator('[data-testid="toast"]')).toContainText('Conversation archived.');
    // #200 was replying to #199: the head of the thread becomes #199.
    await expect(page.locator('[data-testid="row"]').first()).toContainText('n°199');
  });

  test('reply: real prefills, offline send LOGGED, never lost', async () => {
    await page.locator('[data-testid="row"]').first().click();
    await page.keyboard.press('r');
    await expect(page.locator('[data-testid="compose-kicker"]')).toHaveText('Reply');
    await expect(page.locator('[data-testid="compose-to"]')).toHaveValue(/@exemple\.fr$/);
    // Form from the prototype ("Re: "), real quote from the core.
    await expect(page.locator('[data-testid="compose-subject"]')).toHaveValue(/^Re: /);
    // The editor is rich (PLAN-COMPOSITION-HTML): the quote lives in
    // a blockquote, no longer in "> " prefixes — the text is read at the node.
    const body = page.locator('[data-testid="compose-body"]');
    await expect(body).toContainText('a écrit :'); // lang:fr
    await expect(body).toContainText('Corps du message n°199'); // lang:fr
    await expect(body.locator('blockquote')).toContainText('Corps du message n°199'); // lang:fr

    const cite = await body.innerText();
    await body.fill(`Réponse E2E.\n${cite}`); // lang:fr
    await page.locator('[data-testid="compose-send"]').click();
    await expect(page.locator('[data-testid="compose"]')).toHaveCount(0);
    await expect(page.locator('[data-testid="toast"]')).toContainText('Message sent.');
    // Offline by construction: the golden rule, VISIBLE — the
    // blameless wait lives on the progress line (10 s probe).
    await expect(page.locator('[data-testid="progress"]')).toContainText(
      'Outbox · 1 message waiting',
    );
  });

  test('draft: Escape keeps it, the Drafts folder restores it intact', async () => {
    await page.keyboard.press('c');
    await page.locator('[data-testid="compose-subject"]').fill('Brouillon E2E');
    await page.locator('[data-testid="compose-body"]').fill('Texte précieux.'); // lang:fr
    await page.keyboard.press('Escape'); // leave the field…
    await page.keyboard.press('Escape'); // …close: keep, never discard
    await expect(page.locator('[data-testid="compose"]')).toHaveCount(0);
    await expect(page.locator('[data-testid="toast"]')).toContainText('Draft saved.');

    // No more slot (PLAN-BROUILLONS): the draft lives IN THE FOLDER —
    // without a recipient, the dimmed text says so — and the click reopens it INTACT.
    await expect(page.locator('[data-testid="slot-notice"]')).toHaveCount(0);
    await page.locator('[data-testid="nav-folder"][data-category="drafts"]').click();
    const draftRow = page.locator('[data-testid="row-draft"]', { hasText: 'Brouillon E2E' });
    await expect(draftRow).toContainText('(no recipient)');
    await draftRow.click();
    await expect(page.locator('[data-testid="compose-subject"]')).toHaveValue('Brouillon E2E');
    await expect(page.locator('[data-testid="compose-body"]')).toHaveText('Texte précieux.'); // lang:fr
    // Empty then close: the only case where closing deletes — the row
    // leaves the folder without waiting for the probe.
    await page.locator('[data-testid="compose-subject"]').fill('');
    // `fill('')` on a contenteditable is a Chromium no-op (empty insertText
    // does not delete the selection): we empty it like the user would —
    // select all, delete.
    await page.locator('[data-testid="compose-body"]').click();
    await page.keyboard.press('Control+a');
    await page.keyboard.press('Delete');
    await page.locator('[data-testid="compose-cancel"]').click();
    await expect(page.locator('[data-testid="row-draft"]')).toHaveCount(0);
    // Back to Inbox: the rest of the serial chain plays out on the
    // mailbox.
    await page.locator('[data-testid="nav-folder"][data-category="inbox"]').click();
    await expect(page.locator('[data-testid="row"]').first()).toBeVisible();
  });

  test('formatting: bold applies, the RICH draft survives reopening (PLAN-COMPOSITION-HTML)', async () => {
    await page.keyboard.press('c');
    await page.locator('[data-testid="compose-subject"]').fill('Brouillon riche E2E');
    const body = page.locator('[data-testid="compose-body"]');
    await body.click();
    await page.keyboard.type('mot');
    // Select all INSIDE the editor, apply Bold: the output is
    // ammonia's vocabulary (<b>), the active state is stated (aria-pressed).
    await page.keyboard.press('Control+a');
    await page.locator('[data-testid="compose-format-bold"]').click();
    await expect(page.locator('[data-testid="compose-format-bold"]')).toHaveAttribute(
      'aria-pressed',
      'true',
    );
    expect(await body.evaluate((n) => n.innerHTML)).toContain('<b>');

    // "Clear formatting" cleans up — then bold is reapplied to
    // prove SURVIVAL across the draft round-trip (sanitized on the Rust side).
    await page.locator('[data-testid="compose-format-clear"]').click();
    expect(await body.evaluate((n) => n.innerHTML)).not.toContain('<b>');
    await page.locator('[data-testid="compose-format-bold"]').click();
    await page.keyboard.press('Escape'); // leave the field…
    await page.keyboard.press('Escape'); // …close: keep
    await expect(page.locator('[data-testid="toast"]')).toContainText('Draft saved.');

    await page.locator('[data-testid="nav-folder"][data-category="drafts"]').click();
    const draftRow = page.locator('[data-testid="row-draft"]', {
      hasText: 'Brouillon riche E2E',
    });
    await draftRow.click();
    await expect(page.locator('[data-testid="compose-subject"]')).toHaveValue(
      'Brouillon riche E2E',
    );
    await expect(body).toHaveText('mot');
    expect(await body.evaluate((n) => n.innerHTML)).toContain('<b>');

    // Cleanup: empty then close — the draft leaves the folder.
    // (Same trap: `fill('')` does not empty a contenteditable.)
    await page.locator('[data-testid="compose-subject"]').fill('');
    await body.click();
    await page.keyboard.press('Control+a');
    await page.keyboard.press('Delete');
    await page.locator('[data-testid="compose-cancel"]').click();
    await expect(page.locator('[data-testid="row-draft"]')).toHaveCount(0);
    await page.locator('[data-testid="nav-folder"][data-category="inbox"]').click();
    await expect(page.locator('[data-testid="row"]').first()).toBeVisible();
  });

  test('attachments: the row carries its chips, the reading pane states the attachments (PLAN-RETOURS-V3 R1)', async () => {
    // #190 (thread 189+190, attachment on 190). A29's "bare row" is
    // REVERSED (Chief Engineer verdict 2026-08-16, D1/D2): the row carries the
    // prototype's chip rank — the Thread head rules ("N
    // messages" if the thread has more than one, "N files" if there are
    // attachments), CONTENT-HUGGING HEIGHT (Chief Engineer field visit the
    // same day, reverses D1): the rank only exists on carrier rows and
    // enlarges their row.
    const carrier = page.locator('[data-testid="row"]', { hasText: 'n°190' }).first();
    await expect(carrier.locator('[data-testid="chips-row"]')).toContainText('2 messages');
    await expect(carrier.locator('[data-testid="chips-row"]')).toContainText('1 file');
    // A row WITHOUT chips has no rank at all — it is
    // shorter: two templates, the windowing mechanics from before A29.
    const bare = page.locator('[data-testid="row"]', { hasText: 'n°198' }).first();
    await expect(bare.locator('[data-testid="chips-row"]')).toHaveCount(0);
    expect(await bare.evaluate((el) => el.offsetHeight)).toBeLessThan(
      await carrier.evaluate((el) => el.offsetHeight),
    );

    await carrier.click();
    await expect(page.locator('[data-testid="reading-pane"] [data-testid="thread-subject"]')).toContainText('n°190');
    // UI v3: the pane shows the thread — two separate chips per the
    // mockup's template ("2 messages" · "1 file"), no longer the old
    // composed chip.
    await expect(page.locator('[data-testid="reading-pane"]')).toContainText('2 messages');
    await expect(page.locator('[data-testid="reading-pane"]')).toContainText('1 file');
  });

  test('conversations: one row per thread, counter, full-screen navigable exchange', async () => {
    // #189 has no row of its own: #190 represents the thread.
    await expect(
      page.locator('[data-testid="row"]', { hasText: 'n°189' }),
    ).toHaveCount(0);
    await page.locator('[data-testid="see-conversation"]').click();
    await expect(page.locator('[data-testid="conversation"] [data-testid="thread-subject"]')).toContainText('n°190');
    await expect(page.locator('[data-testid="message-expanded"]')).toHaveCount(1);
    await expect(page.locator('[data-testid="message-collapsed"]')).toHaveCount(1);
    await expect(page.locator('[data-testid="message-expanded"]')).toContainText('facture-190.pdf');

    // Expand the oldest without leaving the thread. The proof lives in the
    // BODY (iframe S1): since A45 the card header no longer repeats
    // the subject — the card's From/To/Subject block has disappeared (mockup).
    await page.locator('[data-testid="message-collapsed"]').click();
    await expect(page.locator('[data-testid="message-expanded"]')).toHaveCount(2);
    await expect(
      page.locator('[data-testid="message-expanded"]').first().frameLocator('iframe').locator('body'),
    ).toContainText('n°189');
    await page.locator('[data-testid="back-to-mailbox"]').click();
  });

  test('search: "/" focuses, the results serve, archiving removes it (regression #4)', async () => {
    await page.keyboard.press('/');
    await expect(page.locator('[data-testid="search-field"]')).toBeFocused();
    await page.locator('[data-testid="search-field"]').fill('facture');
    const results = page.locator('[data-testid="results"] [data-testid="row"]');
    await expect(results.first()).toBeVisible();

    const before = await results.count();
    expect(before).toBeGreaterThan(0);
    const archive = await results.first().locator('.subject').textContent();

    // Archive the first result WITHOUT leaving search.
    await results.first().click();
    await page.keyboard.press('e');
    await expect(page.locator('[data-testid="toast"]')).toContainText('Conversation archived.');
    await expect(results).toHaveCount(before - 1);
    await expect(page.locator('[data-testid="results"]')).not.toContainText(archive);

    // Escape returns the mailbox as it was.
    await page.locator('[data-testid="search-field"]').press('Escape');
    await expect(page.locator('[data-testid="results"]')).toHaveCount(0);
    await expect(page.locator('[data-testid="row"]').first()).toBeVisible();
  });

  test('draft: delete it from compose, with confirmation (PLAN-RETOURS-3 R3)', async () => {
    // A draft persisted to the folder, reopened, then DISCARDED from the
    // compose window — an explicit destructive gesture, distinct
    // from "Cancel" which keeps it. D3: an irreversible action goes through
    // a confirmation before it happens.
    await page.keyboard.press('c');
    await page.locator('[data-testid="compose-subject"]').fill('Brouillon a jeter');
    await page.locator('[data-testid="compose-body"]').fill('Contenu jetable.');
    await page.keyboard.press('Escape'); // leave the field…
    await page.keyboard.press('Escape'); // …close: keeps
    await expect(page.locator('[data-testid="toast"]')).toContainText('Draft saved.');

    await page.locator('[data-testid="nav-folder"][data-category="drafts"]').click();
    const draftRow = page.locator('[data-testid="row-draft"]', { hasText: 'Brouillon a jeter' });
    await expect(draftRow).toBeVisible();
    await draftRow.click();
    await expect(page.locator('[data-testid="compose-subject"]')).toHaveValue('Brouillon a jeter');

    // First click: the confirmation arms, NOTHING is deleted yet.
    await page.locator('[data-testid="compose-delete"]').click();
    await expect(page.locator('[data-testid="compose-delete-confirm"]')).toBeVisible();
    // Cancel the confirmation: the draft stays, the window stays.
    await page.locator('[data-testid="compose-delete-cancel"]').click();
    await expect(page.locator('[data-testid="compose-delete-confirm"]')).toHaveCount(0);
    await expect(page.locator('[data-testid="compose"]')).toBeVisible();

    // This time we confirm: the window closes, the draft leaves
    // the folder for good.
    await page.locator('[data-testid="compose-delete"]').click();
    await page.locator('[data-testid="compose-delete-confirm"]').click();
    await expect(page.locator('[data-testid="compose"]')).toHaveCount(0);
    await expect(page.locator('[data-testid="toast"]')).toContainText('Draft deleted.');
    await expect(
      page.locator('[data-testid="row-draft"]', { hasText: 'Brouillon a jeter' }),
    ).toHaveCount(0);

    // Back to Inbox for the rest of the serial chain.
    await page.locator('[data-testid="nav-folder"][data-category="inbox"]').click();
    await expect(page.locator('[data-testid="row"]').first()).toBeVisible();
  });
});

test.describe('v1 decor: two accounts merged', () => {
  let app;
  let browser;
  let page;

  test.beforeAll(async () => {
    ({ app, browser, page } = await launchAppV2({
      accounts: [
        { email: 'un@exemple.fr', messages: 30 },
        { email: 'deux@exemple.fr', messages: 20 },
      ],
    }));
  });

  test.afterAll(async () => {
    await closeApp({ app, browser });
  });

  test('unified mailbox: merge by date, one nav entry per real account', async () => {
    await expect(page.locator('[data-testid="nav-mailbox"]')).toHaveCount(3);
    await expect(page.locator('[data-testid="row"]').first()).toContainText('n°30');
  });

  test("reply from the unified mailbox: the message's account is the sender — and is selectable (A10)", async () => {
    await page.locator('[data-testid="row"]').first().click();
    await page.keyboard.press('r');
    const from = page.locator('[data-testid="compose-from"]');
    await expect(from).toHaveValue('un@exemple.fr');
    await expect(from.locator('option')).toHaveCount(2);
    await page.locator('[data-testid="compose-cancel"]').click();
  });
});
