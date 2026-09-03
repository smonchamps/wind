// PLAN-RETOURS-6: signatures, deferred send, "important", composer
// header — the four Chief Engineer findings from 2026-08-21, played on
// the Clarity decor. The decor's accounts carry an invalid token: the
// outbox journals without ever sending anything — the deadline and the
// journal are read via `outbox_status`, as in the field.
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

const invoke = (cmd, args) =>
  page.evaluate(([c, a]) => window.__TAURI__.core.invoke(c, a), [cmd, args ?? {}]);

const bg = (loc) => loc.evaluate((el) => getComputedStyle(el).backgroundColor);

test("R4: the composer header carries Wind's footer background (A66)", async () => {
  await expect(page.locator('[data-testid="row"]').first()).toBeVisible();
  await page.locator('[data-testid="write"]').click();
  await expect(page.locator('[data-testid="compose-kicker"]')).toBeVisible();
  // The same token (--bg since V3 — --panel is dead) on both sides: the
  // comparison is made on the COMPUTED color — a theme that shifts does
  // not break the test.
  const head = await bg(page.locator('[data-testid="compose"] .head'));
  const footer = await bg(page.locator('[data-testid="status"]'));
  expect(head).toBe(footer);
  await page.locator('[data-testid="compose-cancel"]').click();
  await expect(page.locator('[data-testid="compose"]')).toHaveCount(0);
});

test('R1: the signature is set in Settings, appears on a new message — and closing without typing sows nothing', async () => {
  // Set the first account's signature, via the real surface.
  await page.locator('[data-testid="settings"]').click();
  await page.locator('[data-testid="settings-group"][data-group="signature"]').click();
  const editor = page.locator('[data-testid="signature-editor"]').first();
  await expect(editor).toBeVisible();
  await editor.click();
  await editor.pressSequentially('Cordialement, Léa'); // lang:fr
  await page.locator('[data-testid="signature-save"]').first().click();
  await expect(page.locator('[data-testid="signature-state"]').first()).toContainText(
    'Signature saved.',
  );
  // Field finding 2026-08-21: "Apply to all accounts" copies the
  // signature AND its scope — and it SHOWS on the 2nd account's block.
  await page.locator('[data-testid="signature-all"]').first().click();
  await expect(page.locator('[data-testid="signature-state"]').first()).toContainText(
    'applied to all accounts',
  );
  await expect(page.locator('[data-testid="signature-editor"]').nth(1)).toContainText(
    'Cordialement, Léa', // lang:fr
  );
  // Then we CLEAR the 2nd account's signature: the reload on sender
  // account change, below, must show.
  await page.locator('[data-testid="signature-clear"]').nth(1).click();
  await expect(page.locator('[data-testid="signature-editor"]').nth(1)).toHaveText('');
  await page.locator('[data-testid="settings-done"]').click();

  // A new message carries it, under two blank lines.
  const before = (await invoke('list_drafts')).length;
  await page.locator('[data-testid="write"]').click();
  await expect(page.locator('[data-testid="compose-body"]')).toContainText(
    'Cordialement, Léa', // lang:fr
  );
  // Field finding 2026-08-21: the signature FOLLOWS the sender account
  // — the 2nd account (signature cleared) empties the body it carried.
  const fromSelect = page.locator('select[data-testid="compose-from"]');
  const emails = await fromSelect.locator('option').allTextContents();
  await fromSelect.selectOption(emails[1]);
  await expect(page.locator('[data-testid="compose-body"]')).not.toContainText(
    'Cordialement, Léa', // lang:fr
  );
  await fromSelect.selectOption(emails[0]);
  await expect(page.locator('[data-testid="compose-body"]')).toContainText(
    'Cordialement, Léa', // lang:fr
  );
  // Closing WITHOUT TYPING: the signature alone does not make a
  // draft — no ghost sown on every open (anti-churn guard).
  await page.locator('[data-testid="compose-cancel"]').click();
  await expect(page.locator('[data-testid="compose"]')).toHaveCount(0);
  expect((await invoke('list_drafts')).length).toBe(before);

  // D4, default scope "new messages only": a reply does NOT carry the
  // signature until the account has opted it in.
  await page
    .locator('[data-testid="row"]', { hasText: 'Relecture du contrat Vantis' }) // lang:fr
    .click();
  await page.locator('[data-testid="reply"]').first().click();
  await expect(page.locator('[data-testid="compose-kicker"]')).toHaveText('Reply');
  await expect(page.locator('[data-testid="compose-body"]')).not.toContainText(
    'Cordialement, Léa', // lang:fr
  );
  await page.locator('[data-testid="compose-cancel"]').click();
  await expect(page.locator('[data-testid="compose"]')).toHaveCount(0);

  // Scope enabled on account 1: the reply carries the signature
  // between the lead-in and the quote — and switching accounts
  // recomposes it WITHOUT losing the quote (field finding 2026-08-21,
  // 2nd pass).
  await page.locator('[data-testid="settings"]').click();
  await page.locator('[data-testid="settings-group"][data-group="signature"]').click();
  await page.locator('[data-testid="signature-replies"]').first().click();
  await expect(
    page.locator('[data-testid="signature-replies"]').first(),
  ).toHaveAttribute('aria-checked', 'true');
  await page.locator('[data-testid="settings-done"]').click();
  // The last message of the Vantis thread carries a body (decor's
  // contract, same target as screen02's reply journey).
  await page.locator('[data-testid="reply"]').last().click();
  const body = page.locator('[data-testid="compose-body"]');
  await expect(body).toContainText('Cordialement, Léa'); // lang:fr
  await expect(body).toContainText('a écrit :'); // lang:fr
  const fromSelect2 = page.locator('select[data-testid="compose-from"]');
  const emails2 = await fromSelect2.locator('option').allTextContents();
  await fromSelect2.selectOption(emails2[1]);
  // Account 2: no signature (cleared) — the quote, though, stays.
  await expect(body).not.toContainText('Cordialement, Léa'); // lang:fr
  await expect(body).toContainText('a écrit :'); // lang:fr
  await page.locator('[data-testid="compose-cancel"]').click();
  await expect(page.locator('[data-testid="compose"]')).toHaveCount(0);
});

