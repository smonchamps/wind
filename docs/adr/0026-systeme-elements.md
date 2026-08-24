# ADR 0026 — Le Système « Elements » remplace « Clarity / Wada »

Date : 2026-08-24 · Statut : accepté

## Contexte

Le CE a fait instruire une direction artistique nouvelle — « Elements » :
une seule règle de forme partout, marqueurs au centre géométrique, une
seule distance décidée, aucune correction optique. Deux spikes l'ont
jugée sur pièces ([`spikes/direction-elements/`](../../spikes/direction-elements/README.md),
`spikes/marque-hitofude/`) : banc de contraste 74 mesures 0 échec
(palette corrigée du minimum à teinte constante, remède A8), 78 glyphes
redessinés en trois tours de retours CE, 7 signatures animées
confrontées, centrage du disque mesuré à 0,00 px. Le générateur du
spike a produit un Système complet (`systeme.v2.dc.html`) portant 14
décisions CE au journal (V1–V14) — dont zéro rayon (V14), validée au
terrain le 2026-08-24 sur le rendu réel.

## Décision

Le Système « Elements » **devient le Système de référence**
(`docs/design/systeme.dc.html`, le chemin que lisent les gates) et
**l'UI le livre** (PLAN-ELEMENTS, E1–E5). Décisions CE du 2026-08-24 :

- **D1** — le HTML est LA source, éditée à la main (DC-D2 inchangé) ;
  le générateur reste figé en spike (trace) ; l'ancien Système est
  archivé (`docs/archives/systeme.v1.dc.html`) ; la série V est close,
  le journal continue en A-n (A79 = l'adoption).
- **D2** — attribution des glyphes : « Jeu original de Wind, dessiné
  d'après Material Symbols (Google, Apache 2.0) » ; la LICENSE Apache
  reste au dépôt au titre de la provenance.
- **D3** — les trois familles de fusion restent différenciées par
  leurs marques actuelles (verdict au STOP E2 : suffisant à 16 px,
  aucun redessin).
- **D4** — les maîtres 24 réduits sont livrés ; le palier 16 (74
  dessins + 12 paliers 10-12) est une dette consignée (D-35), à
  rouvrir si le terrain voit le flou.
- **D5** — véhicule : **0.9.0, MINEUR** ; la preuve OAuth du second
  poste (différée de la 0.8.0) peut se faire sur la 0.9.0.

## Ce que cela renverse, et ce qui tient

Renversé : A42 (28 thèmes → 2), A28/A36/A40/A52-signature (le trait
hitofude meurt — remplacé par la paire disque/anneau, V2), A29 point 2
(la pastille pleine de nav → nombre nu + disque de rangée, V4), A30
(la marque passe au glyphe/tuile Elements, V1/V11), les rayons (V14 :
zéro, trois jetons de forme sur `html`, exception plateforme 15/64).
Tient : A3 (une icône, un sens — le jeu dédié des repères, les
réservés), A8 (jamais la couleur seule ; deux corrections de palette
au minimum), A18 (renforcé : le relevé d'icônes est vérifié par la
gate dans les DEUX sens, tracés compris), A52 (le % dans le texte),
A61 (la dalle claire du corps), A74 (le nuancier des repères, V5
contre la doctrine — l'accessibilité prime).

## Réversibilité

V14 se rembobine en une ligne (remettre 10px/6px/2px aux trois
jetons). Le retour à Wada entier serait un chantier (l'archive
`systeme.v1.dc.html` et l'historique git portent tout) — assumé : la
table de 28 thèmes n'est plus tenue ni mesurée.

## Preuves

Cinq commits gate-verts (`fb32238`, `fa45db7`, `3aa8a2d`, `84d46ea`,
`fed73e5` — pile réécrite avant push : un accent dans un message de
commit, STANDARD §2.9), 124/124 e2e à chaque étape, quatre STOP
visuels CE le jour
même (socle, icônes, formes, marque), contraste 220 paires 0 échec.
Terrain complet au STOP 2 de PLAN-ELEMENTS.
