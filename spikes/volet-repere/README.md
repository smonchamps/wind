# Le repère de boîte dans le volet central — sept dessins

**Exploration. Jetable. Rien ici n'est livré.**

Énoncé du Chef Ingénieur (2026-08-24) : retirer de la rangée de liste la
tuile aux **initiales de l'expéditeur**, et rendre le repère de la
**boîte de réception** « à la fois visible et discret ». Forme libre.

```bash
node spikes/volet-repere/planche.mjs   # les sept dessins comparés -> planche.html
node spikes/volet-repere/o2.mjs        # O2 en situation, fenetre entiere -> o2.html
node spikes/volet-repere/organisation.mjs  # quinze organisations du volet -> organisation.html
node spikes/volet-repere/ligne-expediteur.mjs  # « Expediteur sur <icone> Boite » -> ligne-expediteur.html
node spikes/volet-repere/v1v7.mjs      # V1+V7 en situation, 3 largeurs -> v1v7.html
```

`socle.mjs` porte la matière commune — lecture des jetons, nuancier,
glyphes, dessin de la rangée, **et la forme retenue** (`rangSur`,
`CSS_SUR`). `fenetre.mjs` porte la fenêtre de mise en situation (entête,
nav, volet, lecture, barre d'état). Deux copies divergeraient en silence :
il n'y en a qu'une de chaque.

## Ce que la planche garantit

| | |
|---|---|
| **Elle ne peut pas mentir sur le produit** | Les 17 jetons et les 24 hex du nuancier se **lisent** de `apps/desktop/ui-v2/src/systeme.css` par le parseur des gates (`e2e/jetons.mjs`) ; les glyphes s'importent du jeu livré (`lib/icones.js`) ; les initiales du témoin viennent de `lib/initiales.js`. Le script **sort en échec** si les deux thèmes ou les 12 × 2 teintes ne se lisent pas. |
| **Elle compare une seule chose** | Mêmes six rangées, mêmes trois comptes, même largeur de 400 px (le défaut de `lib/largeurs.svelte.js`), même rangée choisie (la quatrième). Seuls la place et la forme du repère changent. |
| **Elle se juge aux deux polarités** | Un repère se juge sur le fond où il se pose : chaque dessin est rendu en clair ET en nuit, côte à côte. |
| **Les contrastes sont calculés, pas recopiés** | Le banc d'O2 (96 mesures) applique les formules de `e2e/contraste.mjs` aux hex lus dans le CSS. |

## Mesuré au rendu (et non supposé)

Largeur offerte à l'objet et à l'aperçu, volet à 400 px — 365 px moins la
colonne de tête et sa gouttière :

| | témoin | O1 | O2 | O3 | O4 | O5 · O6 · O7 |
|---|---|---|---|---|---|---|
| largeur de texte | 327 | 339 | 337 | 337 | 339 | **365** |

- **Les deux gabarits d'A44 tiennent** partout (88 / 115 px au rendu de la
  planche ; le code part de 90 / 117 puis sonde) — **sauf O7**, qui n'a
  plus qu'un gabarit : 115 px pour toutes les rangées, soit **−23,4 % de
  rangées à l'écran**.
- **Aucun des sept n'introduit une paire de contraste neuve.** Le nuancier
  d'A74 les avait déjà toutes payées.
- **Pire cas du glyphe nu (O2)** sur `bg` / `hover` / `sel` / `tuile`, les
  12 teintes × 2 polarités : **4,97:1** — bien au-dessus du seuil
  composant de 3:1.
- **O3** : la rive est centrée à **0 px** d'écart du centre géométrique
  des rangées nues comme des porteuses. Le premier jet flottait 35 px
  trop haut (`grid-row:1/-1` ne couvre que la première ligne d'une grille
  implicite) — mesuré, corrigé.
