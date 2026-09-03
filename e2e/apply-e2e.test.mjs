// PLAN-ENGLISH-SWITCH E6a (2026-09-03): the e2e/scripts applier
// (`scripts/rename/apply-e2e.mjs`) renames the layer's identifiers from
// `dictionary.csv` rows `layer=e2e-scripts`, renames the spec and tool
// files per GLOSSARY §5.1 + D26, follows the import paths, and rewrites
// the file-name POINTERS held by the dependents outside the layer
// (gate.ps1, ci.yml, the gate skill, the UI comments, the living docs —
// D25). The identifier scanner is the one E5b proved on the UI, moved to
// `scripts/rename/lib.mjs` so the two appliers share it. Each rule is
// pinned on a fixture here before the applier touches ~90 files.
import test from 'node:test';
import assert from 'node:assert/strict';
import { renameJs } from '../scripts/rename/lib.mjs';
import { rewritePointers, rewriteImports, FILES } from '../scripts/rename/apply-e2e.mjs';

const dict = new Map([
  ['dossier', 'folder'],
  ['volet', 'pane'],
  ['injecterArrivee', 'injectArrival'],
  ['rapport', 'report'],
]);

test('identifiers: whole words only, never strings, comments, template text or regex literals', () => {
  const src = [
    "// le dossier courant, un volet", // lang:fr
    "import { injecterArrivee } from '../launch.mjs';",
    "const dossier = (categorie) => page.locator(`[data-testid=\"nav-folder\"][data-category=\"${categorie}\"] .volet`);",
    "const dossiers = dossier('inbox'); /* volet */",
    "await expect(page.getByText('dossier')).toBeVisible();",
    "const re = /dossier|volet/g; const r = rapport(a, b);",
    "injecterArrivee({ email, sujet: `volet ${volet}` });",
  ].join('\n');
  const out = renameJs(src, dict, 'e2e/tests/x.spec.js');
  assert.equal(out, [
    "// le dossier courant, un volet", // lang:fr
    "import { injectArrival } from '../launch.mjs';",
    "const folder = (categorie) => page.locator(`[data-testid=\"nav-folder\"][data-category=\"${categorie}\"] .volet`);",
    "const dossiers = folder('inbox'); /* volet */",
    "await expect(page.getByText('dossier')).toBeVisible();",
    "const re = /dossier|volet/g; const r = report(a, b);",
    "injectArrival({ email, sujet: `volet ${pane}` });",
  ].join('\n'));
});

test('identifiers: a string literal equal to a dictionary word is reported, not rewritten', () => {
  const found = { strings: [], dataset: [] };
  renameJs("const k = etat['dossier']; el.dataset.volet;", dict, 'e2e/x.mjs', found);
  assert.equal(found.strings.length, 1);
  assert.match(found.strings[0], /'dossier'/);
  assert.deepEqual(found.dataset, ['e2e/x.mjs: dataset.volet']);
});

test('files: the §5.1 + D26 table covers every French spec and tool name', () => {
  assert.equal(FILES['tests/refonte-ecran02.spec.js'], 'tests/redesign-screen02.spec.js');
  assert.equal(FILES['tests/barres-fil.spec.js'], 'tests/thread-bars.spec.js');
  assert.equal(FILES['tests/retours-14.spec.js'], 'tests/feedback-14.spec.js');
  assert.equal(FILES['sonde-gel.py'], 'freeze-probe.py');
  assert.equal(FILES['bascule-sombre.ps1'], 'dark-toggle.ps1');
  assert.equal(FILES['catalogues.test.mjs'], 'catalogs.test.mjs');
  assert.equal(FILES['tests/horizon-import.spec.js'], undefined);
});

test('imports: a relative path that names a renamed tool follows it', () => {
  const src = [
    "import { argsNavigateur } from './args-navigateur.mjs';",
    "import { tenirBarre } from '../geste-defilement.mjs';",
    "import { lireThemes } from '../e2e/jetons.mjs';",
    "import { launchAppV2 } from '../launch.mjs';",
  ].join('\n');
  assert.equal(rewriteImports(src), [
    "import { argsNavigateur } from './browser-args.mjs';",
    "import { tenirBarre } from '../scroll-gesture.mjs';",
    "import { lireThemes } from '../e2e/tokens.mjs';",
    "import { launchAppV2 } from '../launch.mjs';",
  ].join('\n'));
});

test('pointers: a file name with its extension, or a hyphenated stem, is rewritten in prose and scripts; a bare French word is not', () => {
  const src = [
    'Step 3 "WCAG contrasts (A8)" { node e2e/contraste.mjs }',
    '`python e2e/sonde-gel.py <base.db>` — le contraste est mesuré', // lang:fr
    'flaky `selection-multiple:174` ×3, `barres-fil:25`, refonte-ecran02.spec.js',
    'le démarrage (demarrage.spec.js) — un demarrage lent', // lang:fr
    'measured by e2e/contraste.mjs and mesure-ram.ps1; spikes/direction-elements/contraste.mjs',
  ].join('\n');
  assert.equal(rewritePointers(src), [
    'Step 3 "WCAG contrasts (A8)" { node e2e/contrast.mjs }',
    '`python e2e/freeze-probe.py <base.db>` — le contraste est mesuré', // lang:fr
    'flaky `multi-select:174` ×3, `thread-bars:25`, redesign-screen02.spec.js',
    'le démarrage (startup.spec.js) — un demarrage lent', // lang:fr
    'measured by e2e/contrast.mjs and measure-ram.ps1; spikes/direction-elements/contraste.mjs',
  ].join('\n'));
});
