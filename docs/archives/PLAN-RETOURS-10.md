# PLAN-RETOURS-10 — quatre retours CE (sélection multiple, marque)

> **CHANTIER SOLDÉ le 2026-08-27 — terrain complet.** Ouvert et clos
> le même jour. GO CE du plan (D1-D5) le 2026-08-27 au matin ; terrain
> en DEUX passes le jour même — première passe : R2/R3 OK + 8 constats
> (D6-D8 posées en route, tous corrigés dans la session) ; seconde
> passe : **« Terrain OK — tout passe »**. Commit `a72f341`, CI verte
> (run 33111561147). Dette : **D-41** (coche clavier). **LIVRÉ en
> 0.11.0, PUBLIÉE le 2026-08-27** (MINEUR, D5 — `d0f9c8c`, vérifiée
> §2.10 18/18, auto-update prouvé sur les DEUX postes le jour même :
> « release ok, auto update ok sur les 2 postes »).
>
> Chiffres kaizen : 2 gates complètes jouées (2,2 puis 2,1 min) + le
> pre-push ; 8 constats KO à la première passe terrain, 0 à la
> seconde ; revue à regard neuf 8 angles / 10 trouvailles / 9 corrigées
> avant terrain ; e2e 137 → 148.

## 1. Constat — instruction sur pièces (2026-08-27)

Quatre retours du Chef Ingénieur, instruits sur le code et le Système.

### R1 — Sélection multiple de messages

Demande : shift-clic, Ctrl-clic, et un sélecteur de cases à cocher.

Faits établis :

- La sélection est strictement **singulière** :
  `let selection = $state(null)` (`Liste.svelte:94`), clé
  `account_id/mailbox/uid`, `choisir(l)` écrase, `estChoisie(l)` compare
  par égalité stricte. Aucun `Set`, aucun tableau.
- La rangée est un `<div class="ligne" role="button">` sans case à
  cocher, sans `aria-selected`/`aria-multiselectable`
  (`Liste.svelte:860-877`).
- Les actions vivent dans `App.svelte`, **toutes unitaires** :
  `archiver(ligne)` (1210), `supprimer(cible)` (1236),
  `signalerSpam(ligne)` (1265) ; raccourcis `e`/`Delete`/`r`/`f`
  conditionnés à `if (selectionnee)` (1033-1048). Aucune action groupée,
  aucune barre d'actions groupées.
- Le clic simple **ouvre** le fil et marque lu (`surSelection`,
  `App.svelte:1198-1206`) — la sélection multiple doit cohabiter avec ce
  geste sans le casser.
- e2e concernés : `refonte-ecran02.spec.js` (sélection, archivage,
  retour intact).

C'est le seul des quatre retours qui soit une **capacité nouvelle**
(MINEUR §2.9). Aucune ancre du Système ne couvre la sélection multiple —
amendement A-n à créer.

### R2 — L'icône de l'application Windows est l'ancienne marque

Demande : mettre la nouvelle icône Wind comme icône d'application.

Faits établis :

- `tauri.conf.json:26-28` pointe `icons/icon.ico`, généré par
  `scripts/make-icon.ps1` depuis la géométrie **« W-pastille »**
  (enveloppe à coins arrondis `#e2ebe8`/`#365a4f` + pastille « W »),
  soit la marque d'AVANT Elements.
- La marque actuelle (V1/V11, PLAN-ELEMENTS, CE 2026-08-24) est
  l'enveloppe à coins vifs + rabat demi-disque teal, régime **tuile**
  figé hors thèmes : fond `#F2EDE3`, structure `#141414` trait 2,3,
  rabat `#1F8A8A`, rayon de plateforme 15/64 — `Marque.svelte`,
  `MARQUE` dans `lib/icones.js:101`, Système § Marque (l. 618).
- `git log 84d46ea..HEAD -- apps/desktop/icons assets/marque
  scripts/make-icon.ps1` est **vide** : l'icône Windows (barre des
  tâches, exécutable, installeur) est désynchronisée du Système depuis
  le 2026-08-24. Le commit `211a591` l'annonçait déjà comme reste.

