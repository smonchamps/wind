// System coherence gate (PLAN-DC E3, decision DC-D6): the
// normative document (docs/design/systeme.dc.html) must never
// drift from the shipped values (apps/desktop/ui-v2/src/system.css).
//
//   node system-coherence.mjs   -> named gaps + verdict
//
// Checks:
//   1. The token contract table (data-theme/data-jeton cells)
//      equals the :root of system.css, VALUE FOR VALUE, in both
//      directions — a CSS token missing from the doc is as much a
//      failure as a wrong value, and an orphan cell (token dead in
//      CSS) as much as a missing cell.
//   2. The amendments log is present. (The old checks
//      "no glyph count outside the log" and "reference to
//      assets/icones/README.md" died with the font: since V8
//      the Icons section's listing IS the inventory — the doc says
//      "78 glyphs" and it is right to say so.)
//   3. The CARDS' swatches (lib/theme.js) equal the CSS tokens
//      [accent, bg, border, surface, ink] — the Settings selector's
//      thumbnails show each theme without applying it, the
//      copy must never drift (A42 review). `panel` is dead (V3).
//
// The remedy is always the same: amend the System in the offending
// commit (DC-D2) — never twist the gate.
import { readFileSync, readdirSync } from 'node:fs';
import path from 'node:path';
import { readThemes, readMarkers, EXPECTED_COUNT } from './tokens.mjs';

const root = path.resolve(import.meta.dirname, '..');
const css = readFileSync(
  path.join(root, 'apps', 'desktop', 'ui-v2', 'src', 'system.css'),
  'utf8',
);
const doc = readFileSync(
  path.join(root, 'docs', 'design', 'systeme.dc.html'),
  'utf8',
);

let failures = 0;
const failure = (message) => {
  failures += 1;
  console.log(`FAIL ${message}`);
};

// --- 1. The tokens: system.css on one side… --------------------------
// The parser shared by both gates (tokens.mjs — born at the A42 review,
// after two fixes of the same bug in two copies); the
// EXPECTED_COUNT floor closes the silence of an unrecognized block.
const cssThemes = readThemes(css);
if (Object.keys(cssThemes).length !== EXPECTED_COUNT) {
  failure(`${Object.keys(cssThemes).length} theme(s) extracted from system.css — ${EXPECTED_COUNT} expected (tokens.mjs): a block escapes the pattern, or the table changed without amending the floor`);
}

// --- …the doc's contract table on the other --------------------------
const docThemes = {};
for (const [, theme, token, content] of doc.matchAll(
  /<td data-theme="([a-z-]+)" data-jeton="([a-zA-Z0-9]+)"[^>]*>([^<]*)<\/td>/g,
)) {
  (docThemes[theme] ??= {})[token] = content.replace(/\s+/g, ' ').trim();
}

if (Object.keys(docThemes).length === 0) {
  failure('the token contract table is not found in the doc (data-theme/data-jeton cells)');
}

for (const [name, cssTokens] of Object.entries(cssThemes)) {
  const docTokens = docThemes[name] ?? {};
  for (const [token, cssValue] of Object.entries(cssTokens)) {
    if (!(token in docTokens)) {
      failure(`${name} · --${token}: shipped "${cssValue}", missing from the doc`);
    } else if (docTokens[token] !== cssValue) {
      failure(`${name} · --${token}: doc "${docTokens[token]}", shipped "${cssValue}"`);
    }
  }
  for (const token of Object.keys(docTokens)) {
    if (!(token in cssTokens)) {
      failure(`${name} · --${token}: in the doc but dead in system.css`);
    }
  }
}
for (const name of Object.keys(docThemes)) {
  if (!(name in cssThemes)) {
    failure(`theme "${name}": in the doc but absent from system.css (ghost theme — DC-D5)`);
  }
}

// --- 2. The amendments log is present -----------------------
if (!doc.includes('Journal des amendements')) {
  failure('the "Journal des amendements" section is not found');
}

// --- 3. The CARDS' swatches say the shipped tokens -------------
const themeJs = readFileSync(
  path.join(root, 'apps', 'desktop', 'ui-v2', 'src', 'lib', 'theme.js'),
  'utf8',
);
const BADGE_ROLES = ['accent', 'bg', 'border', 'surface', 'ink'];
const cards = [...themeJs.matchAll(/\{ id: '([a-z-]+)', swatches: \[([^\]]*)\] \}/g)];
if (cards.length !== EXPECTED_COUNT) {
  failure(`${cards.length} card(s) read in lib/theme.js — ${EXPECTED_COUNT} expected: THEME_CARDS changed shape, or a card is missing`);
}
for (const [, id, raw] of cards) {
  const badges = [...raw.matchAll(/'([^']+)'/g)].map(([, v]) => v);
  const tokens = cssThemes[id];
  if (!tokens) {
    failure(`card "${id}": in the selector but absent from system.css`);
    continue;
  }
  BADGE_ROLES.forEach((role, i) => {
    if (badges[i] !== tokens[role]) {
      failure(`card "${id}" · swatch ${role}: theme.js "${badges[i]}", shipped "${tokens[role]}"`);
    }
  });
}

