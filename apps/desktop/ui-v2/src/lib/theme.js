// Wind's themes (V7 amended by A94/ADR 0027: the table is short
// and LIVING): `elements` (default, no attribute set) and its night,
// then `innamoramento` and its night (PLAN-MONA, born “Mona”, renamed
// by the CE on 2026-08-29 — A95). The 28-theme Wada table
// (A42) stays removed; the OS-tracking mechanics are unchanged: when
// dark OS tracking is active and the OS is dark, the -night variant
// of the CHOSEN theme is shown — the persisted choice stays the base
// theme, the suffix is a derived state, never stored. A manually
// chosen -night theme is left in peace.

const KEY = 'wind-theme';
const AUTO_KEY = 'wind-theme-auto';

// The selector's cards (Settings, onboarding) — swatches VERBATIM
// from the contract table, in the order accent, background, net,
// surface, ink (`panel` is dead — V3; the net takes its place: since
// V3 it is THE one that draws the separation, the thumbnails show it).
// The same values live as tokens in system.css: the swatches must
// show each theme WITHOUT applying it, hence the hex values repeated
// here — the system-coherence.mjs gate keeps them equal to the
// shipped tokens. Labels and descriptions live in the catalogue
// (`theme.<id>.nom` / `theme.<id>.desc` — PLAN-LANGUES, A15).
// Declared BEFORE the migration below: the guard derives from it.
export const THEME_CARDS = [
  { id: 'elements', swatches: ['#1A7A7A', '#F3F2EE', '#CBC8BB', '#FFFFFF', '#191D1E'] },
  { id: 'elements-nuit', swatches: ['#3FA39C', '#0D100F', '#333B3A', '#171B1A', '#ECEDEA'] },
  { id: 'innamoramento', swatches: ['#AD204C', '#F4F0F1', '#CDBFC4', '#FFFFFF', '#1D181A'] },
  { id: 'innamoramento-nuit', swatches: ['#E58BA4', '#151012', '#3E3339', '#1E171A', '#EEEAEC'] },
];

// The list of ids is DERIVED from the cards: a single table to
// maintain — a card without a theme (or the reverse) is impossible
// by construction, and the system-coherence.mjs gate keeps the
// swatches equal to the shipped tokens.
export const THEMES = THEME_CARDS.map((f) => f.id);

// Copy of the Discovery keys from before the switch (PLAN-WIND E3):
// the choice survives the rename, the old keys are removed. The
// WebView2 profile is relocated as is by the application — the
// old keys are therefore indeed there on Wind's first launch.
try {
  for (const [old, fresh] of [['discovery-theme', KEY], ['discovery-theme-auto', AUTO_KEY]]) {
    const value = localStorage.getItem(old);
    if (value !== null) {
      if (localStorage.getItem(fresh) === null) localStorage.setItem(fresh, value);
      localStorage.removeItem(old);
    }
  }
  // V7: the Wada table is removed — POLARITY is the only choice that
  // survives, and it is WRITTEN (A42's pattern, which migrated “nuit”
  // to nature-nuit, replayed on the whole table): any old dark choice
  // (`night` from before A42, or a `<wada>-nuit`) becomes
  // `elements-nuit`. Old LIGHT choices fall back to the default via
  // themeActuel()'s guard rail, silently — like the five themes
  // removed by A42 before them.
  // A95: “Mona” is renamed “Innamoramento” (CE, 2026-08-29,
  // never published in a release) — a choice persisted under the old
  // id follows the rename, BEFORE the -night orphan rule (otherwise
  // `mona-nuit` would fall back to elements-nuit). The polarities are
  // DERIVED from the base pair (the THEMES pattern: a hardcoded
  // -night pair forgotten would miss the migration in silence).
  const choiceToRename = localStorage.getItem(KEY);
  for (const [oldId, newId] of [['mona', 'innamoramento']]) {
    for (const suffix of ['', '-nuit']) {
      if (choiceToRename === oldId + suffix) localStorage.setItem(KEY, newId + suffix);
    }
  }
  // A94: VALID choices are derived from THEMES (declared above,
  // PLAN-MONA review) — a hardcoded list here rearmed the bug on
  // every addition: without “innamoramento-nuit”, a persisted dark
  // choice was rewritten to elements-nuit on every startup (proven RED).
  const choice = localStorage.getItem(KEY);
  if (choice !== null && !THEMES.includes(choice)
      && (choice === 'nuit' || choice.endsWith('-nuit'))) {
    localStorage.setItem(KEY, 'elements-nuit');
  }
} catch { /* storage unavailable: nothing to migrate */ }

