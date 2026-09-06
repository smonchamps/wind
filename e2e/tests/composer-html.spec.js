import { test, expect } from '@playwright/test';
import { launchAppV2, closeApp } from '../launch.mjs';

let app;
let browser;
let page;

test.beforeAll(async () => { ({ app, browser, page } = await launchAppV2()); });
test.afterAll(async () => { await closeApp({ app, browser }); });

async function paste(html, text = '', target = page.locator('[data-testid="compose-body"]')) {
  return target.evaluate((field, content) => {
    field.focus();
    const range = document.createRange();
    range.selectNodeContents(field);
    range.collapse(false);
    const selection = window.getSelection();
    selection.removeAllRanges();
    selection.addRange(range);
    const data = new DataTransfer();
    data.setData('text/html', content.html);
    data.setData('text/plain', content.text);
    return !field.dispatchEvent(new ClipboardEvent('paste', {
      clipboardData: data, bubbles: true, cancelable: true,
    }));
  }, { html, text });
}

test('rich paste is filtered before insertion and retained image URLs survive saving', async () => {
  const attempts = [];
  await page.context().route('https://audit.invalid/**', async (route) => {
    attempts.push(route.request().url());
    await route.abort();
  });
  await page.locator('[data-testid="write"]').click();
  await page.locator('[data-testid="compose-subject"]').fill('Filtered rich paste');
  expect(await paste('<p style="position:fixed;inset:0;background:u\\72l(https://audit.invalid/css);COLOR:red">Keep this text.</p><img src="https://audit.invalid/image">', 'Keep this text.')).toBe(true);
  const body = page.locator('[data-testid="compose-body"]');
  await expect(body).toContainText('Keep this text.');
  await expect(body.locator('p')).toHaveCSS('position', 'static');
  await expect(body.locator('p')).toHaveCSS('color', 'rgb(255, 0, 0)');
  await expect(body.locator('img')).toHaveAttribute('src', /^data:image\/gif/);
  expect(attempts).toEqual([]);
  await page.locator('[data-testid="compose-cancel"]').click();
  await expect(page.locator('[data-testid="compose"]')).toHaveCount(0);
  const saved = await page.evaluate(() => window.__TAURI__.core.invoke('list_drafts'));
  const draft = saved.find((draft) => draft.subject === 'Filtered rich paste');
  expect(draft.body_html).toContain('https://audit.invalid/image');
  expect(draft.body_html).not.toContain('https://audit.invalid/css');
  expect(attempts).toEqual([]);
});

test('an image-only composition is kept when closed', async () => {
  await page.locator('[data-testid="write"]').click();
  await page.locator('[data-testid="compose-body"]').fill('');
  expect(await paste('<img src="data:image/png;base64,AA==">')).toBe(true);
  await expect(page.locator('[data-testid="compose-body"] img')).toHaveCount(1);
  await page.locator('[data-testid="compose-cancel"]').click();
  await expect(page.locator('[data-testid="compose"]')).toHaveCount(0);
  const saved = await page.evaluate(() => window.__TAURI__.core.invoke('list_drafts'));
  expect(saved.some((draft) => draft.body_html?.includes('data:image/png;base64,AA=='))).toBe(true);
});

test('reopening and editing a rich draft preserves only the surviving text and images', async () => {
  await page.locator('[data-testid="write"]').click();
  await page.locator('[data-testid="compose-subject"]').fill('Edited rich draft');
  await paste('<p>Remove this sentence.</p><p>Keep this sentence.</p><img src="https://audit.invalid/removed"><img src="https://audit.invalid/retained">');
  const body = page.locator('[data-testid="compose-body"]');
  await expect(body.locator('img')).toHaveCount(2);
  await page.locator('[data-testid="compose-cancel"]').click();
  await expect(page.locator('[data-testid="compose"]')).toHaveCount(0);
  await page.locator('[data-testid="nav-folder"][data-category="drafts"]').click();
  await page.locator('[data-testid="row-draft"]', { hasText: 'Edited rich draft' }).click();
  await expect(body.locator('img')).toHaveCount(2);
  await expect(body.locator('img').first()).toHaveAttribute('src', /^data:image\/gif/);
  await body.evaluate((field) => {
    field.querySelector('p').remove();
    field.querySelector('img').remove();
    field.dispatchEvent(new InputEvent('input', { bubbles: true, inputType: 'deleteContentForward' }));
  });
  await page.locator('[data-testid="compose-cancel"]').click();
  await expect(page.locator('[data-testid="compose"]')).toHaveCount(0);
  const saved = await page.evaluate(() => window.__TAURI__.core.invoke('list_drafts'));
  const html = saved.find((draft) => draft.subject === 'Edited rich draft').body_html;
  expect(html).toContain('Keep this sentence.');
  expect(html).toContain('https://audit.invalid/retained');
  expect(html).not.toContain('Remove this sentence.');
  expect(html).not.toContain('https://audit.invalid/removed');
});