- **O6** : sur la rangée choisie, le liseré rend `#1A7A7A` (l'accent) et
  non la teinte du compte. Le conflit des deux sens sur deux pixels n'est
  pas une crainte, il se voit sur la planche.

## Ce que la planche ne peut PAS dire

- Le rendu à la **fenêtre réelle** (WebView2, échelle de texte du poste)
  — un navigateur n'est pas la fenêtre livrée ; la réserve Fluent de V14
  vaut ici aussi.
- La **fréquence d'alternance des comptes** en boîte unifiée sur de
  vraies boîtes : elle décide d'O4 à elle seule (la planche en montre
  4 marques sur 6, et son décor est favorable).
- Le passage de la **poignée de défilement** sur la rive d'O3 (barre
  native en surimpression, 0 px réservé — A44).

## O2 en situation (`o2.html`)

La planche comparative ne dit pas la seule chose qui décide d'O2 : un
tracé de 2 unités rendu à 18 px **se trouve-t-il d'un balayage** ?
`o2.mjs` rend donc la **fenêtre entière** — entête, nav 248, volet 400,
volet de lecture, barre d'état — à 1280 × 860, aux deux polarités,
avec quatorze rangées dont **six tiennent à l'écran** (mesuré : zone de
liste 667 px). Trois comptes qui alternent comme ils alternent quand le
tri est la date : le cas le plus défavorable au glyphe nu.

Ce qui s'y juge et ne se mesure pas : (1) le glyphe se trouve-t-il sans
le chercher ; (2) la nav porte ses pastilles **pleines** de 20 px et la
liste ses glyphes **nus** — le même compte se lit-il comme le même
compte ; (3) sur `--sel` et en nuit, le tracé tient-il.

Le volet de lecture y est **schématique** — assez fidèle pour donner
l'échelle, pas une proposition — et il garde la tuile aux initiales
(arbitrage D-c).

## Quinze organisations du volet (`organisation.html`)

Deuxième énoncé : quinze propositions **neuves** d'*organisation* du
volet — la première planche déplaçait une marque, celle-ci réorganise
l'ordre des rangs, le regroupement du flot, ce qui appartient à la
colonne plutôt qu'à la rangée, le bandeau et le pied. Quatre familles,
chacune rendue à 400 px dans les deux polarités.

**Règle de comparaison** : là où une organisation a besoin d'une marque
sans que ce soit son sujet, c'est le glyphe nu d'O2 qui sert — le même
partout. Décor identique à `o2.html` : 14 rangées, 3 comptes,
**12 suites**, **6 journées** — défavorable à tout ce qui regroupe, et
c'est dit là où ça compte.

### Mesuré au rendu

| | |
|---|---|
| **Deux gabarits partout** | Tous les dessins gardent exactement deux hauteurs. P1 : 87 / 114 · P3 : 67 / 94 · les autres : 88 / 115. |
| **P3, la rangée dense** | 88 → 67 px, soit **+33 % de rangées** à l'écran. |
| **P8, les voies** | **133 px par voie** à 400 px — l'aperçu a dû être abandonné dans la maquette elle-même. |
| **P11, le sommaire** | 32 px de chrome, **un tiers de rangée** — pas une rangée. |
| **P14, le pied à deux registres** | 52 px perdus (0,6 rangée) ; la grammaire des **onglets déborde de 11 px** à 400 px, celle des segments laisse 61 px de marge. |
| **P13, le bandeau segmenté** | 4 segments, **65 px de marge** — un cinquième compte déborde. |
| **P7, le peloton** | Le filet passe de la rangée à la suite : la hauteur de n rangées n'est plus n × h. Trouvé à la mesure, pas au raisonnement. |
| **Zéro rayon** | Les seuls rayons rendus dans les quinze volets : `0px` et `50%` — V14 tenu. |

Un défaut de la première génération, trouvé à la mesure et corrigé : la
règle `.sans-tete` manquait, la grille gardait « auto 1fr » — 10 px de
gouttière payés pour rien et l'aperçu placé en colonne 2 (P1 rendait
alors trois hauteurs au lieu de deux).

## « Expéditeur sur ▣ Boîte » (`ligne-expediteur.html`)

