# Spike — la direction « Elements » appliquée à l'interface de Wind

**Jetable.** Hors du workspace de production (STANDARD §2.2) : rien dans
`apps/desktop/ui-v2/` n'a été touché. Le spike sert à **juger une
direction artistique sur pièce**, pas à préparer une migration.

| | |
|---|---|
| Le prototype | `index.html` — l'interface (aucun build, aucun réseau) |
| La planche | `planche.html` — les 78 icônes, générée par `node planche.mjs` |
| La confrontation | `comparaison.html` — Elements **contre** le Système, générée par `node comparaison.mjs` |
| Le jeu | `jeu.mjs` — le catalogue des 78 glyphes, **la source** |
| Bancs | `node contraste.mjs` — **74 mesures, 0 échec**<br>`node chiffrage.mjs --tout` — le poste F<br>`node controle-sprite.mjs` — le sprite du prototype contre le catalogue |
| Source de la direction | `~/Downloads/elements-icones-jeu-complet-disques_2.html` (« Trois paliers, zéro arbitrage ») |
| Décor | fictif, 22 messages, 2 comptes, 6 dossiers — la forme servie par le cœur (compte, boîte, fil, pièces, invitation) |

Le prototype est **interactif** : dossiers, filtre de compte, onglets,
recherche vivante (elle traverse comptes et dossiers, comme au produit),
sélection au clic et aux flèches ↑/↓, lecture qui éteint le non-lu,
repli/dépliage des messages du fil, composeur, cycle de synchronisation.
La pilule en bas à droite est **hors produit** : clair / sombre, et
**Repères**, qui montre les cotes comme le document montre les siennes.

---

## 1. Ce que la direction dit, et ce que j'en ai fait

Le document ne donne pas un style, il donne **une doctrine** : une seule
règle de forme appliquée partout, des marqueurs posés sur le centre
géométrique de leur contenant, aucune correction optique, et *une seule
distance décidée dans tout le système — et elle a une raison*. C'est
transposable ligne à ligne.

| Le document | L'interface |
|---|---|
| Le disque marque **l'élément** dans un dessin achromatique | Le disque marque **ce qui est vivant** : teal = état du système (non-lu, cycle en cours), teinte de compte = identité |
| Un seul rayon, fixé par la cellule de River | **Ø 9 px partout** — non-lu, compte, indicateur de statut. Aucune autre valeur |
| Marqueur sur le centre géométrique | Le disque de non-lu est centré sur **toute** la rangée — mesuré à 0,00 px d'écart, rangée à puces ou non |
| Wind = la seule forme **orientée**, demi-disque tangent au bord intérieur haut | Le glyphe Wind reste la marque (entête, écran vide, bouton « Écrire ») avec son arc `r 3.25` **verbatim** |
| La tuile `#F2EDE3` est le sol du jeu d'icônes | `--tuile` devient le sol des objets Wind : boîte en cours, rangée épinglée, tuile d'initiales, tuile de date d'invitation |
| Structure `#141414`, trait 2 u, bouts nets, jonctions vives | **les 78 icônes** de l'inventaire redessinées dans cette grammaire (§6), **achromatiques** — dans l'application l'élément est acquis, la couleur redevient un état |
| Sourcil 11,5 px / .2em / capitales, titre graisse 340 / -.03em | Sourcils sur les sections (« Épinglés », « Boîtes », « Conversation · 4 messages ») ; titres de boîte et de fil en graisse 340 |
| Panneaux à rayon 18, fond carte | **Non repris.** Voir §4-A |

### Les deux endroits où j'ai tranché plutôt que copié

