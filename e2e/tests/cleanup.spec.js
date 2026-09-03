// PLAN-HORIZON-NETTOYAGE section B — the Spring cleanup, 5th section
// of Organized mode. The intro (range + scope + Start), the sort by
// sender GROUPS in the Screener's vocabulary (the verdict applies to
// the group: the range's stock AND the future — D5), the progress
// bar at the top, navigation INSIDE a group (view, never sort by
// message), the PERSISTED session (D8: resume after a reload), and
// the clean exit.
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
});

test.afterAll(async () => {
  await closeApp({ app, browser });
});

test('the 5th section only exists in Organized mode, and its intro says the Chief Engineer’s text', async () => {
  await expect(page.locator('[data-testid="row"]').first()).toBeVisible();
  // In classic mode: no Cleanup.
  await expect(
    page.locator('[data-testid="nav-folder"][data-category="cleanup"]'),
  ).toHaveCount(0);

  await page.locator('[data-testid="organized-mode"]').click();
  const rank = page.locator('[data-testid="nav-folder"][data-category="cleanup"]');
  await expect(rank).toContainText('Nettoyage de printemps');
  await rank.click();

  // The intro: title with glyph, subtext the Chief Engineer's exact
  // wording, range (default 1 year), scope (default Inbox only), Start.
  await expect(page.locator('[data-testid="cleanup-title"] svg')).toHaveCount(1);
  await expect(page.locator('[data-testid="cleanup"]')).toContainText(
    'En lançant un nettoyage de printemps, vous allez pouvoir trier vos archives',
  );
  await expect(page.locator('[data-testid="cleanup-range"]')).toHaveCount(6);
  await expect(
    page.locator('[data-testid="cleanup-range"][data-range="1a"]'),
  ).toHaveAttribute('aria-checked', 'true');
  await expect(page.locator('[data-testid="cleanup-scope"]')).toHaveCount(4);
  await expect(
    page.locator('[data-testid="cleanup-scope"][data-scope="inbox"]'),
  ).toHaveAttribute('aria-checked', 'true');
  await expect(page.locator('[data-testid="cleanup-start"]')).toBeVisible();
});

test('starting opens the sort: groups by sender, progress at 0%, navigation inside a group', async () => {
  // The seeded decor dates back to 2020 (template): the "all" range
  // covers it — and proves along the way that the range choice is
  // indeed sent.
  await page.locator('[data-testid="cleanup-range"][data-range="all"]').click();
  await page.locator('[data-testid="cleanup-start"]').click();
  await expect(page.locator('[data-testid="cleanup-group"]').first()).toBeVisible();
  await expect(page.locator('[data-testid="cleanup-progress"]')).toContainText('0 %');

  // Navigate inside a group: its messages show — and collapse.
  await page.locator('[data-testid="cleanup-open"]').first().click();
  await expect(page.locator('[data-testid="cleanup-messages"]')).toBeVisible();
  await page.locator('[data-testid="cleanup-open"]').first().click();
  await expect(page.locator('[data-testid="cleanup-messages"]')).toHaveCount(0);
});

test('Yes processes the whole group; No makes its mail leave the Inbox (D5)', async () => {
  const groups = page.locator('[data-testid="cleanup-group"]');
  const before = await groups.count();
  expect(before).toBeGreaterThan(1);

  // Group Yes: it leaves the list, the progress advances.
  await page.locator('[data-testid="cleanup-yes"]').first().click();
  await expect(groups).toHaveCount(before - 1);
  await expect(page.locator('[data-testid="cleanup-progress"]')).not.toContainText('0 %');

  // No (shipped default: Trash): the group leaves, and its STOCK from
  // the range leaves the local mailbox — the Inbox no longer shows it.
  const noSenderName = await groups.first().locator('.sender').innerText();
  await page.locator('[data-testid="cleanup-no"]').first().click();
  await expect(groups).toHaveCount(before - 2);
  await page.locator('[data-testid="nav-folder"][data-category="inbox"]').click();
  await expect(page.locator('[data-testid="list"]')).not.toContainText(noSenderName);
});

test('the session PERSISTS (D8): a reload resumes the sort where it left off', async () => {
  await page.reload();
  await expect(page.locator('[data-testid="organized-mode"]')).toHaveAttribute(
    'aria-checked',
    'true',
  );
  await page.locator('[data-testid="nav-folder"][data-category="cleanup"]').click();
  // Not the intro: the sort, with its progress already under way.
  await expect(page.locator('[data-testid="cleanup-start"]')).toHaveCount(0);
  await expect(page.locator('[data-testid="cleanup-progress"]')).not.toContainText('0 %');
});

test('finishing gives back the intro; leaving the mode gives back the classic nav', async () => {
  await page.locator('[data-testid="cleanup-finish"]').click();
  await expect(page.locator('[data-testid="cleanup-start"]')).toBeVisible();

  await page.locator('[data-testid="organized-mode"]').click();
  await expect(page.locator('[data-testid="nav-folder"]')).toHaveCount(6);
  await expect(
    page.locator('[data-testid="nav-folder"][data-category="cleanup"]'),
  ).toHaveCount(0);
});
