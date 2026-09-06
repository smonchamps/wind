import { test, expect } from '@playwright/test';
import { launchAppV2, closeApp } from '../launch.mjs';

let app;
let browser;
let page;

test.beforeAll(async () => {
  ({ app, browser, page } = await launchAppV2({ lang: 'fr' }));
});

test.afterAll(async () => {
  await closeApp({ app, browser });
});

test('offline enqueue reports a queued message, never an accepted send', async ({}, testInfo) => {
  await page.locator('[data-testid="write"]').click();
  await page.locator('[data-testid="compose-to"]').fill('fixture@example.invalid');
  await page.locator('[data-testid="compose-subject"]').fill('Delivery fixture');
  await page.locator('[data-testid="compose-body"]').fill('Synthetic local message.');
  await page.locator('[data-testid="compose-send"]').click();
  await expect(page.locator('[data-testid="compose"]')).toHaveCount(0);
  const status = await page.evaluate(() => window.__TAURI__.core.invoke('outbox_status'));
  expect(status.entries.some((entry) => entry.subject === 'Delivery fixture' && entry.state === 'queued')).toBe(true);
  const toast = page.locator('[data-testid="toast"]');
  await expect(toast).toHaveText('Message mis en file d’envoi.', { timeout: 2000 }); // lang:fr
  await page.screenshot({ path: testInfo.outputPath('queued-fr.png') });
  await toast.screenshot({ path: testInfo.outputPath('queued-toast-fr.png') });
});

test('uncertain delivery directs verification before the existing resend decision', async ({}, testInfo) => {
  // Only the status read is substituted; the production notice renders the real schema.
  await page.evaluate(() => {
    const fetch = window.fetch;
    window.restoreDeliveryFetch = () => { window.fetch = fetch; };
    window.deliveryStatusReads = 0;
    const outbox = { queued: 0, interrupted: 1, rejected: 0, scheduled: 0,
      next_scheduled_epoch: null, refused_actions: 0, entries: [{
        id: -1, subject: 'Delivery fixture', state: 'interrupted', to: 'fixture@example.invalid',
        attempts: 1, error: 'response acknowledgement lost', attachments: 1, send_at_epoch: null,
      }] };
    window.fetch = async (...args) => {
      const response = await fetch(...args);
      const command = new URL(String(args[0]), location.href).pathname.slice(1);
      if (command !== 'ui_state' && command !== 'outbox_status') return response;
      const state = await response.json();
      window.deliveryStatusReads += 1;
      return new Response(JSON.stringify(command === 'ui_state' ? { ...state, outbox } : outbox), {
        status: response.status, headers: response.headers,
      });
    };
  });
  try {
    await expect.poll(() => page.evaluate(() => window.deliveryStatusReads)).toBeGreaterThan(0);
    const notice = page.locator('[data-testid="slot-notice"]');
    await expect(notice).toContainText('La remise de « Delivery fixture » est incertaine.'); // lang:fr
    await expect(notice).toContainText('Vérifiez les Envoyés avant de décider de renvoyer.'); // lang:fr
    await expect(notice.getByRole('button', { name: 'Renvoyer', exact: true })).toBeVisible(); // lang:fr
    await expect(notice.getByRole('button', { name: 'Abandonner', exact: true })).toBeVisible(); // lang:fr
    await page.screenshot({ path: testInfo.outputPath('uncertain-fr.png') });
    await notice.screenshot({ path: testInfo.outputPath('uncertain-notice-fr.png') });
  } finally {
    await page.evaluate(() => { window.restoreDeliveryFetch(); delete window.restoreDeliveryFetch; delete window.deliveryStatusReads; });
  }
});

