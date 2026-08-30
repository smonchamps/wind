# PLAN-MODE-ORGANISE — Portier, Kiosque, Registre, Mis de côté, Groupes

> **E5bis LIVRÉE le 2026-08-30 — le Kiosque en cartes** (décision CE :
> « avant la release » ; Système A100). La vue routée devient une
> scène de lecture : cartes déjà ouvertes (corps ENTIER — le document
> auto-CSP de l'écran de lecture, iframe S1, `corpsAuto` extrait vers
> `lib/corps.js`, garde d'images par message), corps du CACHE par page
> servie de 20 (D5/S3 — jamais un réseau par carte), cartes ajoutées
> page à page au défilement (LIMITE DITE : pas de fenêtrage — un
> Kiosque se compte en dizaines, pas en milliers), ⋯ de gestes par
> carte, bouton Replier/Déplier sur la ligne de l'objet (STOP visuel
> CE en TROIS passes, chaque constat corrigé en séance). Le « déjà
> ouvert » prouvé au filet en le cassant. Revue ciblée 6/5 corrigées
> (dédoublonnage des pages, recharge aux relèves, vide honnête, garde
> d'images par carte, sync_state hors boucle). **Terrain CE OK le
> 2026-08-30, zéro constat. Reste : LA RELEASE MINEUR E1-E5bis.**

> **E4 et E5 LIVRÉES le 2026-08-30** (D7 amendée : la première MINEUR
> porte E1-E5). **E4 — la Réception organisée** : sections « Nouveau
> pour vous · n » / « Déjà consulté » servies par UN flot ordonné
> non-lus d'abord (S1/A2 industrialisé : index partiels d'expression
> global + par compte, gardes de plan sur les TROIS chemins — banc
> 200 k : 0,03 / 1,6 ms aux offsets 0 / 100 k, couture 25 ms hors
> chemin d'affichage ; migration qui REBÂTIT l'index E2 dont la clé
> n'a pas les sections) ; colonne centrée 760 px sans volet, clic →
> écran 03, retour ressert (un lu quitte sa section), ⋯ de gestes à
> gauche de l'heure (place réservée). Deux pièges de fenêtrage payés
> en capture : l'entête absolue exige un ESPACEUR RÉEL dans le flux
> (les rangées s'empilent en flex — un décrochement qui ne vivrait que
> dans les maths chevauche), et une marge auto en travers d'une
> colonne flex ÉTEINT le stretch (rangées rétrécies au contenu).
> STOP visuel CE : un constat (l'air avant le titre de section — bande
> 34 → 52 px), le reste GO. **E5 — Mis de côté** : table `mis_de_cote`
> (patron pins, purges comprises), état par FIL semé de la ligne
> servie (patron épingle — la revue a retiré l'IPC par ouverture),
> exclusion partagée de TOUTES les vues organisées par UNE écriture
> (`exclusion_organisee()` — la revue a trouvé la pastille de nav qui
> comptait encore les mis de côté), pile bas-droite + éventail +
> tableau plein écran, bascule à la barre du fil et au ⋯, la pile
> s'efface sous l'écran 03. Revue à regard neuf E4+E5 : **10
> trouvailles, 9 corrigées** (dont le triage clavier qui marquait lue
> une conversation jamais montrée, la couture d'une autre source, le
> testid vacant du filet) ; refus motivé : « Terminé » sur un fil dont
> l'expéditeur a été écarté suit le VERDICT (caché du mode organisé —
> le comportement du Non, pas une perte). Dette **D-47** (menus ×3 et
> jumeaux pins/pile). Tests mail-core 406 → **410**, e2e 162 → **166**
> (chaque mécanisme prouvé en le cassant). **Système A99.**
> **Terrain CE E4+E5 OK le 2026-08-30, zéro constat.** L'écart de
> périmètre relevé — le « Kiosque en cartes déjà ouvertes » du
> prototype n'était assigné à AUCUNE étape — est TRANCHÉ par le CE le
> même jour : « Avant la release » → **étape E5bis, le Kiosque en
> cartes** (corps préchargés par page servie, D5/S3), puis la release
> MINEUR E1-E5bis.

> **E3 LIVRÉE le 2026-08-30 — les règles du Non à la synchro** (D2/D4,
> Système A98 ; **terrain CE OK le jour même, zéro constat** —
> exécution serveur vérifiée au webmail). À l'arrivée d'un message d'un écarté avec règle :
> action journalisée **dans la transaction du lot** (revue E3 — la
> première forme, exécution après commit, laissait une fenêtre de
> crash où la règle se perdait pour toujours) via `pending_actions`
> (rejouée en tête de chaque synchro), retrait local sans écho.
> `archive` → Archive, `corbeille` → Delete (la corbeille serveur,
> jamais définitive — D4), `spam` → le dossier indésirable résolu.
> Garde anti-doublon (le retrait local fait reculer `max_uid` : une
> re-livraison sur rejeu en échec re-retirait ET re-journalisait — la
> seconde action identique aurait coincé la file). « Ses prochains
> messages » seulement : rien d'antérieur au verdict (un backfill ne
> jette jamais l'historique). **Limites dites** : un en-tête Date
> falsifié antérieur au verdict esquive la règle (le drapeau cache le
> message du mode organisé quand même) ; spam sans dossier indésirable
> reconnu ne fait RIEN (dégrade en Non nu) — jamais une destination
> inventée ; pendant qu'une action attend son rejeu hors ligne, le
> cycle CONDSTORE paie l'inventaire complet (comportement préexistant
> des gestes, aggravé en fréquence — à surveiller au terrain).
> « Dites à l'historique » (§6) est lu comme : l'historique DIT la
> règle posée (E2) — pas de compteur d'exécutions ; arbitrage CE au
> terrain si l'intention était autre. Tests mail-core 401 → **406**
> (re-livraison, extinction D2, antériorité, spam résolu/irrésolu),
> e2e 161 → **162** (règle prouvée en la cassant + témoin de synchro
> au filet — un négatif seul serait vacant).
> **Reste : la release MINEUR E1-E3 (D7), puis E4-E6.**

