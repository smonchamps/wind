# Le Système v2 « Elements » — générateur

**Exploration. Jetable. Rien ici n'est livré.**

Produit [`docs/design/systeme.v2.dc.html`](../../../docs/design/systeme.v2.dc.html) —
une réécriture complète du Système dans la direction « Elements », avec
les 78 glyphes du spike et l'icône Wind du document d'icônes, en **deux
thèmes** (clair + nuit) tirés des couleurs posées autour de cette icône.

```bash
node spikes/direction-elements/v2/faire.mjs                    # le document
node spikes/direction-elements/v2/apercu.mjs ecran02           # une partie seule, pour la regarder
node spikes/direction-elements/v2/apercu.mjs icones ecran04 --nuit
node spikes/direction-elements/v2/apercu.mjs coins              # la trace de l'arbitrage (V14)
```

## Zéro rayon (V14)

Verdict du Chef Ingénieur : **coin vif partout**. Les trois **jetons de
forme** — `--r-surface`, `--r-controle`, `--r-tuile` — valent **0**, et
il n'y a plus un seul littéral de rayon à écrire dans le système :
« aucune autre valeur » cesse d'être une règle qu'on obéit à la main.
Ils sont déclarés sur `html` et non sur `:root` — ils ne dépendent pas de
la polarité, et le contrat des jetons de couleur ne doit pas s'en
trouver gonflé.

Restent **deux formes rondes**, et chacune dit quelque chose : le
**disque** (l'état, l'identité — non-lu, cycle, repère, poignée
d'interrupteur) et la **pilule** (le glissement — la piste de
l'interrupteur, et elle seule). Plus **une exception déclarée** :
l'icône d'application garde son rayon de plateforme (15/64).

**Validé au terrain le 2026-08-24** — constat du Chef Ingénieur sur le
rendu réel du document dans le navigateur. Réserve nommée : ce qui a été
regardé est le document, pas une fenêtre d'application posée à côté des
applications Windows ; l'idiome Fluent se re-constatera à la première
fenêtre livrée.

**Rembobiner tient en une ligne** : remettre `10px / 6px / 2px` aux trois
jetons dans `socle.mjs`. C'est précisément pour ça qu'ils existent, et
ils restent.

La bascule qui servait à l'arbitrage a été **retirée** : un Système qui
offre deux états de sa propre règle est un Système qui n'a pas tranché.
La comparaison reste, à la section « Les coins », comme trace de ce qui a
été écarté.

## Trois gardes — le script SORT EN ÉCHEC si l'une cède

| | |
|---|---|
| **Le relevé couvre le catalogue**, dans les **deux sens** | A18 rendu mécanique. Le Système livré *promettait* « ce qu'il dessine est livré, ce qui est livré s'y dessine » et l'avait perdu sur **dix glyphes** sans le voir : une promesse ne tient pas, une assertion oui. |
| **Aucun contraste sous son seuil** | 76 paires + 24 repères, **calculés** à la génération (mêmes formules WCAG, même table que `e2e/contraste.mjs`), jamais recopiés. |
| **Le journal compte ≥ 70 amendements** | Il est relu dans `docs/design/systeme.dc.html` et repris **verbatim** — si la source change de forme, on le sait au lieu de produire un trou. |

## Les fichiers

| | |
|---|---|
| `socle.mjs` | les 17 jetons × 2 thèmes, le nuancier des repères, le banc, le rendu des glyphes, la feuille de style |
| `parties-1.mjs` | en-tête, sommaire, Principes, Marque, Couleurs, Thèmes, Typographie, Troncature, Formes, Kit |
| `parties-2.mjs` | Icônes (le jeu complet), Écran 01, Écran 02, Barre d'état — et les briques d'interface partagées |
| `parties-3.mjs` | Écran 03, Écran 04, Réglages, Migration, Avis, Ligne de message, Journal + les décisions V1–V14 |
| `parties-coins.mjs` | la section « Les coins » — l'arbitrage V14, mesuré et posé côte à côte |
| `faire.mjs` | assemble, normalise (`scope="col"`), écrit, mesure, garde |
| `apercu.mjs` | rend une ou plusieurs parties seules dans `apercu.html` — contrôle visuel, hors document |

## Deux règles de tenue, apprises en se relisant

**Aucun hex hors de la table des jetons**, à trois exceptions **nommées** :
les deux couleurs figées de la marque en tuile (W-D3), les 24 teintes du
nuancier des repères (A74), et les deux valeurs que `mail-render` bake
dans le corps d'un courriel (`#222222` / `#ffffff`, `Palette::default`).
Tout le reste passe par un jeton — sinon le thème nuit ment.

**La teinte d'un repère passe par `.rep[data-teinte]`**, jamais par un
style en ligne. Une pastille figée au clair rend son glyphe à **2,35:1**
en nuit : c'est exactement la régression que V5 prétend éviter, et la
première rédaction de ce document la commettait dans ses propres
maquettes. Seules les 24 pastilles de la **table de référence** du
nuancier portent une teinte figée, et elles le disent par la classe
`.echantillon`.

## Ce que la gate en dit

`e2e/coherence-systeme.mjs` ne lit que `docs/design/systeme.dc.html` : ce
document lui est **invisible**, rien ne casse. En revanche, l'adopter
coûte **sept fichiers et trois contrôles de gate** — le relevé complet vit
à la section « Thèmes » du document, sous le titre *Ce que coûte
l'adoption*. Ce n'est pas « seulement `NOMBRE_ATTENDU` » ; la première
rédaction l'affirmait, et c'était faux.