for (const uncertain of [true, false]) {
  test(uncertain ? 'a lost move confirmation requests inspection without offering automatic replay' : 'an unsupported mutation reports the actual capability refusal', async ({}, testInfo) => {
    await page.evaluate((uncertain) => {
      const fetch = window.fetch;
      window.restoreMutationFetch = () => { window.fetch = fetch; };
      const outbox = { queued: 0, interrupted: 0, rejected: 0, scheduled: 0, next_scheduled_epoch: null, entries: [],
        refused_actions: uncertain ? 0 : 1, refused_action_reason: uncertain ? null : 'moving requires MOVE or UIDPLUS',
        action_incidents: uncertain ? [{ id: -9, account_id: 1, source: 'INBOX', destination: 'Archive', account: 'journal@example.invalid', subject: 'Contract fixture', sender: 'sender@example.invalid', uid: 1, reason: 'confirmation lost' }] : [] };
      window.fetch = async (...args) => {
        const response = await fetch(...args);
        const command = new URL(String(args[0]), location.href).pathname.slice(1);
        if (command !== 'ui_state' && command !== 'outbox_status') return response;
        const state = await response.json();
        return new Response(JSON.stringify(command === 'ui_state' ? { ...state, outbox } : outbox), { status: response.status, headers: response.headers });
      };
    }, uncertain);
    try {
      const notice = page.locator('[data-testid="slot-notice"]');
      if (uncertain) {
        await expect(notice).toContainText('Le déplacement de INBOX vers Archive est incertain.'); // lang:fr
        await expect(notice).toContainText('Vérifiez les deux dossiers avant une nouvelle action.'); // lang:fr
        await expect(notice).toContainText('journal@example.invalid');
        await expect(notice).toContainText('Contract fixture');
        await expect(notice).toContainText('sender@example.invalid');
        await page.screenshot({ path: testInfo.outputPath('move-context-fr.png') });
        await expect(notice.getByRole('button', { name: 'J’ai vérifié', exact: true })).toBeVisible(); // lang:fr
        await expect(notice.getByRole('button', { name: /Renvoyer|Réessayer/ })).toHaveCount(0); // lang:fr
      } else {
        await expect(notice).toContainText('moving requires MOVE or UIDPLUS');
      }
    } finally {
      await page.evaluate(() => { window.restoreMutationFetch(); delete window.restoreMutationFetch; });
    }
  });
}

test('resending suspends both decisions while the SMTP operation is pending', async () => {
  await page.evaluate(() => {
    const fetch = window.fetch;
    window.restoreDecisionFetch = () => { window.fetch = fetch; };
    window.decisionCalls = [];
    const outbox = { queued: 0, interrupted: 1, rejected: 0, scheduled: 0, next_scheduled_epoch: null, refused_actions: 0,
      entries: [{ id: -1, message_id: '<decision@example.invalid>', subject: 'Decision fixture', state: 'interrupted', to: 'fixture@example.invalid', attempts: 1, error: null, attachments: 0, send_at_epoch: null }] };
    window.fetch = async (...args) => {
      const command = new URL(String(args[0]), location.href).pathname.slice(1);
      if (command === 'outbox_requeue' || command === 'flush_outbox') {
        window.decisionCalls.push(command);
        if (command === 'flush_outbox') await new Promise((resolve) => { window.releaseDecision = resolve; });
        return new Response(JSON.stringify(command === 'flush_outbox' ? { sent: 0 } : null), { headers: { 'Tauri-Response': 'ok', 'Content-Type': 'application/json' } });
      }
      const response = await fetch(...args);
      if (command !== 'ui_state' && command !== 'outbox_status') return response;
      const state = await response.json();
      return new Response(JSON.stringify(command === 'ui_state' ? { ...state, outbox } : outbox), { status: response.status, headers: response.headers });
    };
  });
  try {
    const notice = page.locator('[data-testid="slot-notice"]');
    await expect(notice).toContainText('Decision fixture');
    await notice.getByRole('button', { name: 'Renvoyer', exact: true }).click(); // lang:fr
    await expect.poll(() => page.evaluate(() => window.decisionCalls.includes('flush_outbox'))).toBe(true);
    await expect(notice.getByRole('button', { name: 'Abandonner', exact: true })).toBeDisabled(); // lang:fr
    await expect(notice.getByRole('button', { name: 'Renvoyer', exact: true })).toBeDisabled(); // lang:fr
  } finally {
    await page.evaluate(() => { window.releaseDecision?.(); window.restoreDecisionFetch(); delete window.restoreDecisionFetch; });
  }
});
