import { test, expect } from '@playwright/test';
import { execFileSync } from 'node:child_process';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { launchAppV2, closeApp, purgeLocals } from '../launch.mjs';

const root = fileURLToPath(new URL('../../', import.meta.url));
const email = 'guest@example.fr';
const db = path.join(root, 'target/e2e/parcours-v2-inbox.db');
let app, browser, page;
const source = (title, extra = '', sequence = 2, method = 'REQUEST', event = title) =>
  `BEGIN:VCALENDAR\r\nVERSION:2.0\r\nMETHOD:${method}\r\nBEGIN:VEVENT\r\nUID:${event}\r\nDTSTAMP:20260906T120000Z\r\nSEQUENCE:${sequence}\r\nSUMMARY:${title}\r\nORGANIZER:mailto:owner@example.fr\r\nDTSTART:20260910T120000Z\r\n${extra}END:VEVENT\r\nEND:VCALENDAR\r\n`;
const seed = (ics, mode = '-', uid = '') => execFileSync(path.join(root, 'target/debug/examples/seed_invitation.exe'), [db, email, ics, mode, String(uid)], { cwd: root, encoding: 'utf8' }).trim();
const card = () => page.getByTestId('reading-pane').getByTestId('invitation');
const open = async title => {
  await page.reload();
  await page.getByTestId('row').filter({ hasText: title }).first().click();
  await expect(card()).toBeVisible();
};

test.beforeAll(async () => {
  ({ app, browser, page } = await launchAppV2({ accounts: [{ email, messages: 1 }] }));
  await purgeLocals(page);
});
test.afterAll(async () => { await purgeLocals(page); await closeApp({ app, browser }); });

test('unsupported recurrence stays readable with an explanation and no response action', async () => {
  seed(source('Complex recurrence', 'RECURRENCE-ID;RANGE=THISANDFUTURE:20260908T120000Z\r\n'));
  await open('Complex recurrence');
  await expect(card().getByTestId('invitation-actions')).toHaveCount(0);
  await expect(card()).toContainText('Wind cannot reply to this calendar format.');
  await page.screenshot({ path: path.join(root, 'target/e2e/calendar-unsupported.png') });
  await expect(page.frameLocator('[data-testid="reading-pane"] iframe').locator('body')).toContainText('available offline');
});

test('the response to a moved occurrence retains its original date and timezone', async () => {
  seed(source('Moved occurrence', 'RECURRENCE-ID;TZID=Europe/Paris:20260908T140059\r\n'));
  await open('Moved occurrence');
  await card().getByTestId('inv-accept').click();
  await expect(card().getByTestId('inv-accept')).toHaveAttribute('aria-pressed', 'true');
  await expect.poll(() => seed('inspect')).toContain('RECURRENCE-ID;TZID=Europe/Paris:20260908T140059');
  await card().getByTestId('inv-refuse').click();
  await expect.poll(() => seed('inspect')).toContain('PARTSTAT=DECLINED');
});

test('a stale card cannot reply after cancellation; a later revision remains answerable', async () => {
  seed(source('Versioned meeting'));
  await open('Versioned meeting');
  const queuedBefore = seed('inspect');
  seed(source('Meeting cancellation', '', 3, 'CANCEL', 'Versioned meeting'));
  await card().getByTestId('inv-accept').click();
  await expect(page.getByTestId('toast')).toContainText('changed');
  expect(seed('inspect')).toBe(queuedBefore);
  await open('Versioned meeting');
  await expect(card().getByTestId('invitation-actions')).toHaveCount(0);
  seed(source('New meeting revision', '', 4, 'REQUEST', 'Versioned meeting'));
  await open('New meeting revision');
  await expect(card().getByTestId('inv-accept')).toBeVisible();
  await open('Versioned meeting');
  await expect(card()).toContainText('A newer invitation replaces this version.');
});

test('legacy verification failure preserves the cached message and retry recovers the card', async () => {
  const ics = source('Legacy calendar');
  const uid = seed(ics, 'legacy');
  await open('Legacy calendar');
  await expect(card().getByTestId('invitation-actions')).toHaveCount(0);
  await expect(card().getByTestId('inv-refresh')).toBeVisible();
  await card().getByTestId('inv-refresh').click();
  await expect(page.getByTestId('toast')).toBeVisible();
  await expect(page.frameLocator('[data-testid="reading-pane"] iframe').locator('body')).toContainText('available offline');
  await expect(card().getByTestId('inv-refresh')).toBeEnabled();
  await page.screenshot({ path: path.join(root, 'target/e2e/calendar-legacy.png') });
  seed(ics, '-', uid);
  await expect(card().getByTestId('inv-refresh')).toBeVisible();
  await card().getByTestId('inv-refresh').click();
  await expect(card().getByTestId('inv-accept')).toBeVisible();
  await expect(card().getByTestId('inv-refresh')).toHaveCount(0);
});

test('a delayed reply failure cannot overwrite the next opened invitation', async () => {
  seed(source('First delayed invitation'));
  seed(source('Second delayed invitation'));
  await open('First delayed invitation');
  await page.evaluate(() => {
    window.__e2eFailure = ['reply_invitation'];
    window.__e2eHoldCommands = ['reply_invitation'];
    window.__e2eHold = new Promise(resolve => { window.__releaseCalendarReply = resolve; });
  });
  await card().getByTestId('inv-accept').click();
  await page.getByTestId('row').filter({ hasText: 'Second delayed invitation' }).first().click();
  await expect(card().getByTestId('invitation-title')).toHaveText('Second delayed invitation');
  await page.evaluate(() => {
    window.__releaseCalendarReply();
    delete window.__e2eHold;
    delete window.__e2eHoldCommands;
  });
  await expect(page.getByTestId('toast')).toContainText('e2e failure');
  await expect(card().getByTestId('invitation-status')).toHaveText('You have not replied');
});
