// The DOM-contract applier of PLAN-ENGLISH-SWITCH (E5d, CE decisions
// D15, D18-D21 of 2026-09-03). `dom.csv` is the single table — four
// kinds: `testid`, `class`, `attr` (a `data-*` attribute NAME), `seam`
// (a `window.__e2e*` hook). One command, the specs and the UI in the
// same run so the contract never has two sides:
//
//   node scripts/rename/apply-dom.mjs --report   -> counts, the hand-review lists, no write
//   node scripts/rename/apply-dom.mjs            -> apply in place (git is the undo)
//
// What it rewrites, by place:
// - Svelte markup: the tokens of `class="…"` (a mustache token keeps its
//   expression, its prefix follows: `ton-{x}` → `tone-{x}`), the name of a
//   `class:old=` directive, a bare `class:old` (→ `class:new={old}`: the
//   class moves, the variable stays), `data-testid="…"` and the `testid`
//   prop (exact, or the prefix of a rendered id: `barre-{a}` → `bar-{a}`),
//   the `data-*` attribute names;
// - `<style>` blocks and `system.css`: `.old` bounded on both sides
//   (`.ligne` never touches `.ligne-autre`), `[data-old` — comments kept;
// - JS (the `<script>`, the mustaches, `lib/`, the specs, the e2e tools):
//   the seams as whole identifiers, `dataset.old`, and INSIDE string or
//   template literals only: `[data-testid="old"]`, `getByTestId('old')`,
//   the template prefix `` `old-${ ``, `.old` in a selector position (after
//   a combinator, a quote, or an HTML tag name), `[data-old`,
//   `toHaveClass(/old/)`.
// Never: identifiers (E5b), catalogue keys, prose, the wire values (E5a).
import { readFileSync, writeFileSync, readdirSync, statSync } from 'node:fs';
import path from 'node:path';

const root = path.resolve(import.meta.dirname, '..', '..');
const UI = path.join(root, 'apps', 'desktop', 'ui-v2', 'src');
const E2E = path.join(root, 'e2e');

// --- the tables ----------------------------------------------------------
export function tablesFrom(lines) {
  const T = { testid: new Map(), class: new Map(), attr: new Map(), seam: new Map() };
  for (const l of lines.slice(1)) {
    const [kind, old, nw] = l.split(',').map((s) => s?.trim());
    if (!T[kind] || !old || !nw || old === nw) continue;
    T[kind].set(old, nw);
  }
  // `dataset.teinte` reads the attribute `data-teinte`
  T.dataset = new Map([...T.attr].map(([o, n]) => [o.replace(/^data-/, ''), n.replace(/^data-/, '')]));
  return T;
}
export const loadTables = () =>
  tablesFrom(readFileSync(path.join(root, 'scripts', 'rename', 'dom.csv'), 'utf8').split(/\r?\n/).filter(Boolean));

const found = { literals: [], bareIds: [] };
const HTML_TAG = /^(article|aside|a|button|div|footer|h[1-6]|header|iframe|img|input|label|li|main|nav|ol|p|section|span|svg|td|th|tr|ul|body|html|form|select|textarea|figure|figcaption|summary|details|small|strong|em|b|i)$/;