**Le rond est rendu au disque.** L'avatar de Wind est un cercle
d'initiales — c'est le plus gros rond de l'écran, et il vole au disque son
unicité de sens. Il devient un **carré** (rayon 2 px, le sol `--tuile`,
l'encre `--tuileInk`). Le disque n'a plus qu'un emploi dans tout le
système ; c'est A3 (« une icône, un sens ») dit dans la langue du
document.

**La pastille de non-lus devient un nombre.** Elle portait la même
information que le disque, en plus fort, dans un second dessin. Un nombre
aligné en chiffres tabulaires suffit — et la pastille pleine
`onAccent/accent` disparaît de la nav.

---

## 2. Ce qui a été mesuré

Le banc du spike (`contraste.mjs`) est **le banc expédié** : mêmes
formules WCAG, même table de paires que `e2e/contraste.mjs` — les paires
réellement posées par ui-v2 — plus les rôles propres à la direction.

### La palette du document ne passe pas la gate telle quelle

Deux valeurs, corrigées du **minimum**, à teinte constante (le remède A8) :

| Rôle | Document | Mesure | Corrigé | Mesure |
|---|---|---|---|---|
| `--mut` texte atténué | `#6E7577` | **4,19:1** sur la page ✗ | `#606668` | 5,21:1 ✓ |
| `--line` filet | `#E3E3DD` | **1,15:1** sur la page | `#CBC8BB` | 1,50:1 |

Le seuil du filet n'est pas inventé : c'est **le filet que Wind expédie
déjà** (`#cdc6b8` sur `#f2f0ea` = 1,49:1). Dans un document aéré, 1,15:1
tient ; dans une liste où le filet est le **seul** séparateur de rangées
(A29 : « lignes continues séparées au filet, sans carte ni ombre »), il
disparaît.

### Le teal de Wind est un composant, pas une encre

`#1F8A8A` vaut **3,70:1** sur la page : conforme pour un disque, un filet,
un anneau de focus (seuil 3) — **non conforme pour du texte** (seuil 4,5).
Il est donc gardé **exact** comme `--marque`, et dédoublé d'une encre
`--accent #1A7A7A` (même teinte, luminosité minimale) qui tient 4,56:1.

### La palette de la suite ne survit pas au double thème

Mesure des six teintes en disque nu, sur les cinq fonds de rangée :

| | clair | nuit |
|---|---|---|
| Wind `#1F8A8A` | 3,33 ✓ | 3,26 ✓ |
| Stone `#B0703C` | 3,23 ✓ | 3,37 ✓ |
| River `#2153A0` | 6,01 ✓ | **1,81 ✗** |
| Flame `#D8332A` | 3,81 ✓ | **2,85 ✗** |
| Moon `#6C4E9C` | 5,26 ✓ | **2,07 ✗** |
| Helios `#E0AE1C` | **1,65 ✗** | 6,60 ✓ |

**Helios ne tient pas sur fond clair** : dans son icône, c'est le
`#141414` qui l'entoure qui lui fabrique son contraste ; posé nu sur du
papier, il s'éteint. Et aucune teinte ne tient dans les deux polarités
hors Wind et Stone — ce qui **re-dérive exactement A74** (« aucune teinte
unique ne tient 3:1 sur les fonds clairs ET sur les fonds -nuit — chaque
famille vit donc en DEUX déclinaisons »). Le prototype applique donc A74
sans le modifier : les déclinaisons claires d'A74 (`#DCAB7C`, `#72BDF0`)
passent toutes sur les fonds -nuit du spike (≥ 6,2:1).

### Un défaut trouvé en mesurant, pas en regardant

Le disque de non-lu était **14 px au-dessus** du centre géométrique sur
les rangées porteuses de puces : la colonne du marqueur enjambait trois
rangs de grille sur quatre. Corrigé en passant la rangée en flex — le
marqueur est centré **par construction**, jamais réglé à l'œil. Écart
re-mesuré : **0,00 px**, avec ou sans puces. (Le correctif intermédiaire
`grid-row:1/-1` ne veut rien dire sans rangs explicites : il a *empiré*
l'écart à 35 px. Mesuré, pas supposé.)

---

## 3. Ce qui n'a **pas** bougé

Une variable à la fois, sinon la comparaison ne vaut rien.

- **La géométrie livrée** : entête 52, colonnes 248 / 400 / 1fr, bandeau
  52, onglets 52, barre de statut 36. Vérifié au banc.
- **Le modèle** : six dossiers canoniques, « Boîtes », les trois onglets,
  les épinglés en tête du même défilement, les puces d'inventaire.
- **Les mots** : repris du catalogue français expédié.
- **Le comportement** : lire éteint le non-lu ; le disque de compte
  n'apparaît que là où les comptes se **mélangent** (boîte unifiée,
  recherche) — la règle A74.

---

## 4. Ce que le Chef Ingénieur doit arbitrer

La direction est cohérente. Elle est aussi **coûteuse**, et elle retire
des choses qui ont été payées. Rien de tout cela ne se décide ici.

**A — Le cadre plat, ou les panneaux à rayon 18 ?** Le document est fait
de cartes bordées à rayon 18. Wind est délibérément plat (A29/A30 : ni
carte ni ombre dans la liste). J'ai gardé le plat, et la carte ne revient
que pour un **message** — c'est un objet, il est sur du papier. Choix
assumé : à l'écran d'un client courrier dense, la carte partout redevient
une page web. **À confirmer.**

**B — Le trait hitofude meurt.** A28, A36, A40, A52 : la signature
calligraphique de Wind n'a aucune place dans un système entièrement
construit. Remplacée par la **paire disque / anneau** — pleine au repos,
évidée et tournante pendant un cycle. C'est fidèle au document (« le
disque plein (…) désigne un objectif ») et c'est **une perte** : le trait
a coûté plusieurs chantiers, dont la découverte A40 (SMIL dans le
`<mask>`). Décision de marque, pas d'ingénierie.

