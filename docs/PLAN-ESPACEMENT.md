# PLAN-ESPACEMENT — trois crans d'air entre les messages

> **SOLDÉ au terrain le 2026-08-25 — 7/7, zéro constat.** STOP 1 passé
> le même jour (§6 : D1 = 13/19/25, D2 = Réglages seuls, D3 = corriger
> le défaut préexistant ici), six étapes livrées, revue à regard neuf
> passée (7 angles, 31 candidats, **29 retenus** — §9), **gate complète
> VERTE en 2 min** (9/9, e2e 129 → **137**).

> Énoncé (2026-08-25) : « Ajoute une option d'affichage qui propose 3
> niveaux d'espacement entre chaque email dans le volet central.
> L'espacement actuel est "Faible", les deux autres niveaux sont
> "Moyen" et "Élevé". »

---

## 1. Constat

La rangée de la liste a **un seul air possible** : `padding:13px 16px`
(`Liste.svelte`), soit une rangée de **88 px** nue et **115 px**
porteuse d'un rang de puces — mesuré au banc et confirmé au terrain du
2026-08-24. Rien ne le règle.

Ce n'est pas un défaut : c'est un choix qui n'a jamais été offert. La
demande est d'en faire un réglage à trois crans, « Faible » étant
l'existant **au pixel près**.

## 2. Périmètre — et ce qu'on ne fait pas

**Dans le périmètre** : les rangées du **volet central** (la liste),
c'est-à-dire les cinq poses de `.ligne` — sondes, ligne d'attente,
rangée du flot, section épinglée, dossier Brouillons — plus le réglage
aux Réglages > Affichage et sa persistance.

**Refusés explicitement** (§2.6 du STANDARD) :

| | |
|---|---|
| Le **fil** et le volet de lecture | L'énoncé dit « le volet central ». Les cartes de message ont leur propre grammaire d'espacement ; les régler ensemble mélangerait deux dessins. |
| Une **étape d'accueil** dédiée | Voir D2 : amender A75 (quatre étapes, récapitulatif à trois cartes, captures réelles et leurs e2e) coûte un chantier à lui seul, pour un réglage qu'on ajuste une fois. |
| Une **densité de police** ou de contenu | « Espacement » ici veut dire l'air entre les messages, pas la taille du texte ni le nombre de lignes d'aperçu. Un mode « compact » qui retirerait l'aperçu est un autre sujet. |

## 3. Le point dur, et sa mesure

**Le risque est le fenêtrage, et il est chiffré.** La liste mesure
`h1`/`h2` avec deux rangées « sondes » rendues une fois puis **retirées
du DOM** ; `sondees` n'est **jamais** remis à `false` (une seule
écriture dans tout le fichier, et l'effet de changement de source remet
neuf champs à zéro mais pas celui-là). Changer le cran redessinerait
donc les rangées à la nouvelle hauteur pendant que les gabarits
resteraient figés sur l'ancienne :

| | |
|---|---|
| Barre de défilement | ment de **13,6 %** (Faible → Moyen), **27,3 %** (→ Élevé) |
| Saut à l'index 500 | pose la fenêtre **6 000 px** (Moyen) à **12 000 px** (Élevé) au-dessus de sa position réelle |
| Au-delà de ~6 rangées d'écart | la fenêtre sort de la zone visible : **écran blanc** jusqu'au prochain défilement |

**Deux voies, et la mesure les départage** (banc jeté
`spikes/espacement/sondes.mjs`, msedge — le moteur réel de WebView2,
géométrie exacte du produit, une seule rangée servie : le pire cas) :

