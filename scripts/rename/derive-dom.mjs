// PLAN-BASCULE-ANGLAIS E5d — completes `dom.csv` from what the UI and the
// specs ACTUALLY carry (the E5b lesson: the E0 inventory missed what it
// did not parse — 124 classes, the `data-*` attribute names, the ids the
// `Menu` prop and the specs build from a value). Every name found is
// derived segment by segment from `tokens.csv`; a name whose derivation
// differs and that `dom.csv` does not know yet becomes a row. Never
// patched by hand: strike or add a word in `tokens.csv`, run again.
//
//   node scripts/rename/derive-dom.mjs           -> report (rows to add, merges to check)
//   node scripts/rename/derive-dom.mjs --write   -> append the rows to dom.csv
import { readFileSync, writeFileSync, readdirSync, statSync } from 'node:fs';
import path from 'node:path';

const root = path.resolve(import.meta.dirname, '..', '..');
const UI = path.join(root, 'apps', 'desktop', 'ui-v2', 'src');
const E2E = path.join(root, 'e2e');
const write = process.argv.includes('--write');
const rd = (f) => readFileSync(f, 'utf8');
const csv = (f) => rd(path.join(root, 'scripts', 'rename', f)).split(/\r?\n/).filter(Boolean);

const tokens = new Map();
for (const l of csv('tokens.csv').slice(1)) { const [o, n] = l.split(','); if (o && !tokens.has(o)) tokens.set(o, n); }
const dom = new Map(); // kind:old -> new
const known = new Set(); // every old and new, per kind
for (const l of csv('dom.csv').slice(1)) { const [k, o, n] = l.split(','); dom.set(`${k}:${o}`, n); known.add(`${k}:${o}`); known.add(`${k}:${n}`); }

// a segment the glossary does not know is kept (an English word, an
// abbreviation, a letter the System names)
// a token that is a wire VALUE (`paper_trail`) becomes kebab in a DOM name
const derive = (name) => name.split("-").map((s) => (tokens.get(s) ?? s).replace(/_/g, "-")).join("-");

function walk(d, re, out = []) { for (const f of readdirSync(d)) { const p = path.join(d, f); if (f === 'node_modules' || f === 'test-results') continue; if (statSync(p).isDirectory()) walk(p, re, out); else if (re.test(f)) out.push(p); } return out; }
const uiFiles = walk(UI, /\.(svelte|js|css)$/);
// the applier's own test carries French fixtures on purpose
const e2eFiles = [...walk(path.join(E2E, 'tests'), /\.js$/), ...readdirSync(E2E).filter((f) => /\.mjs$/.test(f) && f !== 'apply-dom.test.mjs').map((f) => path.join(E2E, f))];

const found = { testid: new Map(), class: new Map(), attr: new Map() }; // name -> Set(files)
const add = (kind, name, f) => { if (!found[kind].has(name)) found[kind].set(name, new Set()); found[kind].get(name).add(path.relative(root, f).replace(/\\/g, '/')); };