> **E2 LIVRÉE le 2026-08-30 — le Portier.** La rétention (D3 « arrivées
> seules ») : `envelopes.sender_norm` (colonne générée + index — piège
> payé : SQLite n'emploie un index d'EXPRESSION que contre un littéral,
> jamais en jointure), `portier_attente` matérialisée à l'arrivée
> (spike S2-bis : toute forme calculée à la requête s'effondre à
> l'offset profond — 299 ms ; le drapeau `threads.organise_hors`
> entretenu par `thread::refresh` + index partiel miroir vaut MIEUX que
> le témoin, 4,2 vs 6,5 ms) ; exclusion partagée flot + totaux +
> épingles préposées + pastille de nav. Page du guichet (forme du
> prototype, STOP visuel CE GO le 2026-08-30, **terrain CE OK le même
> jour — zéro constat**, mesure S4 du flux d'inconnus ENGAGÉE sur les
> deux postes pour une semaine), Oui/Non + minis ⋯,
> historique, réintégration (mêmes règles que l'arrivée), glyphe
> `more_horiz` (86ᵉ). Revue à regard neuf 10/10 corrigées — dont
> QUATRE prouvées RED : la règle d'or (un fil mêlé à un écarté
> RESTE — `ecarte` n'a pas de vue), le message SANS Date qui
> contournait le guichet, la réintégration hors-arrivées, et le
> rattrapage E1→E2 de la migration (les postes du terrain E1 ont pu
> graver l'époque). Tests mail-core 387 → **401**, e2e 157 → **161**
> (filet prouvé en cassant la rétention). Système A97. NOTE E3 : les
> toasts du Non promettent l'exécution automatique (« partiront aux
> Indésirables ») — textes CE du prototype ; la promesse n'est tenue
> qu'à E3, les deux partent dans la MÊME release (D7).
> **Prochaine étape : E3 — les règles du Non à la synchro.**

> **CHANTIER OUVERT le 2026-08-29** (STOP 1 : D1-D9 tranchées, §7).
> **E1 LIVRÉE le jour même** : socle complet — pref `mode_organise`
> (prefs SQLite + époque de première activation), va-et-vient
> d'entête, nav organisée (Kiosque, Registre — le Portier vient avec
> sa page à E2), table `routage_expediteurs` + 6 commandes, vues
> servies par le squelette exact de la Réception (garde de plan),
> « Déplacer vers… » à la barre du fil (adresse résolue au cœur,
> jamais soi — revue), 5 glyphes au jeu (85). ADR 0028 ; Système A96 ;
> spikes S1-S3 mesurés (§5bis) ; STOP visuel + terrain CE OK (7/7,
> zéro constat après correction du dist périmé D-33 en séance) ; revue
> à regard neuf 8/8 corrigées (dont deux « perte de courrier »
> prouvées RED : tête de fil = Envoyés, épingles exclues) ; e2e 153 →
> **157**, tests mail-core 383 → **387**, gate verte 2,6 min.
> **Prochaine étape : E2 — le Portier.**