**C — Le disque de compte perd son glyphe, et c'est le point dur.** Le
jeu d'icônes ne met **jamais rien dans un disque**, et la mesure confirme
qu'il ne le pourrait pas : aucune des six teintes ne porte un glyphe à
4,5:1 dans les deux polarités. Mais A74 met un glyphe dans la pastille
**précisément** pour que le compte ne soit pas dit par la couleur seule
(A8, WCAG 1.4.1). Appliquée à la lettre, la direction **régresse
l'accessibilité**. Le prototype montre la version doctrinale — disque nu,
identité portée par `aria-label` et `title` — pour que le coût soit
visible. Trois issues : garder le glyphe (et renoncer à la pureté du
disque), porter l'identité ailleurs dans la rangée en texte, ou assumer.
**Je recommande de ne pas assumer.**

**D — `--panel` disparaît.** Le document a deux sols (page, carte) plus la
tuile. Clarity en a trois (`bg`, `panel`, `surface`). Le spike fond
`panel` dans `bg` : nav, entête et barre de statut cessent d'être en
retrait, les filets font tout le travail. C'est plus simple et plus
« papier » — mais c'est **un jeton de moins dans 28 thèmes**.

**E — Et les 28 thèmes ?** La direction est **une** palette, pas quatorze
Wada et leurs nuits (A42). Le chemin le moins cher : Elements devient le
thème **par défaut** (clair + nuit), la table Wada reste offerte. Tuer la
table est une décision séparée, et plus grosse.

**F — Le sous-ensemble Material Symbols.** Le jeu entier est désormais
dessiné et chiffré — **voir §6**, qui remplace ce qui n'était ici qu'une
inquiétude.

---

---

## 6. Poste F, chiffré

Les **78 glyphes de l'inventaire** sont dessinés dans la grammaire du
document — `jeu.mjs` est la source, [`planche.html`](planche.html) les
montre, `chiffrage.mjs` les compte. La planche est **générée** depuis le
catalogue : elle ne peut pas montrer autre chose que ce qui est mesuré.

```bash
node spikes/direction-elements/chiffrage.mjs --tout
node spikes/direction-elements/planche.mjs
```

### Périmètre, relu à la source à chaque exécution

78 glyphes à l'inventaire, dont **4 réservés** — `open_in_new`, `link`,
`format_quote`, `storage`. Un balayage indépendant des sources ui-v2
retrouve exactement ces quatre-là : le README des icônes dit vrai.
Restent **74 à produire**.

### Ce que la grammaire absorbe, et ce qu'elle refuse