test("R2: a scheduled send waits for its time, states itself, and cancels back to draft (D1/D2)", async () => {
  await page.locator('[data-testid="write"]').click();
  await page.locator('[data-testid="compose-to"]').fill('dest@exemple.fr');
  await page.locator('[data-testid="compose-subject"]').fill('Départ programmé'); // lang:fr
  await page.locator('[data-testid="compose-later"]').click();
  // The card states the local semantics (D1) and presets +1 h.
  await expect(page.locator('[data-testid="compose-deferred"]')).toContainText(
    'if Wind is open',
  );
  await page.locator('[data-testid="compose-deferred-confirm"]').click();
  await expect(page.locator('[data-testid="compose"]')).toHaveCount(0);
  // The toast states the DEADLINE, never "sent" — nothing has left.
  await expect(page.locator('[data-testid="toast"]')).toContainText('Send scheduled');

  // The journal carries the deadline, apart from "pending" ones.
  const status = await invoke('outbox_status');
  expect(status.scheduled).toBe(1);
  expect(status.queued).toBe(0);
  const entry = status.entries.find((e) => e.subject === 'Départ programmé'); // lang:fr
  expect(entry.send_at_epoch).toBeGreaterThan(Math.floor(Date.now() / 1000));

  // The status bar states it (10 s probe), and the slot offers cancellation.
  await expect(page.locator('[data-testid="progress"]')).toContainText('scheduled');
  const slot = page.locator('[data-testid="slot-notice"]');
  await expect(slot).toContainText('Départ programmé'); // lang:fr
  await slot.getByRole('button', { name: 'Cancel send' }).click();
  await expect(page.locator('[data-testid="toast"]')).toContainText('Send cancelled');

  // D2: the draft is BACK, the journal is empty.
  const after = await invoke('outbox_status');
  expect(after.scheduled).toBe(0);
  expect(after.entries.length).toBe(0);
  const drafts = await invoke('list_drafts');
  expect(drafts.some((b) => b.subject === 'Départ programmé')).toBe(true); // lang:fr
});

test('R3: "Important" marks itself, follows the resumed draft, and reaches the journal on send', async () => {
  await page.locator('[data-testid="write"]').click();
  const brand = page.locator('[data-testid="compose-important"]');
  await expect(brand).toHaveAttribute('aria-pressed', 'false');
  await brand.click();
  await expect(brand).toHaveAttribute('aria-pressed', 'true');
  await page.locator('[data-testid="compose-to"]').fill('dest@exemple.fr');
  await page.locator('[data-testid="compose-subject"]').fill('Marqué important'); // lang:fr
  await page.locator('[data-testid="compose-draft"]').click();
  await expect(page.locator('[data-testid="compose"]')).toHaveCount(0);

  // Resuming restores the marking — the state lives on the draft. (The
  // Drafts folder has ITS OWN testid: `row-draft`, not `row`.)
  await page
    .locator('[data-testid="nav-folder"][data-category="drafts"]')
    .click();
  await page
    .locator('[data-testid="row-draft"]', { hasText: 'Marqué important' }) // lang:fr
    .click();
  await expect(page.locator('[data-testid="compose-important"]')).toHaveAttribute(
    'aria-pressed',
    'true',
  );
  // Sending journals (offline account: it stays queued) — the journal's
  // marking and the SMTP headers are proven on the Rust side.
  await page.locator('[data-testid="compose-send"]').click();
  await expect(page.locator('[data-testid="compose"]')).toHaveCount(0);
  await expect(page.locator('[data-testid="toast"]')).toContainText('Message sent.');
});
