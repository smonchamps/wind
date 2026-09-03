// The UI applier of PLAN-BASCULE-ANGLAIS (E5b) — the applier GLOSSARY §6
// promised. It renames WHOLE identifiers only, and never inside string
// literals, template-literal text, comments, regex literals, `<style>`
// blocks, HTML attribute names or markup text (the E5a lesson: a bare
// `\bword\b` pass reached all of those). Three passes, one command:
//
//   node scripts/rename/apply-ui.mjs --report   -> the hand-review lists, no write
//   node scripts/rename/apply-ui.mjs            -> apply in place (git is the undo)
//
// 1. identifiers: `dictionary.csv` rows `layer=ui` + the PascalCase
//    component names derived from FILES; in `.svelte` files the JS of
//    `<script>` and of every `{…}` mustache, plus the attribute NAMES
//    of component tags (they are props — `<Foo avis={x}>` must follow
//    `let { avis } = $props()`), never the attribute names of HTML tags
//    (`data-teinte`, `class:ouvert` are the DOM contract, E5d);
// 2. catalogue keys: every string literal exactly equal to an old key of
//    `keys.csv`, in the UI and the two catalogues, plus the template
//    prefixes (`boite.${dest}` → `mailbox.${dest}`);
// 3. files: `git mv` per GLOSSARY §5.1 and every import path that names
//    them, in the UI and in the e2e nets that read them by path.
//
// What it reports (the lists to review by hand before trusting the run):
// string literals in code equal to a dictionary word (a value that bridges
// a renamed object key — `etat['liste']`), a `new` name already declared
// in a file that also declares `old` (a silent shadowing risk), and
// identifiers read after `dataset.` (an attribute name, not renamed).
import { readFileSync, writeFileSync, readdirSync, statSync, existsSync } from 'node:fs';
import { execFileSync } from 'node:child_process';
import path from 'node:path';

const root = path.resolve(import.meta.dirname, '..', '..');
const UI = path.join(root, 'apps', 'desktop', 'ui-v2', 'src');
const report = process.argv.includes('--report');

// --- §5.1 files ---------------------------------------------------------
const FILES = {
  'BarreFil.svelte': 'ThreadBar.svelte',
  'Composition.svelte': 'Compose.svelte',
  'DrapeauUE.svelte': 'EUFlag.svelte',
  'FenteAvis.svelte': 'NoticeSlot.svelte',
  'Fil.svelte': 'Thread.svelte',
  'GuichetCompte.svelte': 'AccountDesk.svelte',
  'Icone.svelte': 'Icon.svelte',
  'Kiosque.svelte': 'Feed.svelte',
  'Lecture.svelte': 'Reading.svelte',
  'Liste.svelte': 'List.svelte',
  'Marque.svelte': 'Brand.svelte',
  'ModaleMigration.svelte': 'MigrationModal.svelte',
  'Nettoyage.svelte': 'Cleanup.svelte',
  'PileMisDeCote.svelte': 'SetAsidePile.svelte',
  'Portier.svelte': 'Screener.svelte',
  'Registre.svelte': 'PaperTrail.svelte',
  'Reglages.svelte': 'Settings.svelte',
  'Retour.svelte': 'Feedback.svelte',
  'TriSection.svelte': 'SectionSort.svelte',
  'lib/accueil.js': 'lib/onboarding.js',
  'lib/boite.js': 'lib/mailbox.js',
  'lib/clavier.js': 'lib/keyboard.js',
  'lib/corps.js': 'lib/body.js',
  'lib/icones.js': 'lib/icons.js',
  'lib/liens.js': 'lib/links.js',
  'lib/portier.js': 'lib/screener.js',
  'lib/quand.js': 'lib/when.js',
  'lib/reperes.js': 'lib/markers.js',
  'lib/tri.js': 'lib/sort.js',
  'lib/vocabulaires.js': 'lib/vocabularies.js',
  'lib/espacement.svelte.js': 'lib/spacing.svelte.js',
  'lib/fil.svelte.js': 'lib/thread.svelte.js',
  'lib/largeurs.svelte.js': 'lib/widths.svelte.js',
  'lib/organise.svelte.js': 'lib/organized.svelte.js',
  'lib/texte.svelte.js': 'lib/text.svelte.js',
  'lib/volets.svelte.js': 'lib/panes.svelte.js',
  'lib/catalogue.fr.js': 'lib/catalog.fr.js',
  'lib/catalogue.en.js': 'lib/catalog.en.js',
};

