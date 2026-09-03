// Reading system.css tokens — the SINGLE parser for both gates
// (contrast.mjs, system-coherence.mjs). Born at the review of A42: each
// carried its own copy of the same regex, and the same bug was fixed
// there twice in a row (the [a-zA-Z] class that lost "ink2" at
// PLAN-DC E3, then [a-z]+ which left the 14 -nuit themes with NO
// measurement at all). One single implementation, one single place to fix.
//
// EXPECTED_COUNT is the completeness floor: a block the pattern
// doesn't recognize isn't a pair to skip, it's the gate going
// silently blind. Update it with the table, in the
// same commit (DC-D2) — the gap is loud, that's the point.
export const EXPECTED_COUNT = 4;

// [a-z-]+: the night declension carries a hyphen
// ("elements-nuit", V7). [a-zA-Z][a-zA-Z0-9]* on the token side: "ink2"
// carries a digit. `valuePattern`: contrast.mjs only extracts
// hex #RRGGBB (the only measurable ones), system-coherence.mjs takes any
// value (shadows, scrims) — the default.
export function readThemes(css, { valuePattern = /[^;]+/ } = {}) {
  const themes = {};
  for (const [, name, body] of css.matchAll(
    /:root(?:\[data-theme="([a-z-]+)"\])?\s*\{([^}]+)\}/g,
  )) {
    const tokens = {};
    for (const [, key, value] of body.matchAll(
      new RegExp(`--([a-zA-Z][a-zA-Z0-9]*)\\s*:\\s*(${valuePattern.source})`, 'g'),
    )) {
      tokens[key] = value.replace(/\s+/g, ' ').trim();
    }
    if (Object.keys(tokens).length > 0) themes[name ?? 'elements'] = tokens;
  }
  return themes;
}

// A94 (PLAN-MONA review) — the second shared pattern: the
// --mk-<hue> blocks by polarity. The light marker table lives under
// `:root[data-theme$="-nuit"]` (served to ANY night theme, like A44's
// color-scheme), the dark one under the bare :root. Each gate
// carried its own copy of this regex and the A94 diff had to edit them
// in lockstep — the birth bug of this file, replayed; now
// ONE implementation. Returns {hue: hex} for the requested polarity
// (contrast.mjs measures the hex values, system-coherence.mjs only takes
// the names).
export function readMarkers(css, { night }) {
  const markers = {};
  for (const [, theme, body] of css.matchAll(
    /:root(\[data-theme\$="-nuit"\])?\s*\{([^}]+)\}/g,
  )) {
    if (Boolean(theme) !== night) continue;
    for (const [, hue, hex] of body.matchAll(
      /--mk-([a-z]+)\s*:\s*(#[0-9a-fA-F]{6})/g,
    )) {
      markers[hue] = hex;
    }
  }
  return markers;
}