> **DOSSIER D'INSTRUCTION d'origine.** Préparé le
> 2026-08-29 sur la base du prototype cliquable validé par le CE en
> six passes de retours le jour même
> (`spikes/mode-organise/index.html`, artifact
> <https://claude.ai/code/artifact/914fd918-b122-4b42-b5c7-b4df8f64e4d2>).
> Le prochain sujet inscrit à [ETAT.md](ETAT.md) reste **la première
> vague bêta** (PLAN-BETA, bloquant CE) — ce chantier vient APRÈS,
> sauf décision contraire du CE.
>
> **Pour lancer :** `/chantier Feature : le Mode organisé —
> PLAN-MODE-ORGANISE.md porte le dossier d'instruction.` La session
> jouera la Phase 0 (instruction sur pièces, §3-§5 à confirmer), la
> Phase 1 (conception set-based, spikes du §6), puis présentera le
> § Décisions CE au STOP 1. **Aucun code de production avant ce GO.**

---

## 1. L'énoncé

Un second mode de tri du courrier, inspiré des six fonctionnalités
HEY fournies par le CE (PDF « Hey Features / Must Have »), accessible
par un **va-et-vient « Organisé » à droite de la barre de recherche**.
Le mode classique reste l'app d'aujourd'hui, intacte, et reste le
défaut. Noms arrêtés par le CE sur prototype : **Portier** (The
Screener), **Kiosque** (The Feed), **Registre** (Paper Trail),
**Mis de côté** (Set Aside), **Grouper** (Bundle), Réception en deux
sections (The Imbox).

## 2. Le produit — comportements arrêtés au prototype

Les six passes de retours CE sur la planche ont déjà tranché la forme.
Ce qui suit est **acquis** ; le chantier ne le renégocie pas sans
constat terrain.

| Capacité | Comportement arrêté |
|---|---|
| **Va-et-vient** | À droite de la recherche ; pilule + disque (les deux seules formes rondes légitimes, V14). Le mode est une préférence locale, il survit au redémarrage. |
| **Portier** | Les expéditeurs qui écrivent pour la première fois attendent ici, leurs messages RETENUS hors de la Réception. La page : titre et sous-titre centrés, règle-libellé « Voulez-vous recevoir leurs messages ? » au dessin de « Historique du Portier » (libellé nu, 8 px, filet du premier rang), puis UN rang par expéditeur **au format des rangées du volet central** (disque non-lu, expéditeur, heure qui ne cède jamais, objet, aperçu) + l'adresse en clair. Boutons **à droite** : Oui / Non, 44 px. Chaque bouton porte un **mini ⋯ au coin haut-droit** : sur Oui il oriente (Réception / Kiosque / Registre), sur Non il pose la règle (signalés indésirables / archivés automatiquement / supprimés automatiquement). Le clic nu : Oui → Réception, Non → écarté sans règle. **Un oui/non, rien d'autre — ni tri ni traitement du message au guichet.** L'expéditeur n'est jamais prévenu ; l'« Historique du Portier » dit la règle choisie et « Réintégrer » la défait (les messages des 90 derniers jours réapparaissent). Le choix de destination au guichet et le filtrage par domaine ont été **retirés** (passes 3 et 2). |
| **Réception organisée** | SANS volet de lecture : fil de messages centré (colonne ~760 px), deux sections au dessin du Portier — « Nouveau pour vous · n » / « Déjà consulté » (le lu, l'envoyé). Un clic ouvre **l'écran 03** (la surimpression plein écran du classique 1-2 volets : entête 52 px sur `--surface`, « ← <boîte> » + « Écrire », colonne 960 px, barre du fil au pied, Échap ferme). Le ⋯ de gestes apparaît au survol **à gauche de l'heure**, place réservée (opacité seule, la géométrie ne bouge pas). |
| **Kiosque** | Les lettres d'information **déjà ouvertes**, la plus récente en tête, défilement sans traitement (rien n'est « à lire »). Gestes par message (⋯). |
| **Registre** | Reçus, confirmations, factures — même format de rangées que la Réception, même colonne centrée, pas de cadre englobant. |
| **Mis de côté** | Pile en bas à droite de la Réception ; clic = éventail des mini-cartes ; « Voir le tableau » = aperçus en grille sur un écran ; « Terminé » renvoie le message d'où il vient. Bascule depuis la barre du fil (« Mettre de côté » / « Reprendre ») et le ⋯. |
| **Grouper** | Un expéditeur groupé tient en UNE rangée de la Réception (« Groupé · n nouveaux »), quel que soit le volume ; clic = ses nouveaux messages sur une page (sinon tous), avec « Dégrouper » et « Tout marquer lu ». Bascule par expéditeur depuis le ⋯ et la barre du fil. |
| **Gestes par message** | « Déplacer vers… » (Réception / Kiosque / Registre) déplace l'expéditeur ENTIER et ses messages existants suivent (règle HEY, confirmée au prototype), « Mettre de côté », « Grouper/Dégrouper l'expéditeur », « Écarter cet expéditeur ». |

Le détail cliquable fait foi : `spikes/mode-organise/` (README = journal
des six passes).

## 3. Instruction sur pièces — l'existant qui porte

Vérifié au dépôt le 2026-08-29 (tables : `crates/mail-core`) :

- **`images_expediteurs`** (A89, PLAN-RETOURS-11) : une règle globale
  au poste, à clé **adresse exacte normalisée**, autorité au CŒUR —
  c'est **le patron exact du routage par expéditeur** que le Portier
  et « Déplacer vers… » exigent.
- **`pins`** (A73) : table locale à clé d'enveloppe, qui survit à la
  reconstruction des fils et ne touche JAMAIS un flag serveur — le
  patron de **Mis de côté**, et la jurisprudence « exclusion
  partagée » (le flot paginé ET les totaux excluent les épinglées ;
  garde de plan `CROSS JOIN` directif, ~24 ms payés à 200 k sans
  elle).
- **`correspondants`** (A65) : l'annuaire appris du courrier vu,
  rattrapé une fois sur l'existant — la matière du « déjà connu »
  du Portier (D3) ; JAMAIS un parcours d'enveloppes par frappe.
- **`prefs`** + `PREFS_PAR_COMPTE` : la préférence `mode_organise`
  (par poste) et sa purge éventuelle.
- **`pending_actions`** : le chemin existant des gestes serveur
  (archiver, spam, supprimer) — les règles du Non s'y greffent.
- **Fenêtrage de la liste** (PLAN-DEFILEMENT-PROFOND,
  PLAN-ESPACEMENT) : un seul vol de page, sondes permanentes en cage,
  `enrichir_lignes` borné à la PAGE — toute section ou repli de
  groupe doit passer par ces chemins, jamais les contourner.
- **Écran 03** (`Conversation.svelte`, D4 UI v3) : la surimpression
  existe, elle se RÉUTILISE telle quelle en mode organisé.
- **Catalogue** `catalogue.fr/en.js`, gate de cohérence (glyphes),
  gate de contraste : les canaux normaux des textes et dessins neufs.

**À instruire en Phase 0 (rien de supposé)** : la forme exacte des
requêtes chaudes de `list_category`/`category_total` et le coût d'une
exclusion « expéditeur retenu au Portier » ; le chemin d'arrivée d'un
message en synchro (où se joue une règle du Non) ; le budget du
préchargement des corps du Kiosque ; le volume d'expéditeurs inconnus
sur les vraies boîtes du CE (dimensionne D3).

### 3bis. Phase 0 jouée le 2026-08-29 — faits relevés au code

- **Requêtes chaudes confirmées** : `list_category`
  (`apps/desktop/src/commands.rs:1727`) sert la Réception par
  `unified_recent_scoped` puis `enrichir_lignes` (borné à la page,
  `nav.rs:421`) ; `category_total` est une commande SÉPARÉE, appelée
  APRÈS le premier rendu (~240 ms/200 k pour une intégrale — jamais
  sur le chemin d'affichage). L'exclusion des épinglées passe par
  `PINNED_THREADS` (`store.rs:575`, `CROSS JOIN` directif) dans le
  flot ET les totaux, garde de plan
  `la_boite_unifiee_ne_materialise_pas_son_tri` (`store.rs:6135`,
  `EXPLAIN QUERY PLAN`). L'exclusion « retenu au Portier » suivra ce
  patron exact, garde de plan comprise (S2).
- **`images_expediteurs`** confirmée (`store.rs:319`) : clé
  `address TEXT PRIMARY KEY` en minuscules, normalisation UNIQUE
  `adresse_images()` (`store.rs:3190`) — à réutiliser, jamais une
  seconde. **Nuance relevée : cette table n'est PAS purgée à
  `delete_account`/`reset_mailbox`** — c'est voulu (mémoire globale au
  poste). `routage_expediteurs` et `groupes_expediteurs` (clé adresse,
  globales) suivront ce statut ; `mis_de_cote` (clé d'enveloppe) suit
  au contraire `pins`/`images_messages` : purge obligatoire.
- **Chemin d'arrivée** : insertion par `upsert_envelopes`
  (`store.rs:1372`), appelée d'`initial_sync`/`incremental_sync`
  (`sync.rs:117/141`) — le point de greffe des règles du Non.
  `pending_actions` (`store.rs:141`) porte déjà `Archive`, `Delete`,
  `MoveTo(dossier)` (`action.rs:11`) : les trois règles du Non
  s'écrivent avec l'existant, rejouées par `replay_actions`
  (`sync.rs:207`) en tête de chaque synchro.
- **« Déjà connu »** : `correspondants` (`store.rs:287`, clé adresse
  minuscule) n'a pas encore de test de présence nue — à écrire sur le
  patron d'`images_allowed` (`store.rs:2502`), une sonde PK.
- **⚠️ Écart factuel au §3** : le thème ET l'espacement vivent en
  **`localStorage`** (`wind-theme`, `wind-espacement`), PAS dans
  `prefs` SQLite — « comme le thème » signifierait un mode invisible
  du Rust. Or **les règles du Non se jouent côté Rust à la synchro** :
  si elles doivent s'éteindre quand le mode se désactive, le cœur
  doit savoir l'état du mode → `prefs` SQLite (lisible des deux
  côtés). Tranché en D2 (amendée).
- **Écran 03 réutilisable tel quel, confirmé** :
  `Conversation.svelte` est piloté par le store partagé `fil`
  (`cadre === 'plein'`), rien n'y présuppose le mode classique ; les
  props/callbacks sont déjà normalisés dans `App.svelte:1526`.
- **Va-et-vient** : l'entête (`App.svelte:1471`) est un flex
  `gap:12px`, recherche bornée à 520 px, `margin-left:auto` sur
  « Écrire » — la pilule s'insère entre la recherche et « Écrire »
  sans toucher au flex.
- **Nav** : `Nav.svelte` construit `dossiers` par un tableau statique
  `{id, icone, libelle}` + `onchoisir({categorie})` — l'extension
  Kiosque/Registre/Portier est mécanique.
- **Fenêtrage** : `Liste.svelte` (PAGE=200, OVER=8), un vol de page
  (`lancer`, réponse ignorée si la source a changé), sondes
  permanentes en cage `bind:offsetHeight`. Sections et repli de
  groupes s'intègrent soit au service (patron de la tranche `echos`
  de `category_page`, `nav.rs:626`), soit à l'affichage — S1 les
  départage au banc.