// --- the dictionary ------------------------------------------------------
const csv = (f) => readFileSync(path.join(root, 'scripts', 'rename', f), 'utf8').split(/\r?\n/).filter(Boolean);
const dict = new Map();
for (const line of csv('dictionary.csv').slice(1)) {
  const [layer, old, nw] = line.split(',');
  if (layer === 'ui' && old && nw && old !== nw) dict.set(old, nw);
}
for (const [from, to] of Object.entries(FILES)) {
  if (from.endsWith('.svelte')) dict.set(from.replace('.svelte', ''), to.replace('.svelte', ''));
}
const keys = new Map();
for (const line of csv('keys.csv').slice(1)) {
  const [old, nw] = line.split(',');
  if (old && nw && old !== nw) keys.set(old, nw);
}
// Template prefixes: an old namespace that maps to one new namespace
// across every row (`boite.` → `mailbox.`), applied to the head of a
// template literal (`\`boite.${dest}\``).
const prefixes = new Map();
for (const [old, nw] of keys) {
  const o = old.match(/^(.*[._])[^._]+$/), n = nw.match(/^(.*[._])[^._]+$/);
  if (!o || !n) continue;
  const seen = prefixes.get(o[1]);
  if (seen === undefined) prefixes.set(o[1], n[1]);
  else if (seen !== n[1]) prefixes.set(o[1], null); // ambiguous: by hand
}
for (const [k, v] of [...prefixes]) if (v === null || v === k) prefixes.delete(k);
// longest prefix first: `nettoyage.perimetre.` must win over `nettoyage.`
const PREFIXES = [...prefixes].sort((x, y) => y[0].length - x[0].length);

// --- the JS tokenizer ----------------------------------------------------
const ID_START = /[A-Za-z_$]/, ID = /[\w$]/;
const found = { strings: [], dataset: [] };

// Renames identifiers in a JS source; `stop` (a char) ends the scan at
// depth 0 (the closing brace of a mustache). Returns [output, index].
function js(src, i, stop, file) {
  let out = '';
  let depth = 0;
  let prev = ''; // last significant char, for the regex-literal heuristic
  const N = src.length;
  while (i < N) {
    const c = src[i];
    if (stop && c === stop && depth === 0) return [out, i];
    // `{/if}`, `{/each}`: a block close, not a regex literal
    if (c === '/' && out === '' && stop === '}') { out += c; i++; continue; }
    if (c === '/' && src[i + 1] === '/') { const e = src.indexOf('\n', i); const j = e < 0 ? N : e; out += src.slice(i, j); i = j; continue; }
    if (c === '/' && src[i + 1] === '*') { const e = src.indexOf('*/', i + 2); const j = e < 0 ? N : e + 2; out += src.slice(i, j); i = j; continue; }
    if (c === "'" || c === '"') {
      let j = i + 1;
      while (j < N && src[j] !== c) { if (src[j] === '\\') j++; j++; }
      const lit = src.slice(i + 1, j);
      if (dict.has(lit)) found.strings.push(`${file}: '${lit}' (→ ${dict.get(lit)})  …${src.slice(Math.max(0, i - 45), j + 12).replace(/\s+/g, ' ')}…`);
      out += c + (keys.get(lit) ?? lit) + c;
      i = j + 1; prev = c; continue;
    }
    if (c === '`') {
      out += c; i++;
      let text = '';
      const flush = () => {
        // the head of the template may be a catalogue-key prefix
        if (out.endsWith('`')) for (const [o, n] of PREFIXES) if (text.startsWith(o)) { text = n + text.slice(o.length); break; }
        out += text; text = '';
      };
      while (i < N && src[i] !== '`') {
        if (src[i] === '\\') { text += src[i] + src[i + 1]; i += 2; continue; }
        if (src[i] === '$' && src[i + 1] === '{') {
          flush(); out += '${';
          const [inner, j] = js(src, i + 2, '}', file);
          out += inner + '}'; i = j + 1; continue;
        }
        text += src[i++];
      }
      flush(); out += '`'; i++; prev = '`'; continue;
    }
    if (c === '/' && !/[\w$)\]]/.test(prev)) { // regex literal
      let j = i + 1, cls = false;
      while (j < N && (cls || src[j] !== '/')) { if (src[j] === '\\') j++; else if (src[j] === '[') cls = true; else if (src[j] === ']') cls = false; j++; }
      while (ID.test(src[j + 1] ?? '')) j++;
      out += src.slice(i, j + 1); i = j + 1; prev = '/'; continue;
    }
    if (ID_START.test(c)) {
      let j = i + 1;
      while (j < N && ID.test(src[j])) j++;
      const word = src.slice(i, j);
      const afterDataset = out.endsWith('dataset.');
      // the glyph ids of icons.js are the System's contract (figcaptions,
      // DC-D2) — they move with the System, not with the dictionary
      if (file.endsWith('lib/icons.js') && /(^|\n) {2}$/.test(out) && src[j] === ':') { out += word; i = j; prev = word[word.length - 1]; continue; }
      if (afterDataset && dict.has(word)) found.dataset.push(`${file}: dataset.${word}`);
      out += !afterDataset && dict.has(word) ? dict.get(word) : word;
      i = j; prev = word[word.length - 1]; continue;
    }
    if (c === '{') depth++;
    else if (c === '}') depth--;
    if (!/\s/.test(c)) prev = c;
    out += c; i++;
  }
  return [out, i];
}

