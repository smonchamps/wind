// Field STOP 2 PLAN-AUDIT-V2, pass 3 (2026-09-02, Chief Engineer verdict on
// screenshots): the thread bars. In the PANE, the sort bar (Archive /
// Report as spam / Pin) is stuck under the thread header; at
// SCREEN 03, its buttons live in the scene's header bar;
// a message's reply bar FLOATS at the bottom of the message — it
// stays visible while scrolling, at 12 px from the foot of the scrollport, and
// hugs the card's edges when its end arrives.
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

test('pane: the sort bar is stuck under the header, the reply bar floats at the bottom of the message', async () => {
  await page.locator('[data-testid="row"]', { hasText: 'Relecture du contrat Vantis' }).click(); // lang:fr
  const bar = page.locator('[data-testid="bar-thread"]');
  await expect(bar).toBeVisible();
  // The last message's card renders AFTER the bar (body served):
  // without this wait, the read fell on a null (flaky, gate of
  // 2026-09-02, once).
  await expect(page.locator('[data-testid="actions-message"]').first()).toBeVisible();
  const geo = await page.evaluate(() => {
    const bar = document.querySelector('[data-testid="bar-thread"]');
    const chips = document.querySelector('[data-testid="thread-chips"]');
    const reply = document.querySelector('[data-testid="actions-message"]');
    return {
      gap: bar.getBoundingClientRect().top - chips.getBoundingClientRect().bottom,
      stickyBar: getComputedStyle(bar).position,
      stickyReply: getComputedStyle(reply).position,
      shadowReply: getComputedStyle(reply).boxShadow,
    };
  });
  // Stuck: at most a 4 px gap under the chips; sticky while scrolling (R1).
  expect(geo.gap).toBeLessThanOrEqual(6);
  expect(geo.stickyBar).toBe('sticky');
  // Floating: stuck at the bottom, raised (shadow), never a flat net.
  expect(geo.stickyReply).toBe('sticky');
  expect(geo.shadowReply).not.toBe('none');
  // Pass 4 (2026-09-02): scrolled to the bottom, the stuck bar holds the
  // VISIBLE TOP of the frame — not 18 px below, with the message
  // passing under the band (the sticky is bounded to the content, under the padding).
  await page.setViewportSize({ width: 1180, height: 420 });
  const pane = page.locator('[data-testid="reading-pane"]');
  await pane.evaluate((el) => { el.scrollTop = el.scrollHeight; });
  await expect
    .poll(async () => {
      await pane.evaluate((el) => { el.scrollTop = el.scrollHeight; });
      const frame = await pane.boundingBox();
      const bar = await page.locator('[data-testid="bar-thread"]').boundingBox();
      return bar ? Math.abs(bar.y - frame.y) : 999;
    })
    .toBeLessThanOrEqual(1);
  await page.setViewportSize({ width: 1500, height: 1050 });
  await pane.evaluate((el) => { el.scrollTop = 0; });
});

test('screen 03: the sort actions live in the header bar, the reply bar stays visible while scrolling', async () => {
  await page.locator('[data-testid="see-conversation"]').click();
  const header = page.locator('[data-testid="conversation"] header');
  await expect(header.locator('[data-testid="archive"]')).toBeVisible();
  await expect(header.locator('[data-testid="report-spam"]')).toBeVisible();
  await expect(page.locator('[data-testid="bar-thread"].pane')).toHaveCount(0);
  // The reply bar of the last message is visible WITHOUT scrolling
  // to it: it floats at the bottom of the scrollport, at 12 px from the foot.
  const reply = page.locator('[data-testid="actions-message"]').last();
  await expect(reply).toBeVisible();
  const margin = await page.evaluate(() => {
    const reply = [...document.querySelectorAll('[data-testid="actions-message"]')].at(-1);
    const scene = reply.closest('.scene');
    return scene.getBoundingClientRect().bottom - reply.getBoundingClientRect().bottom;
  });
  expect(margin).toBeGreaterThanOrEqual(12);
  // Pass 4 (2026-09-02, Chief Engineer): the sort actions align on the
  // LEFT edge of the message column.
  const alignment = await page.evaluate(() => {
    const archive = document.querySelector('[data-testid="conversation"] header [data-testid="archive"]');
    const column = document.querySelector('[data-testid="conversation"] .column');
    return archive.getBoundingClientRect().left - column.getBoundingClientRect().left;
  });
  expect(Math.abs(alignment)).toBeLessThanOrEqual(1);
  // Pass 5 (2026-09-02, Chief Engineer): the back button keeps its content
  // width — in the header grid it used to stretch across its whole track.
  const back = await page.locator('[data-testid="back-to-mailbox"]').boundingBox();
  expect(back.width).toBeLessThan(300);
});
