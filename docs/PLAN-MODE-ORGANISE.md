# PLAN-MODE-ORGANISE — Portier, Kiosque, Registre, Mis de côté, Groupes

> **DOSSIER D'INSTRUCTION — chantier NON OUVERT.** Préparé le
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

## 7. Décisions CE — à trancher au STOP 1

- **D1 — routage local seul.** Jamais de déplacement IMAP : la
  destination est une présentation locale (patron `pins`/A89) ; les
  autres clients du compte voient le courrier inchangé.
  *Recommandation : oui — déplacer côté serveur ferait de Wind un
  client qui réécrit la boîte, et le retour arrière serait
  irréversible.*
- **D2 — portée du mode** : préférence par POSTE (recommandé, comme
  le thème) ou par compte ?
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