// --- 4. The catalogs name every shipped theme --------------------
// A42 review: fr↔en parity (redesign-language.spec) doesn't say that a
// SHIPPED theme has its label — a renamed id without the catalogs rendered
// the raw `theme.<id>.name` key on the card, green everywhere.
for (const language of ['fr', 'en']) {
  const catalog = readFileSync(
    path.join(root, 'apps', 'desktop', 'ui-v2', 'src', 'lib', `catalog.${language}.js`),
    'utf8',
  );
  const ids = new Set(
    [...catalog.matchAll(/'theme\.([a-z-]+)\.name'/g)].map(([, id]) => id),
  );
  for (const id of Object.keys(cssThemes)) {
    if (!ids.has(id)) {
      failure(`catalog.${language}: theme.${id}.name missing — the selector card would show the raw key`);
    }
  }
  for (const id of ids) {
    if (!cssThemes[id]) {
      failure(`catalog.${language}: theme.${id}.name with no shipped theme in system.css`);
    }
  }
}

// --- 5. No scrollbar rule at all (A44) — (numbering
//     squared away at PLAN-ELEMENTS E1: two "5"s used to coexist)
// The scrollbars are NATIVE overlays (OverlayScrollbar): a SINGLE
// `::-webkit-scrollbar` / `scrollbar-width` /
// `scrollbar-color` rule drops the element back to the classic path and
// gives it back ~15 px of gutter — the regression is silent to
// the tests' eye. Comments are stripped before the scan (the
// A44 comment in system.css names these exact rules).
const srcUi = path.join(root, 'apps', 'desktop', 'ui-v2', 'src');
const uiFiles = (folder) =>
  readdirSync(folder, { withFileTypes: true }).flatMap((e) =>
    e.isDirectory()
      ? uiFiles(path.join(folder, e.name))
      : /\.(css|svelte)$/.test(e.name)
        ? [path.join(folder, e.name)]
        : [],
  );