// --- the Svelte walker ---------------------------------------------------
function svelte(src, file) {
  let out = '', i = 0;
  const N = src.length;
  while (i < N) {
    if (src.startsWith('<!--', i)) { const e = src.indexOf('-->', i); const j = e < 0 ? N : e + 3; out += src.slice(i, j); i = j; continue; }
    if (src.startsWith('<style', i)) { const e = src.indexOf('</style>', i); const j = e < 0 ? N : e + 8; out += src.slice(i, j); i = j; continue; }
    if (src.startsWith('<script', i)) {
      const open = src.indexOf('>', i) + 1;
      out += src.slice(i, open);
      const e = src.indexOf('</script>', open);
      const [body] = js(src.slice(open, e), 0, null, file);
      out += body + '</script>'; i = e + 9; continue;
    }
    if (src[i] === '{') {
      const [inner, j] = js(src, i + 1, '}', file);
      out += '{' + inner + '}'; i = j + 1; continue;
    }
    if (src[i] === '<' && /[A-Za-z/]/.test(src[i + 1] ?? '')) {
      // a tag: rename the tag name if a component, the attribute names
      // if a component tag, the mustaches everywhere
      let j = i + 1;
      if (src[j] === '/') j++;
      let k = j;
      while (k < N && /[\w.:-]/.test(src[k])) k++;
      const tag = src.slice(j, k);
      const comp = /^[A-Z]/.test(tag);
      out += src.slice(i, j) + (comp && dict.has(tag) ? dict.get(tag) : tag);
      i = k;
      // attributes until '>'
      while (i < N && src[i] !== '>') {
        const c = src[i];
        if (c === '{') { const [inner, j2] = js(src, i + 1, '}', file); out += '{' + inner + '}'; i = j2 + 1; continue; }
        if (c === '"' || c === "'") {
          // quoted attribute value: mustaches inside are expressions
          let j2 = i + 1; out += c;
          while (j2 < N && src[j2] !== c) {
            if (src[j2] === '{') { const [inner, j3] = js(src, j2 + 1, '}', file); out += '{' + inner + '}'; j2 = j3 + 1; continue; }
            out += src[j2++];
          }
          out += c; i = j2 + 1; continue;
        }
        if (/[A-Za-z_$]/.test(c)) {
          let j2 = i;
          while (j2 < N && /[\w$:.-]/.test(src[j2])) j2++;
          let name = src.slice(i, j2);
          if (comp) {
            // `bind:x`, plain `x`; never `on:` / `class:` / `use:` / `style:`
            const m = name.match(/^(bind:)?([\w$]+)$/);
            if (m && dict.has(m[2])) name = (m[1] ?? '') + dict.get(m[2]);
          } else {
            // an action is a function: `use:corpsAuto` → `use:autoBody`;
            // a bare `class:centre` binds the CLASS `centre` (DOM contract,
            // E5d) to the VARIABLE `centre` — the class stays, the
            // variable follows: `class:centre={center}`
            const u = name.match(/^use:([\w$]+)$/);
            if (u && dict.has(u[1])) name = 'use:' + dict.get(u[1]);
            const k = name.match(/^class:([\w$-]+)$/);
            if (k && dict.has(k[1]) && src[j2] !== '=') name = `${name}={${dict.get(k[1])}}`;
          }
          out += name; i = j2; continue;
        }
        out += c; i++;
      }
      continue;
    }
    out += src[i++];
  }
  return out;
}

