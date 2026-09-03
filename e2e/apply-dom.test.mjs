// PLAN-BASCULE-ANGLAIS E5d (2026-09-03): the DOM-contract applier
// (`scripts/rename/apply-dom.mjs`) rewrites test ids, classes, `data-*`
// attribute names and the `__e2e*` seams from `dom.csv` — in the Svelte
// markup, the `<style>` blocks, `system.css`, and the specs' selector
// literals. A rename missed in one of those places is an unstyled
// element or a spec that finds nothing; this net pins each rewrite rule
// on a fixture so the applier is proven before it touches 60 files.
import test from 'node:test';
import assert from 'node:assert/strict';
import { rewriteSvelte, rewriteCss, rewriteJs, tablesFrom } from '../scripts/rename/apply-dom.mjs';

const T = tablesFrom([
  'kind,old,new',
  'testid,ligne,row',
  'testid,ligne-case,row-checkbox',
  'testid,barre,bar',
  'testid,tri-menu,sort-menu',
  'testid,poignee,handle',
  'testid,poignee-list,handle-list',
  'class,ligne,row',
  'class,poignee,handle',
  'class,tuilee,tiled',
  'class,choisie,chosen',
  'class,accueil,onboarding',
  'class,repere-nu,bare-marker',
  'class,tete-message,message-head',
  'class,primaire,primary',
  'class,carte,card',
  'class,ton,tone',
  'class,nu,bare',
  'attr,data-teinte,data-hue',
  'attr,data-adresse,data-address',
  'seam,__e2eRetenue,__e2eHold',
]);

test('svelte markup: class tokens, class: directives, test ids, attribute names', () => {
  const src = [
    '<div class="ligne tuilee autre" class:choisie={isChosen(row)} data-testid="ligne" data-teinte={hue}>',
    '  <span class="puce ton-{chip.tone}" data-testid="ligne-case" data-adresse="x"></span>',
    '  <button data-testid="barre-{g.action}" class="btn">{t(\'ligne\')}</button>',
    '  <div class="poignee" data-testid="poignee-{pane}"></div>',
    '  <Menu testid="tri-menu" />',
    '  <div class="guichet" class:compact class:accueil></div>',
    '</div>',
  ].join('\n');
  const out = rewriteSvelte(src, T);
  assert.equal(out, [
    '<div class="row tiled autre" class:chosen={isChosen(row)} data-testid="row" data-hue={hue}>',
    '  <span class="puce tone-{chip.tone}" data-testid="row-checkbox" data-address="x"></span>',
    '  <button data-testid="bar-{g.action}" class="btn">{t(\'ligne\')}</button>',
    '  <div class="handle" data-testid="handle-{pane}"></div>',
    '  <Menu testid="sort-menu" />',
    '  <div class="guichet" class:compact class:onboarding={accueil}></div>',
    '</div>',
  ].join('\n'));
});

test('svelte <style> and system.css: bounded class selectors, attribute names', () => {
  const css = [
    '.ligne { display:flex; }',
    '.ligne.tuilee .tete-message, .ligne-autre { color:red; }',
    ':global(.repere-nu[data-teinte="blue"]) { color:var(--mk-blue); }',
    '.carte:hover .nu { opacity:1; } /* .ligne in a comment stays */',
  ].join('\n');
  const want = [
    '.row { display:flex; }',
    '.row.tiled .message-head, .ligne-autre { color:red; }',
    ':global(.bare-marker[data-hue="blue"]) { color:var(--mk-blue); }',
    '.card:hover .bare { opacity:1; } /* .ligne in a comment stays */',
  ].join('\n');
  assert.equal(rewriteCss(css, T), want);
  assert.equal(rewriteSvelte(`<script>let x;</script>\n<style>\n${css}\n</style>`, T), `<script>let x;</script>\n<style>\n${want}\n</style>`);
});

test('svelte script and JS: seams, dataset reads, the template prefixes of test ids', () => {
  const src = [
    'if (window.__e2eRetenue) hold();',
    'const a = el.dataset.adresse;',
    'const sel = `[data-testid="poignee-${pane}"]`;',
    "const cls = main ? 'primaire' : 'other';",
  ].join('\n');
  assert.equal(rewriteJs(src, T), [
    'if (window.__e2eHold) hold();',
    'const a = el.dataset.address;',
    'const sel = `[data-testid="handle-${pane}"]`;',
    "const cls = main ? 'primaire' : 'other';",
  ].join('\n'));
});

test('specs: selector literals, getByTestId, toHaveClass, template ids, seams', () => {
  const src = [
    "const rows = () => page.locator('[data-testid=\"ligne\"]');",
    "await rows().nth(0).locator('[data-testid=\"ligne-case\"]').click();",
    "await page.getByTestId('poignee-list').hover();",
    "await expect(rows().nth(0)).toHaveClass(/choisie/);",
    "await expect(btn).toHaveClass(/nu/);",
    "const t = page.locator('.tete-message .objet, article.carte > .ligne');",
    "await page.locator(`[data-testid=\"barre-${geste}\"]`).click();",
    "const h = page.locator('[data-testid=\"nav-repere\"] .repere-nu[data-teinte=\"blue\"]');",
    "await page.evaluate(() => { window.__e2eRetenue = true; });",
    "const adr = await el.evaluate((e) => e.dataset.adresse);",
    "await page.goto('wind.db'); const f = 'catalog.fr.js';",
    "const n = page.locator('[data-testid=\"ligne\"].choisie [data-testid=\"ligne-case\"]');",
    "await expect(m).toHaveAttribute('data-teinte', 'blue'); const k = el.classList.contains('choisie');",
  ].join('\n');
  assert.equal(rewriteJs(src, T, 'e2e/tests/x.spec.js'), [
    "const rows = () => page.locator('[data-testid=\"row\"]');",
    "await rows().nth(0).locator('[data-testid=\"row-checkbox\"]').click();",
    "await page.getByTestId('handle-list').hover();",
    "await expect(rows().nth(0)).toHaveClass(/chosen/);",
    "await expect(btn).toHaveClass(/bare/);",
    "const t = page.locator('.message-head .objet, article.card > .row');",
    "await page.locator(`[data-testid=\"bar-${geste}\"]`).click();",
    "const h = page.locator('[data-testid=\"nav-repere\"] .bare-marker[data-hue=\"blue\"]');",
    "await page.evaluate(() => { window.__e2eHold = true; });",
    "const adr = await el.evaluate((e) => e.dataset.address);",
    "await page.goto('wind.db'); const f = 'catalog.fr.js';",
    "const n = page.locator('[data-testid=\"row\"].chosen [data-testid=\"row-checkbox\"]');",
    "await expect(m).toHaveAttribute('data-hue', 'blue'); const k = el.classList.contains('chosen');",
  ].join('\n'));
});
