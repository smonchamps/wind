# Icônes de Wind — provenance

Depuis PLAN-ELEMENTS (V8, décision CE D2 du 2026-08-24), Wind ne
livre **plus aucune fonte d'icônes** : les 78 glyphes sont un **jeu
original**, dessiné dans la grammaire « Elements » (grille 24, trait
2 unités, bouts nets, jonctions vives), servi en SVG en ligne par
`apps/desktop/ui-v2/src/Icone.svelte` depuis le catalogue
`apps/desktop/ui-v2/src/lib/icones.js`.

**L'inventaire normatif vit au Système** : le relevé de la section
« Icônes » de `docs/design/systeme.dc.html` — un glyphe, un sens, un
emploi. La gate `e2e/coherence-systeme.mjs` tient le relevé et le
catalogue égaux dans les deux sens et vérifie chaque tracé (A18).

Les formes sont **dessinées d'après** Material Symbols (Google),
publié sous licence Apache 2.0 — conservée ici ([LICENSE](LICENSE))
au titre de la provenance. La mention utilisateur vit dans
Réglages > À propos (`reglages.iconesValeur`).
