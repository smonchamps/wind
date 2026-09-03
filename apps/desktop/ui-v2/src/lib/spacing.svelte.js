// List row spacing (PLAN-ESPACEMENT, CE decision D1 from
// 2026-08-25): three notches of air between messages — “Low”
// (the existing one, TO THE PIXEL), “Medium”, “High”. Pure UI
// preference, the panes.svelte.js pattern: localStorage, restore
// BEFORE the first render, unknown value → default.
//
// The Rust shell has nothing to read from it: it is a display
// choice, so it NEVER goes to the database (the rule is written at
// the top of panes.svelte.js, and A26 is the exact precedent — the
// Layout).
//
// Shared `$state`: the list that reads `rowPad()` re-renders on
// change, as for the pane widths.
//
// THE VALUES ARE VERTICAL PADDING, and this is not an implementation
// detail: `offsetHeight` — on which the ENTIRE windowing depends —
// only sees the border box. A margin or a `row-gap` would give
// 12.375 px per row INVISIBLE to the probe: the render would then lie
// while the probe told the truth. No margin, no gap. Never.
//
// The delta is arithmetic: +6 px of padding = +12 px of row.
// Measured rows (bench, msedge): 88 / 100 / 112 px bare.
const KEY = 'wind-espacement';
export const DEFAULT = 'low';
export const NOTCHES = {
  low: 13,
  medium: 19,
  high: 25,
};
// The selector's display order (Settings > Display).
export const LEVELS = ['low', 'medium', 'high'];
// The values persisted before E5b were French (D-54): read them, write
// the English ones — a tester's choice must not reset on update.
const LEGACY = { 'faible': 'low', 'moyen': 'medium', 'eleve': 'high' }; // quoted: the applier must not rename them

const state = $state({ level: DEFAULT });

export function currentSpacing() {
  return state.level;
}

// The vertical padding of the current notch, in px — what the list
// sets in its `--rangee-pad` token.
export function rowPad() {
  // `LEVELS.includes` rather than a `??` on the indexing: a
  // prototype key would render a function, which CSS would accept as
  // an invalid value — padding 0, crushed list, and no error.
  return LEVELS.includes(state.level) ? NOTCHES[state.level] : NOTCHES[DEFAULT];
}

// The guard goes through the LIST, never through `in`: `'toString' in
// CRANS` is true (the operator climbs the prototype chain), and a
// tampered-with localStorage would then render a function in place of
// padding. This is also the panes.svelte.js pattern, of which this
// module is the tracing — the guard there is an `includes` (review
// from 2026-08-25).
export function applySpacing(level) {
  if (!LEVELS.includes(level)) return;
  state.level = level;
  try {
    localStorage.setItem(KEY, level);
  } catch { /* storage unavailable: the choice will not survive, nothing else to do */ }
}

// Restores BEFORE the first render (no geometry jump, and above
// all: the FIRST probe already measures the right notch).
export function restoreSpacing() {
  let read = null;
  try {
    read = localStorage.getItem(KEY);
  } catch { /* unreadable storage: default */ }
  read = LEGACY[read] ?? read;
  state.level = LEVELS.includes(read) ? read : DEFAULT;
}