// --- import paths --------------------------------------------------------
function paths(src) {
  for (const [from, to] of Object.entries(FILES)) {
    const base = path.basename(from), nb = path.basename(to);
    src = src.replace(new RegExp(`(['"\`][^'"\`]*/)${base.replace(/\./g, '\\.')}(['"\`])`, 'g'), `$1${nb}$2`);
  }
  return src;
}

// --- run -----------------------------------------------------------------
function walk(d, out = []) { for (const f of readdirSync(d)) { const p = path.join(d, f); if (statSync(p).isDirectory()) walk(p, out); else if (/\.(js|svelte)$/.test(f)) out.push(p); } return out; }
const changed = [];
for (const f of walk(UI)) {
  const rel = path.relative(root, f).replace(/\\/g, '/');
  const src = readFileSync(f, 'utf8');
  const isCatalog = /catalogue\.(fr|en)\.js$/.test(f);
  let out;
  if (isCatalog) {
    // keys only — the values are the delivered text (D3)
    out = src.replace(/^(\s+)'([^']+)':/gm, (m, ws, k) => `${ws}'${keys.get(k) ?? k}':`);
  } else if (f.endsWith('.svelte')) out = svelte(src, rel);
  else [out] = js(src, 0, null, rel);
  out = paths(out);
  if (out !== src) { changed.push(rel); if (!report) writeFileSync(f, out); }
}

// e2e nets and specs that import a renamed UI file by path
for (const rel of ['e2e/catalogues.test.mjs', 'e2e/tests/refonte-langue.spec.js', 'e2e/coherence-systeme.mjs']) {
  const f = path.join(root, rel);
  const src = readFileSync(f, 'utf8');
  const out = paths(src)
    .replace(/catalogue\.\$\{langue\}\.js/g, 'catalog.${langue}.js')
    .replace(/'reperes\.js'/g, "'markers.js'")
    .replace(/'icones\.js'/g, "'icons.js'")
    .replace(/lib\$\{path\.sep\}icones\.js/g, 'lib${path.sep}icons.js');
  if (out !== src) { changed.push(rel); if (!report) writeFileSync(f, out); }
}

// shadowing risk: `new` already declared in a file that declares `old`
const shadow = [];
for (const f of walk(UI)) {
  const rel = path.relative(root, f).replace(/\\/g, '/');
  const src = readFileSync(f, 'utf8');
  for (const [old, nw] of dict) {
    if (!/^[a-z]/.test(old)) continue;
    const decl = (w) => new RegExp(`\\b(let|const|var|function|import)\\s+(\\{[^}]*\\b)?${w}\\b|\\(([^)]*,\\s*)?${w}\\s*[,)=]`).test(src);
    if (decl(old) && decl(nw)) shadow.push(`${rel}: ${old} → ${nw}, and ${nw} is already declared`);
  }
}

if (report) {
  console.log(`== would change ${changed.length} files ==\n${changed.join('\n')}`);
  console.log(`\n== template prefixes ==\n${PREFIXES.map(([a, b]) => `${a} → ${b}`).join('\n')}`);
  console.log(`\n== string literals equal to a dictionary word (code, not comments) ==\n${found.strings.join('\n')}`);
  console.log(`\n== dataset.<word> reads (attribute names, NOT renamed) ==\n${found.dataset.join('\n')}`);
  console.log(`\n== shadowing risk (new already declared next to old) ==\n${shadow.join('\n')}`);
  process.exit(0);
}

// files last: the imports above already name the new paths
for (const [from, to] of Object.entries(FILES)) {
  const a = path.join(UI, from), b = path.join(UI, to);
  if (existsSync(a) && !existsSync(b)) execFileSync('git', ['mv', a, b], { cwd: root });
}
console.log(`applied: ${changed.length} files rewritten, ${Object.keys(FILES).length} files renamed`);
