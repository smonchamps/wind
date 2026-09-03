// PLAN-RETOURS-7: the four Chief Engineer findings from 2026-08-21,
// played on the Clarity decor — (R1) hovering an attachment STATES the
// action, (R2) attached files BEFORE the body, (R3) screen 03 flat
// like the pane (A46 extended), (R4) pinning a conversation to the top
// of the Inbox (D3-D5).
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

test('attached files live BEFORE the message body (R2)', async () => {
  // The Vantis thread: its last message carries Contrat_Vantis_v4.pdf.
  await page.locator('[data-testid="row"]').first().click();
  const expanded = page.locator(
    '[data-testid="reading-pane"] [data-testid="message-expanded"]',
  );
  const files = expanded.locator('[data-testid="reading-files"]');
  await expect(files).toContainText('Contrat_Vantis_v4.pdf');
  // The ORDER proof (no test carried it): the files section PRECEDES
  // the body's iframe in the message's flow.
  const before = await expanded.evaluate((el) => {
    const files = el.querySelector('[data-testid="reading-files"]');
    const body = el.querySelector('iframe');
    return Boolean(
      files.compareDocumentPosition(body) & Node.DOCUMENT_POSITION_FOLLOWING,
    );
  });
  expect(before).toBe(true);
});

// (An ECHO's chips stay inert and without a veil — the veil is not
// rendered on an echo, and their inertness is already guarded by "an
// outbound echo states its recipients and its attachment",
// redesign-screen02.)
test('a body the core does not serve states itself and replays (PLAN-AUDIT-V2 E10)', async () => {
  // The next `message_body` fails (__e2eFailure seam): the frame
  // states the failure and offers "Réessayer" — before, an empty
  // frame forever.
  await page.evaluate(() => {
    window.__e2eFailure = ['message_body'];
  });
  await page.locator('[data-testid="row"]').nth(1).click();
  const failure = page.locator('[data-testid="reading-pane"] [data-testid="body-failure"]');
  await expect(failure).toBeVisible();
  await page.locator('[data-testid="body-retry"]').click();
  await expect(failure).toHaveCount(0);
  await expect(
    page.frameLocator('[data-testid="reading-pane"] [data-testid="message-expanded"] iframe').first().locator('body'),
  ).not.toBeEmpty();
  await page.locator('[data-testid="row"]').first().click();
  await expect(page.locator('[data-testid="reading-pane"] [data-testid="reading-files"]')).toBeVisible();
});

test('hovering an attachment states "Enregistrer" (R1, D1)', async () => {
  const attachment = page
    .locator('[data-testid="reading-files"] [data-testid="attachment"]')
    .first();
  const veil = attachment.locator('.veil');
  // At rest: the chip states the file, the veil does not exist to the eye.
  await expect.poll(() => veil.evaluate((el) => getComputedStyle(el).display)).toBe('none');
  // On hover: the veil COVERS the chip — a download glyph + the
  // product's word (D1: "Enregistrer", the click opens "Enregistrer
  // sous") — without changing its geometry (the row does not reflow).
  const widthBefore = await attachment.evaluate((el) => el.offsetWidth);
  await attachment.hover();
  // (`inline-flex` set is computed as `flex`: the absolute
  // blockifies — we assert presence, not the value.)
  // PLAN-AUDIT-V2 E9: retried — the veil follows the hover, not the instant.
  await expect.poll(() => veil.evaluate((el) => getComputedStyle(el).display)).not.toBe('none');
  await expect(veil).toContainText('Enregistrer');
  await expect(veil.locator('.ic')).toHaveAttribute('data-name', 'download');
  await expect.poll(() => attachment.evaluate((el) => el.offsetWidth)).toBe(widthBefore);
  // Leaving: the veil retreats.
  await page.locator('[data-testid="thread-subject"]').hover();
  await expect.poll(() => veil.evaluate((el) => getComputedStyle(el).display)).toBe('none');
});

