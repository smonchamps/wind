# ADR 0027 — La table des thèmes est courte et vivante ; « Mona » entre

Date : 2026-08-29 · Statut : accepté

> **Addendum (2026-08-29, A95)** : le thème est renommé
> « Innamoramento » par le CE le jour même, avant toute release —
> identifiants `innamoramento`/`innamoramento-nuit`, migration écrite
> des anciens ids persistés. Les décisions D1-D3 ci-dessous sont
> inchangées ; « Mona » y reste le nom de naissance.

## Contexte

V7 (ADR 0026, 2026-08-24) avait figé la table à « deux thèmes, et
deux seulement ». Le CE demande un troisième thème, « Mona » —
couleur principale `#AD204C`, couleur de tuiles `#A0868F`, en clair
et en sombre. L'instruction sur pièces a mesuré, au banc exact de la
gate (`e2e/contraste.mjs`, mêmes paires, mêmes seuils) :

- `#AD204C` tient **6,80:1 sur blanc** — accent ET marque du clair,
  tel quel (le dédoublement d'A8 est sans objet) ; c'est aussi l'hex
  de `--rep-rose`, coïncidence assumée ;
- `#A0868F` comme `--tuile` verbatim est **arithmétiquement
  impossible** : 2,04:1 sous `ink2` (seuil 4,5), 1,88:1 sous le pire
  repère partagé (seuil 3) ; en nuit, le blanc pur ne donne que
  3,33:1 — aucune encre ne peut tenir.

## Décision (CE, 2026-08-29)

- **D1** — V7 est **amendée** : « nous pouvons ajouter ou supprimer
  des thèmes de temps à autre ». La table reste COURTE (jamais le
  retour des 28 Wada) ; Mona et Mona · nuit sont les 3ᵉ et 4ᵉ thèmes.
- **D2** — la tuile **décline la teinte** de `#A0868F` par polarité
  (`#EFDFE4` clair / `#2C2126` nuit), le geste exact d'Elements sur
  sa propre tuile.
- **D3** — l'accent nuit est `#E58BA4` (éclairci à teinte constante,
  motif `#1A7A7A` → `#3FA39C`).

## Ce que cela renverse, et ce qui tient

Renversé : la lettre de V7 (« deux seulement »). Tient : son esprit —
une direction par thème, chaque thème mesuré ENTIER (17 jetons × 2
polarités) aux seuils communs, jamais une combinatoire ; les 24
`--rep-*` restent la table unique par polarité (leur bloc nuit est
servi par `[data-theme$="-nuit"]`, comme le `color-scheme` d'A44) ;
la mécanique A42 (suivi OS, polarité dérivée jamais persistée)
inchangée. Journal du Système : **A94**.

## Réversibilité

Retirer un thème = le trajet inverse du même contrat (« Ce que coûte
l'adoption » au Système) : blocs CSS, fiche, catalogues,
`NOMBRE_ATTENDU`, deux `toHaveCount`, table du doc — et la garde de
migration de `theme.js` apprend le retrait (le motif V7 : la
polarité survit, le reste retombe au défaut).

## Preuves

Contraste **440 paires, 0 échec** (220 → 440 — 110 par thème neuf :
38 paires de jetons, 60 repère × fond, 12 glyphes) ; cohérence 4 thèmes /
68 jetons, le doc dit le livré ; garde de migration **prouvée en la
cassant** (sans `mona-nuit` dans sa liste, un choix persisté était
réécrit `elements-nuit` — RED montré, puis GREEN) ; e2e des deux
specs impactées 66/66 ; STOP visuel CE du 2026-08-29 : GO sur
captures réelles clair + nuit.
