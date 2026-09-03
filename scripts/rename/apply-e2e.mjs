// The e2e/scripts applier of PLAN-BASCULE-ANGLAIS (E6a). Three passes,
// one command, on the scanner of `lib.mjs` (the one E5b proved):
//
//   node scripts/rename/apply-e2e.mjs --report   -> the hand-review lists, no write
//   node scripts/rename/apply-e2e.mjs            -> apply in place (git is the undo)
//
// 1. identifiers: `dictionary.csv` rows `layer=e2e-scripts` (whole
//    identifiers only — never strings, template text, comments, regex
//    literals) in every `.js`/`.mjs` of `e2e/` and `scripts/`; the rows
//    of the `ligne` family are left to the hand (D26: a LIST row is `row`
//    per D18, a text line stays `line`);
// 2. files: `git mv` per GLOSSARY §5.1 + D26 (FILES below) and every
//    relative import path that names them;
// 3. pointers: the file NAMES held by the dependents outside the layer —
//    the gate, the CI, the gate skill, the scripts, the UI comments, the
//    living docs (D25) — rewritten as text: a full basename
//    (`contraste.mjs`) or a hyphenated stem (`refonte-ecran02`,
//    `selection-multiple:174`), never a bare French word (`contraste`,
//    `demarrage`), and never under another directory
//    (`spikes/direction-elements/contraste.mjs` is another file).
//
// What it reports: string literals equal to a dictionary word (a value
// that bridges a renamed object key), `dataset.<word>` reads, and a `new`
// name already declared in a file that also declares `old`.
import { readFileSync, writeFileSync, existsSync } from 'node:fs';
import { execFileSync } from 'node:child_process';
import path from 'node:path';
import { csvRows, walk, renameJs, rewriteImportPaths, findShadowing, relTo } from './lib.mjs';

const root = path.resolve(import.meta.dirname, '..', '..');
const E2E = path.join(root, 'e2e');

// --- §5.1 + D26 files (relative to e2e/) ---------------------------------
export const FILES = {
  'args-navigateur.mjs': 'browser-args.mjs',
  'bascule-sombre.ps1': 'dark-toggle.ps1',
  'capture-accueil.mjs': 'capture-onboarding.mjs',
  'catalogues.test.mjs': 'catalogs.test.mjs',
  'coherence-systeme.mjs': 'system-coherence.mjs',
  'contraste.mjs': 'contrast.mjs',
  'garde-thread-principal.mjs': 'main-thread-guard.mjs',
  'geste-defilement.mjs': 'scroll-gesture.mjs',
  'jetons.mjs': 'tokens.mjs',
  'mesure-defilement.mjs': 'measure-scroll.mjs',
  'mesure-ram.ps1': 'measure-ram.ps1',
  'mesure-scrollbar.mjs': 'measure-scrollbar.mjs',
  'mesure-v2.mjs': 'measure-v2.mjs',
  'sonde-gel.py': 'freeze-probe.py',
  'tests/banc-ram-kiosque.spec.js': 'tests/bench-ram-feed.spec.js',
  'tests/barres-fil.spec.js': 'tests/thread-bars.spec.js',
  'tests/demarrage.spec.js': 'tests/startup.spec.js',
  'tests/espacement.spec.js': 'tests/spacing.spec.js',
  'tests/kiosque-images.spec.js': 'tests/feed-images.spec.js',
  'tests/menu-clavier.spec.js': 'tests/keyboard-menu.spec.js',
  'tests/mode-organise.spec.js': 'tests/organized-mode.spec.js',
  'tests/nettoyage.spec.js': 'tests/cleanup.spec.js',
  'tests/refonte-defilement.spec.js': 'tests/redesign-scroll.spec.js',
  'tests/refonte-ecran02.spec.js': 'tests/redesign-screen02.spec.js',
  'tests/refonte-invitations.spec.js': 'tests/redesign-invitations.spec.js',
  'tests/refonte-langue.spec.js': 'tests/redesign-language.spec.js',
  'tests/refonte-onboarding.spec.js': 'tests/redesign-onboarding.spec.js',
  'tests/refonte-parcours-portes.spec.js': 'tests/redesign-gated-journeys.spec.js',
  'tests/refonte-reconnexion.spec.js': 'tests/redesign-reconnect.spec.js',
  'tests/refonte-retours-3.spec.js': 'tests/redesign-feedback-3.spec.js',
  'tests/refonte-retours-6.spec.js': 'tests/redesign-feedback-6.spec.js',
  'tests/refonte-retours-7.spec.js': 'tests/redesign-feedback-7.spec.js',
  'tests/refonte-retours-8.spec.js': 'tests/redesign-feedback-8.spec.js',
  'tests/refonte-retrait-compte.spec.js': 'tests/redesign-account-removal.spec.js',
  'tests/refonte-volets.spec.js': 'tests/redesign-panes.spec.js',
  'tests/repere-ligne.spec.js': 'tests/row-marker.spec.js',
  'tests/retours-12-entete.spec.js': 'tests/feedback-12-header.spec.js',
  'tests/retours-12.spec.js': 'tests/feedback-12.spec.js',
  'tests/retours-14-reception.spec.js': 'tests/feedback-14-inbox.spec.js',
  'tests/retours-14.spec.js': 'tests/feedback-14.spec.js',
  'tests/retours-9-nom-compte.spec.js': 'tests/feedback-9-account-name.spec.js',
  'tests/sections-liste.spec.js': 'tests/list-sections.spec.js',
  'tests/selection-multiple.spec.js': 'tests/multi-select.spec.js',
};
// Files that are not renamed but whose NAME is French on the machine.
const EXTRA_POINTERS = [['rapport.json', 'report.json']];