test('screen 03 is FLAT: each message in its own elevation, the conversation without (R3, D2)', async () => {
  await page.locator('[data-testid="see-conversation"]').click();
  const conv = page.locator('[data-testid="conversation"]');
  await expect(conv.locator('[data-testid="thread-subject"]')).toHaveText(
    'Relecture du contrat Vantis',
  );
  // No elevation or enclosing surface between the screen's root and
  // the thread object — "screen 03 keeps its full card" (A46) is
  // reversed (RETOURS-7 R3).
  const enclosing = await conv.evaluate((root) => {
    const subject = root.querySelector('.thread-subject');
    const culprits = [];
    for (let el = subject.parentElement; el && el !== root; el = el.parentElement) {
      const s = getComputedStyle(el);
      if (s.boxShadow !== 'none' || s.borderTopWidth !== '0px') {
        culprits.push(el.className);
      }
    }
    return culprits;
  });
  expect(enclosing).toEqual([]);
  // The thread's head has no border, like at the pane (A46's twin guard).
  expect(
    await conv.locator('.head').first().evaluate((el) => getComputedStyle(el).borderBottomWidth),
  ).toBe('0px');
  // The scene scrolls as one flow, reading column bounded (D2).
  expect(await conv.locator('.scene').evaluate((el) => getComputedStyle(el).overflowY)).toBe(
    'auto',
  );
  expect(
    await conv.locator('.column').evaluate((el) => getComputedStyle(el).maxWidth),
  ).toBe('960px');
  // The message cards, THEY, keep their elevation.
  const shadow = await conv
    .locator('[data-testid="message-expanded"]')
    .first()
    .evaluate((el) => getComputedStyle(el).boxShadow);
  expect(shadow).not.toBe('none');
  await page.locator('[data-testid="back-to-mailbox"]').click();
  await expect(conv).toHaveCount(0);
});

test('pinning puts the conversation at the top of the Inbox — one row, reversible (R4, D3-D5)', async () => {
  // A row from the MIDDLE of the list: the "moves to top" effect is
  // visible. We hold onto its object to track it.
  const row = page.locator('[data-testid="row"]').nth(2);
  const subject = (await row.locator('.subject').innerText()).trim();
  await row.click();
  const pin = page.locator('[data-testid="reading-pane"] [data-testid="pin"]');
  await expect(pin).toContainText('Épingler');
  await expect(pin).toHaveAttribute('aria-pressed', 'false');
  await pin.click();
  // The button toggles, the pinned section opens at the top with ITS
  // row — marked "Épinglé" — and the flow no longer shows it (D5:
  // never the same conversation twice).
  await expect(pin).toContainText('Désépingler');
  await expect(pin).toHaveAttribute('aria-pressed', 'true');
  const section = page.locator('[data-testid="pins"]');
  await expect(section.locator('[data-testid="row"]')).toHaveCount(1);
  await expect(section).toContainText(subject);
  await expect(section.locator('[data-testid="chips-row"]')).toContainText('Épinglé');
  // Field finding (2026-08-21): the pinned row carries the SAME
  // DRAWING as the current mailbox's tile (nav, W2-D5) — same computed
  // background.
  const bgOf = (loc) => loc.evaluate((el) => getComputedStyle(el).backgroundColor);
  expect(await bgOf(section.locator('[data-testid="row"]'))).toBe(
    await bgOf(page.locator('[data-testid="nav-mailbox"][aria-current="true"]')),
  );
  await expect(
    page.locator('[data-testid="row"]', { hasText: subject }),
  ).toHaveCount(1);
  // Unpinning: the section closes, the row resumes its date slot in the flow.
  await pin.click();
  await expect(pin).toContainText('Épingler');
  await expect(section).toHaveCount(0);
  await expect(
    page.locator('[data-testid="row"]', { hasText: subject }),
  ).toHaveCount(1);
});

test('outside the Inbox, the thread bar does not offer the pin (R4, D4)', async () => {
  await page
    .locator('[data-testid="nav-folder"][data-category="archive"]')
    .click();
  await page.locator('[data-testid="row"]').first().click();
  await expect(
    page.locator('[data-testid="reading-pane"] [data-testid="thread-subject"]'),
  ).toBeVisible();
  await expect(page.locator('[data-testid="pin"]')).toHaveCount(0);
});

test('"e" pressed inside a message body archives the conversation', async () => {
  // PLAN-AUDIT-V2 E11: shortcuts live on the PARENT window; a click
  // inside a body made them inert — every key pressed inside the
  // frame is replayed onto the window.
  const row = page.locator('[data-testid="row"]').first();
  const subject = (await row.locator('.subject').textContent()).trim();
  await row.click();
  // Focus enters the body's frame (without a script, S1: Playwright
  // cannot evaluate anything there — we focus it from the parent),
  // then the REAL key: it is the replay to the parent window that is proven.
  const frame = page.locator('[data-testid="reading-pane"] [data-testid="message-expanded"] iframe').first();
  await expect(frame).toBeVisible();
  await frame.focus();
  await page.keyboard.press('e');
  await expect
    .poll(async () =>
      (await page.locator('[data-testid="row"] .subject').allTextContents()).map((s) => s.trim()).includes(subject))
    .toBe(false);
});