Correctif : réécrire le rendu GDI+ de `make-icon.ps1` sur la géométrie
Elements (tuile + enveloppe + rabat), régénérer `icon.ico`
(256/48/32/16), mettre `assets/marque/*.svg` d'équerre, PNG d'aperçu
pour verdict CE. Les tailles 32/16 demandent un arbitrage de lisibilité
(trait plancher ; le rabat à 16 px est à prouver à l'aperçu).

### R3 — La marque d'entête est petite

Demande : augmenter la taille de l'icône Wind en haut à gauche.

Faits établis : `App.svelte:1369` (et 1479, écran 03) rend
`<Marque taille={20} />` dans l'entête de 52 px, à côté de « Wind » en
18 px/600. Le régime glyphe suit `currentColor`. Fiche V11 du Système
(l. 667-684) décrit ce rendu 20 px — elle devra être amendée avec la
nouvelle cote.

### R4 — Les glyphes de la nav ne s'alignent pas sur les lettres

Demande : aligner le **bas des glyphes** avec le **bas des lettres**
(la baseline — sans compter les descentes de p/q).

Faits établis :

- Les rangées de la nav sont en `display:flex; align-items:center`
  (`Nav.svelte:117-121, 145-150`) : le glyphe 16 px est **centré** sur
  la boîte de ligne du libellé 14 px (13 px sur la tuile), pas posé sur
  sa baseline. Un SVG de 16 px centré sur du texte de 14 px déborde
  sous la baseline d'environ 2-3 px — c'est le décalage que l'œil du CE
  a vu.
- Trois porteurs à traiter identiquement : `.icone` (dossiers),
  `.repere-nu` (repère de compte, A82 — « la nav et la ligne portent
  exactement le même objet »), `.icone-tuile` (boîte en cours).
- Mécanique retenue à instruire : poser le conteneur du glyphe en
  alignement **baseline** (un élément remplacé inline assoit son bord
  bas sur la baseline du texte — exactement la demande), plutôt qu'un
  nudge en pixels par glyphe. À prouver au STOP visuel : les tracés ont
  ~2 px de marge interne dans le viewBox 24, le rendu optique tranchera.

## 2. Périmètre

- R1 : sélection multiple **dans la liste de l'écran 02** (shift-clic,
  Ctrl-clic, cases à cocher), barre d'actions groupées, actions du
  périmètre D1, exécution séquentielle des commandes cœur existantes.
- R2 : `make-icon.ps1` réécrit sur Elements, `icon.ico` régénéré,
  `assets/marque/` d'équerre.
- R3 : la cote de la marque d'entête (les deux emplois).
- R4 : l'alignement baseline des glyphes de la nav (trois porteurs).

## 3. Refus de périmètre (§2.6)

- **Pas de nouvelles commandes cœur groupées** (R1) : les actions
  groupées appellent les commandes unitaires existantes
  (`archive_message`, `delete_message`, `report_spam`) en séquence,
  avec un seul toast récapitulatif. Une commande SQL groupée est une
  optimisation sans preuve de besoin — la sélection courante se compte
  en dizaines, pas en milliers.
- **Pas de « Sélectionner tout le dossier »** (R1) : la sélection vit
  dans les rangées chargées et visibles du fenêtrage. « Tout un dossier
  de 200 k messages » est un autre chantier, avec d'autres gardes.
- **Pas de drag-sélection** (rubber band) ni de gestes tactiles.
- **Pas d'alignement baseline généralisé** (R4) : la demande vise le
  volet de gauche ; les autres surfaces (barre du fil, Réglages…) ne
  bougent pas sans constat.
- **Pas de refonte des SVG de `assets/marque/`** au-delà de la mise en
  cohérence Elements (R2) — pas de déclinaisons neuves.

## 4. Étapes

État au 2026-08-27, fin de journée : **tout est livré et validé au
terrain.** E1 (STOP visuel CE : « GO — conforme » sur les aperçus
256/48/32/16) ; E2/E3/E4 livrées, STOP visuel groupé « GO — conforme ».
Filet : **9 e2e neufs** (`selection-multiple.spec.js`), 1 e2e de calage
nav (`refonte-ecran02`), RED prouvé avant implémentation sur les deux
fronts. Glyphe `check` neuf (78 → 79, grille et comptes du Système
amendés, A86-A88).

**Revue à regard neuf** (8 angles) : 10 trouvailles rapportées — 9
corrigées avant le terrain (fermeture du fil sur échec, échos en succès
de façade, régression de focus A38 au Ctrl-clic, barre active pendant
un lot en vol, sélection fantôme après geste unitaire, non-lu médian
d'un fil, sémantique de fil → **D6**, diagnostic spam perdu,
conventions), 1 renversée ensuite par le terrain (teinte des épinglées
cochées, R1-7). Une trouvaille majeure d'un angle a été **réfutée** sur
pièces (recharger() ressert bien une recherche active).

**Terrain, deux passes le 2026-08-27** : première passe — R2/R3 OK,
8 constats (R4 recalage optique, R1-1 focus du Ctrl-clic, R1-2 ancre du
Shift = sélection, R1-3 gouttière de la case, R1-7 teinte des
épinglées, R1-8 raccourcis sur le lot), **tous corrigés le jour même**
(D6-D8 posées en route, planche de 3 calages pour R4) ; seconde passe :
**« Terrain OK — tout passe »**. Gate complète finale VERTE en 2,1 min,
e2e 137 → **148**.

