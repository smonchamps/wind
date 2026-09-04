> **Historical record — French, frozen** (closed on 2026-08-30; PLAN-ENGLISH-SWITCH
> D1, debt D-58). Not translated; the living documentation is in `docs/`.

# PLAN-RETOURS-13 — douze retours CE sur le Mode organisé (post-0.14.0)

> **CHANTIER SOLDÉ le 2026-08-30 — terrain complet.** Douze retours
> CE sur la 0.14.0 (Mode organisé E1-E5bis), ouvert et clos le même
> jour. GO CE au STOP 1 le 2026-08-30 (D1-D5), deux STOP visuels
> validés (E2/E4 puis E5), terrain en DEUX passes (5 constats +
> glyphe kiosque B corrigés le jour même, verdict final « tout ok »).
> Commit `5ab1f15`, CI verte run 33323808766. Journal A101.
>
> Chiffres kaizen : 4 gates jouées (2 arrêtées tôt — fmt, rayon V14 —
> 2 complètes vertes, 2,5-3,3 min) ; 5 constats KO à la première
> passe terrain, 0 à la seconde ; mesure T1 cumulative au poste
> 425,9 M équiv. input (l'outil ne ventile pas par chantier).

## Constat (instruction sur pièces, 2026-08-30)

Source UI : `apps/desktop/ui-v2/src`. Système : A96-A100 (les cinq
entrées du Mode organisé). L'état actuel, vérifié dans le code :

1. **Sombre Automatique** vit dans Réglages > Affichage
   (`Reglages.svelte:673-686`, testid `affichage-auto`), pas dans
   Thèmes.
2. **Rail des Réglages** : `.rang { align-items:center }`
   (`Reglages.svelte:949-953`) — la nav utilise le calage optique
   validé CE (variante C, 2026-08-27) : `align-items:baseline` +
   `translateY(2px)` sur le glyphe (`Nav.svelte:139-147`). Divergence
   de patron réelle.
3. **Libellé** : `boite.reception` = « Boîte de réception » dans les
   deux modes (`catalogue.fr.js:10`).
4. **Sous-titre Portier** actuel (2 lignes, `catalogue.fr.js:36-37`) :
   « Ces expéditeurs vous écrivent pour la première fois. / Vous
   décidez si vous voulez les entendre. »
5. **Clic nu Oui** → déjà la Réception (`Portier.svelte:138`,
   `destination:'reception'`). **Clic nu Non** → écarté SANS règle
   (`destination:'ecarte'`, `regle:null`) — le message ne bouge pas
   vers la corbeille ; seuls les ⋯ posent une règle
   (spam/archive/corbeille).
6. **Historique vide** : « Aucun expéditeur écarté. Votre choix reste
   privé — un expéditeur écarté n'en sait rien, et tout se rejoue
   ici. » (`catalogue.fr.js:57-58`).
7-8. **Entête Portier centrée** (`h2 { text-align:center }`,
   `Portier.svelte:213-217`), sans glyphe ; « Personne n'attend au
   Portier… » centré aussi.
9. **Aucune section Portier** aux Réglages ; les prefs backend passent
   par la table `prefs` SQLite (`set_text_prefs`, patron existant).
10. **Kiosque** : un seul flot chronologique en cartes, AUCUN état lu
    (A100 : « rien n'est marqué lu »), pagination par 20, pas de
    sections ni de groupement.
11. **Entête Kiosque** : pas de titre — une note d'intro avec icône
    `info` (`kiosque.note`).
12. **Nav** : les 3 dossiers organisés s'insèrent dans le même
    `{#each dossiers}` que Réception/Envoyés/… (`Nav.svelte:40-60`),
    sans séparateur ; le seul filet existant est celui de `.boites`.

## Périmètre

Les douze retours, rien d'autre. **Refus explicites (§2.6)** :

- Pas de refonte du fenêtrage Kiosque (limite dite au
  PLAN-MODE-ORGANISE) — les sections travaillent sur le flot paginé
  existant.
- Pas de synchronisation inter-postes de l'état « lu » Kiosque : état
  local au poste, comme `pins` et `mis_de_cote`.
- Le libellé « Réception » (R3) ne vaut qu'en mode organisé — le mode
  classique garde « Boîte de réception » mot pour mot.
- Les règles du Non existantes (exécution à l'arrivée,
  `pending_actions`, A98) ne changent pas de mécanique : seul le
  DÉFAUT du clic nu change.
- Pas de retrait des menus ⋯ des boutons Oui/Non sans décision CE
  (D3).

## Étapes

- **E1 — Réglages, forme** (R1, R2) : « Sombre Automatique » déplacé
  en tête de la section Thèmes ; rail des groupes calé au patron nav
  (baseline + 2 px). Gate : e2e `refonte-ecran02` (bascule
  `affichage-auto` retrouvée sous Thèmes), spec espacement/volets qui
  cliquent `affichage`.
- **E2 — Entête et textes du Portier** (R4, R6, R7, R8) : glyphe
  `portier` à gauche du titre ; titre + sous-titre justifiés à gauche
  sur la colonne des rangs ; sous-titre en 3 lignes (texte CE mot pour
  mot, D1 orthographe) ; « Personne n'attend au Portier… » justifié à
  gauche ; historique vide : « Vous n'avez écarté aucun expéditeur
  pour le moment ». EN traduit en miroir. **STOP visuel précoce** dès
  l'entête rendue.
- **E3 — Actions par défaut + section Réglages Portier** (R5, R9) :
  prefs `portier_defaut_oui` / `portier_defaut_non` (table `prefs`,
  patron A26) ; défauts livrés : Oui → Réception, Non → Corbeille
  (règle `corbeille`, mécanique A98 inchangée) ; section « Portier »
  aux Réglages avec deux sélecteurs (options : D2). TDD : RED sur le
  routage du clic nu Non → corbeille.
- **E4 — Nav du mode organisé** (R3, R12) : libellé « Réception » en
  mode organisé (clé dédiée, le classique intact) ; filet séparateur
  après le bloc des 4 dossiers organisés (patron du filet `.boites`).
- **E5 — Kiosque en deux sections** (R10, R11) : entête au format
  Portier (glyphe `kiosque` + titre + sous-texte CE mot pour mot,
  justifiés à gauche sur la colonne) ; section « Non lus » (cartes
  dépliées) ; section « Lus précédemment » (groupes par expéditeur,
  tri alphabétique, repliés par défaut — D5) ; un message devient lu
  quand le BAS de sa carte dépliée est entré dans la fenêtre
  (IntersectionObserver sur un témoin de pied de carte) ; persistance
  D4. Amende A100 (le « rien n'est marqué lu » est renversé — journal
  A101+). **STOP visuel précoce** dès les deux sections rendues.

Chaque étape : boucle intérieure sur `mode-organise.spec.js` /
`refonte-ecran02.spec.js` en fichier entier ; gate complète aux
moments dits (fin d'implémentation, avant commit).

## § Décisions CE

- **D1 — Orthographe du sous-titre Portier.** Le texte CE porte
  « Les autorisez vous à vous contacter ? ». Corriger en « Les
  autorisez-vous » (traits d'union) ou garder mot pour mot ?
- **D2 — Options des deux sélecteurs de la section Portier.**
  Proposé : Oui ∈ { Boîte de réception, Kiosque, Registre } (les
  trois destinations du ⋯ actuel) ; Non ∈ { Corbeille, Archive,
  Indésirables, Écarter sans déplacer } (les trois règles du ⋯ + le
  comportement 0.14.0). Valider ou amender les listes.
- **D3 — Devenir des mini ⋯ sur Oui/Non.** Avec un défaut réglable,
  les conserver (choix ponctuel dérogeant au défaut) ou les retirer ?
- **D4 — Persistance du « lu » Kiosque.** Table SQLite locale (patron
  `pins`/`mis_de_cote` : survit aux réinstallations de l'UI, autorité
  au cœur) — recommandé — ou `localStorage` (léger, mais perdu avec le
  profil WebView) ?
- **D5 — Forme d'un groupe replié de « Lus précédemment ».** Proposé :
  une rangée expéditeur + nombre de lettres ; le clic déplie ses
  cartes (repliées sur la ligne d'objet, dépliables une à une). Autre
  forme ?

## Verdicts (STOP 1, 2026-08-30)

- **D1** : « Corriger (Recommandé) » — « Les autorisez-vous à vous
  contacter ? », traits d'union.
- **D2** : « Valider (Recommandé) » — Oui ∈ { Boîte de réception,
  Kiosque, Registre } ; Non ∈ { Corbeille, Archive, Indésirables,
  Écarter sans déplacer }.
- **D3** : « Conserver (Recommandé) » — les mini ⋯ restent, choix
  ponctuel dérogeant au défaut.
- **D4** : « SQLite (Recommandé) » — table locale au patron
  `pins`/`mis_de_cote`.
- **D5** : « Rangée + dépli + un visuel d'élévations empilées » — la
  rangée repliée montre un empilement de cartes (patron visuel de la
  pile « Mis de côté ») ; le clic déplie les cartes sur leur ligne
  d'objet, dépliables une à une.
- **GO** donné le 2026-08-30 — E1→E5, STOP visuels précoces sur E2
  et E5.

## Revue à regard neuf (2026-08-30, 8 angles)

Retenues et **corrigées** : le témoin « lu » du Kiosque ne se
réarmait pas après un échec d'écriture (« au prochain passage » était
un mensonge) ; les sélecteurs Réglages > Portier pouvaient réécrire
l'AUTRE défaut avec la valeur livrée si l'on cliquait avant la réponse
de la base (ils ne se peignent plus qu'avec l'état persisté) ; le
`Promise.all` du Portier couplait la lecture des défauts au
rafraîchissement du guichet (un hoquet de pref aurait caché des rangs
en silence — défauts lus UNE fois, avant le premier rang, hors du
chemin des décisions) ; la collation du tri des groupes suivait la
locale de l'HÔTE (épinglée à la langue de l'UI) ; la règle du libellé
« Réception » vivait en quatre copies (→ `cleLibelleBoite`, une) ;
l'entête Portier/Kiosque était dupliquée (→ `systeme.css`, une
copie) ; le vocabulaire des défauts re-littéralisait les tables de
routage (→ dérivé de `DESTINATIONS_ROUTAGE`/`REGLES_ROUTAGE`).

Consignées SANS correction (refus motivés) : le calage baseline du
rail reste une copie de celui de la nav (les deux commentaires se
pointent — hisser la règle obligerait à toucher le markup de la nav
dont le calage est validé CE sur planche, gain < risque) ; les
écritures « lu » partent une par carte sous le verrou global (un
Kiosque se compte en dizaines — un lot se justifierait à l'échelle,
pas ici) ; **dette D-48** : la liste ne suit pas une écriture externe
(un `retirer_routage` hors gestes de la Liste reste invisible jusqu'à
une navigation — le pas e2e qui vivait d'une recharge fortuite de la
sonde est désormais honnête, il ressert par l'aller-retour de
dossier ; le vrai correctif est un signal d'invalidation, chantier
futur).

## Livraison

- E1 (R1, R2) — Réglages : bascule sous Thèmes, calage du rail.
- E2 (R4, R6, R7, R8) — entête et textes du Portier, à gauche.
- E3 (R5, R9) — défauts du clic nu (cœur + section Réglages Portier,
  mode organisé seul).
- E4 (R3, R12) — « Réception » (règle unique) + filet de nav.
- E5 (R10, R11) — Kiosque : entête au format Portier, table
  `kiosque_lus`, sections Non lus / Lus précédemment.
- Tests : mail-core 410 → **412** (défauts, kiosque_lus) ; e2e 166 →
  **169** (défauts du Portier ×2, sections du Kiosque ; assertions
  d'entête/nav/textes ajoutées aux tests existants).

## Terrain (2026-08-30, deux passes)

**Première passe** : 7 points OK, **cinq constats — corrigés le jour
même, dans la session** :

- C1 — le glyphe des titres de vue en NON GRAS (trait 1,5 en unités
  du viewBox : à 26 px le trait de 2 pesait face au display 340).
- C2 — le titre de section du Portier reste visible sans nouvel
  expéditeur.
- C3 — « Supprimés automatiquement » → « Déplacés automatiquement
  dans la corbeille » (rien n'est supprimé, D4).
- C4 — la section Réglages > Portier visible QUEL QUE SOIT le mode
  (renverse le « organisé seul » de la première passe — verdict CE).
- C5 — « Non lus » vide : titre conservé + coche du Portier + « Vous
  avez lu toutes les nouvelles actualités de votre Kiosque. »
  (accord posé comme en D1).

**Retour complémentaire** : le glyphe `kiosque` redevient un kiosque
À JOURNAUX — planche de 7 variantes (A-G, l'actuel en référence),
verdict CE : **« B — Feston »** (auvent festonné en arches, guérite,
comptoir). Jeu et relevé du Système amendés, cohérence verte.

**Seconde passe** (captures de preuve + app) : **« tout ok »** —
terrain VALIDÉ le 2026-08-30. Re-gate complète verte (2,5 min,
169/169).
