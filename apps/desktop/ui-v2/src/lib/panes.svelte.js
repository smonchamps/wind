// The number of panes on screen 02 (PLAN-VOLETS, V-D1/V-D4): 3 —
// nav, list, reading (default) —, 2 — nav + full-width list, reading
// opens full screen (screen 03) — or 1 — list only, the nav lives
// in a drawer (E2). Pure UI preference: the shell has nothing to
// read from it — localStorage, the theme's pattern (D6), never the
// database. Shared `$state`: any template that reads `currentPanes()`
// re-renders on toggle, like the language.
const KEY = 'wind-volets';
const VALUES = [3, 2, 1];

const state = $state({ panes: 3 });

export function currentPanes() {
  return state.panes;
}

// Applies AND persists — the theme's gesture: immediate, no
// confirmation. Any unknown value is refused (the `applyTheme`
// pattern).
export function applyPanes(n) {
  if (!VALUES.includes(n)) return;
  state.panes = n;
  try {
    localStorage.setItem(KEY, String(n));
  } catch { /* storage unavailable: the choice will not survive, nothing else to do */ }
}

// Restores BEFORE the first render (no grid flash); any unknown
// value falls back to 3 (the `currentTheme` pattern).
export function restorePanes() {
  let n = 3;
  try {
    n = Number(localStorage.getItem(KEY) ?? '3');
  } catch { /* storage unavailable: default */ }
  state.panes = VALUES.includes(n) ? n : 3;
}
