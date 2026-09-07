// Field finding 2026-09-07 (Lot 5 E13, STOP 2 item 2): a few seconds
// after launch, "Always show images" on a newsletter blanked the reading
// pane until the initial synchronization ended — more than ten seconds.
// The grant is a WRITE; it waited for SQLite's writer lock behind a sync
// batch (busy_timeout 30 s) while HOLDING the commands' lock, and every
// pure READ queued behind it: the pane's own re-read of the body, the
// next message, the list. In WAL a reader never waits for a writer —
// only the commands' lock made it wait. The net below holds the writer
// from outside (a Python connection in BEGIN IMMEDIATE) and opens the
// NEXT unread message — its own mark-as-read is the write that queues;
// the message must open at once.
import { spawn } from 'node:child_process';
import { test, expect } from '@playwright/test';
import { launchAppV2, closeApp, purgeLocals } from '../launch.mjs';

let app, browser, page, db;
const pane = () => page.getByTestId('reading-pane');
const frame = () => pane().locator('iframe.body');
const rows = () => page.getByTestId('row');

// Holds SQLite's writer lock on the app's database for `seconds` — the
// stand-in for a synchronization batch. Python's sqlite3 is what
// `freeze-probe.py` already relies on.
function holdWriter(path, seconds) {
  return spawn(
    'python',
    [
      '-c',
      'import sqlite3, sys, time\n'
        + 'c = sqlite3.connect(sys.argv[1], timeout=30, isolation_level=None)\n'
        + 'c.execute("BEGIN IMMEDIATE")\n'
        + 'print("held", flush=True)\n'
        + 'time.sleep(float(sys.argv[2]))\n'
        + 'c.execute("COMMIT")\n',
      path,
      String(seconds),
    ],
    { stdio: ['ignore', 'pipe', 'inherit'] },
  );
}

test.beforeAll(async () => {
  ({ app, browser, page, db } = await launchAppV2({ accounts: [{ email: 'reader@example.fr', messages: 4 }] }));
  await purgeLocals(page);
});
test.afterAll(async () => {
  await purgeLocals(page);
  await closeApp({ app, browser });
});

test('a read never waits behind a write that waits for the synchronization', async () => {
  await rows().first().click();
  await expect(frame()).toHaveAttribute('srcdoc', /.+/);
  const firstSubject = await page.getByTestId('thread-subject').innerText();

  const writer = holdWriter(db, 12);
  await new Promise((resolve) => writer.stdout.once('data', resolve));
  try {
    // Opening an unread row is ITSELF a read followed by a write (the
    // row is marked seen): the write queues on SQLite's writer, as the
    // field's grant did; the reads of the same gesture — the thread,
    // the body — must not queue behind it. Three seconds, not thirty.
    const opened = Date.now();
    await rows().nth(1).click();
    await expect(page.getByTestId('thread-subject')).not.toHaveText(firstSubject, { timeout: 3000 });
    await expect(frame()).toHaveAttribute('srcdoc', /.+/, { timeout: 3000 });
    expect(Date.now() - opened).toBeLessThan(3000);
  } finally {
    writer.kill();
  }
});