// The dependents outside the layer that hold a file name (D25): names
// only, the prose stays for E7/E8.
const POINTER_FILES = [
  'scripts/gate.ps1',
  '.github/workflows/ci.yml',
  '.claude/skills/gate/SKILL.md',
  'scripts/build-wind.mjs',
  'scripts/run-wind.ps1',
  'scripts/install-workstation.ps1',
  'apps/desktop/ui-v2/src/List.svelte',
  'apps/desktop/ui-v2/src/Onboarding.svelte',
  'apps/desktop/ui-v2/src/lib/icons.js',
  'apps/desktop/ui-v2/src/lib/markers.js',
  'apps/desktop/ui-v2/src/lib/theme.js',
  'apps/desktop/ui-v2/src/main.js',
  'apps/desktop/ui-v2/src/system.css',
  'docs/STANDARD.md',
  'docs/STATE.md',
  'docs/DEBT.md',
  'docs/WORKFLOW.md',
  'docs/AUDIT-2026-09-01.md',
  'docs/design/systeme.dc.html',
  'docs/architecture/index.html',
];

// --- the dictionary ------------------------------------------------------
const HAND = new Set(['ligne', 'Ligne', 'lignes', 'nLignes', 'ligneAtelier']); // D26
const dict = new Map();
for (const [layer, old, nw] of csvRows(root, 'dictionary.csv').slice(1)) {
  if (layer === 'e2e-scripts' && old && nw && old !== nw && !HAND.has(old)) dict.set(old, nw);
}

// --- imports -------------------------------------------------------------
export const rewriteImports = (src) => rewriteImportPaths(src, FILES);

// --- pointers ------------------------------------------------------------
// One table, longest name first: the full basename, then the stem when it
// is hyphenated (a bare word — `contraste`, `demarrage`, `nettoyage` — is
// prose, not a pointer).
const POINTERS = (() => {
  const m = new Map();
  for (const [from, to] of [...Object.entries(FILES), ...EXTRA_POINTERS]) {
    const base = path.basename(from), nb = path.basename(to);
    if (base === nb) continue;
    m.set(base, nb);
    const stem = base.replace(/\.(spec\.js|test\.mjs|mjs|ps1|py|json)$/, '');
    const nstem = nb.replace(/\.(spec\.js|test\.mjs|mjs|ps1|py|json)$/, '');
    if (stem.includes('-') && stem !== nstem) m.set(stem, nstem);
  }
  return [...m].sort((a, b) => b[0].length - a[0].length);
})();
const POINTER_RE = new RegExp(
  `(?<![\\w.-])((?:[\\w.-]+[\\\\/])*)(${POINTERS.map(([o]) => o.replace(/\./g, '\\.')).join('|')})(?![\\w-])`,
  'g',
);
const OWN_DIRS = new Set(['e2e', 'tests', 'test-results']);
export function rewritePointers(src) {
  const map = new Map(POINTERS);
  return src.replace(POINTER_RE, (m, prefix, name) => {
    if (prefix) {
      const segs = prefix.split(/[\\/]/).filter(Boolean);
      if (!OWN_DIRS.has(segs[segs.length - 1])) return m;
    }
    return prefix + map.get(name);
  });
}

// --- run -----------------------------------------------------------------
const report = process.argv.includes('--report');
const found = { strings: [], dataset: [] };
const changed = [];
const rel = relTo(root);
const write = (f, src, out) => { if (out !== src) { changed.push(rel(f)); if (!report) writeFileSync(f, out); } };

if (import.meta.url === `file:///${process.argv[1].replace(/\\/g, '/')}`) {
  const code = [...walk(E2E, /\.(js|mjs)$/), ...walk(path.join(root, 'scripts'), /\.mjs$/)]
    // not the ratchet (it holds the French word list) nor the two applier
    // nets (their fixtures are French on purpose)
    .filter((f) => !rel(f).startsWith('scripts/rename/') && !/language-gate\.mjs$|apply-(dom|e2e)\.test\.mjs$/.test(rel(f)));
  for (const f of code) {
    const src = readFileSync(f, 'utf8');
    write(f, src, rewritePointers(rewriteImports(renameJs(src, dict, rel(f), found))));
  }
  // the PowerShell, Python, JSON and markdown of the layer: pointers only
  // (their identifiers are the hand's, the README is rewritten — D24)
  for (const f of walk(E2E, /\.(ps1|py|json)$/).filter((f) => !/language-baseline|isolation-oauth/.test(f))) {
    const src = readFileSync(f, 'utf8');
    write(f, src, rewritePointers(src));
  }
  for (const p of POINTER_FILES) {
    const f = path.join(root, p);
    const src = readFileSync(f, 'utf8');
    write(f, src, rewritePointers(src));
  }

  const shadow = findShadowing(code, dict, rel);

  if (report) {
    console.log(`== would change ${changed.length} files ==\n${changed.join('\n')}`);
    console.log(`\n== string literals equal to a dictionary word (code, not comments) ==\n${found.strings.join('\n')}`);
    console.log(`\n== dataset.<word> reads (attribute names, NOT renamed) ==\n${found.dataset.join('\n')}`);
    console.log(`\n== shadowing risk (new already declared next to old) ==\n${shadow.join('\n')}`);
    process.exit(0);
  }

  // files last: the imports above already name the new paths
  let moved = 0;
  for (const [from, to] of Object.entries(FILES)) {
    const a = path.join(E2E, from), b = path.join(E2E, to);
    if (from !== to && existsSync(a) && !existsSync(b)) { execFileSync('git', ['mv', a, b], { cwd: root }); moved++; }
  }
  console.log(`applied: ${changed.length} files rewritten, ${moved} files renamed`);
}