| | | |
|---|---:|---|
| **direct** — aucun arbitrage | **38** (49 %) | la grammaire suffit |
| **arbitrage** — une décision à valider | **26** (33 %) | une courbe, une diagonale, une réduction |
| **dur** — la grammaire ne le porte pas | **14** (18 %) | dessinés, mais c'est un report |

Les quatorze durs : `schedule_send`, `person_add`, `group_add`,
`keyboard`, `format_list_numbered`, et les neuf pictogrammes
`account_balance`, `eco`, `favorite`, `flight`, `pets`, `school`,
`sports_esports`, `star`, `volunteer_activism`. (`music_note` en est
sorti au premier tour de retours — voir §6 ter.)

Écart mesuré à la grammaire : **22 glyphes sur 78 emploient au moins un
arc** (28 %) — quand le document n'en emploie que dans le rabat de Wind.
**95 % sont en coordonnées entières.** 1 350 nœuds, 229 sous-chemins.

### Le coût réel n'est pas le dessin — c'est le palier

C'est le résultat qui compte, et il est contre-intuitif. Le document
impose trois paliers ; **les tailles d'emploi de Wind n'en atteignent
qu'un seul.** Un trait de 2 unités sur une grille de 24, rendu à P px,
mesure 2 ÷ 24 × P :

| Rendu | Trait | Où |
|---:|---:|---|
| 10 px | **0,83 px** — sous le pixel | repère de compte, rangée |
| 12 px | 1,00 px | repère de compte, nav |
| 14 px | 1,17 px | puces, barre d'état |
| **16 px** | **1,33 px** | `.ms` — le défaut, partout ailleurs |
| 18 px | 1,50 px | barres d'actions |

**Aucune icône de Wind n'atteint 21 px.** Le palier 24 et le palier maître
ne servent que la marque et l'écran vide : tout le reste tombe dans le
**palier 16**, qui se cale à la main, rectangle par rectangle. Et le
maître ne s'y met pas à l'échelle — **37 % seulement de ses coordonnées
survivent** au passage 24 → 16 (il faut être multiple de 3 pour tomber
juste). Les 63 % restants atterrissent sur des tiers de pixel : ce n'est
pas une réduction, c'est un second dessin.

Les douze repères de compte sont pires : rendus à 10-12 px, ils passent
**sous le palier 16 lui-même**. Il leur faudrait un quatrième palier, que
le document n'a pas.

### Le chiffre

| Branche | À produire | Dessins | Faits |
|---|---|---:|---:|
| **Disque nu** (§4-C tranché pour la doctrine) | maîtres 24 | 62 | 62 |
| | paliers 16, calés à la main | 62 | 0 |
| | **total** | **124** | **62 · 50 %** |
| **Le glyphe reste** (A74 conservé) | maîtres 24 | 74 | 74 |
| | paliers 16 | 74 | 0 |
| | palier 10-12, à inventer | 12 | 0 |
| | **total** | **160** | **74 · 46 %** |

**Le poste F vaut 124 dessins si la décision C tranche pour le disque nu,
160 si le glyphe de compte reste.** La moitié est faite : les maîtres.
L'autre moitié — les paliers 16 — n'est pas commencée, et c'est celle qui
ne se délègue pas à une mise à l'échelle.

### Trois fusions que la grammaire force

`archive` = `inventory_2` · `download` = `system_update_alt` ·
`check_circle` = `cancel` = `error` = `info`.

Réduits à la grammaire, ces glyphes retombent sur le même dessin. Les
garder distincts demande d'ajouter du détail — donc de sortir de la
grammaire. **Huit entrées du jeu Material pour trois dessins** : ce sont
trois décisions à prendre, pas trois défauts à corriger. (Il y en avait
quatre : `report` repassé en octogone au premier tour de retours a rendu
son triangle à `warning`.)

### Deux choses que le dessin a apprises

**`signature` est le glyphe impossible.** Son dessin d'origine est une
trace calligraphique — exactement ce que cette direction retire au trait
hitofude (§4-B). Le bannir d'un côté et le garder de l'autre ne tient pas.
Rendu ici en trace brisée, ce qui est un compromis, pas une réponse.