- **Glyphes** : `lib/icones.js` (JEU, 80 glyphes) ; la gate de
  cohérence (`e2e/coherence-systeme.mjs:252`) exige l'entrée au JEU
  ET la `figcaption` au Système dans les DEUX sens, chaque tracé `d`
  littéralement présent dans le doc. Les 5 glyphes neufs ne sont PAS
  des repères de compte (pas de cinquième liste).

## 4. Architecture proposée (à confirmer set-based en Phase 1)

**Le routage est LOCAL et l'autorité est au CŒUR.** Rien ne déplace
jamais un message côté IMAP (D1) : la destination est une donnée de
présentation, comme `pins`.

- `routage_expediteurs(adresse TEXT PK normalisée, destination TEXT
  CHECK IN ('reception','kiosque','registre','ecarte'), regle TEXT
  NULL CHECK IN ('spam','archive','suppression'), decide_epoch)` —
  patron `images_expediteurs`. « Réintégrer » = DELETE de la ligne.
- **Portier** = les expéditeurs de la Réception qui n'ont NI ligne de
  routage NI présomption d'acceptation (D3). Leurs messages restent
  en base, **exclus du flot et des totaux** de la Réception
  (exclusion partagée, leçon `pins`). Un Oui/Non écrit UNE ligne ;
  les messages existants « suivent » par construction (la requête lit
  le routage au service, rien à déplacer).
