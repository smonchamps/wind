// The language decision (PLAN-ENGLISH-SWITCH E5b, CE decision D4 of
// 2026-09-02; ADR 0016 amended): English is the reference language of
// the interface. Pure, no WebView — `e2e/language.test.mjs` is its net;
// `text.svelte.js` holds the reactive state and the catalogues.
export const DEFAULT_LANGUAGE = 'en';
export const LANGUAGES = ['en', 'fr'];

// The first launch, when no preference is stored yet: the system
// language if it is French, English otherwise (the reference, and the
// language every other tester reads).
export function detectLanguage(systemLanguage) {
  const code = String(systemLanguage ?? '').toLowerCase();
  return code.startsWith('fr') ? 'fr' : DEFAULT_LANGUAGE;
}
