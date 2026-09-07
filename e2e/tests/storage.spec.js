// Lot 5 E14b (audit G02, Chief Engineer decision D3): Settings › Your
// data — a copy of the database into a file the user names, and the
// restoration of a copy, staged for the next start. What the user sees:
// the two rows, the confirmation card, the file written, the copy staged
// next to the database. The swap itself and the held outbox are proven
// by the shell's own tests (`restore.rs`) and by the field: an e2e
// session cannot survive the restart.
import fs from 'node:fs';
import path from 'node:path';
import { test, expect } from '@playwright/test';
import { launchAppV2, closeApp, purgeLocals } from '../launch.mjs';

let app, browser, page, db;
const storage = () => page.getByTestId('settings-storage');

test.beforeAll(async () => {
  ({ app, browser, page, db } = await launchAppV2({ accounts: [{ email: 'keeper@example.fr', messages: 3 }] }));
  await purgeLocals(page);
});
test.afterAll(async () => {
  await purgeLocals(page);
  await closeApp({ app, browser });
});

test('the copy is written where the user asked, and is a database', async ({}, info) => {
  await page.getByTestId('settings').click();
  await page.locator('[data-testid="settings-group"][data-group="stockage"]').click();
  await expect(storage()).toBeVisible();
  await expect(storage()).toContainText('Save a copy of your data');
  await expect(storage()).toContainText('Passwords stay in the system vault');
  await page.screenshot({ path: info.outputPath('settings-storage.png') });

  const dest = path.join(path.dirname(db), `copy-${Date.now()}.db`);
  await page.evaluate((p) => { window.__e2eDestination = p; }, dest);
  await page.getByTestId('backup-save').click();
  await expect(page.getByTestId('toast')).toContainText('Copy saved');
  await expect.poll(() => fs.existsSync(dest)).toBe(true);
  const header = fs.readFileSync(dest).subarray(0, 15).toString('latin1');
  expect(header).toBe('SQLite format 3');
  // A second copy never overwrites the first.
  await page.getByTestId('backup-save').click();
  await expect(page.getByTestId('toast')).toContainText('Could not save the copy');
});

test('restoring a copy confirms, stages it next to the database, and refuses a foreign file', async ({}, info) => {
  const copy = path.join(path.dirname(db), `restore-${Date.now()}.db`);
  await page.evaluate((p) => { window.__e2eDestination = p; }, copy);
  await page.getByTestId('backup-save').click();
  await expect.poll(() => fs.existsSync(copy)).toBe(true);

  await page.evaluate((p) => { window.__e2eSource = p; window.__e2eNoRestart = true; }, copy);
  await page.getByTestId('restore-pick').click();
  await expect(page.getByTestId('restore-confirm')).toContainText(path.basename(copy));
  await expect(page.getByTestId('restore-confirm')).toContainText('Wind will restart on this copy');
  await page.screenshot({ path: info.outputPath('settings-restore-confirm.png') });
  await page.getByTestId('restore-cancel').click();
  await expect(page.getByTestId('restore-confirm')).toHaveCount(0);

  await page.getByTestId('restore-pick').click();
  await page.getByTestId('restore-now').click();
  await expect(page.getByTestId('restore-confirm')).toHaveCount(0);
  await expect.poll(() => fs.existsSync(`${db}.restore`)).toBe(true);
  fs.rmSync(`${db}.restore`, { force: true });

  // Not a database: refused with the reason, nothing staged.
  const foreign = path.join(path.dirname(db), `foreign-${Date.now()}.txt`);
  fs.writeFileSync(foreign, 'not a database');
  await page.evaluate((p) => { window.__e2eSource = p; }, foreign);
  await page.getByTestId('restore-pick').click();
  await page.getByTestId('restore-now').click();
  await expect(page.getByTestId('toast')).toContainText('Could not restore the copy');
  expect(fs.existsSync(`${db}.restore`)).toBe(false);
});