- **Règles du Non** : à l'arrivée d'un message d'un expéditeur
  `ecarte` avec règle, le cœur enfile l'action existante
  (`pending_actions`) — spam / archive / **corbeille** (jamais de
  suppression définitive, règle d'or : on ne perd pas de courrier).
- `mis_de_cote(clé d'enveloppe)` — copie du patron `pins` (purges
  `reset_mailbox`/`remove_local` comprises, leçon RETOURS-11).
- `groupes_expediteurs(adresse PK)` ; le repli en une rangée se fait
  **au service de page** ou **à l'affichage** — à départager au spike
  S1 (§6), pas à l'avis.
- **Sections de la Réception** : « Nouveau pour vous » = non-lus,
  « Déjà consulté » = lus + envoyés — deux bornes dans la même source
  paginée, à concevoir avec le fenêtrage (spike S1).
- **UI** : la nav organisée (Réception, Kiosque, Registre, Portier,
  puis les dossiers), les vues centrées, l'écran 03 réutilisé, le
  va-et-vient dans l'entête. Mode classique : **zéro diff de rendu**
  (garde e2e dédiée).
- **Cinq glyphes neufs** au catalogue : `portier` (majordome — tête,
  buste de `person`, nœud papillon), `kiosque`, `registre`, `pile`,
  `groupe` — dessinés au spike à la grammaire du jeu (grille 24,
  trait 2, butt/miter) ; relevé au Système + gate de cohérence,
  preuve n/n (A18).

## 5. Points durs — front-loading OBLIGATOIRE (§2.2)

À spiker et MESURER avant toute écriture de production :

- **S1 — fenêtrage sections + groupes** (le plus dur) : sections et
  repli de groupes changent le comptage des rangées servies ; les
  leçons DEFILEMENT-PROFOND (un vol de page, totaux hors chemin
  d'affichage) et ESPACEMENT (hauteurs sondées) s'appliquent. Banc
  sur base 200 k+ : coût du service de page avec routage + sections +
  repli, contre l'existant. Budget : pas de régression mesurable sur
  `list_category` chaud.
- **S2 — plan SQLite du routage** : l'exclusion « retenu au
  Portier » et le filtre de destination dans les requêtes chaudes —
  vérifier le plan (`EXPLAIN QUERY PLAN`), garde de plan si SQLite
  scanne (jurisprudence `CROSS JOIN` de `pins`).
- **S3 — Kiosque « déjà ouvert »** : corps disponibles au défilement
  sans requête chaude par rangée — préchargement borné à la page
  servie (patron `enrichir_lignes`) ; mesurer le coût.
- **S4 — activation du mode** : sur une boîte réelle, combien
  d'expéditeurs « inconnus » au premier jour ? (dimensionne D3 ;
  mesure sur les deux postes du CE).

### 5bis. Verdicts des spikes — mesurés le 2026-08-29 (set-based, §2.2)

Trois spikes joués en worktrees isolés, rapatriés dans `spikes/`
(`routage-plan/`, `fenetrage-organise/`, `kiosque-precharge/` — READMEs
= protocoles et chiffres complets). Bases synthétiques 200 k, schémas
et requêtes de prod reproduits à l'identique, 20 itérations méd/p95.

- **S2 — plan du routage : l'exclusion par MESSAGE est gratuite.**
  Page unifiée témoin 0,228 ms ; avec `NOT EXISTS` sur
  `routage_expediteurs` : 0,209 ms (avec époque : 0,178 ms) — sonde
  PK par index couvrant, jamais de scan, **aucun `CROSS JOIN`
  directif nécessaire** (le piège des pins ne se pose pas : la table
  n'entre qu'en corrélé après pagination). L'exclusion **par FIL
  façon pins** coûte 13,5 ms même avec index couvrant (59×) — le
  patron pins ne se transpose PAS. Page Kiosque (filtre destination) :
  0,087 ms. `category_totals` + exclusion : +18 ms (67,1 vs 49,1).
  ⚠️ Point d'industrialisation : l'exclusion doit se poser DANS les
  tranches par boîte (avant pagination), sinon pages courtes (37/50
  mesuré). Garde de plan à écrire en prod (moteur node ≠ rusqlite).
- **S1 — sections et groupes : AU SERVICE, avec index partiels.**
  Sections au service SANS index : 310-539 ms en fond de section ;
  avec **2 index partiels** (`unseen>0`/`=0`) : **1,7-7,5 ms**, profil
  du témoin, un flot + couture par COUNT (0,37 ms). Sections à
  l'affichage : le 200ᵉ non-lu est au rang 1 693 → 9 vols de page
  pour la première page de section — ÉCARTÉ. Repli de groupes au
  service NAÏF : ~1 510 ms à toute page (UNION+GROUP BY matérialisé)
  — ÉCARTÉ ; à l'affichage : une rafale de 600 traverse 5 vols pour
  UNE rangée (rendement 0,17 %, compte n faux) — ÉCARTÉ ;
  **industrialisé au service** (drapeau `threads.groupe` maintenu en
  transaction comme `size`/`unseen`, groupes servis À PART sur le
  motif de `pinned_unified_scoped`) : **1,62-6,59 ms + ~0 ms les
  rangées de groupe, offset stable par construction** — RETENU.
  Reste à définir : l'expéditeur d'un fil multi-expéditeurs.
- **S3 — préchargement Kiosque : dans le budget.** Lot borné à la
  page (patron `enrichir_lignes`) : page de 20 corps 12,2 ms froid /
  1,8 ms chaud (1,6 Mo) ; page de 50 : 29,6/3,9 ms. Corps unitaire
  (`Store::body`) : 1,02 ms froid. Aperçu « d'abord » gratuit (déjà
  dans SELECT_UNIFIED). Zéro scan aux plans. Réserve : NVMe local —
  à confirmer au poste x64 avant de graver le budget ; le coût de
  RENDU WebView de 20 corps n'est pas mesuré.
