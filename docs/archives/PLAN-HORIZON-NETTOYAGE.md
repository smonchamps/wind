> **Historical record — French, frozen** (closed on 2026-08-30; PLAN-ENGLISH-SWITCH
> D1, debt D-58). Not translated; the living documentation is in `docs/`.

# PLAN-HORIZON-NETTOYAGE — profondeur d'historique à l'ajout d'un compte, et « Nettoyage de printemps »

> Ouvert le 2026-08-30. Deux volets demandés ensemble par le CE.
>
> **CHANTIER SOLDÉ le 2026-08-30 — terrain complet.** STOP 1 GO
> (D1-D12 tranchées le jour même), quatre STOP visuels CE validés
> (guichet, planche de glyphes, intro, tri), gate complète VERTE
> (2,9 min, e2e 177/177), terrain CE **« Tout OK » 12/12** le
> 2026-08-30, zéro constat. Commit **`f66d1e6`**, CI verte run
> **33333151630** (2 min 28 s). Retouche terrain EA2 au fil de l'eau :
> la note « serveur détecté automatiquement » retirée (retour CE au
> STOP visuel). **Non publié — partira avec RETOURS-13 dans la
> release due avant la bêta (D12)** ; l'entrée CHANGELOG s'écrira à
> la publication, datée ce jour-là (§2.9, piège du 2026-08-25).
>
> **Chiffres kaizen** : ~24 M équiv. input (session 7f4bb0d2, 2,7 h,
> 508 tours), 3 appels `gate.ps1` (2 rouges — fmt puis un
> `let_and_return` clippy, corrigés sur-le-champ — puis verte) +
> pre-push verte, **0 constat KO au STOP 2**.
> Rappel d'état : une release portant RETOURS-13 est due avant la
> première vague bêta (ETAT) — l'ordre de passage est une décision CE (D12).

---

## Volet A — choisir la profondeur d'historique importée à l'ajout d'un compte

### Constat (sur pièces, 2026-08-30)

- **Aucune borne n'existe aujourd'hui.** `SyncEngine::sync()`
  (`crates/mail-core/src/sync.rs:55-115`) liste TOUS les UID de chaque
  boîte et les rapatrie par lots de 500. Aucun paramètre de date ni de
  quota dans la signature.
- **C'est une décision gelée** : [ADR 0010](adr/0010-full-synchronization.md)
  (2026-07-25) — « l'intégralité de la boîte, tous dossiers confondus,
  sans horizon » — qui a révisé l'ADR 0007 (horizon 12 mois). Toucher à
  la profondeur d'import exige un ADR neuf qui l'amende.
- **La plomberie de bornage existe à moitié** : les pompes de
  rattrapage (`crates/mail-core/src/backfill.rs`) prennent toutes un
  `since_epoch` ; la production passe `NO_HORIZON` partout
  (6 sites dans `commands.rs`). Commentaire de `backfill.rs:39-41` :
  « un futur réglage utilisateur la retrouverait telle quelle ». Le
  gabarit de test borné existe
  (`backfill_ignores_what_lies_beyond_the_horizon`, backfill.rs:456).
- **La recherche suit la base** : index FTS5 sans contenu, maintenu aux
  points d'écriture (`search.rs`) — la portée de la recherche est
  exactement ce qui est stocké. Borner l'import borne la recherche sans
  code côté recherche.
- **L'UI d'ajout est UNE surface** : `GuichetCompte.svelte` sert
  l'accueil (Onboarding étape 1/5) ET Réglages > Comptes. Trois
  chemins : `add_account` (Google OAuth), `add_microsoft_account`,
  `add_generic_account` (IMAP). L'`account_id` n'existe qu'au retour de
  la commande — le choix doit voyager DANS la commande d'ajout.
- **Le patron « réglage par compte » est éprouvé** : table `prefs`, clé
  `xxx.{account_id}` (signature, repère de compte), purge au retrait
  via `PREFS_PAR_COMPTE`.

### Options (set-based sur pièces — pas de spike : les coûts sont connus)

- **A1 — borner tout** (enveloppes + corps) : `UID SEARCH SINCE` (ou
  filtre à la date d'enveloppe) dans `SyncEngine`. Base minimale, la
  liste des dossiers s'arrête à l'horizon. Renverse frontalement
  l'ADR 0010 ; les messages plus anciens n'existent nulle part dans
  l'app (ni liste, ni lecture, ni recherche d'objet).