Troisième énoncé : la boîte se dit **sur la ligne de l'expéditeur, en
toutes lettres**. Sept versions — la phrase, le point médian, le glyphe
seul, le sourcil, la puce, le fer à droite, la boîte incoupable — plus
**un témoin** (la ligne d'aujourd'hui), rendues à **400 px et à 300 px**,
la borne basse du volet (`BORNES.liste`). Une version qui ne tient qu'au
défaut n'est pas livrable : la poignée existe.

Le banc est écrit **par la page elle-même** à l'ouverture (mesure du DOM
rendu) : il ne peut pas se désynchroniser du dessin.

### Ce que le banc a dit

| | bloc boîte | place au nom (400) | coupés (400) | place au nom (300) | coupés (300) | hauteur |
|---|---|---|---|---|---|---|
| Témoin | — | 304 px | 0/8 | 204 px | 0/8 | 88,4 |
| V1 la phrase | 83 px | 219 px | 0/8 | 119 px | 2/8 | 88,4 |
| V2 point médian | 69 px | 233 px | 0/8 | 133 px | 2/8 | 88,4 |
| V3 glyphe seul | 38 px | 260 px | 0/8 | 160 px | 1/8 | 88,4 |
| V4 le sourcil | 78 px | 225 px | 0/8 | 125 px | 2/8 | 88,4 |
| V5 la puce | 70 px | 232 px | 0/8 | 132 px | 2/8 | **89,7** |
| V6 fer à droite | 83 px | 219 px | 0/8 | 119 px | 2/8 | 88,4 |
| V7 incoupable | 83 px | 219 px | 0/8 | **162 px** | **0/8** | 88,4 |

- **La famille tient au défaut** : aucune des sept ne coupe un nom à
  400 px, « Bibliothèque universitaire » compris. Ce n'était pas acquis.
- **Le prix se paie à 300 px** : deux noms coupés partout, sauf V3 (un)
  et V7 (aucun).
- **V7 n'est pas une forme, c'est une mécanique** — ordre de troncature
  + repli au seuil (requête de conteneur à 360 px) : elle s'applique à
  n'importe laquelle des six autres.
- **V5 est la seule à toucher la hauteur** (+1,3 px), pour une taille de
  puce qui n'existe pas au Système (A33 fixe 24 px / marges de 12).

### Le fait que le décor cachait

Les trois comptes s'appellent ici **Travail**, **Maison**, **Études** —
six à sept caractères. D4 (PLAN-RETOURS-9) accepte un nom de compte
jusqu'à **60 caractères**. Toute cette famille suppose des **noms
courts**, et cette supposition n'était écrite nulle part. À régler avant
le dessin : nom court dédié (~12 caractères), ou repli de V7 déclenché
par la LONGUEUR du libellé et non par la largeur du volet.

## La forme retenue, en situation (`v1v7.html`)

Verdict du Chef Ingénieur sur la première mise en situation
(2026-08-24) — cinq décisions :

| | |
|---|---|
| **1** | La phrase se lit : elle évite d'avoir à se souvenir en permanence d'une couleur ou d'un logo. **Forme confirmée.** |
| **2** | Que la nav et la ligne disent la même chose n'est pas choquant ; le **glyphe doit être exactement le même**. Il l'est par construction (`reperes[account_id].icone`, une seule table). |
| **3** | Le glyphe reste : chaleur et humanité discrète ; couleur **et** forme couvrent la majorité des goûts pour une implémentation simple. |
| **4** | **Changement** : le repli au seuil (V7) est écarté. Le libellé de boîte se **tronque à l'ellipse** quand il s'approche de l'heure. |
| **5** | **Ajout** : le même schéma derrière le nom de l'expéditeur au **volet de lecture** (carte dépliée + rangées repliées). |

### La troncature — trois règles

- L'heure ne se coupe **jamais** (repère de lecture de la colonne).
- Le bloc boîte cède **trois fois plus vite** que l'expéditeur et ne
  prend **jamais plus du tiers** de la ligne.
- Les deux se terminent à l'**ellipse**, jamais à la coupe sèche.