test('closing waits for rich paste preparation and saves the inserted content', async () => {
  await page.locator('[data-testid="write"]').click();
  await page.locator('[data-testid="compose-subject"]').fill('Paste before close');
  await page.evaluate(() => {
    window.__e2eLog = [];
    window.__e2eHold = new Promise((release) => { window.releaseHtmlCalls = release; });
  });
  try {
    await paste('<b>Prepared before closing.</b>');
    await expect.poll(() => page.evaluate(() => window.__e2eLog.some((entry) =>
      entry.command === 'prepare_composer_html'))).toBe(true);
    await page.locator('[data-testid="compose-cancel"]').click();
    await expect(page.locator('[data-testid="compose"]')).toBeVisible();
  } finally {
    await page.evaluate(() => { delete window.__e2eHold; window.releaseHtmlCalls(); });
  }
  await expect(page.locator('[data-testid="compose"]')).toHaveCount(0);
  const saved = await page.evaluate(() => window.__TAURI__.core.invoke('list_drafts'));
  expect(saved.find((draft) => draft.subject === 'Paste before close').body)
    .toBe('Prepared before closing.');
});

test('a delayed paste cannot overwrite new typing and a successful paste can be undone', async () => {
  await page.locator('[data-testid="write"]').click();
  const body = page.locator('[data-testid="compose-body"]');
  await body.fill('Initial text.');
  await page.evaluate(() => {
    window.__e2eHold = new Promise((release) => { window.releaseHtmlCalls = release; });
  });
  try {
    await paste('<b>Delayed paste.</b>');
    await body.fill('New typing must survive.');
  } finally {
    await page.evaluate(() => { delete window.__e2eHold; window.releaseHtmlCalls(); });
  }
  await expect(page.locator('[data-testid="toast"]')).toContainText('could not be pasted');
  await expect(body).toHaveText('New typing must survive.');
  await paste('<b>Undo this paste.</b>');
  await expect(body).toContainText('Undo this paste.');
  await body.focus();
  await page.keyboard.press('Control+z');
  await expect(body).toHaveText('New typing must survive.');
  await page.locator('[data-testid="compose-cancel"]').click();
  await expect(page.locator('[data-testid="compose"]')).toHaveCount(0);
});

test('signature paste uses the same safe boundary and retains its image when saved', async () => {
  await page.locator('[data-testid="settings"]').click();
  await page.locator('[data-testid="settings-group"][data-group="signature"]').click();
  const editor = page.locator('[data-testid="signature-editor"]').first();
  await expect(editor).toBeVisible();
  expect(await paste('<p style="position:fixed;COLOR:blue">Safe signature</p><img src="https://audit.invalid/signature">', 'Safe signature', editor)).toBe(true);
  await expect(editor.locator('p')).toHaveCSS('position', 'static');
  await expect(editor.locator('img')).toHaveAttribute('src', /^data:image\/gif/);
  await page.locator('[data-testid="signature-save"]').first().click();
  await expect(page.locator('[data-testid="signature-state"]').first()).toContainText('Signature saved.');
  await page.locator('[data-testid="settings-done"]').click();
  await page.locator('[data-testid="settings"]').click();
  await page.locator('[data-testid="settings-group"][data-group="signature"]').click();
  await expect(editor.locator('img')).toHaveAttribute('src', /^data:image\/gif/);
  await expect(editor).toContainText('Safe signature');
  await page.locator('[data-testid="settings-done"]').click();
});

test('saving a signature during its initial load keeps the edited field after Settings closes', async () => {
  await page.locator('[data-testid="settings"]').click();
  await page.evaluate(() => {
    window.__e2eLog = [];
    window.__e2eHold = new Promise((release) => { window.releaseHtmlCalls = release; });
  });
  try {
    await page.locator('[data-testid="settings-group"][data-group="signature"]').click();
    await page.locator('[data-testid="signature-editor"]').first().fill('Saved during signature loading.');
    await page.locator('[data-testid="signature-save"]').first().click();
    await page.locator('[data-testid="settings-done"]').click();
  } finally {
    await page.evaluate(() => { delete window.__e2eHold; window.releaseHtmlCalls(); });
  }
  await expect.poll(() => page.evaluate(() => window.__e2eLog.some((entry) =>
    entry.command === 'signature_set' && entry.arrival !== null))).toBe(true);
  await page.locator('[data-testid="settings"]').click();
  await page.locator('[data-testid="settings-group"][data-group="signature"]').click();
  await expect(page.locator('[data-testid="signature-editor"]').first())
    .toHaveText('Saved during signature loading.');
  await page.locator('[data-testid="settings-done"]').click();
});