for (const file of uiFiles(srcUi)) {
  const withoutComments = readFileSync(file, 'utf8')
    .replace(/\/\*[\s\S]*?\*\//g, '')
    .replace(/<!--[\s\S]*?-->/g, '')
    .replace(/^\s*\/\/.*$/gm, '');
  const forbidden = withoutComments.match(
    /::-webkit-scrollbar|scrollbar-width\s*:|scrollbar-color\s*:/,
  );
  if (forbidden) {
    failure(`${path.relative(root, file)}: rule "${forbidden[0]}" — scrollbars are native overlays (A44), this rule drops the element back to the classic path (~15 px of gutter)`);
  }
}

// --- 6. The dedicated marker set: ONE list, four carriers --------
// (PLAN-RETOURS-8, 2026-08-22 review): the Rust allowlist (commands.rs,
// authoritative at write time), lib/reperes.js (what the UI offers),
// system.css's hues (what gets drawn), and the catalogs
// (labels). A drift = a marker offered but refused, or stored
// but rendered without a color — always silently.
const commandsRs = readFileSync(
  path.join(root, 'apps', 'desktop', 'src', 'commands.rs'),
  'utf8',
);
const wireRs = readFileSync(path.join(root, 'apps', 'desktop', 'src', 'wire.rs'), 'utf8');
const markersJs = readFileSync(
  path.join(root, 'apps', 'desktop', 'ui-v2', 'src', 'lib', 'markers.js'),
  'utf8',
);
const rustListIn = (src, name) => [
  ...(src.match(new RegExp(`const ${name}[^=]*= \\[([^;]*)\\];`))?.[1] ?? '')
    .matchAll(/"([a-z_]+)"/g),
].map(([, v]) => v);
const rustList = (name) => rustListIn(commandsRs, name);
const jsList = (name) => [
  ...(markersJs.match(new RegExp(`export const ${name} = \\[([^\\]]*)\\]`))?.[1] ?? '')
    .matchAll(/'([a-z_]+)'/g),
].map(([, v]) => v);
const compareLists = (what, a, aName, b, bName) => {
  if (a.length === 0) failure(`markers: ${what} list not found in ${aName}`);
  for (const v of a) {
    if (!b.includes(v)) failure(`markers: ${what} "${v}" in ${aName} but not in ${bName}`);
  }
  for (const v of b) {
    if (!a.includes(v)) failure(`markers: ${what} "${v}" in ${bName} but not in ${aName}`);
  }
};
const rustIcons = rustList('MARKER_ICONS');
// E5a (D16): the wire hues live in wire.rs (WIRE_HUES) — the French
// MARKER_HUES of commands.rs is the database allowlist, never seen by the UI.
const rustHues = rustListIn(wireRs, 'WIRE_HUES');
compareLists('icon', rustIcons, 'commands.rs', jsList('MARKER_ICONS'), 'lib/markers.js');
compareLists('hue', rustHues, 'wire.rs', jsList('MARKER_HUES'), 'lib/markers.js');
const cssHues = [
  ...new Set([...css.matchAll(/\.marker\[data-hue="([a-z]+)"\]/g)].map(([, v]) => v)),
];
compareLists('hue', rustHues, 'wire.rs', cssHues, 'system.css');
// A82: the marker is now drawn TWO ways — the Settings swatch
// (background) and the TRACE of the nav and the line (color).
// Checking only the swatch table would let a hue forgotten in the
// trace pass green: the account would carry its color in Settings and
// a glyph in inherited ink everywhere it's actually looked at.
// (`.marker\[` does not match `.bare-marker\[`: the two scans are
// disjoint, no duplicate to worry about.)
const traceHues = [
  ...new Set([...css.matchAll(/\.bare-marker\[data-hue="([a-z]+)"\]/g)].map(([, v]) => v)),
];
compareLists('trace hue', rustHues, 'wire.rs', traceHues, 'system.css (.bare-marker)');
// And the tokens themselves: since A82 the 24 hex values live in --mk-<hue>,
// one table per polarity. A missing token would render `color:var(--mk-x)`
// with no value — the glyph would fall back to the current ink, silently.
// A94: the night table lives under `$="-nuit"` — one table per polarity,
// served to EVERY dark theme (innamoramento-nuit included), never copied. The
// block parser is SHARED with contrast.mjs (tokens.mjs): a
// single regex to keep in step with the CSS.
for (const [polarity, night] of [['clair', false], ['nuit', true]]) {
  const tokens = new Set(Object.keys(readMarkers(css, { night })));
  for (const hue of rustHues) {
    if (!tokens.has(hue)) {
      failure(`system.css: token --mk-${hue} missing in ${polarity} polarity (A82) — this marker's trace would have no color`);
    }
  }
}
for (const language of ['fr', 'en']) {
  const catalog = readFileSync(
    path.join(root, 'apps', 'desktop', 'ui-v2', 'src', 'lib', `catalog.${language}.js`),
    'utf8',
  );
  for (const icon of rustIcons) {
    if (!catalog.includes(`'marker.icon.${icon}'`)) {
      failure(`catalog.${language}: marker.icon.${icon} missing — the picker card would show the raw key as a tooltip`);
    }
  }
  for (const hue of rustHues) {
    if (!catalog.includes(`'marker.hue.${hue}'`)) {
      failure(`catalog.${language}: marker.hue.${hue} missing`);
    }
  }
}

// --- 7. Icons: the System's listing IS the inventory (A18, V8) --
// Since V8 the font is dead: the 78 glyphs live as SVG in
// lib/icones.js, and the System draws them. Three assertions, which
// take over the guard from the v2 generator:
//   a. the catalog's names == the figcaptions of the doc's grid,
//      in BOTH directions (the ten-glyph gap from before V8 cannot
//      be reborn);
//   b. every catalog path (`d`) appears in the doc — a
//      drawing changed in code without amending the System cries out;
//   c. every icon name hardcoded in a component exists in the
//      catalog and is not reserved (A53/A60/A62: a reserved one is not
//      used).
const jsIcons = readFileSync(
  path.join(root, 'apps', 'desktop', 'ui-v2', 'src', 'lib', 'icons.js'),
  'utf8',
);
const catalogNames = [...jsIcons.matchAll(/^ {2}([a-z_0-9]+): *\{/gm)].map(([, n]) => n);
const catalogReserves = new Set(
  [...jsIcons.matchAll(/^ {2}([a-z_0-9]+): *\{[^\n]*\br:true/gm)].map(([, n]) => n),
);
const docNames = [...new Set(
  [...doc.matchAll(/<figcaption>([a-z_0-9]+)<\/figcaption>/g)].map(([, n]) => n),
)];
if (catalogNames.length === 0) failure('lib/icons.js: no glyph read — the catalog changed shape');
if (docNames.length === 0) failure('the doc carries no glyph grid (figcaption) — the Icons section changed shape');
for (const n of catalogNames) {
  if (!docNames.includes(n)) failure(`icon "${n}": in the catalog (lib/icons.js) but absent from the System's grid`);
}
for (const n of docNames) {
  if (!catalogNames.includes(n)) failure(`icon "${n}": drawn in the System but absent from the shipped catalog`);
}
for (const [, name, raw] of jsIcons.matchAll(/^ {2}([a-z_0-9]+): *\{ *d:\[([^\]]*)\]/gm)) {
  for (const [, d] of raw.matchAll(/'([^']+)'/g)) {
    if (!doc.includes(d)) {
      failure(`icon "${name}": path "${d}" from the catalog does not appear in the System — drawing changed without amending the doc (DC-D2)`);
    }
  }
}
// Two forms of use: the literal (<Icon name="x">) AND the config
// tables (`icon: 'x'` — nav folders, Settings groups,
// notices) that flow into a name={dynamic}. Without the
// second, an `icone: 'delet'` would stay green and render an empty SVG
// silently (PLAN-ELEMENTS review, angle A).
const jsUiFiles = (folder) =>
  readdirSync(folder, { withFileTypes: true }).flatMap((e) =>
    e.isDirectory()
      ? jsUiFiles(path.join(folder, e.name))
      : /\.(js|svelte)$/.test(e.name) && !/catalog\.[a-z]+\.js$/.test(e.name)
        ? [path.join(folder, e.name)]
        : [],
  );
for (const file of jsUiFiles(srcUi)) {
  if (file.endsWith(`lib${path.sep}icons.js`)) continue;
  const source = readFileSync(file, 'utf8');
  for (const [, name] of [
    ...source.matchAll(/<Icon[^>]*\bname="([a-z_0-9]+)"/g),
    ...source.matchAll(/\bicon:\s*'([a-z_0-9]+)'/g),
  ]) {
    if (!catalogNames.includes(name)) {
      failure(`${path.relative(root, file)}: icon "${name}" used but absent from the catalog`);
    } else if (catalogReserves.has(name)) {
      failure(`${path.relative(root, file)}: icon "${name}" used but RESERVED (A53/A60/A62)`);
    }
  }
}
// The catalog's `repere:true` flags are the FIFTH list in the
// dedicated set (PLAN-ELEMENTS review): without this comparison, a glyph
// entering or leaving the set in the catalog without the four other
// carriers (commands.rs, lib/reperes.js, system.css, catalogs)
// would drift silently.
const catalogMarkers = [...jsIcons.matchAll(/^ {2}([a-z_0-9]+): *\{[^\n]*\bmarker:true/gm)]
  .map(([, n]) => n);
compareLists('icon', rustIcons, 'commands.rs', catalogMarkers, 'lib/icons.js (marker:true)');

// --- 8. Zero radius: not one border-radius literal left (V14) ---------
// The three shape tokens (--r-surface, --r-control, --r-tile)
// equal 0 and live on `html` (not :root — the color token contract
// doesn't swell with them). Two round shapes remain that mean
// something: the disk (50% — state, identity, the switch
// handle) and the pill of the switch track (999px).
// Any other literal is a gap — V14's rewind fits in
// one line BECAUSE everything goes through the tokens.
for (const token of ['--r-surface', '--r-control', '--r-tile']) {
  if (!new RegExp(`html\\s*\\{[^}]*${token}:0`).test(css)) {
    failure(`system.css: shape token ${token}:0 missing on html (V14)`);
  }
}
for (const file of uiFiles(srcUi)) {
  const source = readFileSync(file, 'utf8')
    .replace(/\/\*[\s\S]*?\*\//g, '')
    .replace(/<!--[\s\S]*?-->/g, '')
    .replace(/^\s*\/\/.*$/gm, '');
  for (const [raw, value] of source.matchAll(/border-radius:\s*([^;}]+)/g)) {
    const v = value.trim();
    if (v === 'var(--r-surface)' || v === 'var(--r-control)' || v === 'var(--r-tile)'
      || v === '50%' || v === '999px'
      // The declared, permanent exception (V14): the tile mark
      // keeps its PLATFORM radius (15/64) — the OS dictates it.
      || v === 'var(--r-plateforme)') continue;
    failure(`${path.relative(root, file)}: "${raw.trim()}" — V14: zero radius, every literal goes through a shape token (or 50% / 999px for the disk and the track)`);
  }
}

const themeCount = Object.keys(cssThemes).length;
const tokenCount = Object.values(cssThemes).reduce((n, t) => n + Object.keys(t).length, 0);
console.log(
  failures === 0
    ? `Everything matches — ${themeCount} themes, ${tokenCount} token values, the doc says the shipped.`
    : `${failures} gap(s) between the System and the shipped. The remedy: amend the System in the offending commit (DC-D2).`,
);
process.exitCode = failures === 0 ? 0 : 1;