### Pourquoi le tiers — six plafonds essayés

Sur un nom de 32 caractères (« Association des parents d'élèves »), aux
trois largeurs :

| plafond | 400 px, nom long | 300 px, nom long |
|---|---|---|
| 50 % | bloc 183 px, **2 expéditeurs coupés** | 3 coupés |
| 42 % | bloc 153 px, 1 coupé | 3 coupés |
| **33 % (le tiers)** | bloc 120 px, **0 coupé** | 3 coupés, **libellés courts intacts** |
| 30 % | bloc 110 px, 0 coupé | **7 libellés coupés** dont des courts |

33, 34, 35 et 36 % donnent le même résultat : c'est un **plateau**, pas
une valeur de justesse — d'où le tiers, qui se dit en un mot.

**Conséquence assumée** : avec ce plafond, un nom de 32 caractères reste
tronqué **même à 640 px**. La boîte est une circonstance : elle ne prend
pas le tiers d'une rangée quelle que soit la largeur.

### Le banc de la page

| cas | bloc boîte | libellé | place au nom | noms coupés |
|---|---|---|---|---|
| 400 px — noms courts | 83 px | entier | 219 px | 0 / 14 |
| 400 px — nom long | 120 px | tronqué (3/14) | 178 px | **0 / 14** |
| 300 px — nom long | 87 px | tronqué (3/14) | 116 px | 3 / 14 |
| 640 px — nom long | 199 px | tronqué (3/14) | 338 px | 0 / 14 |

### Deux constats de passage, sans rapport avec ce dessin

- **La nav tronque aussi** un nom long : 172 px offerts au libellé pour
  199 nécessaires. Règle d'aujourd'hui (`Nav.svelte`, `.libelle`) — mais
  elle dit qu'un nom long n'est **entier nulle part** dans l'écran 02.
- **Le pied déborde à 300 px** : les trois onglets demandent **334 px**
  pour 299 offerts. Le produit livré déborde déjà à sa propre borne
  basse. **À vérifier à la fenêtre réelle**, poignée tirée à fond.

### Décision 6 (2026-08-24) — le glyphe nu dans la nav

Le contenant est tranché : **la pastille pleine quitte l'écran 02**. Les
deux surfaces portent le même objet — le tracé du repère, à la teinte du
compte, sans contenant. Seule la taille suit son contexte : **16 px** dans
la nav (celle des glyphes de dossier, dans la même colonne) et **14 px**
dans la ligne (celle de son texte).

Vérifié au rendu : même tracé (`M3 8h18v11H3z`), même couleur
(`rgb(10,90,143)`), zéro `.repere` dans les fenêtres, et les glyphes de
compte alignés sur les 16 px des dossiers.

- **Gagné** : les rangées de comptes cessent d'être des rangées à part —
  la colonne entière se lit d'un seul rythme, et le mot du volet retrouve
  exactement ce que la nav montre.
- **Perdu** : le fond coloré donnait une présence à distance ; un tracé
  de 2 unités à 16 px pèse 1,3 px. La nav dit le compte plus doucement —
  à constater à la fenêtre réelle.
- **Conséquence de Système, à écrire** : la phrase de V4/V14 « reste un
  seul autre rond dans tout le système, la pastille de repère » tombe.
  **Mesuré sur la fenêtre : la seule forme ronde restante est `.disque`**
  (non-lu de rangée, barre d'état) — dans l'écran de tous les jours, le
  disque ne dit plus QUE l'état. La pastille ne meurt pas : elle reste
  aux Réglages (`Reglages.svelte:460` et le nuancier de choix), où elle
  est une pastille de **choix**, pas une marque d'identité.
- **Contrastes** : rien ne bouge. La teinte tracée sur `bg` / `hover` /
  `sel` / `tuile` est déjà mesurée au seuil composant — pire cas du
  nuancier **4,97:1**. Les 24 mesures « glyphe sur pastille » perdent
  leur objet dans la nav et le gardent aux Réglages.
