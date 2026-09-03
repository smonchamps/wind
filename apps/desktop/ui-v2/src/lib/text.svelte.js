// The language of the interface (PLAN-LANGUES, A15): flat fr/en
// catalogues, immediate switch — the language is a `$state`, every
// template that goes through `t()` re-renders on change, as the theme
// does. An APPLICATION preference persisted in the database (`prefs.lang`,
// the pattern of the arrival bubbles): the Rust shell reads it to compose
// the notifications — localStorage would be invisible to it.
//
// Fallback: any key missing from the active catalogue falls back to
// English (the reference since D4, PLAN-ENGLISH-SWITCH) — never a hole
// on screen; the e2e audit guarantees that the key sets are identical.
import { call } from './transport.js';
import { FR } from './catalog.fr.js';
import { EN } from './catalog.en.js';
import { DEFAULT_LANGUAGE, LANGUAGES, detectLanguage } from './language.js';

export const CATALOGS = { fr: FR, en: EN };
export { LANGUAGES };

// The plural rule, per language: “0 élément” but "0 items" — the strict
// need of the repository, not a CLDR engine.
const PLURAL = {
  fr: (n) => n > 1,
  en: (n) => n !== 1,
};

const state = $state({ language: DEFAULT_LANGUAGE });

export function currentLanguage() {
  return state.language;
}

// Applies WITHOUT persisting — the caller sets the preference (Settings,
// via `lang_set`), like the bubbles switch. `<html lang>` follows (screen
// readers, spell checkers).
export function applyLanguage(code) {
  const language = CATALOGS[code] ? code : DEFAULT_LANGUAGE;
  state.language = language;
  document.documentElement.lang = language;
}

// Restores BEFORE the first render (no flash): the preference in the
// database first — `lang_get` is a read-only probe, the only command
// allowed before `migration_check` (ADR 0012); at the first launch, the
// system language if it is French, English otherwise (D4).
// The detected key is NOT set here: `lang_set` opens the database and
// would pay the adoption of a legacy database silently, without the
// modal (field 2026-08-15) — it is set by `setDetectedLanguage()`, which
// the App calls once the migration is secured.
let toSet = null;

export async function restoreLanguage() {
  let code = null;
  let answered = false;
  try {
    code = await call('lang_get');
    answered = true;
  } catch { /* outside Tauri or unreadable database: session fallback below */ }
  if (!code) {
    code = detectLanguage(globalThis.navigator?.language);
    // The detection is armed TO BE SET only if the database REALLY
    // answered "no preference". A read failure is not an absence
    // (review 2026-08-15): setting afterwards would overwrite an existing
    // preference the probe simply could not read.
    if (answered) toSet = code;
  }
  applyLanguage(code);
}

// The deferred set of the first launch: AFTER the migration modal, so
// that the shell sees the key without waiting for a visit to Settings —
// and without ever touching a database not yet adopted. Returns the
// promise: the App awaits it so that the schema creation of the first
// launch stays SERIALIZED before the fleet of probes.
export function setDetectedLanguage() {
  if (!toSet) return Promise.resolve();
  return call('lang_set', { lang: toSet })
    .then(() => { toSet = null; })
    .catch(() => { /* outside Tauri or write failure: the key stays absent,
      the detection replays at the next launch */ });
}

// `t(key, params)`: `{name}` templates; a bar `|` separates the singular
// from the plural, decided by `params.n` under the language's rule.
// Non-string values (date tables) render as they are.
export function t(key, params) {
  const catalog = CATALOGS[state.language] ?? EN;
  let value = catalog[key];
  if (value === undefined) {
    if (import.meta.env?.DEV) {
      console.warn(`key missing from the ${state.language} catalogue: ${key}`);
    }
    value = EN[key];
  }
  if (value === undefined) return key;
  if (typeof value !== 'string') return value;
  if (value.includes('|') && params && typeof params.n === 'number') {
    const [singular, plural] = value.split('|');
    value = PLURAL[state.language]?.(params.n) ? plural : singular;
  }
  if (!params) return value;
  return value.replace(/\{(\w+)\}/g, (_, name) => String(params[name] ?? ''));
}