**`pets` porte quatre disques.** Quatre coussinets pleins, plus une paume
devenue contour. Dans un système où le disque est réservé à l'état, c'est
**incompatible, pas seulement coûteux** — et c'est un argument de plus pour la branche
« disque nu », qui le fait disparaître avec les onze autres.

---

## 6 ter. Premier tour de retours CE sur le dessin

15 retours, tous appliqués dans `jeu.mjs` — donc repris d'office par la
planche, la comparaison et le chiffrage, qui la lisent. Les six chemins
que le prototype partage avec le catalogue ont été propagés dans son
sprite : `index.html` et `jeu.mjs` ne divergent pas.

| Glyphe | Retour | Ce qui a été fait |
|---|---|---|
| `attach_file` | sans traits collés, essayer un seul trait | UN trait, montants écartés de 4 à **10 u**. Trois arcs → un ; devient 100 % entier |
| `delete` | des traits dans le corps | deux verticales dans la cuve |
| `drafts` | losange plutôt que triangle | le rabat se ferme en losange |
| `error` | prendre `info` et le retourner | symétrie stricte autour de y = 12 : une seule cote pour les deux |
| `format_clear` | rouleau à peinture barré | rouleau + bras + manche, barrés |
| `format_list_numbered` | pas de diagonale pour le 2 | le 2 descend par une verticale — les trois chiffres sont orthogonaux |
| `mark_email_unread` | rabat fermé sur l'enveloppe | rabat ajouté, disque teal conservé |
| `music_note` | note seule, disque plein bas, trait diagonal haut | une tête au lieu de deux, **zéro arc**, tête tangente à la hampe |
| `pets` | espacement identique entre les 4 disques | **6 u exactement** sur les trois intervalles |
| `send` | double triangle du Système, traits Elements | un seul chemin fermé, 4 sommets entiers, l'encoche fait le 2ᵉ triangle |
| `signature` | X à gauche, brisures moins hautes | X du Système, amplitude 10 u → 4 u |
| `unfold_less` | espace entre les deux pointes | 2 u → **4 u**, la cote de `unfold_more` |
| `report` | garder un octogone | octogone |
| `sports_esports` | deux carrés de même taille | 2 × 2 u chacun, symétriques autour de x = 12 |
| `schedule_send` | triangle plein rappelant `send` | même silhouette à l'encoche ; 4 sous-chemins → 2 |

### Ce que les retours ont déplacé dans le chiffrage

- **`report` en octogone tue la fusion « triangle ».** `report` et
  `warning` ne retombent plus sur le même dessin : **4 familles de fusion
  → 3**. Un arbitrage de moins à prendre.
- **`music_note` sort des « durs »** : une tête au lieu de deux, plus
  d'arc. Les durs passent de **15 à 14**.
- **`format_clear` passe de « direct » à « arbitrage »** : le rouleau
  coûte plus cher que le T barré. C'est un choix, pas une régression.
- Bilan : direct 39 → **38**, arbitrage 24 → **26**, dur 15 → **14**.
  Glyphes à arc 23 → **22**. Le total de dessins du poste F ne bouge pas
  (124 / 160) : les retours changent la **qualité** du maître, pas le
  nombre de paliers à caler.

### Deuxième tour de retours CE — 7 glyphes

