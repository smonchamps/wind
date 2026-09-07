import { test, expect } from '@playwright/test';
import { writeFile, open } from 'node:fs/promises';
import { launchAppV2, closeApp } from '../launch.mjs';

let app, browser, page;
test.beforeAll(async () => {
  ({ app, browser, page } = await launchAppV2({ accounts: [
    { email: 'owner@example.fr', messages: 1 },
  ] }));
});
test.afterAll(async () => { await closeApp({ app, browser }); });

for (const edited of [false, true]) {
  test(`${edited ? 'editing session' : 'legacy draft'} refuses the excess file and retains both eligible files`, async ({}, info) => {
    const small = info.outputPath('small.txt');
    const excess = info.outputPath('excess.bin');
    const tail = info.outputPath('tail.txt');
    await writeFile(small, '1234');
    await writeFile(tail, '567');
    const file = await open(excess, 'w');
    try { await file.truncate(25 * 1024 * 1024 - 3); } finally { await file.close(); }
    const report = await page.evaluate(async ({ paths, edited }) => {
      const call = window.__TAURI__.core.invoke;
      const accounts = await call('nav_snapshot');
      const accountId = accounts[0].account_id;
      const token = edited ? 'bounded-attachment-fixture' : null;
      if (token) await call('begin_draft_edit', {
        token, accountId, id: null, incarnation: null, baseEpoch: null,
      });
      const first = await call('attach_files', { accountId, draftId: null, paths: [paths[0]], editToken: token });
      return call('attach_files', {
        accountId, draftId: first.draft_id, paths: paths.slice(1), editToken: token,
      });
    }, { paths: [small, excess, tail], edited });
    expect(report.refused.map(file => file.name)).toEqual(['excess.bin']);
    expect(report.attachments.map(file => file.name)).toEqual(['small.txt', 'tail.txt']);
    expect(report.draft_id).not.toBeNull();
  });
}
