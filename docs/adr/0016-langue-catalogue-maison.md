# ADR 0016 — Langue de l'interface : catalogues plats maison, pas de bibliothèque i18n

Date : 2026-08-12 · Statut : accepté (PLAN-LANGUES, décision L-2) · **amended on 2026-09-03 (PLAN-BASCULE-ANGLAIS, decision D4 — see the end of the page)**.

## Contexte

Le support multilingue (PLAN-LANGUES, amendement A15) demande de sortir
~150 chaînes des composants Svelte et d'en servir deux langues (fr, en),
avec bascule immédiate. L'écosystème offre des moteurs génériques
(svelte-i18n, ICU MessageFormat, CLDR) ; l'UI v2 n'a aujourd'hui AUCUNE
dépendance d'exécution — uniquement svelte/vite en devDependencies — et
le dépôt tient ses formes de dates à la main, exactes au prototype
(`quand.js` : « Hier », « 1ᵉʳ août », semaine glissante).

## Décision

Un module maison : `lib/texte.svelte.js` (fonction `t(cle, params)`,
langue courante en `$state` Svelte 5 — tout gabarit qui lit `t()` se
re-rend à la bascule) et deux catalogues plats `catalogue.fr.js` /
`catalogue.en.js` (clé → chaîne ; gabarits `{nom}` ; `|` sépare
singulier/pluriel, tranché par une règle par langue : fr `n > 1`,
en `n !== 1`). Le français du prototype est la référence : toute clé
absente du catalogue actif retombe sur `fr`, et une spec e2e DIFFE les
jeux de clés — ils ne peuvent pas diverger sans casser le gate.

La préférence vit en base (`prefs.lang`, patron des bulles d'arrivée) :
le shell Rust doit la lire pour composer les notifications (E2) —
localStorage lui serait invisible. Défaut au premier lancement : la
langue du système si couverte, sinon `fr`.

## Conséquences

- Zéro dépendance d'exécution gagnée ; le coût est un module de ~70
  lignes relu en entier, et des catalogues relus à la main (voulu :
  c'est le contrôle éditorial du Système).
- Pas de moteur CLDR : les langues à pluriels complexes (polonais,
  arabe…) demanderaient d'étendre la règle — accepté, elles ne sont pas
  commandées ; le jour venu, la décision se rejoue sur mesure.
- Les formes de dates restent écrites à la main par langue (transposition
  A15), testables, exactes — `Intl` ne produit ni « Hier » contextuel ni
  « 1ᵉʳ ».

## Amendment of 2026-09-03 — English is the reference (PLAN-BASCULE-ANGLAIS, D4)

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
