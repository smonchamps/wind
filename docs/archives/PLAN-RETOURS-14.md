> **Historical record — French, frozen** (closed on 2026-08-31; PLAN-ENGLISH-SWITCH
> D1, debt D-58). Not translated; the living documentation is in `docs/`.

# PLAN-RETOURS-14 — sept retours CE (post-0.15.0)

> Ouvert le 2026-08-31. Sept retours d'utilisation réelle : six
> features UI (barre d'actions du fil, entête de la Réception
> organisée, cadratins, Réglages > Portier, Registre groupé,
> compteurs nav) et un bug (des inconnus visibles en Réception sans
> passage au Portier). **CHANTIER SOLDÉ le 2026-08-31 — terrain
> complet** (trois passes CE le jour même : 1-7 OK, R8-R10 demandés
> puis corrigés en session, verdict final « ok ») ; commit `18a9e61`,
> CI verte run 33408211506, journal **A104**.
>
> **Chiffres kaizen** : session unique, 3,7 h de mur, 3 prompts CE,
> ≈ 40,1 M d'équiv. input fil principal + 7,6 M d'agents (9 agents —
> reconnaissance Sonnet, revue 5 angles) ; **3 gates complètes
> jouées** (+ la pre-push) ; **0 constat KO au STOP 2** (les trois
> retours terrain étaient des extensions, pas des défauts) ; 37 runs
> e2e ciblés (45 min de mur e2e).

## Constat (instruction sur pièces, 2026-08-31)

### R1 — la barre d'actions du fil vit EN BAS

- `Fil.svelte` porte l'unique barre du fil (`.actions` :
  archiver / signaler-spam / épingler, + mettre-de-côté et
  déplacer-vers en mode organisé), rendue **après** la liste des
  messages ([Fil.svelte:559-599](../apps/desktop/ui-v2/src/Thread.svelte)).
  Répondre / Répondre à tous / Transférer / Supprimer sont PAR
  MESSAGE (`.actions-message`, lignes 504-556) — hors sujet.
- Les deux cadres (volet de droite `Lecture.svelte`, écran 03
  `Conversation.svelte`) partagent ce composant : un seul déplacement
  sert les deux. Le scroll appartient au cadre, pas à `Fil.svelte`
  (`.fil { overflow-y:visible }`) — une barre collante devrait être
  posée en connaissance du conteneur défilant de chaque cadre.
- Le Système montre `.barre-fil` après le corps (3 occurrences) et
  porte encore un « Supprimer » périmé dans cette barre — DC-D2 à
  l'étape.
- e2e : `refonte-ecran02.spec.js:544-579` asserte l'ordre des boutons
  dans les DEUX cadres (composition, pas position) — à garder vert.

### R2 — la Réception organisée porte l'entête générique et les onglets

