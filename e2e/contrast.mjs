// WCAG contrast bench for the theme tokens (system.css): every
// (ink, background) pair actually used by ui-v2, across ALL themes
// in the table (V7 amended by A94 — 4 themes, EXPECTED_COUNT is authoritative).
// Thresholds: 4.5:1 for regular text, 3:1 for large text
// and interface components (icons, disks, rings) — plus the
// NET pairs (V3: the net alone carries the separation), at the
// threshold of the net shipped by Clarity (1.49:1 on the background, 1.26:1 on a
// card): below this floor, the net disappears and the separation lies.
//
//   node contrast.mjs        -> full table + verdict
//
// The source is system.css (the shipped tokens), not the prototype:
// what the user sees is what gets measured.
import { readFileSync } from 'node:fs';
import path from 'node:path';
import { readThemes, readMarkers, EXPECTED_COUNT } from './tokens.mjs';

const root = path.resolve(import.meta.dirname, '..');
const css = readFileSync(
  path.join(root, 'apps', 'desktop', 'ui-v2', 'src', 'system.css'),
  'utf8',
);

// The parser shared by both gates (tokens.mjs); only the hex values are
// measurable here (the rgba() shadows and scrims don't carry a pair).
const themes = readThemes(css, { valuePattern: /#[0-9a-fA-F]{6}/ });

// --- Luminance and WCAG ratio ---------------------------------------
function luminance(hex) {
  const c = [1, 3, 5].map((i) => {
    const v = parseInt(hex.slice(i, i + 2), 16) / 255;
    return v <= 0.04045 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4;
  });
  return 0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2];
}
const ratio = (a, b) => {
  const [l1, l2] = [luminance(a), luminance(b)].sort((x, y) => y - x);
  return (l1 + 0.05) / (l2 + 0.05);
};

// --- The pairs actually used by ui-v2 (component survey) -------------
// [ink, background, threshold, where] — the Elements System table (Color
// Themes section), taken over value for value: the doc's bench
// and this gate measure the SAME thing.
const PAIRS = [
  ['ink', 'bg', 4.5, 'titles, list items, nav'],
  ['ink', 'surface', 4.5, 'message cards, fields'],
  ['ink', 'sel', 4.5, 'selected row, open folder'],
  ['ink', 'hover', 4.5, 'hovered row'],
  ['ink', 'tile', 4.5, "a pinned row's item"],
  ['ink2', 'bg', 4.5, 'senders, body text'],
  ['ink2', 'surface', 4.5, 'chips, secondary buttons'],
  ['ink2', 'sel', 4.5, 'previews (selected row)'],
  ['ink2', 'hover', 4.5, 'previews (hovered row)'],
  ['ink2', 'tile', 4.5, "preview of a pinned row"],
  ['muted', 'bg', 4.5, 'times, eyebrows, status bar'],
  ['muted', 'surface', 4.5, 'descriptions, placeholder text'],
  ['muted', 'sel', 4.5, 'times (selected row)'],
  ['muted', 'hover', 4.5, 'times (hovered row)'],
  ['muted', 'tile', 4.5, 'times (pinned row)'],
  ['onAccent', 'accent', 4.5, "primary button label, armed switch handle"],
  ['onAccent', 'accentH', 4.5, 'primary button label (hover)'],
  ['alert', 'bg', 4.5, "error text, Draft mention"],
  ['alert', 'sel', 4.5, 'Draft mention (chosen row)'],
  ['alert', 'hover', 4.5, 'Draft mention (hovered row)'],
  ['alert', 'tile', 4.5, 'Draft mention (pinned row)'],
  ['alert', 'surface', 4.5, 'Draft mention (cards), Delete draft'],
  ['alert', 'surface', 3, "alert icon, anomaly dot, \"Decline\" glyph"],
  ['accent', 'surface', 3, 'icons, checkmark, focus ring, "Accept" glyph'],
  ['accent', 'bg', 3, 'focus ring, hairline, pane handle'],
  ['accent', 'sel', 3, 'hairline of the chosen line, outline of the reply in progress'],
  ['accent', 'tile', 3, "hairline and focus ring on a pinned row — COMPONENT only: below the plain-text threshold, no accent label sits on the tile"],
  ['accent', 'surface', 4.5, 'unread counter, links, accent labels (TEXT)'],
  ['accent', 'bg', 4.5, 'nav unread counter (TEXT, V4)'],
  ['tileInk', 'tile', 4.5, "initials tile, mailbox in progress, pinned row, date tile"],
  ['brand', 'bg', 3, 'unread DISK and cycle ring on the background'],
  ['brand', 'surface', 3, 'DISK on a card'],
  ['brand', 'sel', 3, 'DISK on the chosen row'],
  ['brand', 'hover', 3, 'DISK on the hovered row'],
  ['brand', 'tile', 3, 'DISK on the pinned row'],
  ['border', 'bg', 1.49, 'net on the background — threshold = the net SHIPPED by Clarity'],
  ['border', 'surface', 1.26, 'net on a card — threshold = the net SHIPPED by Clarity'],
  ['border', 'tile', 1.26, "net of the initials tile — the tile is only 1.04:1 on the light background, the net makes it exist"],
];

let failures = 0;
// Completeness floor: a theme the pattern doesn't recognize isn't
// "one theme fewer", it's a theme NEVER measured that ships to
// production — the exact hole of the 14 -nuit themes invisible to [a-z]+ (A42).
if (Object.keys(themes).length !== EXPECTED_COUNT) {
  failures += 1;
  console.log(`FAIL ${Object.keys(themes).length} theme(s) extracted from system.css — ${EXPECTED_COUNT} expected (tokens.mjs): a block escapes the pattern, or the table changed without amending the floor`);
}
// Only the GAPS are printed (A42 review): 28 × 25 "ok" lines
// were drowning the failures — the final verdict carries the counts, a red
// stays loud and localized.
let measurements = 0;
for (const [name, t] of Object.entries(themes)) {
  for (const [ink, bg, threshold, where] of PAIRS) {
    if (!t[ink] || !t[bg]) {
      // A token not found isn't a pair to skip: it's the
      // bench that's lying. Loud, like any failure.
      failures += 1;
      console.log(`FAIL ${name} · ${ink} on ${bg}: token not found in system.css  ${where}`);
      continue;
    }
    measurements += 1;
    const r = ratio(t[ink], t[bg]);
    if (r < threshold) {
      failures += 1;
      console.log(
        `FAIL ${name} · ${ink.padEnd(9)} on ${bg.padEnd(8)} `
        + `${r.toFixed(2).padStart(5)}:1  (threshold ${threshold}:1)  ${where}`,
      );
    }
  }
}
// --- MARKERS (A74, PLAN-RETOURS-8 R1; tokens since A82) ------------
// The account marker swatch set: 12 families × 2 variants
// (dark for light themes, light for -nuit ones — V5: the
// swatch set FOLLOWS polarity). Since A82 each hex serves twice
// (Settings swatch as background, trace as color): the
// SHIPPED hex values live as --mk-<hue> tokens in the :root blocks of
// system.css — we read THOSE tokens, never a copy. Each
// variant must hold 3:1 (component) on the backgrounds where the marker
// sits, and carry its swatch glyph at 4.5:1.
const MARKER_FAMILIES = 12;
// `tile` is part of it (2026-08-22 review): the CURRENT account's
// marker sits on the nav tile — forgetting it left that background,
// precisely, unmeasured. `panel` is dead (V3).
const MARKER_BGS = ['bg', 'sel', 'hover', 'surface', 'tile'];
// The --mk-* block parser is SHARED with the coherence gate
// (tokens.mjs, A94 — readThemes cannot serve: its name class
// [a-zA-Z0-9] deliberately lets through the hyphen of
// --mk-*, see the swatch-set comment in system.css).
const darks = readMarkers(css, { night: false });
const lights = readMarkers(css, { night: true });
// The glyph inks are READ from the shipped CSS, like the backgrounds —
// a local copy would lie as soon as system.css moves (review).
const darkInk = css.match(/\.marker\s*\{[^}]*color:(#[0-9a-fA-F]{6})/)?.[1];
const lightInk = css.match(
  /\[data-theme\$="-nuit"\] \.marker\s*\{[^}]*color:(#[0-9a-fA-F]{6})/,
)?.[1];
if (!darkInk || !lightInk) {
  failures += 1;
  console.log('FAIL marker swatch set: glyph ink not found in system.css (.marker { color:… })');
}
for (const [name, group] of [['dark', darks], ['light', lights]]) {
  if (Object.keys(group).length !== MARKER_FAMILIES) {
    failures += 1;
    console.log(`FAIL marker swatch set: ${Object.keys(group).length} ${name} variant(s) extracted from system.css — ${MARKER_FAMILIES} expected`);
  }
}
for (const [name, t] of Object.entries(themes)) {
  const night = name.endsWith('-nuit');
  const group = night ? lights : darks;
  const glyph = night ? lightInk : darkInk;
  if (!glyph) continue;
  for (const [hue, hex] of Object.entries(group)) {
    for (const bg of MARKER_BGS) {
      if (!t[bg]) continue;
      measurements += 1;
      const r = ratio(hex, t[bg]);
      if (r < 3) {
        failures += 1;
        console.log(`FAIL ${name} · marker ${hue.padEnd(8)} on ${bg.padEnd(8)} ${r.toFixed(2).padStart(5)}:1  (threshold 3:1)  Settings (swatch), nav and line (trace)`);
      }
    }
    measurements += 1;
    const g = ratio(glyph, hex);
    if (g < 4.5) {
      failures += 1;
      console.log(`FAIL ${name} · glyph on marker ${hue.padEnd(8)} ${g.toFixed(2).padStart(5)}:1  (threshold 4.5:1)`);
    }
  }
}

console.log(failures === 0
  ? `All pass — ${Object.keys(themes).length} themes, ${measurements} pairs measured.`
  : `${failures} pair(s) below threshold.`);
process.exitCode = failures === 0 ? 0 : 1;
