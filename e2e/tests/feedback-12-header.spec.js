// PLAN-RETOURS-12 R5 — the expanded message's header, in two lines:
//   line 1: Sender's name <address> [on Mailbox — rule D7]
//   line 2: To: Name <address>, … (and "Cc: …" if there are Cc's, D6)
//
// Recipient names come from the correspondents DIRECTORY
// (decision D4 — to_addrs only stores bare addresses); an unknown
// address displays bare. Clarity decor: Camille and Sofia are
// senders that have been seen, so the directory knows their names.
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

test('the expanded header says "Name <address>" then "To: Name <address>" — the name comes from the directory', async () => {
  await page.locator('[data-testid="row"]').first().click();
  const pane = page.locator('[data-testid="reading-pane"]');
  const expanded = pane.locator('[data-testid="message-expanded"]');

  // Line 1 — the sender of the Vantis thread's last message, name AND
  // address on the same line.
  await expect(expanded.locator('.author')).toHaveText('Camille Rousseau');
  await expect(expanded.locator('.addr-sender')).toHaveText('<c.rousseau@atelier-nord.fr>');

  // Line 2 — "To:"; with no stored recipients (old mail), the
  // fallback is the prototype's heuristic, in the SAME Name <address> format.
  await expect(expanded.locator('[data-testid="row-to"]')).toHaveText(
    'À : Paul Mérand <paul.merand@atelier-nord.fr>',
  );
  // No Cc on this message: the line doesn't exist.
  await expect(expanded.locator('[data-testid="row-cc"]')).toHaveCount(0);
  // The long time doesn't move.
  await expect(expanded.locator('.message-head .when')).toHaveText(/^Aujourd'hui, 09:12$/);
});

test('our own message states its stored recipients, names resolved — and its Cc line (D6)', async () => {
  const pane = page.locator('[data-testid="reading-pane"]');
  // Expand our reply (PM avatar, the first collapsed card).
  await pane.locator('[data-testid="message-collapsed"]').first().click();
  const ours = pane.locator('[data-testid="message-expanded"]').first();

  await expect(ours.locator('.author')).toHaveText('Paul Mérand');
  await expect(ours.locator('.addr-sender')).toHaveText('<paul.merand@atelier-nord.fr>');
  // Stored To/Cc (R4) + names from the directory (both are
  // senders that have been seen in the decor).
  await expect(ours.locator('[data-testid="row-to"]')).toHaveText(
    'À : Camille Rousseau <c.rousseau@atelier-nord.fr>',
  );
  await expect(ours.locator('[data-testid="row-cc"]')).toHaveText(
    'Cc : Sofia Nardi <s.nardi@atelier-nord.fr>',
  );
});
