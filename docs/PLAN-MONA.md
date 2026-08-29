# PLAN-MONA — un troisième thème « Mona » (clair + nuit)

> **CHANTIER SOLDÉ le 2026-08-29 — terrain complet.** Commit
> `409c8ae`, CI verte run 33270609284. GO CE du plan (D1-D3) et GO du
> STOP visuel le 2026-08-29 ; terrain CE le jour même : « Terrain OK
> sur les deux thèmes, GO » — **zéro constat, zéro retouche**.
> Système : A94 ; ADR 0027 ; dette neuve D-45 (vignettes hors gate).
> À embarquer dans la prochaine release (entrée CHANGELOG à écrire à
> ce moment-là, §2.9). Kaizen : session 7b76f83d, 8,7 M équiv. input,
> 1 gate complète (2,7 min) + 1 pre-push, 0 KO au STOP 2.

> Énoncé CE (2026-08-29) : « Implémente un thème dont la couleur
> principale est #AD204C et la couleur des tuiles est #A0868F. Ce
> thème s'appelle "Mona". Fais une version claire et son équivalent
> sombre. »

## 1. Constat — les faits, les chiffres

- **La décision V7 est en travers de la route.** Le Système (journal
  V7, 2026-08-24) et l'ADR 0026 disent « **Deux thèmes, et deux
  seulement** » — la table Wada de 28 thèmes a été retirée il y a
  cinq jours. Ajouter « Mona » amende V7 : 2 → 4 thèmes
  (`mona` + `mona-nuit`). C'est une décision CE, pas une décision
  d'ingénierie (→ D1).
- **`#AD204C` tient comme couleur principale, mesuré** : 6,80:1 sur
  blanc (seuil texte 4,5:1) — il peut servir d'`--accent` ET de
  `--marque` du thème clair **tel quel**, sans le dédoublement
  qu'Elements a dû payer (remède A8). Fait connexe : ce hex est déjà
  dans le produit — c'est `--rep-rose`, le repère de compte rose.
- **`#A0868F` ne peut PAS être servi tel quel comme `--tuile`** —
  trois impossibilités arithmétiques, mesurées au banc exact de la
  gate (`e2e/contraste.mjs`, mêmes paires, mêmes seuils) :
  - encre secondaire dessus : **2,04:1** (seuil 4,5:1) ;
  - accent `#AD204C` dessus : **2,04:1** (seuil 3:1) ;
  - pire repère sombre (`--rep-ocre`) dessus : **1,88:1** (seuil
    3:1) — et le nuancier des repères est PARTAGÉ entre thèmes, il
    ne peut pas être re-décliné pour Mona seul ;
  - en nuit, c'est pire : la meilleure encre possible (blanc pur)
    donne **3,33:1** — le seuil 4,5:1 de `--tuileInk` est
    **inatteignable** quelle que soit l'encre.
- **La voie qui marche est celle qu'Elements pratique déjà** : la
  tuile se décline par polarité à teinte constante
  (`#F2EDE3` clair / `#241F17` nuit). Pour Mona : la teinte
  mauve-rosée de `#A0868F` (≈ 340°), éclaircie en clair
  (`#EFDFE4`), assombrie en nuit (`#2C2126`) (→ D2).