- **S4 — volume d'inconnus** : reformulé par D3 (arrivées seules) —
  la mesure porte sur le FLUX quotidien d'expéditeurs sans ligne de
  routage, à relever sur les deux postes du CE pendant la première
  semaine du mode (dimensionne l'ergonomie du Portier, pas le
  schéma). À faire au terrain d'E2.

## 6. Découpage proposé — six étapes, chacune gate-verte et commitée

Chaque étape : TDD (RED d'abord), filet e2e **prouvé non-vacant en le
cassant** (leçon PLAN-ESPACEMENT), boucle intérieure ciblée, gate
complète UNE fois avant commit. Livraison en **deux releases MINEUR
minimum** (§2.9) — proposition : E1-E3 puis E4-E6.

1. **E1 — le socle** : pref `mode_organise`, va-et-vient d'entête,
   nav organisée, table `routage_expediteurs` + commandes de lecture/
   écriture, vues Kiosque et Registre (routage manuel « Déplacer
   vers… » seul). Garde : mode classique inchangé au pixel.
2. **E2 — le Portier** : rétention des inconnus (exclusion partagée),
   page Portier (forme arrêtée §2), Oui/Non + minis ⋯, historique,
   réintégration. Les règles du Non SANS l'exécution automatique.
3. **E3 — les règles du Non à la synchro** : spam / archive /
   corbeille automatiques via `pending_actions`, dites à l'historique.
4. **E4 — la Réception organisée** : sections, colonne centrée sans
   volet, écran 03 au clic, ⋯ à gauche de l'heure (spike S1 payé
   avant).