| variante | fantôme à 120 px de cadre | à 150 | à 203 | h1/h2 rendus |
|---|---|---|---|---|
| **A** — sondes retirées (l'actuel) | 0 | 0 | 0 | *plus mesurables* |
| **B** — sondes permanentes, `absolute` | **85 px** | 55 | 2 | 88/115 ✓ |
| **C** — cage `height:0;overflow:hidden` | **85 px** | 55 | 2 | 88/115 ✓ |
| **D** — cage **`position:relative`** + `height:0;overflow:hidden` | **0** | **0** | **0** | **88/115 ✓** |

« Fantôme » = ce que la barre de défilement offre en trop.

**La cage naïve (C) ne protège rien**, et c'est le genre de détail
qu'un raisonnement rate : la cage n'étant pas positionnée, elle n'est
pas le bloc conteneur des sondes en `position:absolute` — elles se
calent sur `.cadre` et échappent au clip. Il suffit d'un
`position:relative` (D) pour qu'elles soient clippées et sortent de la
région défilante.

**Verdict** : **D**. Les sondes restent montées, se re-mesurent seules
au changement de cran (`bind:offsetHeight`, qui compile vers un
ResizeObserver — le patron déjà employé pour `hautEpingles`), et le
coût qu'on leur reprochait est mesuré **nul à toutes les hauteurs**.
`sondees`, `sonder()` et tout l'ordonnancement disparaissent : la
classe de bug devient **impossible** au lieu d'être corrigée à un
endroit. C'est aussi ce que le Système affirme déjà (A9 : « les
gabarits de la liste fenêtrée se sondent à l'exécution »).

## 4. Ce que l'instruction a établi — faits vérifiés

1. **Le porteur existe déjà.** Le patron des largeurs de volets
   (`style="--l-nav:{lNav}px"` sur un conteneur, lu en
   `var(--l-nav, 248px)`) est exactement la forme voulue. Un jeton
   `--rangee-pad` posé en `style=` sur `.cadre` prend les **cinq**
   poses de `.ligne` d'un coup, **sondes comprises** — le périmètre se
   règle sans une ligne de code de plus, et la première sonde d'un
   démarrage mesure déjà le bon cran.
2. **Le trait d'union protège.** `--rangee-pad` échappe au contrat des
   17 jetons de thème (le parseur de `jetons.mjs` ne capture pas les
   noms à trait d'union) — comme `--l-nav` et `--rep-*`. Aucun contrôle
   de `coherence-systeme.mjs` n'est déclenché, et c'est voulu : c'est
   une dimension de mise en page, pas un jeton de thème.
3. **L'espacement DOIT rester dans le `padding`.** `offsetHeight` ne
   voit que la boîte de bordure : une `margin-bottom:12px` ou un
   `row-gap:12px` sur `.fenetre` donnent **12,375 px par rangée
   invisibles à la sonde** — la sonde rendrait la bonne valeur et le
   rendu mentirait quand même. À graver au plan : **ni marge, ni gap**.
4. **Deux « décisions » n'en sont pas.** Le normatif tranche déjà :
   préférence pure UI → `localStorage` (le shell n'a rien à en lire),
   et contrôle → **sélecteur natif** habillé aux jetons, avec A26 pour
   précédent exact (rangée « Disposition », même groupe, même patron).
   Aucun dessin neuf, aucune décision à poser.
5. **`extraPuce` reste 27 px** quel que soit le cran (24 px de puces +
   3 de `row-gap`) : le coût marginal d'un rang est invariant, la
   correction itérative d'A44 n'est pas touchée.
6. **Aucun test existant ne casse, aucun ne protège.** Aucun e2e
   n'assertionne une hauteur de rangée en pixels ; les mesures de
   rangée existantes sont relatives et survivent aux trois crans. Le
   filet de sûreté est **intégralement à écrire**.
7. **Zéro risque neuf sur les gates** : `contraste.mjs` ne lit que des
   hex (aucune couleur touchée), `coherence-systeme.mjs` ne lit des
   `.svelte` que les icônes et les littéraux de rayon (V14 intact).

## 5. Les trois crans — options chiffrées

Le delta est arithmétique et exact : **+6 px de padding = +12 px de
rangée**. Rangées **pleines** visibles, par option :

| | Faible | Moyen | Élevé |
|---|---|---|---|
| **Option A — 13 / 19 / 25 px** | 88 px | 100 px | 112 px |
| blanc entre deux messages | 27 px | 39 px | 51 px |
| fenêtre par défaut (1000×700) | 5 | **5** | 4 |
| 1080p maximisé | 9 | 8 | 7 |
| 1440p maximisé | 13 | 11 | 10 |
| **Option B — 13 / 21 / 29 px** | 88 px | 104 px | 120 px |
| blanc entre deux messages | 27 px | 43 px | 59 px |
| fenêtre par défaut (1000×700) | 5 | **4** | 4 |
| 1080p maximisé | 9 | 7 | 6 |
| 1440p maximisé | 13 | 11 | 9 |

**Le point honnête à poser** : avec l'option A, à la fenêtre **par
défaut**, « Faible » et « Moyen » montrent le **même nombre** de
rangées pleines (5) — le cran se voit à l'air, pas au compte. Si le
Chef Ingénieur veut que le compte bouge dès le défaut, c'est l'option
B, au prix d'un « Élevé » à 120 px (une rangée et demie de l'ancien
gabarit).

Dans les deux cas, **le défaut reste 13 px** : l'existant au pixel
près. Un défaut déplacé ferait bouger l'écran de tous les postes sans
que personne l'ait demandé, et ajouterait une variable à la série du
banc P1, déjà non re-basée (dette D-14).

## 6. Décisions CE

| | | |
|---|---|---|
| **D1** | **Quelles valeurs** — option A (13/19/25) ou B (13/21/29) ? | Recommandation : **A**. Le saut de 12 px se voit franchement, et « Élevé » à 112 px reste une rangée, pas un pavé. B se justifie seulement si le compte de rangées doit bouger dès la fenêtre par défaut. |
| **D2** | **Le réglage entre-t-il au parcours d'accueil** (5e étape) ? | Recommandation : **non**, et le refus s'écrit. Le défaut étant l'existant, personne ne perd rien à ne pas le voir au premier lancement ; l'ajouter touche A75, le récapitulatif, les captures réelles et leurs e2e — un chantier à lui seul. |
| **D3** | **Le défaut préexistant de `visibles`** (la fenêtre ne se recalcule pas au redimensionnement : agrandir de plus de ~8 rangées laisse une bande vide jusqu'au prochain défilement) — corrigé ici, ou inscrit en dette ? | Recommandation : **corrigé ici** (~6 lignes, un ResizeObserver sur `.cadre`). Motif de méthode, pas de confort : ce chantier va le **masquer par intermittence** (chaque changement de cran recalcule `visibles`), ce qui le rendra irreproductible au terrain suivant. Si le CE préfère la dette, elle doit porter cette phrase-là, sinon elle sera classée non reproductible. |

**Décision d'ingénierie, tranchée par la mesure (§3)** — elle n'appelle
pas d'arbitrage CE, elle est consignée : les sondes deviennent
**permanentes, en cage positionnée** (variante D). Zéro fantôme mesuré,
`sondees`/`sonder()` supprimés.

### Verdicts du Chef Ingénieur — 2026-08-25

| | réponse | |
|---|---|---|
| **D1** | **« 13 / 19 / 25 px »** | Rangées 88 / 100 / 112 px. Le cran se voit à l'air ; « Faible » et « Moyen » montrent le même compte de rangées à la fenêtre par défaut, c'est assumé. |
| **D2** | **« Non, Réglages seuls »** | 4e rangée du groupe Affichage, après « Disposition ». A75 reste à quatre étapes — le refus est écrit au §2. |
| **D3** | **« Corriger dans ce chantier »** | Le défaut de `visibles` au redimensionnement est corrigé ici (ResizeObserver sur `.cadre`) plutôt qu'inscrit en dette : le chantier l'aurait rendu intermittent, donc irreproductible. Le chantier passe à **six** étapes. |

## 7. Étapes

| | | gate |
|---|---|---|
| **E1** | Le jeton `--rangee-pad` sur `.cadre`, les trois crans en dur (aucune logique, aucun réglage) — vérifiable à l'œil. **STOP visuel CE** sur les trois airs avant d'aller plus loin. | boucle intérieure |
| **E2** | Les sondes permanentes en cage positionnée ; `sondees`/`sonder()` meurent, `bind:offsetHeight` les remplace. Re-ancrage : `premier` capturé **avant** l'écriture du cran, `aller(premier)` en **dernier** geste. Ne toucher ni à `source` ni à `generation` (ce serait rejeter les pages servies, vider la sélection et refaire les appels — pour un changement cosmétique). | e2e de sûreté (E4) |
| **E3** | La préférence (`lib/espacement.svelte.js`, calque de `volets.svelte.js`), la rangée aux Réglages > Affichage (sélecteur natif, patron A26), les 5 clés × 2 catalogues. | parité fr/en |
| **E4** | L'**e2e de sûreté** — le vrai livrable. Après bascule à chaud : (i) `etat().h1` vaut la valeur attendue du cran, (ii) le `scrollHeight` du cadre vaut `total × h1` à la rangée près, (iii) **la ligne en tête de fenêtre est la MÊME qu'avant la bascule**. Le (iii) attrape la classe de bug du §3. | — |
| **E5** | **D3** : le défaut préexistant de `visibles` — un ResizeObserver sur `.cadre` alimente un `$state` de hauteur, la fenêtre se recalcule au redimensionnement. e2e dédié. | e2e neuf |
| **E6** | Le Système (DC-D2) : rangée dans la fiche Affichage, le passage « deux gabarits… tous deux sondés » de « Ligne de message » qui ne dit pas qu'ils sont réglables, amendement **A83** au journal. | gate complète, **une fois** |

**Fichiers** : 7 modifiés + 1 neuf. `App.svelte` n'est **pas** touché —
le patron du dépôt est le `$state` partagé importé directement par qui
lit (c'est ainsi que `volets.svelte.js` est consommé), aucune plomberie
de props.

## 8. Terrain — la liste de contrôle

À dérouler après E5, sur de vrais comptes :

1. Les trois crans se distinguent-ils **à l'œil**, et « Élevé » est-il
   utile ou seulement vide ?
2. **Bascule à chaud, liste défilée en profondeur** : la ligne du haut
   reste la même, la barre ne saute pas, aucun écran blanc. *C'est le
   point où ce chantier peut coûter.*
3. Bascule à chaud **avec une conversation épinglée** en tête.
4. Le dossier **Brouillons** et les **résultats de recherche** suivent
   le cran (même dessin que le flot).
5. La ligne d'**attente** (défilement rapide dans une grande boîte) a
   la même hauteur que les rangées servies — la fenêtre ne tremble pas.
6. Le réglage **survit au relancement**, et une valeur inconnue
   retombe sur « Faible ».
7. Fenêtre **très courte** (~400 px de haut) : aucune barre de
   défilement fantôme, aucune rangée cachée. *La garde de la mesure
   du §3.*

**Verdict CE du 2026-08-25 : 7/7, aucun constat.**

---

## 9. Revue à regard neuf — 2026-08-25

Sept angles indépendants (le fenêtrage en tête), puis un vérificateur
sceptique par candidat, **autorisé à mesurer** dans msedge plutôt qu'à
raisonner. 31 candidats, **29 retenus**, tous corrigés. Deux d'entre eux
étaient dans le code écrit une heure plus tôt, et aucun n'aurait été vu
sans banc.

| | Défaut | Remède |
|---|---|---|
| **R1** | **L'ordre des effets.** Le ré-ancrage lisait `premier` dans l'effet de `h1` — mais les épinglées sont des rangées : elles grandissent avec le cran, leur observateur de taille se déclenche dans le même lot, et l'effet qui les surveille (déclaré plus haut, donc joué **avant**) avait déjà réécrit `premier` avec la nouvelle hauteur contre l'ancien défilement. **44 rangées de dérive mesurées** avec deux épingles. Mon commentaire « lu avant que la nouvelle géométrie ne serve » était faux. | Deux temps : la **capture** se déclenche sur `padRangee()` — un état amont, qui bouge avant tout relayout —, l'**application** attend `h1`. |
| **R2** | **`aller()` partait dans toutes les vues.** Au dossier Brouillons, `total` reste 0, donc `aller(0)` : chaque changement de cran remontait la liste en haut. En recherche, il la déplaçait aussi. | Garde : le flot fenêtré seulement (`resultats === null && lignesBrouillons === null`). |
| **R3** | **Ré-ancrer depuis la bande épinglée la faisait sortir de l'écran** : `aller(0)` pose le défilement *sous* elle, précisément pour quelqu'un qui la regardait. | On ne touche à rien quand `scrollTop < hautEpingles`. |
| **R4** | **`'toString' in CRANS` vaut vrai** — l'opérateur remonte la chaîne de prototypes. Un `localStorage` tripoté rendait une **fonction** en guise de padding, que le CSS aurait traitée en valeur invalide (padding 0, liste écrasée, aucune erreur). Le patron de `volets.svelte.js`, dont ce module se dit le calque, utilise `includes`. | Garde par la liste `NIVEAUX`, aux trois points d'entrée. |
| **R5** | `let hAncre = h1` déclenchait l'avertissement Svelte `state_referenced_locally` — la construction n'était pas propre, contrairement à ce que j'avais annoncé. | Mort avec la réécriture de R1. |
| **R6** | Le banc `crans.mjs` écrivait ses PNG **à la racine du dépôt**, qui n'est pas ignorée. | Ils restent chez lui ; README ajouté au spike. |
| **R7** | A83 disait « les **cinq** poses » en comptant deux fois le même code (flot et épinglées partagent leur snippet), et la note du journal annonçait encore 96 lignes. | Corrigés : la phrase dit les rangées sans les compter faux, le journal dit 97 lignes / A1-A83. |

### Le filet de sûreté était décoratif — c'est la vraie leçon

La revue a démontré que **trois des cinq tests ne pouvaient pas
échouer** :

- « la ligne du haut ne bouge pas » lisait l'état interne `premier`, que
  rien ne recalcule quand `h1` change **hors** section épinglée :
  l'assertion passait même en **supprimant tout le ré-ancrage** ;
- « la barre dit la vraie hauteur » comparait deux membres tirés du
  **même** `h1` — une identité arithmétique, increvable ;
- « zéro barre fantôme » se jouait sur un cadre de 500 px quand le
  fantôme en fait 203 : retirer le `position:relative` de la cage
  laissait le test vert.

Et le décor n'avait **ni épingle** (le chemin de R1) **ni rangée
porteuse** (`h2` jamais vérifié), pendant que l'e2e de redimensionnement
promis à la décision D3 n'existait pas.

Le filet réécrit — **8 tests** — lit ce que l'utilisateur **voit** (le
sujet du message réellement en haut de l'écran), joue le chemin avec
épingle, mesure les deux gabarits dans le DOM plutôt que de les relire
du gabarit qui a servi à les calculer, rétrécit le cadre pour que le
fantôme puisse seulement apparaître, exerce les valeurs tordues
`toString` et `constructor`, et couvre le redimensionnement.

**Il a été prouvé non-vacant** : le ré-ancrage remis à sa version
fautive, le test à l'épingle **tombe**.
