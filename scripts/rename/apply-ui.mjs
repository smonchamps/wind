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
import { readFileSync, writeFileSync, existsSync } from 'node:fs';
import { execFileSync } from 'node:child_process';
import path from 'node:path';
import { scanJs, csvRows, walk, rewriteImportPaths, findShadowing, relTo } from './lib.mjs';

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
const dict = new Map();
for (const [layer, old, nw] of csvRows(root, 'dictionary.csv').slice(1)) {
  if (layer === 'ui' && old && nw && old !== nw) dict.set(old, nw);
}
for (const [from, to] of Object.entries(FILES)) {
  if (from.endsWith('.svelte')) dict.set(from.replace('.svelte', ''), to.replace('.svelte', ''));
}
const keys = new Map();
for (const [old, nw] of csvRows(root, 'keys.csv').slice(1)) {
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

// --- the JS tokenizer (shared with apply-e2e.mjs since E6a: lib.mjs) ----
const found = { strings: [], dataset: [] };
// the glyph ids of icons.js are the System's contract (figcaptions,
// DC-D2) — they move with the System, not with the dictionary
const keep = (file, out, src, j) => file.endsWith('lib/icons.js') && /(^|\n) {2}$/.test(out) && src[j] === ':';
const ctx = { dict, keys, prefixes: PREFIXES, found, keep };
const js = (src, i, stop, file) => scanJs(src, i, stop, file, ctx);

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
const paths = (src) => rewriteImportPaths(src, FILES);

// --- run -----------------------------------------------------------------
const UI_FILES = () => walk(UI, /\.(js|svelte)$/);
const rel = relTo(root);
const changed = [];
for (const f of UI_FILES()) {
  const src = readFileSync(f, 'utf8');
  const isCatalog = /catalogue\.(fr|en)\.js$/.test(f);
  let out;
  if (isCatalog) {
    // keys only — the values are the delivered text (D3)
    out = src.replace(/^(\s+)'([^']+)':/gm, (m, ws, k) => `${ws}'${keys.get(k) ?? k}':`);
  } else if (f.endsWith('.svelte')) out = svelte(src, rel(f));
  else [out] = js(src, 0, null, rel(f));
  out = paths(out);
  if (out !== src) { changed.push(rel(f)); if (!report) writeFileSync(f, out); }
}

// e2e nets and specs that import a renamed UI file by path
// (their E6a names — the files were renamed by apply-e2e.mjs)
for (const p of ['e2e/catalogs.test.mjs', 'e2e/tests/redesign-language.spec.js', 'e2e/system-coherence.mjs']) {
  const f = path.join(root, p);
  const src = readFileSync(f, 'utf8');
  const out = paths(src)
    .replace(/catalogue\.\$\{langue\}\.js/g, 'catalog.${langue}.js')
    .replace(/'reperes\.js'/g, "'markers.js'")
    .replace(/'icones\.js'/g, "'icons.js'")
    .replace(/lib\$\{path\.sep\}icones\.js/g, 'lib${path.sep}icons.js');
  if (out !== src) { changed.push(p); if (!report) writeFileSync(f, out); }
}

// shadowing risk: `new` already declared in a file that declares `old`
const shadow = findShadowing(UI_FILES(), dict, rel);

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