export function currentTheme() {
  let name = 'elements';
  try { name = localStorage.getItem(KEY) || 'elements'; } catch { /* storage unavailable: default */ }
  return THEMES.includes(name) ? name : 'elements';
}

// A42 field finding (2026-08-16, bench probes): in Tauri/wry's
// WebView2, prefers-color-scheme does NOT follow the OS — never
// dark, zero events, even under a real toggle (registry + broadcast
// WM_SETTINGCHANGE). The source is the Tauri window API (theme() +
// onThemeChanged), which fired correctly both ways on every toggle;
// matchMedia stays the fallback outside Tauri and the e2e bench's
// handle (emulateMedia). BOTH channels write the same state and the
// last signal wins — never a permanent OR: with the host machine
// dark, an OR would make emulateMedia('light') forever losing
// (observed at the bench, D6 red on the remedy's first version).
let darkFlagged = null;

function osDark() {
  return darkFlagged
    ?? (globalThis.matchMedia?.('(prefers-color-scheme: dark)').matches ?? false);
}

function setThemeAttribute(name) {
  if (name === 'elements') delete document.documentElement.dataset.theme;
  else document.documentElement.dataset.theme = name;
  // A44: the native bars' `color-scheme` is NOT set here —
  // it lives in CSS, next to the tokens (`:root[data-theme$="-nuit"]`,
  // system.css): every path that sets data-theme gets it, without JS.
  // A42 review: the Settings checkmark follows the DISPLAYED card —
  // the signal says “the set theme has just changed”, whatever the
  // path (choice, tracking toggle, OS event during the session).
  document.dispatchEvent(new CustomEvent('wind:theme-affiche'));
}

// The theme SET on <html> — the derived state, never persisted (A42).
// This is what the Settings checkmark designates (A42 review): under
// OS tracking + dark OS, the eye sees the -night variant, so does the
// checkmark.
export function displayedTheme() {
  return document.documentElement.dataset.theme ?? 'elements';
}

// R3 (PLAN-RETOURS-4, D3, 2026-08-18): `paletteLecture()` is REMOVED.
// A message's body now displays on a light slab in every theme (the
// core bakes `Palette::default` — see `message_body`): A42's dark
// slab made sender-colored text unreadable. The front therefore no
// longer has a palette to pass on.

// The one place that decides the DISPLAYED theme: when OS tracking
// is active and the OS is dark, the -night variant of the
// chosen theme (A42). Membership in THEMES is the sole guard: it
// leaves an already -night theme in peace (“elements-nuit-nuit” does
// not exist) and refuses to set an orphan attribute that no CSS
// block would serve — the choice is shown as is rather than as a
// default palette.
function reflectTheme() {
  const chosen = currentTheme();
  const night = `${chosen}-nuit`;
  setThemeAttribute(osTracking() && osDark() && THEMES.includes(night) ? night : chosen);
}

export function osTracking() {
  try { return localStorage.getItem(AUTO_KEY) === '1'; } catch { return false; }
}

export function applyOsTracking(active) {
  try { localStorage.setItem(AUTO_KEY, active ? '1' : '0'); } catch { /* the choice will not survive, nothing else to do */ }
  reflectTheme();
}

export function applyTheme(name) {
  if (!THEMES.includes(name)) return;
  try { localStorage.setItem(KEY, name); } catch { /* storage unavailable: the choice will not survive, nothing else to do */ }
  reflectTheme();
}

export function restoreTheme() {
  // The OS can toggle mid-session (scheduled night mode): the
  // reflection follows without a restart, through the Tauri channel
  // (the only one alive in production — A42 field finding) AND through
  // matchMedia (fallback outside Tauri, e2e handle). Rejections stay
  // silent: outside Tauri the window does not exist, the fallback is
  // authoritative.
  const window = globalThis.window?.__TAURI__?.window?.getCurrentWindow?.();
  if (window) {
    window.theme()
      .then((t) => {
        // The initial state never overrides a signal that has already arrived.
        if (darkFlagged === null) {
          darkFlagged = t === 'dark';
          reflectTheme();
        }
      })
      .catch(() => { /* unreadable theme: the matchMedia fallback is authoritative */ });
    window.onThemeChanged(({ payload }) => {
      darkFlagged = payload === 'dark';
      reflectTheme();
    }).catch(() => { /* listen refused: the matchMedia fallback is authoritative */ });
  }
  reflectTheme();
  globalThis.matchMedia?.('(prefers-color-scheme: dark)')
    .addEventListener?.('change', (e) => {
      darkFlagged = e.matches;
      reflectTheme();
    });
  return currentTheme();
}
