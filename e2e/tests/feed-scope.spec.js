import { test, expect } from '@playwright/test';
import { launchAppV2, closeApp, injectArrival } from '../launch.mjs';

let app;
let browser;
let page;
const first = 'first@example.fr';
const second = 'second@example.fr';
const cards = () => page.locator('[data-testid="feed-card"]');
const account = (email) => page.locator('[data-testid="nav-mailbox"]').filter({ hasText: email });

test.beforeEach(async () => {
  ({ app, browser, page } = await launchAppV2({ accounts: [
    { email: first, messages: 1 }, { email: second, messages: 1 },
  ] }));
  for (const email of [first, second]) {
    injectArrival({ email, sender: 'newsletter@example.fr', subject: email, n: email === first ? 25 : 2 });
  }
  await page.reload();
  await page.locator('[data-testid="organized-mode"]').click();
  await page.evaluate(() => window.__TAURI__.core.invoke('route_sender', {
    address: 'newsletter@example.fr', destination: 'feed', rule: null,
  }));
  await account(first).click();
  await page.locator('[data-testid="nav-folder"][data-category="feed"]').click();
  await expect(cards()).toHaveCount(20);
});

test.afterEach(async () => { await closeApp({ app, browser }); });

async function nextPage() {
  await expect.poll(async () => {
    await page.locator('[data-testid="feed"]').evaluate((node) => {
      node.scrollTop = node.scrollHeight;
      node.dispatchEvent(new Event('scroll'));
    });
    return cards().count();
  }).toBe(25);
}

test('switching account drops all old pages before the new response arrives', async () => {
  await nextPage();
  await page.evaluate(() => {
    window.__e2eHold = new Promise((resolve) => { window.releaseFeed = resolve; });
  });
  await account(second).click();
  await expect(cards()).toHaveCount(0);
  await page.evaluate(() => { delete window.__e2eHold; window.releaseFeed(); });
  await expect(cards()).toHaveCount(2);
  await expect(cards().filter({ hasText: first })).toHaveCount(0);
  expect(await page.locator('[data-testid="feed"]').evaluate((node) => node.scrollTop)).toBe(0);
});

test('same-scope invalidation removes a deleted card beyond the first page', async () => {
  await nextPage();
  const key = await cards().last().getAttribute('data-key');
  const [accountId, mailbox, , , uid] = key.split(':');
  await page.evaluate((args) => window.__TAURI__.core.invoke('archive_message', args), {
    accountId: Number(accountId), mailbox, uid: Number(uid),
  });
  await expect(page.locator(`[data-testid="feed-card"][data-key="${key}"]`)).toHaveCount(0);
  await expect(cards()).toHaveCount(24);
});

test('a scroll requesting the next page survives an in-flight refresh', async () => {
  await page.evaluate(() => {
    window.__e2eLog = [];
    window.__e2eHoldCommands = ['feed_cards'];
    window.__e2eHold = new Promise((resolve) => { window.releaseFeed = resolve; });
  });
  await page.evaluate(() => window.__TAURI__.core.invoke('route_sender', {
    address: 'newsletter@example.fr', destination: 'feed', rule: null,
  }));
  await expect.poll(() => page.evaluate(() => window.__e2eLog
    .some((entry) => entry.command === 'feed_cards' && entry.arrival === null))).toBe(true);
  await page.locator('[data-testid="feed"]').evaluate((node) => {
    node.scrollTop = node.scrollHeight;
    node.dispatchEvent(new Event('scroll'));
  });
  await page.evaluate(() => { delete window.__e2eHold; window.releaseFeed(); });
  await expect(cards()).toHaveCount(25);
});

test('refresh preserves the extent of a page already requested', async () => {
  await page.evaluate(() => {
    window.__e2eLog = [];
    window.__e2eHoldCommands = ['feed_cards'];
    window.__e2eHold = new Promise((resolve) => { window.releaseFeed = resolve; });
  });
  await page.locator('[data-testid="feed"]').evaluate((node) => {
    node.scrollTop = node.scrollHeight;
    node.dispatchEvent(new Event('scroll'));
  });
  await expect.poll(() => page.evaluate(() => window.__e2eLog
    .filter((entry) => entry.command === 'feed_cards').length)).toBe(1);
  await page.evaluate(() => window.__TAURI__.core.invoke('route_sender', {
    address: 'newsletter@example.fr', destination: 'feed', rule: null,
  }));
  await expect.poll(() => page.evaluate(() => window.__e2eLog
    .filter((entry) => entry.command === 'feed_cards').length)).toBeGreaterThanOrEqual(2);
  await page.evaluate(() => { delete window.__e2eHold; window.releaseFeed(); });
  await expect(cards()).toHaveCount(25);
});