- **A2 — enveloppes entières, corps bornés** : la synchro d'enveloppes
  reste intégrale (légère — quelques % du volume, l'essentiel des
  ~50 ko/message est le corps) ; l'horizon choisi remplace `NO_HORIZON`
  aux 6 sites des pompes de corps/en-têtes. Toute la correspondance
  reste listable et cherchable par expéditeur/objet ; le corps au-delà
  de l'horizon se charge à la demande à l'ouverture (chemin existant) ;
  la recherche plein-texte des corps porte sur l'horizon. ADR 0010
  amendé a minima (l'intégralité des enveloppes demeure).
- **Verdict recommandé : A2.** Réutilise la plomberie prévue pour ça,
  garde le client complet (rien ne disparaît des listes), l'économie
  disque est celle qui compte (corps), et la borne de recherche
  correspond à l'énoncé (« accessible par la fonction recherche »).
  → décision CE **D1**.

### Périmètre refusé (volet A)

- Pas de purge des corps déjà rapatriés quand on réduit l'horizon d'un
  compte existant (on n'efface pas ce qu'on a) — sauf décision CE contraire.
- Pas de quota en octets ni de garde disque nouvelle (celle d'ADR 0010 §4 reste).
- Pas d'UI de progression d'import nouvelle (l'existant `backfill_status` suffit).

### Étapes (volet A)

- **EA1 — plomberie Rust** : pref `horizon_import.{account_id}`
  (valeur = durée symbolique `1m/2m/3m/6m/1a/2a/tout`, l'epoch dérivé à
  la lecture — jamais une date figée à l'ajout) ; lecture centralisée
  qui remplace `NO_HORIZON` aux sites des pompes ; ajout à
  `PREFS_PAR_COMPTE`. TDD sur le gabarit borné existant.
- **EA2 — le choix dans le guichet** : sélecteur 7 options dans
  `GuichetCompte.svelte` (les deux peaux), valeur passée aux trois
  commandes d'ajout, pref écrite dans la même séquence que la création
  du compte. **STOP visuel précoce.**
- **EA3 — Réglages > Comptes** (si D3 = réglable après coup) :
  affichage et modification ; étendre relance le rattrapage borné à la
  nouvelle fenêtre.
- **EA4 — filet e2e, ADR neuf (amendement 0010), Système (A-n), catalogue.**

**Volet A LIVRÉ le 2026-08-30** (EA1-EA4, avant gate/commit — Phase 3 en
fin de chantier) : `horizon_epoch`/`HORIZONS_IMPORT` (backfill.rs),
prefs `horizon_import.{id}` (PREFS_PAR_COMPTE), 6 sites de pompes de
corps bornés (en-têtes de fil et destinataires HORS borne — enveloppe),
commandes `add_*` étendues (validation à la frontière avant OAuth),
`horizon_import_get/set`, sélecteur au guichet (STOP visuel CE OK ; au
passage, retour CE : la note « serveur détecté automatiquement » retirée),
porte + carte aux Réglages > Comptes. mail-core 412 → 416 ; e2e 169 → 172
(spec `horizon-import`, **prouvée en la cassant** — la 1re version du
test de persistance était vacante : `toHaveText` passait sur l'état
mémoire ; remède = remise à zéro avant rechargement, patron
expediteursImages). ADR 0029, journal A102.

---

## Volet B — « Nettoyage de printemps », 5e section du Mode organisé

### Constat (sur pièces, 2026-08-30)

- Le Mode organisé insère aujourd'hui 3 entrées de nav sous Réception
  (`Nav.svelte:41-64`) : Kiosque, Registre, Portier — puis séparateur.
  Le Nettoyage serait la 4e entrée insérée (5e section du mode avec la
  Réception organisée).
- **Le Portier fournit le vocabulaire entier** : table
  `routage_expediteurs` (destination ∈ réception/kiosque/registre/écarté,
  règle ∈ spam/archive/corbeille), porte unique `router_expediteur()`
  transactionnelle, défauts réglables du clic nu (A101), historique,
  réintégration. **Mais sa sémantique est « l'avenir seulement »**
  (D1 du Mode organisé : le routage est présentatif ; les règles du Non
  ne s'appliquent qu'aux messages arrivant APRÈS le verdict,
  `store.rs:1595-1659`).
- Le groupement par expéditeur existe au Kiosque
  (`Kiosque.svelte:126-140`, groupes repliés en pile) ; le menu ⋯ borné
  à la fenêtre est un patron partagé Portier/Kiosque/Liste.
- Une seule jauge de progression dans le code
  (`ModaleMigration.svelte:85-88` — `.jauge`/`.remplie`, 6 px).
- Glyphes : `portier/kiosque/registre` existent ; `pile`/`groupe` sont
  réservés ; **aucun glyphe « nettoyage »** — dessin neuf, régime des
  planches (A3 : jeu fermé, réservation).
- Textes CE fournis mot pour mot (sous-texte d'explication) — au
  catalogue tels quels.

### Sémantique proposée (à trancher — D5/D6/D7)

Un « groupe » = un expéditeur × ses messages dans la plage choisie.
Verdict de groupe au vocabulaire du Portier : **Oui** (garder ;
destination par défaut ou choisie au ⋯) / **Non** (écarter ; règle par
défaut ou choisie au ⋯). La différence avec le Portier : le verdict
s'applique **aussi au stock existant** de la plage — c'est ça, nettoyer.
Jamais définitif (D4 du Mode organisé : corbeille du serveur, jamais
une suppression).

### Périmètre refusé (volet B)

- Pas de « peut-être/plus tard » ni de report de groupe : oui, non, ou
  on passe (un groupe non traité reste non traité).
- Pas d'annulation globale d'une session de nettoyage (chaque verdict
  reste réversible unitairement par l'historique du Portier).
- Pas de tri à l'intérieur d'un groupe (message par message) — on
  navigue pour VOIR, le verdict reste au groupe ; le tri fin existe
  déjà ailleurs (liste, sélection multiple).
- Pas de planification/récurrence (« tous les printemps ») — un geste
  manuel.

### Étapes (volet B)

- **EB1 — glyphe + écran d'intro** : planche de glyphes « nettoyage »
  (verdict CE), entrée de nav conditionnelle au mode organisé, écran
  d'intro dans le volet de gauche (titre + glyphe, sous-texte CE mot
  pour mot, plage 3m/6m/1a/2a/5a/tout, « Démarrer le nettoyage »).
  **STOP visuel précoce.**
- **EB2 — le moteur Rust** : requête des groupes (expéditeur × plage ×
  périmètre D6, expéditeurs déjà routés selon D7), commandes Tauri
  (`nettoyage_groupes`, `nettoyage_verdict`, …), session persistée en
  base si D8 (plage, verdicts posés, groupes restants — reprise après
  redémarrage). TDD.
- **EB3 — l'écran de tri** : organisation du Portier, rangs = groupes
  (nom, nombre de messages, aperçus), Oui/Non de groupe + ⋯
  d'orientation, navigation dans un groupe (liste des messages, lecture
  seule), **barre de progression horizontale en haut** (% de groupes
  traités, patron `.jauge`).
- **EB4 — l'application des verdicts** : porte transactionnelle qui
  pose le routage (avenir, comme le Portier) ET applique la règle au
  stock de la plage selon D5 — actions en `pending_actions` dans LA
  transaction du verdict (patron E3), garde anti-doublon, jamais
  définitif.
- **EB5 — filet e2e, Système (écran + A-n), catalogue, ADR si structurant.**

**Volet B LIVRÉ le 2026-08-30** (EB1-EB5, avant gate/commit) : glyphe
`nettoyage` (« courant d'air », verdict CE D sur planche de six,
`spikes/glyphe-nettoyage/`, jeu 86 → 87) ; 5e entrée de nav (filet R12
déplacé après elle, spec mode-organise 9 → 10 dossiers) ; écran
d'intro (plage défaut 1 an + périmètre D6 défaut Réception seule) —
STOP visuel CE OK ; moteur (table `nettoyage_session` à ligne unique,
borne figée au démarrage, `boites_du_perimetre` par canoniques —
archive intégrale hors périmètre, limite dite ; groupes par
`sender_norm` hors routés/soi ; `nettoyage_verdict` transactionnel :
routage + actions du stock DANS la transaction, patron E3, corbeille
jamais définitive ; `nettoyage_messages` pour voir) ; écran de tri
(jauge en haut au dessin de la migration, rangs-groupes façon Portier,
défauts partagés D9, ⋯ d'orientation, navigation repliable, Terminer)
— STOP visuel CE OK. mail-core 416 → **419** ; e2e 172 → **177**
(spec `nettoyage`, 5 tests — dont D5 prouvé au produit : le Non fait
quitter la Réception au stock). Commandes Tauri : `nettoyage_etat /
demarrer / groupes / messages / verdict / terminer`. Journal A103.

**Limites dites** : les dates du gabarit e2e sont figées en 2020 — la
spec choisit « tout » ; l'archive intégrale (Gmail « Tous les
messages ») est hors périmètre ; le coût de la requête de groupes sur
une vraie base (200 k) n'est pas encore mesuré — à regarder au
terrain ; un message SANS en-tête Date compte dans toute plage
(précédent A98 « sans date = aujourd'hui ») — il suit aussi les règles
du stock ; un message du stock portant DÉJÀ une action en file
(mark_seen d'il y a quelques secondes) n'est ni re-journalisé ni
retiré — il reste visible, cohérent avec le serveur ; « Non →
Indésirables » sur un compte sans dossier résolu ne touche pas le
stock (dégrade comme A98).

## § Revue à regard neuf (2026-08-30, 8 angles, ~35 candidats, 10 retenues)

Corrigées le jour même : (1) l'anti-doublon du verdict sautait
l'action mais retirait la copie locale — le message serait revenu à la
relève (retrait désormais conditionné à l'action posée) ; (2) clé de
`{#each}` des messages d'un groupe sans `account_id` (collision
multi-comptes) ; (3) re-jouer l'ajout d'un compte existant écrasait
son horizon avec le défaut du sélecteur (écriture au PREMIER ajout
seul) ; (4) `qui`/`objet` des groupes pris HORS plage/périmètre — et
4 sous-requêtes corrélées par groupe (une passe bare-column-MAX,
bornée) ; (5) progression faussable (double-clic, verdicts posés au
Portier) — verrou `occupe` + jauge dérivée des groupes RESTANTS ;
(6) `remove_local` par message en autocommit — une transaction pour le
lot ; (7) le `$effect` des horizons re-tirait toutes les 10 s via
`chargerNav` (chargement à l'ouverture) ; (8) **D-47 ROUVERTE** (4e
copie du menu ⋯ — consignée, non factorisée : trois surfaces validées
au STOP visuel) ; (9) le cœur du verdict extrait en `poser_verdict`
partagé (LA porte unique tenue) ; (10) sans-date dans toute plage —
consigné en limite (précédent A98), pas corrigé. Au passage :
vocabulaires UI en UNE copie (`lib/vocabulaires.js`), filet
d'exhaustivité `horizon_epoch` × les deux vocabulaires,
`totaux_corps` en une passe, trace §9 sur l'échec de lecture
d'horizon, compte du Système re-mesuré (251 sous-chemins / 746
commandes). mail-core : **419** tests.

---

## § Décisions CE — tranchées le 2026-08-30

- **D1 (volet A)** — mécanisme de la borne.
  _Réponse CE :_ « A2 : corps bornés » — enveloppes entières, corps
  rapatriés dans l'horizon, au-delà à la demande.
- **D2 (volet A)** — valeur par défaut du sélecteur à l'ajout.
  _Réponse CE :_ « 1 an ».
- **D3 (volet A)** — modifiable après coup ?
  _Réponse CE :_ « Oui, réglable » — étendre relance le rattrapage
  borné à la nouvelle fenêtre ; réduire n'efface rien.
- **D4 (volet A)** — comptes existants.
  _Réponse CE :_ « Réputés “tout” » — sans pref, l'horizon lu est
  « tout depuis le début », rien ne change pour eux.
- **D5 (volet B)** — portée du verdict de groupe.
  _Réponse CE :_ « Stock + avenir » — le verdict pose la règle de
  routage ET l'applique aux messages existants de la plage ; jamais
  définitif.
- **D6 (volet B)** — périmètre balayé.
  _Réponse CE, mot pour mot :_ « On laisse le choix à l'utilisateur :
  Réception Seule, Réception + Dossiers, Réception + Dossiers +
  Archives, Réception + Archives. » → un second sélecteur sur l'écran
  d'intro (EB1). Envoyés, Brouillons, Corbeille, Indésirables restent
  hors périmètre dans tous les cas.
- **D7 (volet B)** — expéditeurs déjà routés.
  _Réponse CE :_ « Exclus » — un verdict rendu n'est pas re-demandé ;
  révisable via l'historique du Portier.
- **D8 (volet B)** — session persistée.
  _Réponse CE :_ « Session persistée » — plage, verdicts et
  progression en base ; reprise après redémarrage.
- **D9 (volet B)** — défauts du clic nu.
  _Réponse CE :_ « Mêmes défauts » que le Portier (Réglages > Portier,
  A101) ; le ⋯ pour déroger.
- **D10 (volet B)** — glyphe.
  _Réponse CE :_ « Planche libre » — plusieurs pistes, verdict au STOP
  visuel d'EB1.
- **D11** — ordre des volets.
  _Réponse CE :_ « A puis B » — chaque volet gate-vert avant le suivant.
- **D12** — insertion vis-à-vis de la release due.
  _Réponse CE :_ « Chantier embarque » — la release due portera
  RETOURS-13 ET ces deux volets ; la bêta attend d'autant.