Trois retours renvoyaient au glyphe du Système. Je suis allé le **regarder**
plutôt que de le supposer : rendu en grand depuis la fonte vendorisée, puis
redessiné à côté. C'est ce qui a permis de voir que `drafts` n'a pas de
rabat rapporté (c'est un pentagone) et que les coussinets de `pets` sont
en arc, pas en rangée.

| Glyphe | Retour | Ce qui a été fait |
|---|---|---|
| `attach_file` | rends son repli au trombone | repli rendu. Les quatre montants sont à 7, 11, 15, 19 : **trois intervalles de 4 u**. La version d'origine avait 4 u puis 2 u — c'était la paire à 2 u qui se collait, pas le repli |
| `drafts` | semblable au Système, traits Elements | pentagone du Système, un seul chemin fermé, pic abaissé de 6 à 4 u |
| `format_clear` | la barre monte plus haut | de coin à coin (2,2 → 22,22) : une barre qui commence sur ce qu'elle barre ne barre rien |
| `format_list_numbered` | illisible, recommence | chiffres de 4 → **6 u de haut**, 3 → 5 de large ; lignes reculées pour leur céder la place ; chacun centré sur SA ligne |
| `forum` | ombre d'un deuxième message | seule la part **visible** de la bulle arrière est tracée — deux traits ne se superposent jamais |
| `pets` | disposition du Système | arc du Système : intérieurs relevés, extérieurs écartés, symétriques autour de x = 12 |
| `signature` | pas harmonieux, autre proposition | proposition neuve : x, deux caractères angulaires, ligne **pointillée** (3 tirets de 4 u au pas de 7) |

Deux ont demandé un second essai avant de passer : `drafts` se lisait comme
une **maison**, et les caractères de `signature` étaient trop pleins.

### Deux réserves de ce tour

**`drafts` se lisait comme une maison** — réserve **levée au 3ᵉ tour**. Ce
n'était pas la proportion, c'était le rabat incomplet : redoublé en losange,
la silhouette de toit disparaît. Voir ci-dessous.

**`format_list_numbered` reste « dur », et c'est le verdict honnête.**
Le redessin lui fait gagner 2,7 → 4,0 px de haut par chiffre à 16 px. Trois
chiffres dans une colonne de 5 unités ne seront jamais lisibles à cette
taille, quelle que soit la grammaire. C'est le glyphe qui plaide le plus
fort pour le palier 16 dessiné à la main.

`pets` a fait perdre l'espacement identique obtenu au tour précédent :
quatre centres entiers sur un arc ne peuvent pas tenir trois cordes égales.
Il fallait choisir entre la cote et la ressemblance — la ressemblance a été
choisie, et c'est tracé.

### Troisième tour de retours CE — 3 glyphes

| Glyphe | Retour | Ce qui a été fait |
|---|---|---|
| `drafts` | le même triangle en symétrie pour former un losange | le rabat est redoublé par symétrie autour de la ligne d'épaules (y = 9) — **aucune cote neuve** : la moitié basse est la moitié haute retournée |
| `pets` | écarte les deux disques du haut | 4 → **6 u** de centre à centre ; à 4, avec r = 2, ils étaient exactement **tangents** et se soudaient en une masse. Les extérieurs suivent (4 et 20) : cordes 6,4 / 6,0 / 6,4, plus aucune paire ne se touche |
| `sports_esports` | carrés plus petits, séparés des bords | les carrés passent de **tracés à pleins** : un carré de 2 u tracé au trait de 2 u rendait un pavé de 4 u collé au boîtier ; plein, il fait vraiment 2 u et garde ~1 u de dégagement sur les quatre bords |

Les trois retours ont en commun d'avoir désigné un **symptôme dont la cause
était une autre**. Le losange de `drafts` ne corrige pas une proportion, il
complète un rabat — et c'est ce qui tue le « toit ». L'écart de `pets` ne
tenait pas à un réglage mais à une **tangence exacte** que r = 2 et un pas
de 4 rendaient inévitable. Les carrés de `sports_esports` n'étaient pas trop
grands : ils étaient **tracés**, et un contour de 2 u double toujours la
taille apparente d'une forme de 2 u.

### Le sprite du prototype avait dérivé, deux fois

`index.html` porte son propre sprite, et il s'était désynchronisé du
catalogue à chaque tour de retours — en silence, parce qu'un glyphe périmé
s'affiche parfaitement. Corrigé à la racine : `controle-sprite.mjs` compare
les 22 symboles partagés au catalogue, et `--corriger` les **réécrit**
depuis `jeu.mjs`. Il a trouvé quatre écarts que je n'avais pas vus
(`edit_note`, `archive`, `search`, `check_circle` — dont deux dataient de
la rédaction initiale, pas des retours).

```bash
node spikes/direction-elements/controle-sprite.mjs
```

### Deux réserves du premier tour

**`attach_file` se lisait comme un U** — réserve **levée au 2ᵉ tour** : le
repli lui a été rendu, avec trois intervalles de 4 u au lieu de la paire à
2 u qui collait à l'origine.

**`signature` posait six unités au-dessus de sa ligne** au premier essai —
une signature qui ne touche pas sa ligne n'est pas une signature. Corrigé
avant livraison : le X et la trace posent tous deux à y = 16, ligne à 20.

---

## 6 bis. Le DC et le produit ont divergé — trouvé en comparant

[`comparaison.html`](comparaison.html) pose les deux jeux côte à côte : à
gauche le glyphe que `docs/design/systeme.dc.html` dessine, rendu avec la
**fonte vendorisée réellement expédiée** (embarquée en base64 — la page ne
demande rien au réseau, là où le DC tire Material Symbols du CDN Google
que l'application s'interdit) ; à droite le redessin Elements.

Croiser les inventaires a sorti un écart **qui ne doit rien à cette
direction** :

| | |
|---:|---|
| **72** | glyphes dessinés par le DC |
| **78** | glyphes dans la fonte vendorisée |
| **74** | employés par ui-v2 |

**6 glyphes sont livrés mais ne sont dessinés nulle part dans le DC** :
`error`, `link_off`, `menu`, `person_add`, `system_update_alt`,
`volunteer_activism`. Quatre d'entre eux — `error`, `link_off`,
`system_update_alt`, `volunteer_activism` — sont **exactement** les
« avis RARES de la fente » que `assets/icones/README.md` dit avoir
découverts absents de la police au terrain 0.1.4. Ils ont été ajoutés à la
fonte ; personne ne les a ajoutés au dessin. Ils ne se voient pas, parce
qu'ils s'affichent correctement.

**4 glyphes sont dessinés par le DC mais plus employés par le code** : les
quatre « réservés » — `open_in_new` (A53), `storage` (A60), `link` et
`format_quote` (A62-D1). La fonte les garde volontairement ; le DC dessine
encore les commandes qui les portaient.

A18 dit : « ce document est la source unique : ce qu'il dessine est livré,
ce qui est livré s'y dessine. » **Ce n'est plus vrai sur 10 glyphes.**
Cela se corrige dans le DC, pas dans un spike — et cela ne dépend
d'aucune décision sur Elements.

Le seul des trois contrôles qui passe : **aucun glyphe du DC ne manque à
la fonte**, donc rien ne s'afficherait en toutes lettres sur un poste.

*Vérification du banc lui-même* : la page porte un témoin de chargement.
Mesuré au rendu — la ligature `account_balance` posée à 16 px occupe
exactement 16 px de large (1 em) ; en toutes lettres elle en ferait une
centaine. C'est le contrôle objectif de `apercu.html`, rejoué ici.

---

## 7. Verdict du spike

La direction **tient** sur un client courrier : elle a de quoi dire le
non-lu, la sélection, l'identité de compte, l'épinglé, l'invitation, le
cycle de synchronisation — sans inventer une seule forme hors de sa
grammaire, et sans une seule correction optique. Elle passe la gate de
contraste à deux valeurs près, corrigées sans la dénaturer. La moitié de
son jeu d'icônes (39 glyphes sur 78) se dessine sans le moindre arbitrage.

Elle coûte : le trait hitofude, **124 à 160 dessins d'icônes** dont la
moitié reste à faire, un jeton de moins dans 28 thèmes, et un arbitrage
d'accessibilité réel (§4-C). Un premier tour de retours CE sur le dessin
(§6 ter) a amélioré 15 maîtres sans déplacer ce total : les retours
portent sur la qualité du maître, pas sur le nombre de paliers.

Ce que je ne recommande pas : adopter par morceaux. Deux grammaires
d'icônes qui cohabitent se voient à la première barre d'outils — et le
palier 16 n'étant pas commencé, un demi-passage livrerait des icônes
floues à la taille où Wind les affiche le plus.

Prochain pas, **si** le CE veut aller plus loin : trancher la décision C,
qui fixe le périmètre (124 ou 160), avant de caler le moindre palier 16.