5. **E5 — Mis de côté** : table, pile, éventail, tableau, bascules.
6. **E6 — Groupes** : repli en une rangée, page de groupe, bascules.

## 7. Décisions CE — tranchées au STOP 1 le 2026-08-29

> **Réponses CE du 2026-08-29, mot pour mot** (AskUserQuestion, ordre
> posé) :
>
> - **D1** : « Oui, local seul » — jamais de déplacement IMAP.
> - **D2 (amendée)** : « Par poste, prefs SQLite » — le Rust lit
>   l'état du mode, les règles du Non s'éteignent avec lui.
> - **D3** : « **Non, tout le monde au Portier** » — la
>   recommandation (annuaire = pré-accepté) est REJETÉE : chaque
>   expéditeur, même connu de l'annuaire, doit être validé une fois.
>   Précision demandée et tranchée : « **Arrivées seules** » —
>   l'historique reste en Réception ; un expéditeur passe au Portier
>   à son PREMIER message reçu APRÈS l'activation. Conséquences :
>   pas de présomption `correspondants`, une époque d'activation à
>   stocker (`prefs`), S4 mesure le flux d'inconnus (pas le stock).
> - **D4** : « Oui, corbeille » — jamais de suppression définitive.
> - **D5** : « Oui, page servie » — préchargement des corps du
>   Kiosque borné à la page, budget mesuré S3.
> - **D6** : « Oui, globale » — la recherche traverse tout.
> - **D7** : « Oui, E1-E3 puis E4-E6 » — deux releases MINEUR.
>   **AMENDÉE le 2026-08-30 (CE)** : « Réalisons maintenant E4 et E5
>   et faisons la release à la fin » — la première MINEUR porte
>   E1-E5 ; E6 (Groupes) part avec une release suivante.
> - **D8** : « Oui, maintenant » — le chantier démarre (le bloquant
>   bêta est levé le 2026-08-29 : `feedback-wind@fcts.io` reçoit,
>   c'était la propagation DNS ; les invitations courent en
>   parallèle, côté CE). Le renommage « Mona » → « Innamoramento »
>   se fait d'abord (ids + libellés, avec migration — décision CE du
>   même STOP).
> - **D9** : « Oui, les cinq du prototype » — les dessins validés en
>   six passes entrent par la voie normale ; ajustements par STOP
>   visuel.

Le texte d'instruction d'origine des décisions :

- **D1 — routage local seul.** Jamais de déplacement IMAP : la
  destination est une présentation locale (patron `pins`/A89) ; les
  autres clients du compte voient le courrier inchangé.
  *Recommandation : oui — déplacer côté serveur ferait de Wind un
  client qui réécrit la boîte, et le retour arrière serait
  irréversible.*
- **D2 — portée ET stockage du mode** (amendée en Phase 0) : par
  POSTE (recommandé) ou par compte ; et **`prefs` SQLite** (le Rust
  lit l'état — les règles du Non s'éteignent quand le mode se
  désactive) ou `localStorage` (patron du thème, mais le cœur est
  aveugle : les règles du Non joueraient même en mode classique).
  *Recommandation : par poste, dans `prefs` SQLite.*
- **D3 — qui est « déjà connu » à l'activation** : tout expéditeur
  présent à l'annuaire `correspondants` est réputé accepté →
  Réception (patron HEY « contacts = pré-screenés ») ; seuls les
  NOUVEAUX passent au Portier. *Recommandation : oui — sinon des
  dizaines d'inconnus au premier jour (mesure S4 à l'appui).*
