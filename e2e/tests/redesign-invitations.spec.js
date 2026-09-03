// PLAN-INVITATIONS — the invitation card played on the Clarity decor,
// in ISOLATION (its own instance): "Atelier de septembre" (Sofia,
// work account) carries an invitation to respond. The decor is
// OFFLINE by construction: replying logs the iTIP email in the
// outbox (golden rules), the card and the list row say the reply —
// that's the whole real path except SMTP delivery. The decor's ICS
// goes through the REAL parser (mail-ical) in UTC: "14:30 – 16:00"
// displays in the machine's local time, deterministic whatever the
// run.
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

const workshopRow = () =>
  page.locator('[data-testid="row"]', { hasText: 'Atelier de septembre' }).first();

const openWorkshop = async () => {
  await expect(page.locator('[data-testid="row"]').first()).toBeVisible();
  await workshopRow().click();
  await expect(
    page.locator('[data-testid="reading-pane"] [data-testid="thread-subject"]'),
  ).toHaveText('Atelier de septembre');
  return page.locator('[data-testid="reading-pane"] [data-testid="invitation"]');
};

test('the invitation card shows itself: title, local time, organizer, three gestures', async () => {
  const card = await openWorkshop();
  await expect(card).toBeVisible();
  await expect(card.locator('[data-testid="invitation-title"]')).toHaveText(
    'Atelier de septembre',
  );
  // The TIME goes through the real parser (ICS in UTC → machine
  // local time), the LOCATION comma is UNESCAPED along the way.
  await expect(card).toContainText('14:30 – 16:00');
  await expect(card).toContainText('Grande salle, Atelier Nord');
  await expect(card).toContainText('Sofia Nardi');
  await expect(card.locator('[data-testid="invitation-status"]')).toHaveText(
    'Vous n’avez pas répondu',
  );
  // Three NEUTRAL buttons (D4), none pressed.
  for (const gesture of ['inv-accept', 'inv-tentative', 'inv-refuse']) {
    await expect(card.locator(`[data-testid="${gesture}"]`)).toHaveAttribute(
      'aria-pressed',
      'false',
    );
  }
  // The card PRECEDES the body in the content (A76: it is the
  // message's subject) — the same DOM order guarantee as A71.
  const order = await page
    .locator('[data-testid="reading-pane"] [data-testid="message-expanded"] .content > *')
    .evaluateAll((nodes) => nodes.map((n) => n.dataset.testid ?? n.tagName));
  expect(order[0]).toBe('invitation');
});

test('R10: replying FROM the list — the row carries the gestures, then the chip', async () => {
  // Leave the workshop thread: the gesture plays WITHOUT opening it.
  await page
    .locator('[data-testid="row"]', { hasText: 'Planning de la semaine 33' })
    .first()
    .click();
  // R3'c: the gestures occupy their OWN row (chips-invitation).
  const gestures = workshopRow().locator('[data-testid="chips-invitation"]');
  await expect(gestures.locator('[data-testid="list-accept"]')).toBeVisible();
  await expect(gestures.locator('[data-testid="list-refuse"]')).toBeVisible();

  await gestures.locator('[data-testid="list-tentative"]').click();
  // The chip replaces the gestures INSTANTLY (optimistic) — and the
  // row was NOT selected.
  await expect(
    workshopRow().locator('[data-testid="invitation-chip"]'),
  ).toContainText('Provisoire');
  await expect(gestures).toHaveCount(0);
  await expect(
    page.locator('[data-testid="reading-pane"] [data-testid="thread-subject"]'),
  ).toHaveText('Planning de la semaine 33');

  // The card reads back the same truth (the database, not a screen state).
  const card = await openWorkshop();
  await expect(card.locator('[data-testid="invitation-status"]')).toHaveText(
    'Vous avez répondu provisoirement',
  );
  await expect(card.locator('[data-testid="inv-tentative"]')).toHaveAttribute(
    'aria-pressed',
    'true',
  );
});

test('D6: changing one’s mind from the card — refuse then accept', async () => {
  const card = await openWorkshop();
  await card.locator('[data-testid="inv-refuse"]').click();
  await expect(card.locator('[data-testid="invitation-status"]')).toHaveText(
    'Vous avez refusé',
  );
  await card.locator('[data-testid="inv-accept"]').click();
  await expect(card.locator('[data-testid="invitation-status"]')).toHaveText(
    'Vous avez accepté',
  );
  await expect(card.locator('[data-testid="inv-refuse"]')).toHaveAttribute(
    'aria-pressed',
    'false',
  );
});

test('R11: the reloaded list says the reply as a chip — the reply survives navigation', async () => {
  // A fresh page of the Inbox (folder round trip): the row comes
  // from the core's enrichment, not local state.
  await page.locator('[data-testid="nav-folder"][data-category="archive"]').click();
  await page.locator('[data-testid="nav-folder"][data-category="inbox"]').click();
  await expect(
    workshopRow().locator('[data-testid="invitation-chip"]'),
  ).toContainText('Acceptée');
  const card = await openWorkshop();
  await expect(card.locator('[data-testid="invitation-status"]')).toHaveText(
    'Vous avez accepté',
  );
});