// a rendered id prefix: `barre-{g.action}` → the `barre` row, if any
const prefixOf = (T, word) => T.testid.get(word) ?? null;
// the `data-*` attribute NAME, wherever it stands before `=`, `]` or a selector operator
const DATA_ATTR = /(?<![\w-])(data-[a-z][a-z-]*)(?=[=\]^$*~|])/g;
const dataAttrs = (text, T) => text.replace(DATA_ATTR, (m, a) => T.attr.get(a) ?? m);
// a bare string literal in a spec: an id, an id built on a prefix row
// (`gestes-paper_trail` → `gestures-paper_trail`), an attribute name, or
// a class passed to `classList.*(`; null when it is none of those
function bareName(lit, T, before) {
  if (T.testid.has(lit)) return T.testid.get(lit);
  if (T.attr.has(lit)) return T.attr.get(lit);
  if (/classList\.(contains|add|remove|toggle)\($/.test(before) && T.class.has(lit)) return T.class.get(lit);
  if (/^[a-z][\w-]*-[\w-]+$/.test(lit)) {
    // longest prefix row first: `kiosque-vers-x` before `kiosque-x`
    const parts = lit.split('-');
    for (let n = parts.length - 1; n >= 1; n--) {
      const head = parts.slice(0, n).join('-');
      if (T.testid.has(head)) return `${T.testid.get(head)}-${parts.slice(n).join('-')}`;
    }
  }
  return null;
}

// --- CSS: <style> blocks and system.css ----------------------------------
export function rewriteCss(src, T) {
  return src.split(/(\/\*[\s\S]*?\*\/)/).map((part, i) => {
    if (i % 2) return part; // a comment
    return dataAttrs(part, T)
      .replace(/\.([A-Za-z][\w-]*)(?![\w-])/g, (m, c) => (T.class.has(c) ? `.${T.class.get(c)}` : m))
      // a test id used as a CSS hook: `.header [data-testid="ecrire"]`, either quote
      .replace(/data-testid=(["'])([\w-]+)\1/g, (m, q, id) => (T.testid.has(id) ? `data-testid=${q}${T.testid.get(id)}${q}` : m));
  }).join('');
}

// --- JS: scripts, mustaches, lib/, specs, e2e tools ----------------------
// Inside a string or template literal, the selector forms of the contract.
function literal(text, T, file, beforeExpr = false) {
  const before = text;
  // the head of a template literal may end on an id prefix: `poignee-${pane}`
  if (beforeExpr) text = text.replace(/data-testid=(\\?["'])([a-z][\w-]*)-$/, (m, q, w) => (prefixOf(T, w) ? `data-testid=${q}${prefixOf(T, w)}-` : m));
  text = text
    .replace(/data-testid=(\\?["'])([\w-]+)\1/g, (m, q, id) => (T.testid.has(id) ? `data-testid=${q}${T.testid.get(id)}${q}` : m))
    .replace(/data-testid=(\\?["'])([a-z][\w-]*)-\$\{/g, (m, q, w) => (prefixOf(T, w) ? `data-testid=${q}${prefixOf(T, w)}-\${` : m))
    .replace(DATA_ATTR, (m, a) => T.attr.get(a) ?? m)
    .replace(/(^|[\s>+~,(\[\]])([a-z][a-z0-9]*)?\.([a-z][\w-]*)(?![\w-])/g, (m, pre, tag, c) => {
      if (!T.class.has(c)) return m;
      if (tag && !HTML_TAG.test(tag)) return m;
      return `${pre}${tag ?? ''}.${T.class.get(c)}`;
    });
  if (text !== before && !/data-testid/.test(before)) found.literals.push(`${file}: ${before.trim().slice(0, 70)} → ${text.trim().slice(0, 70)}`);
  return text;
}

export function rewriteJs(src, T, file = '') {
  let out = '', i = 0;
  const N = src.length;
  let prev = '';
  while (i < N) {
    const c = src[i];
    if (c === '/' && src[i + 1] === '/') { const e = src.indexOf('\n', i); const j = e < 0 ? N : e; out += seams(src.slice(i, j), T); i = j; continue; }
    if (c === '/' && src[i + 1] === '*') { const e = src.indexOf('*/', i + 2); const j = e < 0 ? N : e + 2; out += seams(src.slice(i, j), T); i = j; continue; }
    if (c === "'" || c === '"') {
      let j = i + 1;
      while (j < N && src[j] !== c && src[j] !== '\n') { if (src[j] === '\\') j++; j++; }
      const lit = src.slice(i + 1, j);
      // In a spec, a bare literal equal to a contract name is that name:
      // a helper taking the id (`saisir('poignee-list', 120)`), an id
      // compared to a value-built one (`toBe('gestes-paper_trail')`: the
      // prefix row applies), `toHaveAttribute('data-teinte', …)`,
      // `classList.contains('nonlu')`. Every hit goes to the review list.
      const bare = file.startsWith('e2e/') ? bareName(lit, T, out) : null;
      if (bare !== null) { found.bareIds.push(`${file}: '${lit}' → '${bare}'`); out += c + bare + c; i = j + 1; prev = c; continue; }
      out += c + literal(lit, T, file) + (src[j] === c ? c : '');
      i = j + 1; prev = c; continue;
    }
    if (c === '`') {
      out += c; i++;
      let text = '';
      const flush = (beforeExpr) => { out += literal(text, T, file, beforeExpr); text = ''; };
      while (i < N && src[i] !== '`') {
        if (src[i] === '\\') { text += src[i] + src[i + 1]; i += 2; continue; }
        if (src[i] === '$' && src[i + 1] === '{') {
          flush(true); out += '${';
          let depth = 1, j = i + 2;
          while (j < N && depth) { if (src[j] === '{') depth++; else if (src[j] === '}') depth--; if (depth) j++; }
          out += rewriteJs(src.slice(i + 2, j), T, file) + '}'; i = j + 1; continue;
        }
        text += src[i++];
      }
      flush(); out += '`'; i++; prev = '`'; continue;
    }
    if (c === '/' && !/[\w$)\]]/.test(prev)) { // a regex literal: `toHaveClass(/choisie/)`
      let j = i + 1, cls = false;
      while (j < N && (cls || src[j] !== '/') && src[j] !== '\n') { if (src[j] === '\\') j++; else if (src[j] === '[') cls = true; else if (src[j] === ']') cls = false; j++; }
      while (/[\w$]/.test(src[j + 1] ?? '')) j++;
      let re = src.slice(i, j + 1);
      if (out.endsWith('toHaveClass(')) re = re.replace(/^\/([\w-]+)\/$/, (m, w) => (T.class.has(w) ? `/${T.class.get(w)}/` : m));
      out += re; i = j + 1; prev = '/'; continue;
    }
    if (/[A-Za-z_$]/.test(c)) {
      let j = i + 1;
      while (j < N && /[\w$]/.test(src[j])) j++;
      const word = src.slice(i, j);
      if (out.endsWith('dataset.') && T.dataset.has(word)) out += T.dataset.get(word);
      else if (word.startsWith('__e2e') && T.seam.has(word)) out += T.seam.get(word);
      else if (word === 'getByTestId' && src[j] === '(' && /^['"]/.test(src[j + 1] ?? '')) {
        const q = src[j + 1], e = src.indexOf(q, j + 2), id = src.slice(j + 2, e);
        out += `getByTestId(${q}${T.testid.get(id) ?? id}${q}`; i = e + 1; prev = q; continue;
      } else out += word;
      i = j; prev = word[word.length - 1]; continue;
    }
    if (!/\s/.test(c)) prev = c;
    out += c; i++;
  }
  return out;
}
const seams = (text, T) => text.replace(/__e2e[A-Za-z]+/g, (m) => T.seam.get(m) ?? m);

// --- Svelte: markup, <style>, <script>, mustaches -------------------------
const HOLE = /<!H(\d+)!>/g, PREFIX = /^([a-z][\w-]*)-(?=<!H\d+!>)/;
function markup(text, T, file) {
  // mustaches are JS (template ids, seams) — rewritten apart and held by
  // a placeholder meanwhile, so `class="puce ton-{x}"` stays ONE value
  const holes = [];
  // a rendered id in a template mustache: `data-testid={`gestes-${dest}`}`
  text = text.replace(/data-testid=\{`([a-z][\w-]*)-\$\{/g, (m, w) => (prefixOf(T, w) ? `data-testid={\`${prefixOf(T, w)}-\${` : m));
  let flat = '', i = 0;
  const N = text.length;
  while (i < N) {
    if (text[i] === '{') {
      let depth = 1, j = i + 1;
      while (j < N && depth) { if (text[j] === '{') depth++; else if (text[j] === '}') depth--; if (depth) j++; }
      holes.push('{' + rewriteJs(text.slice(i + 1, j), T, file) + '}');
      flat += `<!H${holes.length - 1}!>`; i = j + 1; continue;
    }
    flat += text[i++];
  }
  return flat
    .replace(/class="([^"]*)"/g, (m, v) => `class="${v.split(/(\s+)/).map((tok) => {
      if (!tok || /^\s+$/.test(tok)) return tok;
      const p = tok.match(PREFIX); // `ton-{chip.tone}`: the prefix follows
      if (p) return T.class.has(p[1]) ? `${T.class.get(p[1])}-` + tok.slice(p[0].length) : tok;
      return T.class.get(tok) ?? tok;
    }).join('')}"`)
    .replace(/class:([\w-]+)(?=(=|[\s/>]))/g, (m, c, nx) => {
      if (!T.class.has(c)) return m;
      return nx === '=' ? `class:${T.class.get(c)}` : `class:${T.class.get(c)}={${c}}`;
    })
    .replace(/data-testid="([^"]*)"/g, (m, v) => {
      if (T.testid.has(v)) return `data-testid="${T.testid.get(v)}"`;
      const p = v.match(PREFIX);
      return p && prefixOf(T, p[1]) ? `data-testid="${prefixOf(T, p[1])}-` + v.slice(p[0].length) + '"' : m;
    })
    .replace(/(\s)testid="([^"]*)"/g, (m, ws, v) => (T.testid.has(v) ? `${ws}testid="${T.testid.get(v)}"` : m))
    .replace(DATA_ATTR, (m, a) => T.attr.get(a) ?? m)
    .replace(HOLE, (m, n) => holes[Number(n)]);
}

export function rewriteSvelte(src, T, file = '') {
  let out = '', i = 0;
  const N = src.length;
  while (i < N) {
    if (src.startsWith('<!--', i)) { const e = src.indexOf('-->', i); const j = e < 0 ? N : e + 3; out += src.slice(i, j); i = j; continue; }
    if (src.startsWith('<style', i)) {
      const open = src.indexOf('>', i) + 1, e = src.indexOf('</style>', open);
      out += src.slice(i, open) + rewriteCss(src.slice(open, e), T) + '</style>'; i = e + 8; continue;
    }
    if (src.startsWith('<script', i)) {
      const open = src.indexOf('>', i) + 1, e = src.indexOf('</script>', open);
      out += src.slice(i, open) + rewriteJs(src.slice(open, e), T, file) + '</script>'; i = e + 9; continue;
    }
    // markup up to the next comment / style / script
    let j = N;
    for (const tag of ['<!--', '<style', '<script']) { const k = src.indexOf(tag, i); if (k >= 0 && k < j) j = k; }
    out += markup(src.slice(i, j), T, file); i = j;
  }
  return out;
}

// --- the contract's two sides -------------------------------------------
// Every id a spec or an e2e tool selects must be rendered by the UI: a
// static `data-testid`, the `testid` prop of `Menu`, or an id built on a
// rendered prefix (`bar-{g.action}`). `contents` maps a repo-relative
// path to its text; missing files are read from disk.
export function specIdsAgainstUi(contents = new Map()) {
  const read = (f) => { const rel = path.relative(root, f).replace(/\\/g, '/'); return contents.get(rel) ?? readFileSync(f, 'utf8'); };
  const renderedIds = new Set(), prefixes = new Set();
  for (const f of walk(UI, /\.svelte$/)) {
    const s = read(f);
    for (const m of s.matchAll(/data-testid="([^"{]+)"/g)) renderedIds.add(m[1]);
    for (const m of s.matchAll(/(?:^|\s)testid="([^"{]+)"/g)) renderedIds.add(m[1]);
    for (const m of s.matchAll(/data-testid=(?:"([\w-]+)-\{|\{`([\w-]+)-\$\{)/g)) prefixes.add(m[1] ?? m[2]);
  }
  const prefixList = [...prefixes].map((p) => p + '-');
  const stale = new Map(); // id -> files
  for (const f of specFiles()) {
    const s = read(f);
    for (const m of s.matchAll(/data-testid=\\?["']([\w-]+)\\?["']|getByTestId\(['"]([\w-]+)['"]\)/g)) {
      const id = m[1] ?? m[2];
      if (renderedIds.has(id) || prefixList.some((p) => id.startsWith(p))) continue;
      if (!stale.has(id)) stale.set(id, new Set());
      stale.get(id).add(path.relative(root, f).replace(/\\/g, '/'));
    }
  }
  return { renderedIds, prefixes, stale };
}
const specFiles = () => [...walk(path.join(E2E, 'tests'), /\.js$/), ...readdirSync(E2E).filter((f) => /\.mjs$/.test(f) && !f.endsWith('.test.mjs')).map((f) => path.join(E2E, f))];

// --- run -----------------------------------------------------------------
function walk(d, re, out = []) { for (const f of readdirSync(d)) { const p = path.join(d, f); if (f === 'node_modules' || f === 'test-results') continue; if (statSync(p).isDirectory()) walk(p, re, out); else if (re.test(f)) out.push(p); } return out; }

if (process.argv[1] && path.resolve(process.argv[1]) === path.resolve(import.meta.filename)) {
  const report = process.argv.includes('--report');
  const T = loadTables();
  const files = [...walk(UI, /\.(svelte|js|css)$/), ...specFiles()];
  const changed = [], outputs = new Map();
  for (const f of files) {
    const rel = path.relative(root, f).replace(/\\/g, '/');
    const src = readFileSync(f, 'utf8');
    const out = f.endsWith('.svelte') ? rewriteSvelte(src, T, rel) : f.endsWith('.css') ? rewriteCss(src, T) : rewriteJs(src, T, rel);
    outputs.set(rel, out);
    if (out !== src) { changed.push(rel); if (!report) writeFileSync(f, out); }
  }
  // the specs' ids against the UI's, after the pass — the same check
  // lives on as a net in e2e/dom-contract.test.mjs
  const { stale } = specIdsAgainstUi(outputs);
  console.log(`${report ? 'would rewrite' : 'rewrote'} ${changed.length} files:\n${changed.join('\n')}`);
  console.log(`\n== string literals rewritten in JS other than [data-testid=…] (review) ==\n${[...new Set(found.literals)].join('\n')}`);
  console.log(`\n== bare id literals rewritten in the specs (review) ==\n${found.bareIds.join('\n')}`);
  console.log(`\n== spec ids the UI does not render (after the pass) ==\n${[...stale.keys()].sort().join(' ')}`);
}