- **D4 — « Supprimés automatiquement » = corbeille**, jamais une
  suppression définitive. *Recommandation : oui (règle d'or).*
- **D5 — le Kiosque précharge les corps** de la page servie
  (budget mesuré S3), jamais toute la boîte. *Recommandation : oui.*
- **D6 — la recherche reste globale**, toutes destinations mélangées
  (comme aujourd'hui multi-comptes). *Recommandation : oui.*
- **D7 — l'ordre de livraison** : E1-E3 en première release MINEUR,
  E4-E6 en seconde — ou un autre découpage ?
- **D8 — la place du chantier** : après la première vague bêta
  (PLAN-BETA reste bloquant à ETAT), ou avant ?
- **D9 — les cinq glyphes** entrent au catalogue et au Système
  (relevé + gate) — valider les dessins du spike, dont le majordome.

## 8. Refus de périmètre (§2.6) — dits maintenant

- **Pas de code Speakeasy** (partage d'un code de contournement du
  Portier) — brique serveur absente, fantôme.
- **Pas de recyclage à 90 jours** (suppression automatique du
  Kiosque) — reporté ; consigner en dette si le CE le veut un jour.
- **Pas de groupage multi-expéditeurs ni par sujet** (HEY ne l'a
  pas non plus).
- **Pas de filtrage par domaine** (retiré par le CE à la passe 2 du
  prototype).
- **Pas de refonte du mode classique** : il reste le défaut, au
  pixel.

## 9. Filet de tests et gates

- **e2e neufs** (ordre de grandeur : +12 à +18 specs) : bascule et
  persistance du mode ; rétention Portier (un inconnu n'apparaît PAS
  en Réception, ses messages non comptés) ; Oui nu / Oui orienté /
  Non nu / Non avec règle ; réintégration ; « les existants
  suivent » au Déplacer vers… ; sections (un lu quitte « Nouveau pour
  vous » au retour d'écran 03) ; pile (mettre de côté = quitte la
  liste, Terminé = revient) ; groupe (n messages = 1 rangée, page de
  groupe, dégrouper) ; garde « classique inchangé ». Chaque filet
  **prouvé en le cassant** (enseignement PLAN-ESPACEMENT : trois
  tests sur cinq étaient décoratifs — viser ce que l'utilisateur
  VOIT, pas l'état interne).
- **Tests Rust** : routage (normalisation d'adresse — réutiliser
  celle d'A89, jamais une seconde), exclusions dans les requêtes,
  règles du Non transactionnelles, purges au retrait de compte.
- **Gates existantes** : contraste (objectif : AUCUNE paire neuve —
  tout en jetons existants, comme le prototype) ; cohérence (les 5
  glyphes au relevé, preuve n/n) ; garde du thread principal ;
  clippy ; fmt.
- **Banc** : les chiffres de S1/S2/S3 re-mesurés sur l'implémentation
  réelle avant le STOP 2.

## 10. Risques et invariants surveillés

- **Règles d'or** : jamais perdre de courrier (D4) ; le chemin
  d'envoi n'est pas touché.
- **Le fenêtrage est le chemin le plus chaud du produit** — S1 est le
  risque n° 1 ; si le repli des groupes au service de page coûte, la
  variante « à l'affichage » doit être bancée aussi (set-based, pas
  d'avis).
- **A43/A89** : toute mémoire nouvelle (routage, mis de côté,
  groupes) meurt avec le compte (`delete_account`, `reset_mailbox`) —
  leçon de la purge de la mémoire d'images (un UID recyclé hérite
  sinon d'une décision).
- **D-44 ouverte** (`connectes` sans cycle de rafraîchissement) : ne
  pas bâtir dessus.
- **La sélection multiple** (RETOURS-10) et **les invitations**
  doivent continuer de marcher dans les vues organisées — à couvrir
  au filet.

## 11. Documentation de fin de chantier (Phase 4)

Journal A-n par étape livrée ; **un ADR « routage local par
expéditeur »** (D1, structurant) ; relevé Système : 5 glyphes + les
patrons de vue neufs (règle-libellé, rangs du Portier, pile) ;
CHANGELOG par release (AVANT `faire-release.ps1`, §2.9 —
`gh release list` d'abord) ; ETAT réécrit ; mémoire mise à jour ;
`spikes/mode-organise/` conservé tel quel (jetable, référence de
forme).

## 12. Critères de réussite (solde)

STOP 1 : toutes les décisions D1-D9 consignées mot pour mot, datées.
Chaque étape : gate verte, filet prouvé non-vacant. STOP 2 : checklist
terrain chiffrée sur les VRAIS comptes des deux postes (activation du
mode, un vrai inconnu au Portier, un vrai reçu au Registre, une vraie
lettre au Kiosque, mise de côté, groupe sur un expéditeur bavard
réel, retour au classique sans diff) — un constat = correction le
jour même. Releases vérifiées §2.10 (18/18) et auto-update prouvé aux
deux postes. Zéro régression des 153 e2e existants.
