// Pane widths on screen 02 (PLAN-RETOURS-V3 R3, CE verdict
// D3): nav and list are resized with the mouse, bounded — nav
// 180-400 px, list 300-640 px —, persisted together under a single
// key. Pure UI preference, the panes.svelte.js pattern: localStorage,
// restore BEFORE the first render, unknown value → default.
// Shared `$state`: the grid that reads `currentWidth()` re-renders
// on drag, like the number of panes.
//
// Review (2026-08-16): resizing and persisting are TWO gestures — the
// drag resizes on every pointermove (state only, no write), the
// release persists once. An optional `cap` is added to the bounds:
// the cumulated maximum bounds (400 + 640) exceed the default window
// (1000 px) — without a cap, the thread pane drops to 0 and the
// handle falls off screen, an unrecoverable persisted state. The
// cap comes from the caller: the window is UI knowledge, not this
// module's.
const KEY = 'wind-largeurs';
export const DEFAULTS = { nav: 248, list: 400 };
export const BOUNDS = { nav: [180, 400], list: [300, 640] };
// The shape persisted before E5b named the pane `list` (D-54): read it,
// write the new name — a tester's layout must not reset on update.
const LEGACY = { list: 'liste' };

const state = $state({ ...DEFAULTS });

function bounded(pane, px, cap) {
  const [min, max] = BOUNDS[pane];
  return Math.min(Math.min(max, cap), Math.max(min, Math.round(px)));
}

export function currentWidth(pane) {
  return state[pane];
}

// Resizes WITHOUT persisting — the drag's step.
export function setWidth(pane, px, cap = Infinity) {
  if (!(pane in DEFAULTS) || !Number.isFinite(px)) return;
  state[pane] = bounded(pane, px, cap);
}

// Writes the current state — release, keyboard, double-click.
export function persistWidths() {
  try {
    localStorage.setItem(KEY, JSON.stringify(state));
  } catch { /* storage unavailable: the setting will not survive, nothing else to do */ }
}

// Resizes AND persists — the one-off gesture (keyboard, programmatic).
export function applyWidth(pane, px, cap = Infinity) {
  setWidth(pane, px, cap);
  persistWidths();
}

// The handle's double-click (D3): the boundary returns to its default.
export function defaultWidth(pane) {
  if (!(pane in DEFAULTS)) return;
  state[pane] = DEFAULTS[pane];
  persistWidths();
}

// Restores BEFORE the first render (no grid flash); any absent,
// non-numeric, or out-of-bounds value falls back to the default.
export function restoreWidths() {
  let read = {};
  try {
    read = JSON.parse(localStorage.getItem(KEY) ?? '{}') ?? {};
  } catch { /* unreadable storage or JSON: defaults */ }
  for (const pane of Object.keys(DEFAULTS)) {
    const px = read[pane] ?? read[LEGACY[pane]];
    state[pane] =
      Number.isFinite(px) && px === bounded(pane, px, Infinity)
        ? px
        : DEFAULTS[pane];
  }
}
