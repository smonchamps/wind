// PLAN-RETOURS-14 R1 (D1): the thread's action bar (archive, report
// as spam, pin…) lives AT THE TOP of the conversation and STICKS
// while scrolling — in the reading pane (three panes) as well as at
// screen 03. The net targets what the user SEES: at the bottom of a
// long thread, the bar is still there, at the top of the frame.
import { test, expect } from '@playwright/test';
import { launchAppV2, closeApp } from '../launch.mjs';

let app;
let browser;
let page;

test.describe.configure({ mode: 'serial' });

test.beforeAll(async () => {
  ({ app, browser, page } = await launchAppV2({
    accounts: [{ email: 'principal@exemple.fr', messages: 6 }],
  }));
  // A short window forces the thread to scroll: the sticky behavior
  // is only proven by scrolling for real (a non-vacant net).
  await page.setViewportSize({ width: 1180, height: 420 });
});

test.afterAll(async () => {
  await closeApp({ app, browser });
});

test('reading pane: the thread bar is at the top, before the messages', async () => {
  await expect(page.locator('[data-testid="row"]').first()).toBeVisible();
  await page.locator('[data-testid="row"]').first().click();
  const pane = page.locator('[data-testid="reading-pane"]');
  await expect(pane.locator('[data-testid="archive"]')).toBeVisible();

  // The order SEEN: the bar above the first message.
  const bar = await pane.locator('[data-testid="archive"]').boundingBox();
  const message = await pane.locator('[data-testid="message-expanded"], [data-testid="message-collapsed"]').first().boundingBox();
  expect(bar.y).toBeLessThan(message.y);
});

test('reading pane: the bar stays visible at the bottom of the thread (sticky)', async () => {
  const pane = page.locator('[data-testid="reading-pane"]');
  // Scroll the FRAME to the bottom — and check that there was
  // actually something to scroll, otherwise the test would prove nothing.
  const canScroll = await pane.evaluate((el) => {
    el.scrollTop = el.scrollHeight;
    return el.scrollHeight > el.clientHeight;
  });
  expect(canScroll).toBe(true);

  // Stuck at the top of the frame, not moving with the flow. The
  // poll re-scrolls on every measurement: a body iframe's late load
  // can regrow the flow after the first scroll.
  await expect
    .poll(async () => {
      await pane.evaluate((el) => { el.scrollTop = el.scrollHeight; });
      const frame = await pane.boundingBox();
      const bar = await pane.locator('[data-testid="archive"]').boundingBox();
      if (!bar) return false;
      const offset = bar.y - frame.y;
      // Bounded on BOTH sides: without the sticky behavior, the bar
      // would have left ABOVE the frame (a negative offset) — the
      // net must catch it.
      return offset >= 0 && offset < 48;
    })
    .toBe(true);
  await pane.evaluate((el) => { el.scrollTop = 0; });
});

// Field 2026-09-02 (pass 3 of wave 2's STOP 2, Chief Engineer
// verdict): at screen 03 the bar no longer lives in the scene's
// flow — its buttons are IN the header bar, above the scene,
// whatever the scroll position (BarreFil.svelte, "entete" drawing).
test("screen 03: the sort gestures live in the header bar, above the scene, whatever the scroll position", async () => {
  // Even shorter: a one-message thread's screen 03 is short — enough
  // scrolling is needed for the bar to REACH the top.
  await page.setViewportSize({ width: 1180, height: 320 });
  await page.locator('[data-testid="see-conversation"]').click();
  const conv = page.locator('[data-testid="conversation"]');
  await expect(conv.locator('[data-testid="archive"]')).toBeVisible();

  const scene = conv.locator('.scene');
  const canScroll = await scene.evaluate((el) => {
    el.scrollTop = el.scrollHeight;
    return el.scrollHeight > el.clientHeight;
  });
  expect(canScroll).toBe(true);

  await expect
    .poll(async () => {
      await scene.evaluate((el) => { el.scrollTop = el.scrollHeight; });
      const frame = await scene.boundingBox();
      const bar = await conv.locator('header [data-testid="archive"]').boundingBox();
      if (!bar) return false;
      // Above the scene (in the header), and still visible.
      return bar.y + bar.height <= frame.y + 1 && bar.y >= 0;
    })
    .toBe(true);
  await expect(scene.locator('[data-testid="archive"]')).toHaveCount(0);

  // Back to screen 02 for the following tests.
  await page.locator('[data-testid="back-to-mailbox"]').click();
  await expect(page.locator('[data-testid="reading-pane"]')).toBeVisible();
});