- **La mécanique généralise sans refonte.** `theme.js` dérive tout
  de `FICHES` ; `refleter()` sait déjà faire `mona → mona-nuit` sous
  suivi OS ; Réglages et Accueil bouclent sur `FICHES` ; aucun code
  Rust. Le coût est celui que le Système documente lui-même
  (« Ce que coûte l'adoption ») : CSS, fiches, catalogues,
  `NOMBRE_ATTENDU`, deux `toHaveCount(2)` d'e2e, table de contrat du
  Système.
- **Un piège identifié à la lecture** : la migration V7 de
  `theme.js` (l. 33) réécrit tout choix `*-nuit` inconnu en
  `elements-nuit` — sans extension de sa liste, un choix `mona-nuit`
  persisté serait **écrasé à chaque démarrage**. Un test e2e le
  couvrira (RED d'abord).

## 2. Palette candidate — mesurée VERTE (220 paires dans la gate livrée, 110 par thème neuf, zéro échec)

> La table ci-dessous est l'état ARBITRÉ au GO du 2026-08-29. Le
> contrat qui fait foi (et que la gate tient) vit au Système, section
> Thèmes — en cas d'écart après retouche terrain, c'est lui qui dit
> vrai.

| jeton | Mona (clair) | Mona · nuit |
|---|---|---|
| `--bg` | `#F4F0F1` | `#151012` |
| `--surface` | `#FFFFFF` | `#1E171A` |
| `--ink` | `#1D181A` | `#EEEAEC` |
| `--ink2` | `#59484E` | `#BCB1B6` |
| `--muted` | `#5F4C53` | `#A2969C` |
| `--border` | `#CDBFC4` | `#3E3339` |
| `--accent` | `#AD204C` (CE, tel quel) | `#E58BA4` |
| `--accentH` | `#8E1A3E` | `#EFA5B9` |
| `--marque` | `#AD204C` | `#E58BA4` |
| `--sel` | `#F4DCE4` | `#3A2029` |
| `--hover` | `#ECE3E5` | `#191316` |
| `--tuile` | `#EFDFE4` (teinte de `#A0868F`, éclaircie) | `#2C2126` (assombrie) |
| `--tuileInk` | `#54333F` | `#DFC9D1` |
| `--alert` | `#B02A21` | `#EF9C93` |
| `--onAccent` | `#FFFFFF` | `#33101B` |
| `--shadow` | `0 2px 8px rgba(29,24,26,0.08)` | `0 2px 12px rgba(0,0,0,0.40)` |
| `--scrim` | `rgba(29,24,26,0.28)` | `rgba(0,0,0,0.55)` |

Notes : l'accent nuit suit le motif d'Elements (la nuit éclaircit la
teinte, `#3FA39C` ← `#1A7A7A`). `--alert` reste un rouge-orangé,
volontairement distinct de l'accent rose — les deux voisinent
(mention Brouillon, erreurs) ; l'écart de teinte est le seul
séparateur restant, il est assumé et sera regardé au STOP visuel.

## 3. Refus de périmètre (§2.6)

- **Pas de nuancier de repères propre à Mona** : les 24 `--rep-*`
  restent la table unique par polarité (A74/A82) ; la seule retouche
  est de servir la table claire à TOUTE polarité `-nuit` (sélecteur
  `$="-nuit"`, précédent : le `color-scheme` d'A44), sans changer un
  seul hex.
- **Pas de retour de la table Wada** : Mona est UN thème de plus
  (2 → 4), pas la réouverture des 28.
- **Pas de re-dérivation des couleurs de MARQUE** (W-D3/V11) :
  l'icône d'app, la bande de marque, « Made in EU » restent figés
  hors thèmes.
- **Aucun code Rust** : les thèmes sont front-end purs.

## 4. Étapes

- **E1 — le socle rouge → vert** : `NOMBRE_ATTENDU` 2 → 4 et les
  deux `toHaveCount(2)` → 4 posés d'abord (RED groupé montré :
  contraste, cohérence, e2e) ; puis `systeme.css` (2 blocs de 17
  jetons ; repères claires servis par `[data-theme$="-nuit"]` et
  `lireReperes` de `contraste.mjs` amendé de même), `FICHES`
  (pastilles `[accent, bg, border, surface, ink]`), catalogues fr/en
  (`theme.mona.*`, `theme.mona-nuit.*`), garde de migration
  `theme.js` étendue (`mona`, `mona-nuit`). GREEN groupé.
- **⛔ STOP visuel précoce** : captures de l'app réelle en Mona
  clair et nuit (écrans 01/02, Réglages) — verdict d'apparence CE
  avant de dérouler la suite (leçon A58).
- **E2 — le filet** : e2e de sélection/persistance/suivi OS étendus
  à Mona (spec `refonte-ecran02`, cartes d'accueil
  `refonte-retours-8`), test de la migration NON-écrasante
  (`mona-nuit` persiste au démarrage — prouvé RED sur le code
  actuel).
- **E3 — le Système et l'ADR** : table de contrat + fiches
  visuelles + journal A-n dans `systeme.dc.html` (même commit que
  l'UI, DC-D2) ; **ADR 0027** court : « Mona s'ajoute — V7 amendé
  (2 → 4), la direction reste une palette par thème, mesurée ».
- **E4 — qualité et terrain** : revue à regard neuf, `/gate`
  complète, checklist terrain (STOP 2), commit, CI.

## 5. Décisions CE

- **D1 — Rouvrir V7 ?** V7/ADR 0026 (2026-08-24) fige « deux thèmes,
  deux seulement ». Ajouter Mona = amender cette décision (2 → 4,
  ADR 0027). GO / NO-GO.
- **D2 — La tuile déclinée.** `#A0868F` tel quel est impossible aux
  seuils de la gate (2,04:1 ; 1,88:1 ; nuit inatteignable —
  chiffres §1). Proposition : décliner la TEINTE par polarité
  (`#EFDFE4` / `#2C2126`), comme Elements. Accepter, ou arbitrer
  autrement (ex. assouplir un seuil — déconseillé : la gate est la
  loi commune des thèmes).
- **D3 — L'accent nuit.** `#AD204C` ne tient pas sur les fonds
  sombres (2,6:1 sur `#1E171A`) ; proposition `#E58BA4` (rose
  éclairci, motif Elements-nuit). Accepter, ou fournir une autre
  déclinaison.

### Réponses CE (2026-08-29, mot pour mot)

- **D1** : « Amender V7 en disant que nous pouvons ajouter ou
  supprimer des thèmes de temps à autre. Mona version claire et
  version sombre sont les 3e et 4e thèmes. » — GO. L'ADR 0027
  consignera V7 amendée ainsi : la table des thèmes est COURTE et
  VIVANTE (ajouts/retraits possibles de temps à autre), pas figée à
  deux.
- **D2** : « Décliner la teinte (Recommandé) » — tuile `#EFDFE4`
  clair / `#2C2126` nuit.
- **D3** : « #E58BA4 la nuit (Recommandé) ».

## 6. Avancement

- **E1 livré** (2026-08-29) : RED montré (plancher à 4, les deux
  gates crient), puis GREEN — contraste **440 paires 0 échec**,
  cohérence 4 thèmes / 68 jetons. Les repères claires servis par
  `[data-theme$="-nuit"]` (les deux gates amendées avec).
- **STOP visuel : GO CE le 2026-08-29** (« l'apparence convient »),
  sur captures réelles clair + nuit (écran 02 + sélecteur 4 fiches).
- **E2 livré** : `toHaveCount(4)` aux deux specs, Mona s'applique et
  persiste, migration NON-écrasante **prouvée en la cassant** (RED
  exact : `mona-nuit` → `elements-nuit` sans la garde) ; GREEN groupé
  66/66.
- **E3 livré** : Système amendé (prose, fiches, contrat, journal
  A94) ; **ADR 0027** écrit.
- **E4 livré** : revue à regard neuf 6 angles / 10 retenues /
  **9 corrigées** (le chiffre 166→220 gravé faux, la fuite de thème du
  test de migration, la garde dérivée de THEMES, la regex des blocs
  `--rep-*` centralisée dans jetons.mjs, cinq proses normatives
  remises au vrai) ; la 10ᵉ (vignettes du doc hors gate, motif
  préexistant) assumée → dette au solde. Gate complète **VERTE en
  2,7 min** (contrastes 440/0, e2e **153/153**, zéro flaky).
- **⛔ STOP 2 — terrain CE le 2026-08-29 : « Terrain OK sur les deux
  thèmes, GO. »** Zéro constat.
