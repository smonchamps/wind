# ADR 0016 — Interface language: homegrown flat catalogs, no i18n library

Date: 2026-08-12 · Status: accepted (PLAN-LANGUES, decision L-2) · **amended on 2026-09-03 (PLAN-ENGLISH-SWITCH, decision D4 — see the end of the page)**.

## Context

Multilingual support (PLAN-LANGUES, amendment A15) asks to pull ~150
strings out of the Svelte components and serve two languages (fr, en),
with an immediate toggle. The ecosystem offers generic engines
(svelte-i18n, ICU MessageFormat, CLDR); UI v2 today has NO runtime
dependency at all — only svelte/vite in devDependencies — and the
repository already hand-writes its date forms, exact to the prototype
(`quand.js`: "Yesterday", "Aug 1st", rolling week).

## Decision

A homegrown module: `lib/texte.svelte.js` (function `t(key, params)`,
current language in a Svelte 5 `$state` — any template that reads
`t()` re-renders on toggle) and two flat catalogs `catalogue.fr.js` /
`catalogue.en.js` (key → string; `{name}` templates; `|` separates
singular/plural, settled by a per-language rule: fr `n > 1`,
en `n !== 1`). The prototype's French is the reference: any key
missing from the active catalog falls back to `fr`, and an e2e spec
DIFFs the key sets — they cannot diverge without breaking the gate.

The preference lives in the database (`prefs.lang`, the arrival
bubbles' pattern): the Rust shell must read it to compose the
notifications (E2) — localStorage would be invisible to it. Default on
first launch: the system language if covered, else `fr`.

## Consequences

- Zero runtime dependency gained; the cost is a ~70-line module read in
  full, and catalogs read by hand (intended: this is the System's
  editorial control).
- No CLDR engine: languages with complex plurals (Polish, Arabic…)
  would require extending the rule — accepted, they are not on order;
  when the day comes, the decision is replayed on the merits.
- Date forms stay hand-written per language (A15 transposition),
  testable, exact — `Intl` produces neither a contextual "Yesterday"
  nor "1st".

## Amendment of 2026-09-03 — English is the reference (PLAN-ENGLISH-SWITCH, D4)

Decision D4 of the switch to English (Chief Engineer, 2026-09-02),
applied at step E5b: **English is the reference language of the
interface.** Three consequences, in place:

- `t()` falls back to the ENGLISH text for a key missing from the active
  catalogue (the fr/en key sets stay identical — the e2e spec and the
  System net still diff them);
- the first launch speaks English unless the system language is French
  (`lib/language.js`: one pure decision, `detectLanguage`, tested by
  `e2e/language.test.mjs`); the e2e suite keeps its WebView pinned to
  `--lang=fr`, so the journeys stay French (L-6);
- the shell composes the notifications in English when `prefs.lang` is
  absent or unknown (`Lang::from_pref` defaults to `En`).

The French catalogue is delivered word for word (D3) — only its keys
are English (`catalog.fr.js`, glossary §5.4). Files renamed with the
step: `lib/catalogue.fr.js` / `lib/catalogue.en.js` → `lib/catalog.fr.js`
/ `lib/catalog.en.js`, `lib/texte.svelte.js` → `lib/text.svelte.js`.
The next release is MINOR (a behavior of the first launch changes).