Écarts au plan, dits : la sélection multiple ne pose pas
`aria-multiselectable` (les rangées sont des `role="button"`, pas des
options de listbox — l'état vit sur la case `role="checkbox"
aria-checked`, l'ARIA honnête sans inventer de rôle) ; la coche
CLAVIER passe par Ctrl-clic/souris seulement (geste clavier dédié non
couvert, consigné en reste).

Ordre : du plus petit au plus gros, STOP visuels précoces groupés.

- **E1 — R2, l'icône d'application** : réécrire le rendu de
  `make-icon.ps1` (géométrie Elements, régime tuile figé),
  régénérer `icon.ico` + PNG d'aperçu aux 4 tailles → **STOP visuel CE
  sur les aperçus**, avant toute autre étape. `assets/marque/*.svg`
  remis d'équerre. Gate : build + vérification que l'exe emporte la
  nouvelle icône au terrain (STOP 2).
- **E2 — R3, la marque d'entête** : cote D2 posée aux deux emplois,
  fiche V11 du Système amendée. Gate : e2e écran 02 + entête, STOP
  visuel groupé avec E3.
- **E3 — R4, l'alignement des glyphes de la nav** : alignement baseline
  des trois porteurs, capture avant/après → **STOP visuel CE**. Le
  Système amende A-n (le dessin des pistes, A29). e2e : garde
  d'alignement (position mesurée du bas du glyphe vs baseline) si elle
  peut échouer honnêtement — sinon le dire.
- **E4 — R1, la sélection multiple** : TDD par incréments :
  1. l'état de sélection passe à un `Set` de clés + ancre de shift
     (RED sur le premier test de Ctrl-clic) ;
  2. gestes : Ctrl-clic bascule, shift-clic étend depuis l'ancre,
     clic nu = comportement actuel inchangé (ouvre et sélectionne
     seul) ;
  3. cases à cocher selon D4, `aria-multiselectable` sur la liste,
     `aria-selected` par rangée ;
  4. barre d'actions groupées selon D3 (« N sélectionnés », actions
     D1, « Annuler » qui vide) ;
  5. exécution groupée séquentielle + un toast récapitulatif ; la
     sélection se vide au changement de dossier/compte/recherche.
  STOP visuel dès l'incrément 3 (la première affordance visible).
  Système : amendement A-n complet. e2e : gestes, actions groupées,
  vidage — prouvés non-vacants.
- **E5 — Qualité et clôture** : revue à regard neuf sur le diff
  complet, gate complète, STOP 2 terrain (checklist + commandes),
  documentation (Système, ETAT, CHANGELOG), commit, push, CI.

## 5. Décisions CE

Posées une à une et tranchées par le CE le **2026-08-27** :

- **D1 — Les actions groupées du périmètre (R1)** :
  → **« Archiver + supprimer + indésirable + lu/non-lu »** — les quatre
  gestes de masse ; épingler et déplacer restent unitaires.
- **D2 — La cote de la marque d'entête (R3)** : 20 px aujourd'hui.
  → **« 24 px »** — l'entête de 52 px l'absorbe sans bouger, le mot
  « Wind » (18 px) reste dominant.
- **D3 — Où vit la barre d'actions groupées (R1)** :
  → **« La barre de la liste se transforme »** — « N sélectionnés » +
  actions D1 + Annuler tant que la sélection est non vide ; aucune
  surface neuve.
- **D4 — L'affordance des cases (R1)** :
  → **« Au survol + dès qu'une sélection existe »** — la liste reste
  calme au repos, la case apparaît au survol de la rangée et sur toutes
  les rangées dès qu'au moins une est cochée.
- **D5 — La version cible** :
  → **« 0.11.0, MINEUR »** — la sélection multiple est une capacité
  nouvelle (§2.9) ; R2/R3/R4 voyagent avec.
- **D6 — La sémantique de fil des gestes de masse** (posée en revue,
  le 2026-08-27, devant l'exemple Vantis — un fil de 3 messages
  « archivé » revenait amputé d'un message) :
  → **« Itérer tout le fil côté front »** — le geste groupé rejoue la
  commande unitaire sur CHAQUE message du fil (`thread_messages`,
  purement local) ; le compte du toast reste en conversations, une
  rangée n'est réussie que si tous ses messages le sont.
- **D7 — Le sens de R4** (posée après la première passe terrain) :
  → **« Sur la ligne : recentrer icône et texte »** — le calage
  optique, pas l'alignement en colonne ; tranché sur **planche de
  trois calages** capturés sur la vraie nav :
  → **« C — baseline descendue de 2 px »**.
- **D8 — Les verdicts de la première passe terrain** (2026-08-27,
  consignés mot pour mot) : R2 « ok » ; R3 « ok » ; R1-4/5/6 « OK » ;
  R1-1 « il faudrait que le focus se déplace sur l'email cliqué avec
  CTRL clic » ; R1-2 « l'ensemble des emails entre le premier email de
  la liste et l'email en cible du clic (inclus) devraient être
  cochés » ; R1-3 « la case est trop collée au texte et au bord
  gauche […] au survol il faut décaler un peu le texte vers la
  droite » ; R1-7 « il faut une teinte » ; R1-8 « le raccourci […]
  devrait s'appliquer à tous les messages cochés ». Tous appliqués,
  seconde passe : **« Terrain OK — tout passe »**.

## 6. Reste (dette)

- La coche au CLAVIER (un geste dédié type Ctrl+Espace) n'est pas
  couverte — la sélection multiple est un geste de pointeur ; e/Suppr
  s'appliquent au lot, le reste passe par la souris. À rouvrir sur
  constat.