- `Liste.svelte` : bandeau `h1` générique (l.1070-1074) + onglets
  Tous / Non lus / Brouillons en pied (l.1375-1385). Kiosque et
  Portier ont, eux, l'entête normalisé RETOURS-13 : `h2.display
  .entete-vue` (glyphe 26, trait 1,5) + `p.sous-titre-vue` +
  `p.regle-libelle` — UNE copie CSS dans `systeme.css:237-259`.
- « Titre de section toujours visible au scroll » : **n'existe nulle
  part** (zéro `position:sticky` dans ui-v2/src). Les entêtes de
  section de la liste fenêtrée sont ABSOLUES avec espaceur réel dans
  le flux (piège E4 payé, l.1341-1372) — le collant est à créer
  par-dessus ce mécanisme.
- Brouillons reste accessible par la nav ; retirer les onglets en
  Réception organisée ne perd QUE le filtre Tous / Non lus.

### R3 — les cadratins

- L'écrasante majorité des `—` du source sont des commentaires de
  code (hors périmètre). Les textes MONTRÉS : ~18 clés
  `catalogue.fr.js` + autant en `.en.js`, une dizaine de gabarits
  inline (`Nom — adresse` du composeur et des infobulles,
  `adresse — libellé` de l'historique du Portier, aria-labels
  d'Onboarding…), ~9 messages d'erreur Rust (`commands.rs`).
- Deux replis PONCTUELS affichent « — » comme glyphe de vide (avatar
  `initiales.js:11`, version des Réglages) — pas de la ponctuation.
- Deux assertions e2e cassent avec le motif `Nom — adresse`
  (`retours-9-nom-compte.spec.js:65,79`) — à amender dans le même
  geste.

### R4 — le bug : des inconnus en Réception sans guichet

Les faits établis sur pièces (aucune hypothèse écrite avant mesure) :

- La porte est UNIQUE : `Store::upsert_envelopes`
  ([store.rs:1724-1755](../crates/mail-core/src/store.rs)) — message
  NEUF, boîte INBOX exacte, date > époque, et « connu » si l'une de
  quatre voies : déjà en attente, déjà routé (Oui OU Non), adresse
  d'un compte, **au moins une enveloppe antérieure à l'époque, toutes
  boîtes confondues** (`connu_avant_epoque`). L'annuaire
  `correspondants` ne joue PAS.
- **Suspect n° 1 — le « fil mêlé », comportement VOULU et testé**
  (règle d'or E2, test
  `un_fil_mele_reste_en_reception_et_l_inconnu_attend_quand_meme`) :
  un inconnu qui répond dans un fil dont UN message vient d'un connu
  laisse le fil en Réception (`organise_hors_sql`,
  store.rs:3805-3835) ; l'inconnu EST posé en `portier_attente`, mais
  son message se voit immédiatement dans le fil. Rattachement par
  Message-ID/References sans regard sur l'expéditeur.
- Suspect n° 2 — un faux « connu avant l'époque » : enveloppe mal
  datée (Date falsifiée/absente — un sans-date ne prouve rien depuis
  la revue E2, mais une date fantaisiste antérieure si), ou présence
  dans une autre boîte (Archives, Indésirables) avant l'époque.
- Aucun chemin de synchro ne contourne `upsert_envelopes` (initiale,
  CONDSTORE, différentiel, rejeu d'actions vérifiés).
- Aucune instrumentation S4 dans le code : le diagnostic passe par
  des requêtes SQL sur la base du CE (fournies au STOP 1) — §7.1, je
  ne peux pas la lire.

### R5 — Réglages > Portier : la liste exhaustive existe déjà côté cœur

- La commande Tauri `routages()`
  ([commands.rs:3080-3099](../apps/desktop/src/commands.rs)) rend
  TOUTES les décisions (`address`, `destination`, `regle`, `epoch`),
  tri chronologique. La page Portier n'en montre que les `ecarte`.
- La section Réglages > Portier (RETOURS-13 R9) n'a que les deux
  sélecteurs de défauts. Aucun composant de recherche réutilisable
  n'existe (la recherche d'entête est du markup inline d'App).

### R6 — le Registre est une liste chronologique plate

- `categorie='registre'` servie par `Liste.svelte` +
  `routage_unified_scoped` (tri `last_epoch DESC`). Deux patrons de
  regroupement existent : Kiosque côté client (groupes par expéditeur
  à l'alphabet, repliés en pile — `Kiosque.svelte:126-140, 298-333`)
  et Nettoyage côté cœur (`nettoyage_groupes()`, `GROUP BY
  sender_norm` par récence). Aucun composant partagé : la « pile »
  est dupliquée (la D-47 des menus ⋯ a un cousin ici).

### R7 — les compteurs nav

- Le Portier a DÉJÀ sa pastille (`portier_total` → `Nav.svelte:53`).
  Kiosque et Registre n'ont rien. La commande `category_total(cat,
  compte, non_lus)` existe et sert kiosque/registre au sens IMAP
  `unseen > 0` par fil (`routage_count_scoped`). Le Kiosque a en
  outre SA mémoire « lu » (`kiosque_lus`) — deux sémantiques
  possibles, aucun COUNT agrégé n'existe encore sur `kiosque_lus`.

## Périmètre — refus explicites (§2.6)

- **R1** : on déplace la barre du FIL seule ; les actions PAR MESSAGE
  (répondre, transférer, supprimer) ne bougent pas (D4 de RETOURS-3
  reste debout). Pas de refonte des cadres.
- **R3** : les commentaires de code, docs/, spikes/, messages
  d'assert/tests gardent leurs cadratins — seuls les textes montrés à
  l'utilisateur changent. Les « — » glyphes de vide : selon D4.
- **R4** : aucun correctif écrit avant le diagnostic sur la base du
  CE. Si la racine est le fil mêlé, le correctif éventuel ne
  renversera PAS la règle d'or « jamais perdre de courrier » sans
  décision CE explicite.
- **R5** : recherche CLIENT (la table tient en mémoire — c'est une
  liste de verdicts, pas un corpus) ; pas de recherche serveur.
- **R6** : regrouper n'est pas re-trier le monde : la donnée reste
  `routage_unified_scoped`, le regroupement s'ajoute — pas de refonte
  du fenêtrage de `Liste.svelte` pour le Registre.
- **R7** : pas de pastille pour Nettoyage (session ponctuelle, pas un
  flux) ni pour Envoyés/Archives/Corbeille.

## Étapes (ordre proposé — le bug d'abord en diagnostic, l'UI ensuite)

- **E1 — R4, diagnostic** : requêtes SQL au CE (STOP 1), verdict de
  racine. Puis, selon D5 : correctif TDD (RED sur le chemin fautif)
  ou consignation « comportement voulu » + éventuel signe UI.
- **E2 — R1, la barre du fil en tête** : déplacer `.actions` de
  `Fil.svelte` au-dessus de `.fil` (sous l'entête `.tete`), selon D1
  (collante ou non). STOP visuel précoce (premier rendu → verdict
  CE). e2e d'ordre mis à jour ; DC-D2 (`.barre-fil` de la maquette +
  « Supprimer » périmé corrigé au passage).
- **E3 — R2, l'entête de la Réception organisée** : en
  `categorie === 'reception'` et mode organisé, remplacer le bandeau
  générique par l'entête normalisé (`.entete-vue` + glyphe `inbox`),
  retirer les onglets (D3), sous-titre selon D2 ; les entêtes de
  section (« Nouveau pour vous » / « Déjà consulté ») deviennent
  collantes au scroll (mécanisme à poser SUR la liste fenêtrée —
  entêtes absolues + espaceurs, attention au piège E4). Le bandeau de
  sélection (gestes de masse) reste. STOP visuel précoce.
- **E4 — R7, les pastilles Kiosque et Registre** : `Nav.svelte` +
  `App.svelte` (`chargerNav`), sémantique selon D8 ; si `kiosque_lus`
  retenu, nouvelle requête COUNT anti-jointe au cœur (TDD).
- **E5 — R5, la liste des décisions aux Réglages** : consommer
  `routages()` sans filtre, tri `localeCompare` alphabétique, champ
  de recherche client (filtre sur l'adresse), gestes selon D6.
- **E6 — R6, le Registre groupé** : regroupement par expéditeur selon
  D7 (patron visuel de la pile du Kiosque), données côté cœur si
  volumétrie l'exige — mesure avant d'écrire (banc sur base e2e ;
  coût réel à confronter au terrain).
- **E7 — R3, les cadratins** : catalogues fr/en, gabarits inline,
  messages Rust ; remplacement au cas par cas (deux-points, virgule,
  parenthèses, point) ; motif `Nom — adresse` selon D4 ; e2e amendés
  dans le même geste. Filet : un test qui balaie les catalogues
  expédiés et refuse U+2014.

Chaque étape : boucle intérieure ciblée (specs impactées en fichier
entier), gate complète aux jalons de Phase 3, DC-D2 pour tout commit
UI.

## Livraison (2026-08-31)

Toutes les étapes livrées le jour du GO, journal **A104** (Système
amendé : `.barre-fil` des trois maquettes déplacée en tête, collante,
« Supprimer » périmé retiré au passage).

- **E1 (R4)** : badge « En attente au Portier » sur les messages d'un
  expéditeur en attente (fil mêlé, D5) — commande légère
  `portier_adresses`, `seed_arrivee` sait répondre à un fil
  (`reponse-a`), scénario prouvé e2e de bout en bout (intrus qui
  répond dans le fil d'un connu : visible, badgé, ET au guichet).
  Le diagnostic sur la base du CE reste dû au terrain (requêtes à la
  checklist STOP 2).
- **E2 (R1, D1)** : barre du fil EN TÊTE, collante
  (`position:sticky` au scrollport du cadre), menu « Déplacer
  vers… » ouvert vers le bas ; RED prouvé par stash. Spec neuve
  `retours-14.spec.js` (3 tests, bornés des deux côtés — un filet
  vacant sans le sticky a été identifié et refermé à l'écriture).
- **E3 (R2, D2/D3)** : entête normalisé `.entete-vue` + glyphe
  `inbox`, titre seul ; bandeau générique et onglets morts en
  Réception organisée ; bande de section COLLÉE à hauteur nulle
  (hors géométrie du fenêtrage — le piège E4 des espaceurs ne la
  concerne pas), servie par `premier` (la vérité réactive du scroll).
- **E4 (R7, D8)** : `kiosque_non_ouverts` au cœur (TDD — RED E0599,
  anti-jointe `kiosque_lus` sur la TÊTE du fil), pastille Registre en
  non-lu IMAP (`category_total`), pastilles bornées au MODE (revue).
- **E5 (R5, D6)** : liste exhaustive des décisions aux Réglages >
  Portier (`routages()` sans filtre), alphabet, recherche cliente,
  « Réintégrer » au contrat de la page Portier (toast + propagation) ;
  vocabulaire des verdicts en UNE copie (`lib/portier.js`).
- **E6 (R6, D7)** : `Registre.svelte` — groupes par expéditeur de
  TÊTE, récence en tête (`registre_groupes`, patron nettoyage_groupes,
  TDD), page d'un groupe paginée (« Voir plus »), ⋯ de groupe
  (Déplacer vers Réception/Kiosque, Écarter — `routerAdresse`), le
  volet de lecture reste le lecteur.
- **E7 (R3, D4)** : catalogues fr/en, gabarits inline
  (« Nom (adresse) »), 9 messages Rust ; glyphes de vide gardés ;
  filet `catalogues.test.mjs` PROUVÉ en le cassant (et une leçon
  d'outillage payée : le `git checkout` de dé-sabotage a emporté les
  éditions du fichier — re-posées, re-vérifiées).

## Revue à regard neuf (2026-08-31)

5 angles (diff ligne à ligne, comportements retirés, traceur
inter-fichiers, réutilisation/simplification,
efficience/altitude/conventions) : ~30 candidats, **10 retenues, 10
corrigées** — dont : `liste` nulle aux gestes depuis le Registre
groupé (TypeError avalé, nav et passe d'après-geste sautées →
`rechargerVues()` unique) ; les deux collants qui passaient AU-DESSUS
des voiles modaux (`isolation:isolate` sur le volet et le cadre) ; la
**course relecture-du-mode / première pompe, MESURÉE au décor e2e**
(page 0 à ~85 ms, mode à ~105 ms — la couture des sections n'était
jamais demandée ; la relecture ressert désormais les vues) ; les deux
portes de retour du filtre « Non lus » ; `sender_norm` NULL qui vidait
le Registre entier ; la troncature silencieuse à 200 et les gestes ⋯
perdus du Registre ; la réintégration muette des Réglages ; le filet
VACANT du filtre de compte de la pastille Kiosque ; le coût des
pastilles payé au mode classique.

**Deux défauts d'outillage payés dans la session** : un `git
checkout --` de dé-sabotage qui a emporté de vraies éditions (catalogue
FR re-posé), et un regex de fusion qui a mangé le corps du helper
`rechargerVues()` lui-même (récursion infinie, exception avalée dans
un `.then` — LE symptôme diffus qui a coûté la plus longue
instruction de la session ; leçon : jamais un remplacement par motif
sur le fichier qui contient aussi la définition).

## Limites dites (non corrigées, assumées)

- Les pastilles Kiosque/Registre sont GLOBALES quand la nav est
  filtrée à un compte — la parité de la pastille du Portier, qui fait
  de même ; à revoir si le terrain le dit.
- La clé du badge du fil re-dérive `sender_norm` en JS
  (`toLowerCase` Unicode vs `lower()` SQLite ASCII) — divergence
  assumée sur une majuscule non-ASCII, la même limite que
  `adresse_images` ; le badge peut manquer, jamais mentir.
- Le dessin de la pile et le rang deux-lignes sont recopiés
  (Kiosque/Registre/Nettoyage) et le ⋯ du Registre est une copie de
  plus des menus — famille **D-47**, consignée à DEBT.
- La bande de section collée recouvre brièvement la bande réelle au
  passage d'une frontière (pas de « push » natif) — transitoire.
- Les tests de `retours-14-reception.spec.js` héritent l'un de
  l'autre (style sériel du dépôt) — ils ne se jouent qu'en fichier.

## Terrain — première passe (2026-08-31)

**Verdict CE : 1 à 7 OK.** Trois retours complémentaires, corrigés
dans la session (la voie du jour même) :

- **R8 — un Oui au Portier vaut confiance** (clarifié par
  AskUserQuestion : c'était une demande, pas un constat — le code ne
  posait rien) : le verdict pose AUSSI la règle « toujours afficher
  les images de cet expéditeur », DANS la transaction du verdict
  (`poser_verdict` — tous les chemins : Portier, Déplacer vers…,
  Nettoyage, Réglages) ; un Non ne touche à rien ; révocable aux
  Réglages > Affichage (porte existante). TDD (RED d'abord), prouvé
  e2e de bout en bout (Oui → règle listée → révoquée).
- **R9 — le bouton de TRI des sections** (forme arrêtée en 2e passe
  terrain : MENU déroulant, pas un cycle) : le bouton (dessin des
  boutons nus « Tout déplier », à droite de la ligne de section)
  ouvre un menu des quatre tris — plus récents / plus anciens /
  expéditeur A → Z / Z → A, l'alphabet sur le NOM AFFICHÉ (collation
  de la langue de l'UI) — chaque entrée avec son glyphe : **quatre
  glyphes neufs `tri_recent`/`tri_ancien`/`tri_az`/`tri_za`** (jeu
  87 → 91, demande CE directe — pas de planche). UN composant
  `TriSection.svelte` + `comparateurTri` (lib/tri.js). Posé sur :
  Kiosque (Non lus ET Lus précédemment, chacune son tri), Registre
  (ligne du titre), historique du Portier, Nettoyage. Présentation
  seule — les défauts restent l'ordre d'avant. Filet : menu + les
  quatre ordres prouvés au Registre (l'assertion d'origine comparait
  les ADRESSES — le tri était juste, le filet faux ; corrigé sur le
  nom affiché).
- **R10 — « Réintégrer » devient « Modifier »** aux Réglages >
  Portier : le menu repropose TOUTES les règles (les trois Oui, les
  quatre Non) plus « Renvoyer au portier » (l'ancien Réintégrer) ;
  mêmes toasts que la page Portier, propagation `onroutage`.

Tests mail-core 419 → **422**, e2e 177 → **187** (+ 2 tests node du
filet catalogues).

## § Décisions CE — tranchées au STOP 1, le 2026-08-31

Réponses AskUserQuestion, mot pour mot :

- **D1 (R1)** — barre du fil en tête, défilante ou collante :
  **« Collante au scroll »** — position:sticky en haut du cadre
  défilant, toujours visible même au fond d'un long fil.
- **D2 (R2)** — sous-titre de la Réception organisée : **« Titre
  seul »** — « Réception » avec glyphe, sans sous-titre.
- **D3 (R2)** — retrait des onglets (perte du filtre Tous / Non
  lus) : **« Oui, retirer »** — ni header générique ni footer.
- **D4 (R3)** — cadratins : **« Nom (adresse) ; glyphes gardés »** —
  parenthèses pour le motif nom/adresse ; les « — » de vide restent ;
  le reste au cas par cas (virgule, deux-points, point).
- **D5 (R4)** — si fil mêlé confirmé : **« Accepté + signe dans le
  fil »** — le fil reste entier en Réception, l'inconnu attend au
  Portier ; on SIGNALE visuellement dans le fil que cet expéditeur
  est en attente au guichet. Le diagnostic sur la base CE reste dû au
  terrain (requêtes à la checklist du STOP 2) — si la racine s'avère
  autre, on instruit sur les chiffres.
- **D6 (R5)** — liste des décisions aux Réglages : **« Avec
  “Réintégrer” »** — adresse + destination (+ règle du Non) + retrait
  du verdict (`retirer_routage`).
- **D7 (R6)** — Registre groupé : **« Groupes par récence »** —
  groupes par expéditeur triés par dernier message reçu (patron du
  Nettoyage de printemps), PAS l'alphabet.
- **D8 (R7)** — pastille Kiosque : **« Pas encore ouvert »** —
  adossée à `kiosque_lus`, COUNT neuf au cœur, TDD. Registre :
  non-lu IMAP.