for (const f of uiFiles) {
  const src = rd(f);
  for (const m of src.matchAll(/data-testid="([^"{]+)"/g)) add('testid', m[1], f);
  for (const m of src.matchAll(/(?:^|\s)testid="([^"{]+)"/g)) add('testid', m[1], f);
  // a prefix built at render: `barre-{g.action}`, `` `gestes-${dest}` ``
  for (const m of src.matchAll(/data-testid=(?:"([a-z-]+)-\{|\{`([a-z-]+)-\$\{)/g)) add('testid', m[1] ?? m[2], f);
  if (f.endsWith('.svelte')) {
    for (const m of src.matchAll(/class="([^"]*)"/g)) for (const t of m[1].split(/\s+/)) if (/^[a-zA-Z][\w-]*$/.test(t)) add('class', t, f);
    for (const m of src.matchAll(/class:([a-zA-Z][\w-]*)/g)) add('class', m[1], f);
    // a mustache prefix inside a class attribute: `ton-{chip.tone}`
    for (const m of src.matchAll(/class="[^"]*?\b([a-z]+)-\{/g)) add('class', m[1], f);
  }
  if (/\.(svelte|css)$/.test(f)) {
    const styles = f.endsWith('.css') ? [src] : [...src.matchAll(/<style[^>]*>([\s\S]*?)<\/style>/g)].map((m) => m[1]);
    // `.chip.ton-cancelled`: a chained selector, both classes count
    for (const s of styles) for (const m of s.replace(/\/\*[\s\S]*?\*\//g, '').matchAll(/\.([a-zA-Z][\w-]*)(?![\w-])/g)) add('class', m[1], f);
  }
  for (const m of src.matchAll(/(?<![\w-])data-([a-z][a-z-]*)=/g)) add('attr', m[1], f);
  for (const m of src.matchAll(/dataset\.([a-zA-Z]+)/g)) add('attr', m[1], f);
}
// the specs: the literal ids (built from a value included), the class
// selectors, the attribute names they read
for (const f of e2eFiles) {
  const src = rd(f);
  for (const m of src.matchAll(/data-testid="([^"$]+)"/g)) add('testid', m[1], f);
  for (const m of src.matchAll(/getByTestId\(['"]([^'"]+)['"]\)/g)) add('testid', m[1], f);
  for (const m of src.matchAll(/data-testid="([a-z-]+)-\$\{/g)) add('testid', m[1], f);
  for (const m of src.matchAll(/(?<=['"`(\s>+~,])\.([a-z][a-z0-9-]*)(?=[\s:.,[>#+~)"'`\]]|$)/g)) if (!/^\d/.test(m[1])) add('class', m[1], f);
  for (const m of src.matchAll(/toHaveClass\(\/([a-z-]+)\/\)/g)) add('class', m[1], f);
  for (const m of src.matchAll(/(?<![\w-])data-([a-z][a-z-]*)[=\]^$*]/g)) add('attr', m[1], f);
  for (const m of src.matchAll(/dataset\.([a-zA-Z]+)/g)) add('attr', m[1], f);
}

const NOT_OURS = new Set(['testid', 'test-id', 'test', 'pw-cursor', 'precedence', 'dir', 'view-component', 'href', 'section', 'jeton', 'vision', 'view', 'wind-transfert', 'value', 'types', 'toggle-second-line', 'toggle', 'separator', 'raw', 'properties', 'other', 'theme', 'theme-id', 'startup', 'placeholder', 'th']);
const rows = [], merges = [];
for (const kind of ['testid', 'class', 'attr']) {
  for (const [name, files] of found[kind]) {
    if (kind === 'attr' && NOT_OURS.has(name)) continue;
    if (known.has(`${kind}:${name}`)) continue;
    const nw = derive(name);
    if (nw === name) continue;
    rows.push([kind, kind === 'attr' ? `data-${name}` : name, kind === 'attr' ? `data-${nw}` : nw]);
    // a merge: the new name already exists as a class in one of the same files
    if (kind === 'class' && found.class.has(nw)) {
      const both = [...files].filter((x) => found.class.get(nw).has(x));
      if (both.length) merges.push(`${name} → ${nw} in ${both.join(', ')} (already a class there)`);
    }
  }
}
// the existing rows are checked for merges too
for (const [k, n] of dom) {
  const [kind, old] = k.split(':');
  if (kind !== 'class' || !found.class.has(old) || !found.class.has(n)) continue;
  const both = [...found.class.get(old)].filter((x) => found.class.get(n).has(x));
  if (both.length) merges.push(`${old} → ${n} in ${both.join(', ')} (already a class there, existing row)`);
}
rows.sort((a, b) => a[0].localeCompare(b[0]) || a[1].localeCompare(b[1]));

console.log(`found: ${found.testid.size} test ids, ${found.class.size} classes, ${found.attr.size} attribute names (UI + specs)`);
console.log(`\n== rows to add (${rows.length}) ==\n${rows.map((r) => r.join(',')).join('\n')}`);
console.log(`\n== merges to check by hand (${merges.length}) ==\n${merges.join('\n')}`);
if (write) {
  const p = path.join(root, 'scripts', 'rename', 'dom.csv');
  const cur = rd(p).replace(/\s+$/, '');
  writeFileSync(p, cur + '\n' + rows.map((r) => r.join(',')).join('\n') + '\n');
  console.log(`\nappended ${rows.length} rows to dom.csv`);
}
