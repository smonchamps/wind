# Registre de dette

La dette assumée EN CONNAISSANCE : chaque entrée dit le fait mesuré, le
choix, et ce qui la rouvrirait. Une entrée se solde par un commit qui la
raye — jamais par l'oubli. (STANDARD §2.6 : un report = une ligne
motivée.)

## Ouverte

### D-1 · Ouverture p95 au-dessus du budget sur les très gros corps

- **Fait (2026-08-11, gate R1)** : ouverture p95 mesurée à 52–55 ms
  (budget 50 ms) sur la base réelle — p50 ~14 ms. Le dépassement est
  porté par UN corps > 1 Mo de l'échantillon déterministe ; la base en
  compte 207 (jusqu'à 28 Mo). Identique avec et sans le cycle de
  synchro : coût d'assainissement du HTML, pré-existant à la refonte.
- **Décision CE (2026-08-11)** : acté en dette, à traiter plus tard.
- **Piste** : assainissement paresseux ou streaming des très gros
  corps ; à instruire comme chantier cœur séparé.
- **Rouvre si** : le terrain rapporte des ouvertures perceptiblement
  lentes, ou si le p50 dérive.

### D-2 · Éviction LRU des pages de liste

- **Fait (P1-P2)** : la liste fenêtrée garde toutes les pages servies en
  mémoire ; sur une session très longue à grands défilements, la RAM
  monte sans redescendre.
- **Piste** : éviction LRU des pages hors fenêtre.
- **Rouvre si** : la RAM dépasse le budget (200 Mo) en usage réel.

### D-4 · Piège de focus des surimpressions

- **Amendée (PLAN-AUDIT-V2 E11, 2026-09-02)** : les Réglages ouvrent
  désormais sur leur premier contrôle (le focus entre avec le panneau,
  patron `Retour.svelte`) et les menus posent puis rendent le focus
  (`Menu.svelte`). Le piège de Tab qui SORT de la surimpression, lui,
  demeure.

- **Fait (A8)** : Tab peut sortir d'une surimpression (composition,
  réglages) vers le fond ; Échap et le focus visible couvrent
  l'essentiel.
- **Rouvre si** : le terrain au lecteur d'écran le réclame.

### D-8 · Requêtes chères des sondes périodiques (hors pompe, coût CPU réel)

> ✅ **FERMÉE le 2026-08-26 (PLAN-DEMARRAGE).** Sa clause de
> réouverture s'est réalisée : la base est passée de 1,3 à 12,8 Go et
> les 575 ms de `pending_total` étaient devenus **20 839 ms à froid**,
> tenant le verrou global 8 870 ms à chaque démarrage. Corrigé —
> `pending_total` vaut **107,9 ms**, `backfill_status` **124,9 ms** au
> terrain froid (×71). Le chiffre de 865 ms ci-dessous était **PÉRIMÉ**
> dès l'écriture de cette dette : re-mesuré à ~31 ms froid / ~11 ms
> chaud, `nav_snapshot` ayant été réécrit entre-temps. **La leçon à
> retenir n'est pas le chiffre, c'est qu'une dette porte une mesure
> DATÉE : la re-mesurer avant de s'en servir.**

- **Fait (2026-08-15, PLAN-GELS)** : `nav_snapshot` **865 ms** par
  compte Gmail (compteur Archives d'une intégrale, exclusion par
  `message_id`, 87 k lignes — toutes les 10 s) — **chiffre périmé, voir
  l'encadré** ; `pending_total` **575 ms** (COUNT par boîte, NOT EXISTS
  sur `bodies` — à chaque génération de courrier). Mesurés en SQL
  direct sur base réelle.
- **Décision CE (2026-08-15, D4 du plan)** : depuis `hors_pompe()`
  elles ne gèlent plus rien ni personne — les optimiser sans constat
  serait du travail sans mesure. Famille D-7 (chronos de réactivité).
- **Piste** : cache des compteurs de nav invalidé par génération ;
  `pending_total` en une requête agrégée.
- **Rouvre si** : le terrain désigne le coût (ventilateur, batterie,
  contention d'écriture, latence perçue des sondes).

### D-9 · L'invariant A41 n'a pas de garde structurelle

- **Fait (revue 2026-08-15, A41)** : « rien ne touche la base avant
  `migration_check` » vit dans des commentaires et un test de sonde
  (`la_langue_se_lit_sans_adopter_la_base`) — rien n'empêche une
  future commande pré-modale d'ouvrir la base en plein
  (`Store::open`) : toute la suite resterait verte, le bug se
  redécouvrirait au terrain.
- **Piste** : un drapeau `adopted` sur `MigrationShared` (ou un
  helper d'ouverture partagé des 30+ `Store::open` de `commands.rs`)
  qui fait échouer BRUYAMMENT toute ouverture pleine avant la sonde ;
  à instruire en chantier, pas en voie rapide.
- **Rouvre si** : une commande de démarrage s'ajoute avant la modale.

### D-10 · La pose différée de la langue n'a pas de test UI

- **Fait (revue 2026-08-15)** : la moitié Rust d'A41 est tenue par un
  test qui rembobine une vraie base ; l'ordre côté UI (`assurer()`
  avant `poserLangueDetectee()`, pose seulement si la sonde a
  répondu) n'est asserté par aucun e2e — `redesign-language.spec.js` ne
  joue jamais le premier lancement sur base vierge.
- **Piste** : spec e2e `vierge: true` qui asserte `prefs.lang` posée
  après démarrage, et absente si `migration_check` échoue.
- **Rouvre si** : un refactor d'`onMount` touche à l'ordre de
  démarrage.
- **Rouverte et refermée sans solde (2026-08-22, PLAN-RETOURS-8)** :
  le parcours d'accueil (A75) touche l'ordre du démarrage — vérifié
  sur pièces : la décision d'accueil vit dans `chargerNav`, APRÈS
  `assurer()` et `poserLangueDetectee()` ; l'ordre A41 est intact.
  Les e2e jouent désormais le premier lancement sur base vierge
  (parcours complet), mais l'assertion `prefs.lang` de la piste
  ci-dessus reste à écrire — la dette demeure.

### D-11 · Le banc de bascule de thème est resté calibré à 7 thèmes

- **Fait (revue 2026-08-16, PLAN-WADA-ELARGI)** : `e2e/measure-v2.mjs`
  garde 60 itérations et des commentaires « les 7 thèmes » alors
  qu'A42 en livre 28 — l'échantillon par thème tombe de ~8 à ~2, le
  chiffre « coût de bascule par thème » n'est plus comparable à la
  ligne de base historique.
- **Raison du report** : hors périmètre du chantier (fichier non
  touché par le diff), et recalibrer sans re-mesurer une ligne de base
  serait du travail sans mesure. Famille D-7 (chronos de réactivité).
- **Piste** : à la prochaine passe de mesure, recalibrer (28 × N
  itérations) et re-poser la ligne de base dans le même relevé.
- **Rouvre si** : une passe de mesure compare la bascule de thème à
  l'historique.

### D-15 · Affichage « À : destinataire » cadré sur la catégorie Envoyés

- **Fait (2026-08-16, PLAN-RETOURS-MAIL R4)** : dans la liste, la bascule
  vers « À : X » (au lieu de l'expéditeur = SOI) est gardée par
  `categorie === 'envoyes'` (`Liste.svelte`). Un envoi consulté par une
  autre voie que la catégorie Envoyés — navigation par dossier — montre
  encore l'expéditeur. Le volet de lecture, lui, est correct partout (il
  se cale sur `propre(m)`, pas sur la catégorie).
- **Raison du report** : le retour terrain visait le dossier Envoyés ;
  une détection par ligne (« ce message est de moi ») basculerait aussi
  des lignes de la Réception où NOTRE dernière réponse est en tête, un
  changement de comportement plus large que le retour.
- **Rouvre si** : le terrain consulte des envois hors de la catégorie
  Envoyés et l'expéditeur affiché gêne.

### D-16 · Rattrapage des destinataires : sonde de reliquat non indexée

- **Fait (2026-08-16, PLAN-RETOURS-MAIL)** : `backfill_recipients`
  appelle `recipients_pending_count` (scan `to_addrs IS NULL`, sans
  index) à chaque cycle, INBOX + Envoyés — même après convergence à
  zéro. Même classe que D-8 (sondes périodiques hors pompe : coût CPU
  réel, aucun gel).
- **Raison du report** : aligné sur le motif existant de la passe
  d'en-têtes (`thread_headers_pending_count`, même coût, accepté) ;
  l'optimiser sans constat serait du travail sans mesure. Famille D-8.
- **Piste** : sauter la sonde quand le passage n'a rien rapporté, ou un
  index partiel `WHERE to_addrs IS NULL`.
- **Rouvre si** : le terrain désigne le coût (avec D-8).

### D-17 · Rattrapage des corps aveugle à un bridage Gmail

- **Fait (analyse 2026-08-17)** : sur le chemin du rattrapage des corps,
  une erreur serveur Gmail (bridage : `[OVERQUOTA]`, « bandwidth
  exceeded », rejet de login / « web login required ») est **capturée
  puis jetée**. `run_backfill_all` la pousse bien dans
  `BackfillSummary.errors` (`commands.rs:4135` au connect, `:4158` en
  plein FETCH), mais la boucle UI ne lit JAMAIS `bilan.errors`
  (`App.svelte:391-417`) : elle ne regarde que `remaining` et `fetched`,
  et casse en silence sur `fetched === 0`. Symptôme au terrain :
  « Rattrapage des messages · N restants » se fige sans un mot — le pire
  cas pour diagnostiquer un bridage. Le rattrapage est le seul des trois
  à ignorer son canal d'erreurs (la synchro, elle, remonte via
  `synchroEchec`, `App.svelte:181`).
- **Second défaut couplé** : `is_connection_error` (`mail-imap`,
  `lib.rs:897`) ne reconnaît QUE nos erreurs préfixées `"connexion "`.
  Une réponse serveur Gmail tombe donc dans le `Err(_)` de
  `connect_imap` (`commands.rs:3666`) et est traitée comme « jeton
  mort » → `authenticate_silent` + reconnexion. Le garde anti-martèlement
  du commentaire (`commands.rs:3660-3664`) ne protège que le cas panne
  réseau : un bridage déclenche justement le refresh + reconnexion qu'il
  cherche à éviter.
- **Raison du report** : aucun bridage observé au terrain à ce jour — la
  méthode interdit d'optimiser contre un problème non mesuré. Mais la
  cécité, elle, est structurelle : le jour où ça mord, rien ne le dira.
- **Piste** : (1) dans la boucle UI, si `fetched === 0` et
  `bilan.errors.length > 0`, poser un avis dans la fente (mécanisme déjà
  là, `App.svelte:552-558`) au lieu de casser muet ; (2) reconnaître le
  bridage (élargir `is_connection_error` ou garde dédiée) pour qu'un
  `[OVERQUOTA]` / rejet de login ne déclenche PAS de refresh —
  « laisser respirer » le compte, pas « jeton mort ».
- **Rouvre si** : le terrain rapporte un rattrapage qui se fige sans
  explication, ou un compte Gmail bridé/verrouillé après une grosse
  synchro initiale. Commencer par (1) — rendre l'erreur visible est le
  préalable à tout diagnostic.

### D-18 · « Charger plus » : le bouton et l'append ne sont pas testés en e2e

- **Fait (2026-08-17, PLAN-CHARGER-PLUS)** : le bouton « Afficher les N
  suivants », l'append, la borne douce à 1000 lignes et l'anti-course ne
  sont pas couverts en e2e. Le bouton n'apparaît qu'au-delà de 100 résultats
  (`resultats.length < total`), or les décors du gate (Clarity, inbox à 6-10
  messages) sont bien en dessous. Seul le CŒUR est testé
  (`search_capped_pages_without_gap_or_overlap` : pages sans trou ni
  doublon) ; l'UI a été validée AU TERRAIN (base 251 k) et par revue à
  regard neuf (qui a d'ailleurs attrapé un bug d'anti-course).
- **Raison du report** : un décor e2e à >100 messages avec un terme commun
  (`seed_inbox` à grand `nombre` + `ko_par_corps ≥ 1`, dont les corps
  portent les mots de `MOTS`) est une infrastructure à part, disproportionnée
  pour ce chantier — le terrain a couvert le geste.
- **Piste** : un spec dédié sur une base `seed_inbox` large (≈ 250 messages) ;
  assertions : bouton présent, clic → la liste grandit et « N sur M » monte,
  ~1000 lignes → invite « Affinez votre recherche », terme rare → pas de
  bouton.
- **Rouvre si** : le comportement du bouton régresse au terrain, ou avant de
  retoucher à `chargerPlus`/la borne.

### D-19 · Reprise cross-appareil d'un brouillon ne restitue pas Cc/Cci

- **Fait (2026-08-17, PLAN-RETOURS-2 #4)** : Cc et Cci sont persistés
  **localement** (colonnes `drafts.cc_raw`/`bcc_raw`) — autosave, fermeture
  et reprise sur la MÊME machine les gardent, et le miroir poussé au dossier
  Brouillons Gmail (`draft_bytes`) porte les en-têtes Cc et Bcc. Mais le
  **tirage** d'un brouillon écrit sur un AUTRE appareil (`import_remote_draft`,
  `commands.rs:1449`) ne lit que `to_raw`/`subject`/`body` du message
  distant : les Cc/Cci d'un brouillon rapatrié repartent **vides**.
- **Raison du report (§2.6)** : le chemin d'analyse distant (parser l'en-tête
  Cc, décider du sort du Bcc rapatrié) est une tranche à part ; la perte ne
  touche que la reprise cross-appareil, jamais l'envoi ni la reprise locale.
- **Piste** : étendre `RemoteDraft` + `convert.rs` pour extraire Cc (et,
  décision à prendre, Bcc) du message distant, puis `import_remote_draft`
  peuple `cc_raw`/`bcc_raw`.
- **Rouvre si** : un utilisateur signale des Cc perdus en reprenant un
  brouillon commencé ailleurs.

### D-20 · Cycle Gmail : coût par cycle encore élevé quand beaucoup de vues bougent

- **Fait (2026-08-17, PLAN-RETOURS-2 #1, ADR 0021)** : la cadence à 30 min
  a réglé la FRÉQUENCE (de ~45 % du temps en synchro à ~7 %). Mais un cycle
  complet Gmail coûte encore **jusqu'à ~135 s** quand beaucoup de vues ont
  changé (mesure release : 22 dossiers relevés, ~5 s/dossier changé — bridage
  Gmail probable, cf. **D-17**). L'exclusion des vues virtuelles (Important,
  Suivis) a été **écartée** : marginale après la cadence, et coûteuse (champ
  neuf au type `Folder` du cœur, détection des drapeaux `\Important`/`\Flagged`
  dans l'adaptateur, logique voisine de l'ADR 0010). « Tous les messages »
  est délibérément CONSERVÉ (Archives, mail archivé ailleurs — ADR 0010).
- **Raison du report (§2.6)** : la cadence capte l'essentiel du gain ; le
  reste ne vaut pas la surface de code tant qu'un terrain ne le redemande pas.
- **Piste** : (1) exclure Important/Suivis par drapeau IMAP (non fragile) ;
  (2) attaquer le ~5 s/dossier (bridage — croise D-17) ; (3) LIST-STATUS
  n'aide pas Gmail (non annoncé), l'inventaire reste à ~52 STATUS.
- **Rouvre si** : le cycle à 30 min redevient gênant au terrain, ou si le
  bridage est confirmé (croise D-17).

### D-21 · Pourcentage de rattrapage : double COUNT du corpus par lot

- **Fait (2026-08-18, PLAN-RETOURS-3 R1)** : le dénominateur du `%` de
  rattrapage (A55) ajoute `corpus_total` (un COUNT du corpus par boîte) à
  côté de `pending_total`, recalculés **à chaque lot** de `backfill_bodies`
  (50 corps) pendant la boucle de rattrapage — soit deux+ balayages complets
  par lot sur ~256 k messages × ~40 boîtes. Détecté à la revue
  `/code-review high`.
- **Raison du report (§2.6)** : la sonde est **hors pompe** (ne gèle pas la
  fenêtre) et le terrain a jugé l'app **parfaitement fluide** au rattrapage
  (2026-08-18) — le budget est tenu. Optimiser sans constat serait du travail
  sans mesure. **Famille D-8** (sondes périodiques hors pompe, coût CPU réel).
- **Piste** : une seule requête par boîte rendant `(total, manquants)`
  ensemble ; ou cacher le total (quasi stable dans une boucle) et ne le
  rafraîchir qu'au changement franc.
- **Rouvre si** : le terrain désigne le coût CPU du rattrapage (avec D-8, D-16).

### D-22 · « Signaler comme spam » sur un spam atteint par la recherche

- **Fait (2026-08-18, PLAN-RETOURS-3 R2)** : `report_spam` rend `Ok(())`
  sans rien déplacer quand le message est **déjà** dans le dossier indésirable
  (`spam == mailbox`), mais l'UI flashe « signalé comme indésirable » et ferme
  le fil. Atteignable seulement via la **recherche** (un spam ouvert avec
  `categorie != 'indesirables'` montre encore le bouton « Signaler »).
- **Raison du report (§2.6)** : cas de bord (chemin recherche→spam),
  cosmétique (aucune donnée corrompue) ; le corriger proprement exigerait que
  l'UI connaisse le dossier Junk de chaque compte (le `Fil` ne l'a pas) —
  disproportionné pour la fréquence.
- **Rouvre si** : l'usage réel montre le faux succès gênant.

### D-49 · Propreté reportée de la revue PLAN-AUDIT-V1 (vague 3 de l'audit)

- **Fait (revue à regard neuf, 2026-09-02)** : neuf candidats de
  propreté vérifiés mais non retenus, la vague 1 ne corrigeant que
  des S1 : `into_inner` recopié sept fois (une aide `verrou_repris`) ;
  `hors_pompe(app, |app| auth_for(&app, id))` ×4 (`session_de`) ;
  `trace::trace` et `trace_maj` — deux writers de ligne datée, seul
  `wind.log` est borné au méga ; `is_connection_error` dupliqué
  IMAP/SMTP sur le préfixe « connexion » ; `instance::dossier_de_la_base`
  recalcule la règle de `db_path` (deux sources du chemin, sans test
  qui les lie) ; `sync_inbox`/`sync_inbox_light` toujours jumeaux ;
  `remove_local` à deux chemins (`is_autocommit`) ; `compose()` sans
  `references` (posée après coup à deux sites) ; `SEUIL_QUARANTAINE`
  gravé dans le Store ; `reply_*`/`forward_context` prennent le verrou
  trois fois (chemins rares).
- **Raison du report** : aucun n'est un défaut observable ; la vague 3
  de l'audit (`docs/AUDIT-2026-09-01.md` §5) réorganise ces fichiers.
- **Rouvre si** : un chantier touche l'un de ces sites — le corriger au
  passage, pas en refactor gratuit (§2.6).

### D-50 · Deux limites dites de la vague 1 à confirmer au terrain

- **Fait (PLAN-AUDIT-V1 E8, 2026-09-02)** : (1) le refresh token
  renouvelé par Microsoft est désormais stocké s'il change — non
  prouvé en test (le coffre ne se simule pas) ; à confirmer sur un
  compte Microsoft au-delà de 90 jours ; (2) le repli « ouvrez
  manuellement » (`BrowserFallback`) rend la main sans attendre la
  redirection — cas rare (aucun navigateur), inchangé.
- **Rouvre si** : une déconnexion silencieuse Microsoft après 90 j, ou
  un testeur sans navigateur par défaut.

### D-51 · Un compte sans CONDSTORE ne resynchronise jamais ses drapeaux

- **Fait (audit 2026-09-01 §2.1, décision CE D3 de PLAN-AUDIT-V2 le
  2026-09-02)** : sans l'annonce CONDSTORE, `changes_since` rend `None`
  et le moteur ne relit que le différentiel d'UID — un message lu au
  téléphone reste non-lu ici, à vie (`sync.rs` promettait une
  « resynchro complète » qui n'existe pas). Gmail, Microsoft 365 et
  Dovecot l'annoncent tous ; le cas est théorique en bêta.
- **Raison du report** : une fenêtre `FETCH FLAGS` par cycle coûterait à
  tous pour un serveur qu'on n'a jamais vu. Une ligne de `wind.log`
  nomme le compte sans CONDSTORE à la relève : le terrain dira si le cas
  existe.
- **Rouvre si** : la ligne apparaît chez un testeur.

### D-52 · Limites dites de la vague 2 de l'audit

- **Fait (PLAN-AUDIT-V2, 2026-09-02)** : (1) une retouche DANS le bloc
  transféré est perdue à l'envoi (le bloc est remplacé par le rendu de
  sa source avec ses images, D8) ; un transfert dont la source est un
  AUTRE compte part tel quel, au pixel neutre ; (2) la mesure « RAM
  après cinq pages de Kiosque » n'est pas jouable sur le décor e2e — le
  fenêtrage est en place, son gain se lira au terrain ; (3) `list_drafts`
  reste une liste ENTIÈRE (corps compris) sondée toutes les 10 s, hors
  de la sonde unique `etat_ui` (vague 3, avec la pagination des
  commandes) ; (4) `decode_header` parse encore un message synthétique
  par sujet — non mesuré comme coût ; (5) le test « archiver au
  raccourci depuis l'écran 03 » a flaké deux fois après le coalescement
  des resservies (E10) — passé en front montant à la revue, 79/79 depuis ;
  (6) la sonde `RFC822.SIZE` coûte un aller-retour par lot de 50 corps
  pour une borne (32 Mo) rarement atteinte — l'alternative est de
  stocker la taille à la relève des enveloppes (chantier) ; (7) `etat_ui`
  à 5 s double la cadence de la nav et des envois (la relève par le
  veilleur impose 5 s ; assumé) ; (8) `__e2ePanne` est une cinquième
  couture e2e compilée en production, sans `import.meta.env` (vague 3,
  avec les quatre autres) ; (9) le registre de la porte rapide d'E1 est
  clé par CHEMIN, pas par identité de fichier — sûr sous la mono-instance,
  non gardé par le code.
- **Rouvre si** : un testeur retouche un transfert et perd sa retouche ;
  le compteur « flaky : N » nomme deux fois le même test.

### D-53 · RAM du Kiosque : une page de lettres coûte 70 à 136 Mo, et 94 à 167 Mo restent après retour

- **Constat** (terrain STOP 2 PLAN-AUDIT-V2, 2026-09-02) : 249 Mo de
  working set privé sur 6 processus WebView2 après dix pages de Kiosque
  sur le poste du CE — budget STANDARD §3 « < 200 Mo » (repos 95,5 Mo).
  Banc `e2e/tests/bench-ram-feed.spec.js` (200 lettres de 100 Ko,
  build debug) : fenêtre 12 → +136 Mo à la première page, +217 à
  160 cartes, +167 RETENUS au retour en Réception ; fenêtre 1 → +70,
  +96, +94, stables à +25 s. La largeur de fenêtre (E10) borne, elle ne
  guérit pas : une iframe `srcdoc` de lettre vaut des dizaines de Mo, et
  les documents démontés ne rendent pas leur mémoire (`corpsAuto` et
  `brancherLiens` nettoient — la rétention est ailleurs : documents des
  iframes retirées, heap du rendu qui ne rétrécit pas ?).
- **Passe 2 (fenêtre 5, poste du CE)** : 251,5 Mo sur 6 processus —
  **GPU 132,3 Mo**, rendu 69,6, gestionnaire 36,3, réseau 8,1, stockage
  3,2, crashpad 1,8. La fenêtre ne change rien au total : le processus
  GPU porte plus de la moitié (surfaces composées des iframes et de
  leurs images accordées), le DOM n'est pas le levier.
- **Raison du report** : décision CE D9 sur la fenêtre (réglage
  immédiat, tenu — ne coûte rien) ; la racine — une iframe par carte — est une question de
  conception (une seule iframe pour la carte lue ? cartes repliées par
  défaut ? rendu sans iframe ?) : un chantier set-based, pas un
  réglage. Le budget lui-même est à préciser : « working set privé »
  au repos, ou après le geste le plus lourd du produit ?
- **Piste** : profil mémoire du renderer WebView2 (DevTools, snapshot
  avant/après démontage) pour nommer ce qui retient ; puis options
  mesurées au banc.
- **Rouvre si** : le budget précisé est dépassé sur le poste du CE après
  D9, ou un gel apparaît au défilement du Kiosque.

### D-54 · `multi-select:173` (« le raccourci e archive le LOT coché ») flake une gate sur trois

- **Constat** (2026-09-02, gates de PLAN-AUDIT-V2) : passé au second
  essai dans trois gates sur six (D4 : compté, jamais rouge). Le geste
  : plusieurs rangées cochées, `e` au clavier, un seul toast et les
  fils partent. Sur cette machine seulement jusqu'ici (la CI est verte
  à chaque fois) — STANDARD §7.5, la CI est la référence.
- **Raison du report** : hors périmètre du terrain du jour ; un flaky
  qui passe au second essai ne bloque pas, mais trois occurrences le
  jour même sortent du bruit.
- **Piste** : rejouer la spec seule vingt fois (`--repeat-each`), lire
  la trace du premier échec — course entre la coche et le raccourci
  (focus laissé sur la case, A38) ou entre le toast et l'assertion.
- **Rouvre si** : une quatrième occurrence, ou un rouge en CI.

## Soldée

### ~~D-36 · La colonne fantôme de `echos` naît sur toute base neuve~~ — soldée le 2026-09-01

- **Fait (PLAN-DEMARRAGE, 2026-08-26)** : le littéral `SCHEMA` de
  `store.rs` contient un `antislash-n` **à l'intérieur d'un commentaire SQL
  `--`** d'une chaîne Rust ordinaire. Rust en fait un vrai saut de
  ligne : le commentaire s'arrête là, et SQLite avale la suite comme
  une **colonne**. Reproduit sur base fraîche — une colonne nommée
  `) — la liste d` dont le type absorbe la déclaration de `to_addrs`.
  La vraie `to_addrs` n'existe que parce qu'`add_missing_columns` la
  rajoute plus tard. Les bases du parc sont saines (créées avant ce
  commentaire) ; **toute base neuve ne l'est pas**.
- **Raison du report** : ce n'est pas un défaut de performance, et
  retirer une colonne d'une base existante demande une réécriture de
  table. Hors périmètre d'un chantier de démarrage (refus §2.6).
- **Piste** : corriger le littéral, plus un test asserant les noms de
  colonnes de `echos` sur une base NEUVE — c'est lui qui manque, et son
  absence est la vraie cause.
- **Rouvre si** : une base neuve montre un défaut lié à `to_addrs`, ou
  au premier chantier qui réécrit `echos`.
- **Soldée à la vague 0 de l'[audit du 2026-09-01](AUDIT-2026-09-01.md)**
  (S1-11) : le littéral corrigé (« jointes par un saut de ligne », plus
  aucune séquence d'échappement dans un commentaire SQL) ET le filet qui
  manquait — `une_base_neuve_n_a_aucune_colonne_fantome` : chaque
  colonne de chaque table d'une base neuve porte un nom sain. Prouvé en
  le cassant : RED sur la base d'avant (« ) — la liste d »), puis RED de
  nouveau quand la correction elle-même a réintroduit un `antislash-n` dans le
  commentaire explicatif — le filet a attrapé sa propre correction.
  Les cinq bases de la vague bêta 1 (installées avant) portent la
  colonne fantôme ; inoffensive (`to_addrs` existe par
  `add_missing_columns`), elle ne se retire qu'en réécrivant `echos`.

### ~~D-6 · Flake e2e v1 : « étoiler » (parcours-critiques)~~ — soldée le 2026-08-15

- **Fait (2026-08-13)** : flake une passe sur trois du test v1
  « étoiler », chemin étranger aux chantiers en cours ; consigné, non
  instruit — v1 dormante.
- **Soldée à B2** (PLAN-RETRAIT-V1) : la spec `parcours-critiques` est
  retirée avec l'interface v1, le flake meurt avec elle. Rouvrirait si
  un symptôme équivalent touchait un parcours v2.

### ~~D-3 · Dates en jour de semaine (2 à 6 jours)~~ — soldée le 2026-08-12

- **Fait (P3)** : le prototype affiche « Lundi, 18:20 » pour les
  messages de la semaine ; `quand()` affichait « 8 août ». Écart visuel
  mineur, dit à la livraison P3.
- **Soldée à E2 des Réglages** (R-D1, PLAN-REGLAGES) : `quand()` étendu
  — 2 à 6 jours → jour de semaine, `quandLong()` compose « Lundi,
  18:20 » sans retouche. Sans réglage : la forme du prototype ne
  s'opte pas.

### ~~D-5 · Charset amont (U+FFFD dans les corps stockés)~~ — soldée le 2026-08-11

- **Fait** : des corps de la base réelle portaient U+FFFD dès le HTML
  stocké — décodage du charset MIME à la synchronisation.
- **Soldée par `0f7f059`** (session séparée, PR #1 fusionnée) : feature
  `full_encoding` de mail-parser (gb2312…), repli windows-1252 quand
  les octets ne sont pas de l'UTF-8 valide, et réparation une-fois des
  corps mutilés (purge marquée, retéléchargement au rattrapage, aperçu
  et index refaits au passage).

### D-12 · Ouverture du fil : cascade `thread_messages` → corps, série P1 à re-baser

- **Fait (UI v3, revue du 2026-08-16)** : la sélection ouvre désormais
  le FIL — `thread_messages` puis le corps du dernier, en série (deux
  allers-retours là où v2 n'en faisait qu'un), et le chrono P1
  « ouverture » a changé de définition (sélection → fil affiché,
  pièces exclues). La série historique (< 50 ms, ADR 0015) n'est plus
  comparable.
- **Report assumé** : paralléliser le corps de la ligne de tête avec la
  liste du fil (un fetch perdu dans le cas rare d'une tête plus
  fraîche), et re-baser le banc `measure-v2` sur la nouvelle définition.
  Reporté pour garder le commit v3 sur les verdicts CE ; à instruire
  avec D-7/D-11 (famille bancs).

### D-13 · Agrandir/réduire remonte les iframes du fil

- **Fait (revue v3)** : le changement de cadre démonte puis remonte les
  iframes `srcdoc` des messages dépliés — le réseau ne rejoue rien
  (état partagé), mais le rendu re-parse chaque document et perd le
  défilement interne. Sensible sur un long fil « tout déplié ».
- **Report assumé** : garder les deux cadres montés (`display:none` +
  `inert`) coûterait des testids dupliqués — exactement ce que la
  revue v3 a corrigé ; le remède demande de re-scoper la suite e2e
  d'abord. À instruire si le terrain le sent.

### D-14 · Re-baser le banc P1 sur la géométrie A44

- **Fait (PLAN-RETOURS-V3, 2026-08-16)** : deux changements de
  géométrie dans le même chantier — les barres overlay rendent ~10 px
  de largeur à chaque volet défilant (0 px réservé contre 10 px
  webkit), et la liste est à deux gabarits (h1 nue / h2 porteuse,
  ~+27 px par ligne à puces) : `visibles`, le nombre de lignes rendues
  par saut et le coût d'un reflow ont bougé. Les percentiles « page »
  du banc `measure-v2` ne sont plus comparables à la série d'avant A44.
- **Report assumé** : re-mesurer et re-baser les budgets P1 sur la
  géométrie livrée, en une passe dédiée — à instruire avec la famille
  des bancs (D-7/D-11/D-12), pour ne pas mélanger une re-base de
  budgets avec un chantier de features. Les bancs mesurent DÉJÀ la
  bonne géométrie (browser-args.mjs) ; seule la série de référence
  date.

### D-23 · Téléchargement d'une pièce : chemin réseau non couvert en e2e

- **Fait (PLAN-RETOURS-4, R1, 2026-08-18)** : le nouveau geste
  « Enregistrer sous » (clic puce → `chemin_enregistrement_suggere` →
  dialogue `plugin:dialog|save` → `save_attachment(dest)`) a sa couture
  e2e (`__e2eDestination`) mais aucun test ne l'exerce. Le succès exige
  le rapatriement IMAP des octets — terrain seul par construction
  (§7.5) ; seul le chemin d'annulation (`!dest → return`, ni toast ni
  fetch) est jouable hors ligne.
- **Report assumé** (code-review high, écart assumé) : R1 est validé au
  terrain (CE, 2026-08-18). Ajouter un e2e du seul chemin d'annulation
  apporte peu ; le chemin de succès restera terrain. À instruire si un
  banc de composition/pièces se monte.

### D-24 · Stub PASSATION.md à retirer

- **Fait (PLAN-DOCUMENTATION, 2026-08-19, décision CE D3)** :
  PASSATION.md est scindée en STANDARD.md (le standard de travail) et
  ETAT.md (l'instantané de relève) ; un stub de quelques lignes reste
  au chemin historique — poka-yoke pour les vieilles mémoires et
  l'ancien rituel de reprise.
- **Condition de retrait** : deux reprises à froid consécutives sans
  que rien ne trébuche sur l'ancien chemin ; alors le stub se supprime
  (commit `docs:`) et cette entrée se raye.
- **Avancement** : première reprise comptée le 2026-08-19 (E4 du
  chantier — reprise ordinaire propre, et le stub a rattrapé l'ancien
  rituel collé volontairement). Une reprise propre de plus et il tombe.

### D-25 · Composeur riche : quatre écarts assumés de la revue

- **Fait (PLAN-COMPOSITION-HTML, revue du 2026-08-20)** : quatre écarts
  relevés par la revue à regard neuf, assumés sans correctif —
  1. le miroir JS `texteEnHtml` (Composition.svelte) duplique
     `mail_core::texte_en_html` (8 lignes, documenté des deux côtés) :
     le supprimer exigerait de servir la conversion par le Rust, ce qui
     re-créerait le churn texte→riche que la garde anti-churn vient de
     tuer ; une divergence d'échappement resterait invisible aux tests ;
  2. `mail_smtp::draft_bytes` porte 8 arguments positionnels dont six
     `&str` consécutifs (allow clippy posé) : une inversion Cc/Cci à
     l'appel compilerait — le chemin le moins testé (reflet Brouillons) ;
  3. `DraftContent` n'a pas de `Default` : le prochain champ retouche
     ~45 littéraux de test mécaniquement ;
  4. le triptyque e2e de vidage d'un contenteditable (clic + Ctrl+A +
     Suppr) est recopié trois fois dans les specs.
- **Condition de reprise** : au prochain chantier qui touche les
  brouillons ou l'envoi, régler 2 et 3 (grouper les paramètres, dériver
  `Default`) ; 1 se rouvre seulement si un troisième convertisseur
  apparaît ; 4 au prochain spec qui en aurait besoin (helper partagé).

### D-26 · Pagination profonde des catégories : coût O(offset) assumé

- **Fait (PLAN-DEFILEMENT-PROFOND, 2026-08-20, décision CE D1)** : hors
  réception, `category_page` paie `LIMIT offset+limit` par boîte + tri
  fusionné — la page de 200 coûte 10 ms à l'offset 0, 66 ms à 10 000,
  157 ms à 40 000, **247 ms à 80 000** (SQL brut, base seedée 120 000,
  release). Le budget « page de liste < 100 ms » (STANDARD §3) est
  crevé dès ~20 000 ; la réception, elle, tient (patron `threads` +
  `idx_threads_date_globale`, 14,6 ms à l'offset 200 000). La clause
  d'exclusion de l'intégrale Gmail coûte peu (index partiel
  `idx_envelopes_message`).
- **Pourquoi assumé** : depuis A64, une seule page profonde vole à la
  fois (file bornée, VOL_MAX = 1) et l'écran dit le chargement — la
  latence d'UNE page isolée est vivable ; c'était la rafale ×
  sérialisation qui faisait la panne de plusieurs minutes. Resserre du
  terrain (2026-08-20) : le comptage (`category_totals`, sonde
  NOT EXISTS par ligne d'intégrale, ~240 ms sur 200 k) ne se paie plus
  qu'à la page 0 — la page profonde nue passe de 368 à ~129 ms sur le
  décor intégrale.
- **Condition de reprise** : si le terrain mesure une page profonde
  au-delà de ~1 s sur la vraie base (256 k, 4 comptes), ou si un
  chantier « liste sans limite » s'ouvre (le report existant de la
  recherche virtualisée) — le patron deux-temps de la recherche (A51)
  est le point de départ.

### D-27 · La boîte d'envoi ne retente qu'en fin de cycle ou au geste

- **Fait (terrain PLAN-RETOURS-5, 2026-08-21)** : au premier lancement,
  deux envois cliqués juste après l'ouverture sont restés « en
  attente » pendant toute la session — la vidange déclenchée par le
  clic « Envoyer » est passée avant que les sessions des comptes ne
  soient prêtes, et la boîte d'envoi n'a AUCUNE retentative propre :
  elle ne repart qu'à la fin d'un cycle complet (long sur la vraie
  base), à la passe légère du clic « Synchroniser », ou au retour
  réseau. Les messages sont partis à la première vidange réellement
  déclenchée (clic Synchroniser de la session suivante) — jamais
  perdus, jamais doublés : les règles d'or ont tenu.
- **Pourquoi assumé** : le cas ne se présente qu'à l'envoi dans les
  toutes premières secondes d'un lancement ; en régime établi, la
  vidange d'après-envoi part et aboutit (vérifié au même terrain).
  La barre d'état dit honnêtement « N envois en attente ».
- **Condition de reprise** : si le terrain revoit un envoi en attente
  au-delà d'un cycle, ou au premier retour utilisateur en bêta.
  Piste : une retentative bornée déclenchée à l'établissement des
  sessions (le déclencheur du retour en ligne, R-D3, existe déjà —
  l'y raccorder), jamais un minuteur de martèlement.

### D-28 · L'épingle est portée par la seule enveloppe du geste

- **Fait (revue PLAN-RETOURS-7, 2026-08-21)** : épingler enregistre UNE
  clé `(mailbox_id, uid)` — celle de la tête du fil au moment du geste.
  Le fil se retrouve par jointure (`PINNED_THREADS`), et désépingler
  libère le fil entier ; mais si CE message précis quitte sa boîte
  (suppression par un autre client, politique de rétention, déplacement
  partiel du fil), la conversation se désépingle silencieusement et la
  ligne `pins` orpheline reste en base (pas de FK sur `envelopes` —
  seule la suppression de la BOÎTE cascade). Un UID réutilisé après un
  reset d'UIDVALIDITY pourrait épingler un message étranger.
- **Pourquoi assumé** : le cas exige qu'un tiers supprime exactement le
  message-clé pendant que le reste du fil demeure en Réception — rare ;
  la jointure fait qu'une épingle orpheline n'est jamais SERVIE (aucun
  affichage faux, juste une perte d'épingle et une ligne morte) ; et
  re-épingler est un clic. Le remède complet (cle par `message_id` de
  compte, ou re-ancrage à la synchro) est un chantier de robustesse
  disproportionné pour une v1 locale.
- **Condition de reprise** : si le terrain (ou la bêta) rapporte des
  épingles qui « sautent », ou au premier reset d'UIDVALIDITY observé.
  Piste : ré-ancrer l'épingle sur la tête courante du fil à chaque
  service (`pinned_rows` sait le faire), et balayer les orphelines à la
  vidange.

### D-29 · Un message dont la racine EST le calendrier a un corps vide définitif

- **Fait (revue PLAN-INVITATIONS, 2026-08-22)** : un message sans
  partie texte/HTML dont la racine est `text/calendar` (cas C du
  constat) est désormais AFFICHABLE — corps vide, la carte d'invitation
  est le contenu. Avant, il tombait en « message introuvable » et
  restait éternellement candidat au rattrapage. Contrepartie : le corps
  `""` est mis en cache (`scanned = 1`) — la recherche plein-texte ne
  voit pas les mots du titre de la réunion, et TRANSFÉRER ce message
  produit une citation vide (l'ICS ne suit pas), sans avertissement.
- **Pourquoi assumé** : la carte affiche l'essentiel (titre, horaire,
  lieu, organisateur) ; l'ancien comportement (erreur sèche) était pire
  des deux côtés ; la forme est rare (Google/Outlook émettent en
  multipart/alternative avec un HTML).
- **Condition de reprise** : si le terrain ou la bêta transfère des
  invitations nues ou les cherche par titre. Pistes : indexer le titre
  de l'invitation dans FTS au `save_body_full` ; joindre l'ICS au
  transfert.

### D-30 · Une invitation héritée SANS ligne de pièce calendrier n'a pas de carte

- **Fait (revue PLAN-INVITATIONS, 2026-08-22)** : l'adoption de
  l'existant passe par la réparation `pieces-calendrier` (les corps des
  messages ayant une ligne `attachments` calendrier sont relus). Un
  message scanné AVANT la fonctionnalité dont la partie calendrier
  n'avait PAS été classée en pièce par mail-parser (ex. disposition
  `inline` exotique) est invisible du critère : sa carte ne naîtra qu'à
  une relecture fortuite (reset d'UIDVALIDITY, corps re-fetché).
- **Pourquoi assumé** : forme rare (les producteurs majeurs passent en
  multipart/alternative, classée en pièce), et le seul critère local
  possible serait de relire TOUS les corps — le contraire d'une
  réparation ciblée.
- **Condition de reprise** : un constat terrain « ce vieux message est
  une invitation sans carte ». Piste : élargir la réparation aux
  messages dont le CORPS contient un marqueur BEGIN:VCALENDAR (critère
  SQL LIKE, une passe).

### D-31 · `drafts` ne porte pas `ics_reply` — l'aller-retour brouillon le perdrait

- **Fait (revue PLAN-INVITATIONS, 2026-08-22)** : `Draft.ics_reply` et
  `outbox.ics_reply` existent, `drafts` non. Aujourd'hui inatteignable :
  une réponse d'invitation n'est jamais programmée (l'annulation d'un
  envoi programmé est le seul chemin outbox → brouillon) ni éditée au
  composeur. Mais le contrat « le brouillon recréé est COMPLET »
  (annuler_envoi_programme) est faux pour ce champ.
- **Pourquoi assumé** : le chemin n'existe pas ; ajouter une colonne
  morte serait pire.
- **Condition de reprise** : le jour où une réponse d'invitation
  devient programmable ou éditable au composeur — la colonne `drafts`
  et sa copie dans les deux sens font partie du même chantier.

### D-32 · La gate vit en DEUX encodages — pre-push (sh) et gate.ps1 (PowerShell)

- **Fait (revue PLAN-KAIZEN-CLAUDE vague 2, 2026-08-23)** : les 9
  étapes existent en sh dans `.githooks/pre-push` (avec le chemin
  rapide docs-only) et en PowerShell dans `scripts/gate.ps1` (sans ce
  chemin — voulu : la gate avant commit est toujours entière). Toute
  étape ajoutée ou modifiée doit l'être deux fois, sans garde-fou.
- **Pourquoi assumé** : les deux maisons ont des besoins différents
  (le hook redirige les étapes muettes, le script montre tout ; le
  hook porte le chemin docs-only, le script jamais) ; unifier
  aujourd'hui coûterait plus que le risque couru.
- **Condition de reprise** : la première divergence CONSTATÉE entre
  les deux verdicts, ou l'ajout d'une 10e étape.

### D-33 · Le dist périmé n'est corrigé qu'en JS — `build.rs` n'a pas de `rerun-if-changed`

- **Fait (revue PLAN-KAIZEN-CLAUDE vague 2, 2026-08-23)** : le piège
  « generate_context! n'embarque le dist qu'à la compilation de
  main.rs » est tenu par `e2e/rebuild-v2.mjs` (empreinte + bump) et
  `scripts/build-wind.mjs` — mais un `cargo build` nu, hors de
  ces deux portes, reste exposé.
- **Pourquoi assumé** : ajouter la dépendance dans
  `apps/desktop/build.rs` toucherait le code produit avec une
  sémantique tauri_build à prouver (re-run du build script n'implique
  pas recompilation de main.rs) — hors périmètre de l'outillage.
- **Condition de reprise** : un dist périmé constaté HORS des deux
  portes (release ou terrain), ou un chantier qui touche build.rs.

### D-34 · Le patron « pref par compte » se duplique à chaque table (loaders, commandes, script de release)

- **Fait (revue PLAN-RETOURS-9, 2026-08-23)** : `chargerNoms`/
  `patcherNom` (App.svelte) et `noms_get` (commands.rs) sont des
  clones structurels du duo repères — deuxième occurrence ; chaque
  table paie en outre SON `Store::open` sur la file sérialisée au
  démarrage (~qq ms). Et la table `$oauth` de `make-release.ps1`
  duplique les `option_env!` de `provider.rs` (commentaires croisés
  posés des deux côtés, mais aucun garde-fou).
- **Pourquoi assumé** : à deux occurrences la factorisation coûterait
  plus que la duplication (leçon des patrons du dépôt) ; le coût
  démarrage est négligeable mesuré à l'échelle du poste.
- **Condition de reprise** : la TROISIÈME pref par compte (fusionner
  en `identites_get` + loader générique), ou l'ajout d'un fournisseur
  OAuth (vérifier la table du script AVANT sa première release).

### D-35 · Le palier 16 des icônes n'est pas dessiné (dette V9)

- **Fait (PLAN-ELEMENTS, 2026-08-24)** : les 78 glyphes livrés sont
  les maîtres 24 réduits — 37 % seulement des coordonnées survivent au
  passage 24 → 16 (multiples de 3), le trait vaut 1,33 px à 16 px et
  0,83 px à 10 px (repères de compte, sous le palier 16 lui-même).
  Chiffrage du Système (V9) : 74 paliers 16 + 12 paliers 10-12 à
  dessiner, calés rectangle par rectangle — 86 dessins.
- **Décision CE (D4, 2026-08-24)** : livrer les maîtres réduits ; la
  netteté jugée suffisante au STOP visuel E2 sur le rendu réel
  (anticrénelage, écran de travail).
- **Rouvre si** : le terrain ou la bêta voit le flou à 16 px ou sur
  les repères 10-12 px — alors chantier de dessin dédié, glyphe par
  glyphe (`format_list_numbered` plaide le premier).

### D-37 · `sync_progress` recompte toutes les boîtes, toutes les 5 s, à vie

- **Fait (PLAN-DEMARRAGE, 2026-08-26)** : `store.sync_progress()` est un
  `SUM` de `COUNT` corrélés sur toutes les boîtes — **152 ms à froid**,
  8,6 ms à chaud, rejoué **toutes les 5 secondes pour toujours**, sous
  le verrou global. Sur les 60 premières secondes d'un démarrage, ~1,8 s
  de verrou.
- **Raison du report (décision CE D6, 2026-08-26)** : son correctif est
  un compteur tenu à l'écriture — même famille et même risque de dérive
  que le compteur des corps manquants, pour ~1,8 s contre les ~26 s du
  défaut principal. Un compteur qui ment est pire qu'un compte lent.
- **Piste** : compteur par boîte maintenu à l'écriture, ou une seule
  requête agrégée au lieu de la boucle (le patron mesuré à E1-bis :
  c'est l'index qui portait le coût, pas les allers-retours).
- **Rouvre si** : le terrain désigne le coût — ventilateur, batterie,
  ou latence perçue des sondes au repos.

### D-38 · Le rattrapage des aperçus recharge la liste même quand il n'a rien fait

- **Fait (contre-expertise PLAN-DEMARRAGE, 2026-08-26)** :
  `rattraperApercus` (`App.svelte`) appelle `liste?.recharger()`
  **inconditionnellement**, hors de sa boucle — donc **même quand
  `restants === 0` au premier appel**, ce qui est le cas de toute base à
  jour. Or `recharger()` incrémente la génération, relance `pomper()` et
  `lancerEpingles()` : une page `list_category`, un `pinned_rows` et un
  `category_total` de plus, à t + 1,5 s, pour rien.
- **Raison du report** : trouvé pendant E2, hors du périmètre réduit
  tranché par le CE (le `tick` seul). Deux lignes, mais elles changent
  un comportement de recharge qui mérite son propre filet.
- **Piste** : capturer `const aFaire = restants > 0;` au premier appel
  et ne recharger que si `aFaire`. Filet : compter les `list_category`
  dans `__e2eJournal` après le palier sur un décor sans aperçu manquant
  — la valeur attendue est zéro.
- **Rouvre si** : le prochain chantier touche le rattrapage, ou si le
  banc voit ces trois commandes dans le budget.

### D-39 · La signature Authenticode est gelée — l'installation sur poste SAC reste une loterie

- **Fait (spike `spikes/maj-x64/`, relevés CE des 2026-08-26/27)** :
  Smart App Control (`On` par défaut sur les Windows 11 récents) juge
  les exe non signés Authenticode **binaire par binaire** (verdict
  cloud par hash) : la 0.10.0 se lance, la 0.10.1 est refusée, sur le
  même poste. Toute release non signée peut être bloquée chez tout
  utilisateur SAC — et le verdict peut changer avec le temps.
- **Raison du report (décision CE D2, 2026-08-27)** : E1 a échoué — la
  validation d'identité individuelle Azure Trusted Signing est fermée
  hors USA/Canada (adresse CE en France). Replis chiffrés au
  PLAN-SIGNATURE §2 (Certum open source ~69 €/an, OV cloud
  ~200-400 $/an) : « attendre + filet seul » tranché. Le compte Azure
  `rg-fcts` est à supprimer (le Basic facture 9,99 $/mois).
- **Piste** : guetter la réouverture individuelle Trusted Signing
  (ou Certum si l'attente pèse) ; E2/E3 du PLAN-SIGNATURE sont écrites
  et GELÉES — outillage poste, `signCommand` injecté par
  `make-release.ps1` seulement, contrôle Authenticode ajouté à
  `verify-release.ps1` (18 → 20).
- **Relevés de la bêta (registre PLAN-BETA §3 bis)** — la mesure qui
  rouvrira ce chantier ; un verdict par ligne, poste/version/date,
  sans identité :
  - 2026-08-31, **T1, x64, poste SAC `On` (hors CE) : la 0.15.0
    s'installe** — premier verdict FAVORABLE relevé hors du poste de
    développement. Il ne referme rien (le verdict est rendu par hash,
    donc muet sur la version suivante), mais il donne le **banc** qui
    manquait à la mesure due du filet PLAN-SIGNATURE : le jour où ce
    poste refusera une MAJ, l'échec doit être VISIBLE.
- **Rouvre si** : un retour bêta bute sur SAC, ou la porte Azure
  rouvre, ou le lancement public approche (ADR 0013 le lie au public).

### D-40 · L'issue amont tauri-plugin-updater — OUVERTE le 2026-08-27 (GO CE)

> **SOLDÉE en tant qu'action** :
> https://github.com/tauri-apps/plugins-workspace/issues/3555
> (titre : « updater: ShellExecuteW result is never checked on
> Windows — app exits silently when the installer fails to launch »).
> Ce qui reste vivant est la VEILLE : au prochain bump du crate
> (épinglé `=2.10.1`), vérifier si l'amont a corrigé — le contournement
> local (PLAN-SIGNATURE E4) devient alors candidat au retrait.

- **Fait (sources 2.10.1, `updater.rs:854-865`)** : le retour de
  `ShellExecuteW` n'est jamais testé et le processus sort par
  `exit(0)` — tout refus de Windows ferme l'application hôte sans un
  mot. Wind est protégé par son propre lancement (PLAN-SIGNATURE E4),
  le reste de l'écosystème Tauri ne l'est pas.
- **Raison du report** : action sortante (publier sous le compte
  GitHub du CE) — brouillon à faire valider au STOP 2 du chantier.
- **Piste** : issue courte avec les lignes en cause et le remède
  (tester le retour > 32, sinon rendre l'erreur au lieu de quitter).
- **Rouvre si** : au prochain bump du crate (épinglé `=2.10.1`) — si
  l'amont a corrigé, le contournement local devient candidat au retrait.

### D-41 · La coche de sélection multiple n'a pas de geste clavier dédié

- **Fait (PLAN-RETOURS-10, 2026-08-27)** : la sélection multiple de la
  liste est un geste de pointeur — Ctrl-clic, Shift-clic, case au
  survol. Au clavier, e/Suppr s'appliquent bien au lot coché, mais
  RIEN ne permet de COCHER sans souris (pas de Ctrl+Espace, pas de
  Shift+Flèches).
- **Raison du report** : hors périmètre du chantier (§2.6) — l'énoncé
  CE visait les trois gestes de pointeur, et un vocabulaire clavier de
  sélection multiple mérite sa propre conception (interaction avec le
  triage e/Suppr d'A38 et l'anneau :focus-visible sur nœuds recyclés).
- **Rouvre si** : le terrain ou la bêta le demande — alors concevoir
  le vocabulaire complet (cocher, étendre, tout vider) en une passe.

### D-42 · La mémoire d'images PAR MESSAGE n'a pas de porte de sortie

- **Fait (PLAN-RETOURS-11, revue à regard neuf du 2026-08-28)** : le
  choix « Afficher les images » d'un message s'écrit en base
  (`images_messages`, clé d'enveloppe) mais ne se liste ni ne se
  révoque nulle part — la liste des Réglages (D4) ne couvre que les
  règles d'expéditeur (`images_expediteurs`). Un clic malencontreux
  sur un message suspect recharge son pixel distant à chaque
  réouverture, sans moyen visible de re-bloquer.
- **Raison du report** : périmètre assumé du chantier — la décision
  CE D4 n'a tranché la révocation que pour les expéditeurs ; une
  porte de sortie par message demande sa propre forme (où vivrait le
  geste ? un bandeau inversé ?).
- **Borne** : l'accord meurt avec sa boîte (CASCADE), au retrait
  local du message (`remove_local`) et au changement d'UIDVALIDITY
  (`reset_mailbox`, purge prouvée par test) — jamais d'héritage par
  un UID recyclé.
- **Rouvre si** : le terrain ou la bêta rapporte un « je veux
  re-bloquer les images de CE message ».

### D-43 · L'écho local n'a pas de colonne Cc — l'entête change à la réconciliation

- **Fait (PLAN-RETOURS-12, revue à regard neuf du 2026-08-28)** : la
  table `echos` ne copie que `outbox.recipients` (les À) alors
  qu'`outbox.cc_addrs` existe ; la ligne « Cc : … » de l'entête A92
  n'apparaît donc jamais pendant la fenêtre d'écho. Un envoi avec Cc
  ouvert aussitôt dans Envoyés montre « À : … » seul, puis gagne sa
  ligne Cc quand l'enveloppe serveur remplace l'écho — deux entêtes
  pour le même message selon le moment.
- **Raison du report** : colonne + migration + recopie pour une
  fenêtre de quelques secondes en usage normal ; le filet RETOURS-5
  (« l'écho dit ses destinataires ») reste vrai pour les À.
- **Rouvre si** : le terrain rapporte l'entête qui « change tout
  seul », ou si un chantier touche déjà le schéma des échos.

### D-44 · `connectes` n'est rafraîchi par aucun cycle — un jeton révoqué Wind ouvert dit « Connecté »

- **Fait (PLAN-RETOURS-12, revue à regard neuf du 2026-08-28)** : le
  tableau `connectes` (App.svelte) n'est réhydraté qu'aux gestes —
  démarrage, « Reconnecter », et désormais l'ajout (R1). Aucun cycle
  (synchro 30 min, relève 5 min, ouverture des Réglages) ne le remet
  d'équerre, et `accounts_failed` du bilan de synchro n'y est jamais
  reflété : un jeton OAuth révoqué pendant que Wind tourne laisse
  Réglages > Comptes dire « Connecté » jusqu'au redémarrage — le
  symptôme miroir de R1.
- **Raison du report** : hors périmètre du retour R1 (corrigé au
  geste, tous les chemins d'ajout couverts) ; le bon niveau est un
  rafraîchissement par le cycle ou un état dérivé du bilan du cœur —
  sa propre forme à instruire.
- **Rouvre si** : un constat terrain « déconnecté qui se dit
  connecté » (l'inverse de R1), ou au premier chantier qui touche le
  cycle de synchro.

### D-45 · Les vignettes visuelles des thèmes du Système sont la seule copie d'hex hors gate

- **Fait (PLAN-MONA, revue à regard neuf du 2026-08-29)** : chaque
  thème vit en quatre copies — `systeme.css`, `FICHES` (theme.js), la
  table de contrat du Système et ses vignettes visuelles. Les trois
  premières sont tenues entre elles par les gates (contrôles 1 et 3 de
  `system-coherence.mjs`) ; les vignettes ne le sont PAS : le
  contrôle 1 ne lit que les cellules `data-theme`/`data-jeton`, jamais
  les `style="background:#…"` ni les `title="--bg #…"` des swatches.
  Un jeton retouché après un constat terrain laisserait la vignette à
  l'ancienne couleur, gate verte, pour toujours. ~80 hex exposés
  (4 thèmes), le format `title="--jeton #hex"` est déjà
  machine-lisible.
- **Raison du report** : motif préexistant à Elements (PLAN-MONA n'a
  fait que doubler l'exposition) ; étendre le contrôle 1 aux swatches
  est un chantier de gate à part entière, hors périmètre d'un ajout de
  thème.
- **Rouvre si** : une retouche de jeton au terrain (le cas qui
  matérialise la dérive), ou au prochain chantier qui touche
  `system-coherence.mjs`.

### D-46 · L'anatomie de rangée du Portier est une copie main de celle de la Liste

- **Fait (PLAN-MODE-ORGANISE E2, revue à regard neuf du 2026-08-30)** :
  `Portier.svelte` recopie le dessin des rangées du volet central
  (`l1`/`exp`/`essor`/`heure`/`objet`/`apercu`, graisse du non-lu,
  disque centré) dans son propre `<style>` — le composant promet « LE
  format des rangées » mais aucun mécanisme ne le tient : la prochaine
  retouche du gabarit dans `Liste.svelte` (padding d'A83, calages
  optiques) n'atteindra pas le guichet, dérive silencieuse au pixel.
  Seul `.disque` vit déjà en global (`systeme.css:100`).
- **Raison du report** : promouvoir l'anatomie partagée au
  `systeme.css` touche `Liste.svelte` (le composant le plus chaud de
  l'UI, 8 tests fragiles recensés à l'audit e2e) — hors périmètre
  d'E2 ; la forme actuelle est celle validée au STOP visuel.
- **Rouvre si** : une retouche du gabarit de rangée (espacement,
  typographie) — le constat « le Portier n'a pas suivi » la
  matérialise —, ou au chantier E4 (la Réception organisée réutilise
  ces rangs en sections).

### D-47 · Trois menus contextuels et deux bascules de fil sont des copies main

- **Amendée (PLAN-AUDIT-V2 E11, 2026-09-02, A108) — les MENUS sont
  soldés** : `Menu.svelte` est LE menu du produit (huit surfaces —
  Liste, Kiosque, Portier, Nettoyage, Registre, tri des sections,
  Réglages > Portier, « Déplacer vers… » du fil), dessin ET mécanique
  en une copie (clavier compris, A8 tenu), 24 règles CSS de copies
  retirées, le jeton `--ombre` inexistant avec elles. **Reste ouvert**
  la moitié « cœur » de cette dette : `toggle_mis_de_cote`/
  `etat_mis_de_cote` jumeaux de `toggle_pin`/`pin_state`, et la pile /
  le rang du Registre recopiés du Kiosque — vague 3 de l'audit.

- **Amendée (RETOURS-14, 2026-08-31)** : deux copies de plus — le
  `.menu-groupe` du Registre groupé (`Registre.svelte`), et la FAMILLE
  s'étend au dessin de la PILE (`.empile`/`.rang-groupe` recopiés de
  `Kiosque.svelte` vers `Registre.svelte`) et au rang deux-lignes du
  Nettoyage (`.l1/.l2` recopiés). Le vocabulaire des verdicts, lui,
  a été factorisé au passage (`lib/portier.js`).
- **Fait (revue E4/E5, 2026-08-30)** : le dessin du menu du produit vit
  en trois copies CSS (`Portier.svelte` `.menu`, `Liste.svelte`
  `.menu-gestes`, l'éventail de `PileMisDeCote.svelte`) — l'ombre
  `0 8px 24px` y est déjà écrite trois fois, `min-width` diverge de
  10 px sans raison, et Portier seul passe par `var(--ombre, …)`. Côté
  cœur, `toggle_mis_de_cote`/`etat_mis_de_cote` sont le jumeau
  structurel de `toggle_pin`/`pin_state` (~80 lignes, seule la table
  change), et `pile_mis_de_cote` celui de `pinned_unified_scoped`.
- **Raison du report** : factoriser le menu = un composant partagé qui
  touche trois surfaces validées au STOP visuel ; les jumeaux du cœur
  sont chacun couverts par leurs tests — le refactor n'apporte rien au
  terrain de la release en cours.
- **Rouvre si** : un jeton d'ombre/menu entre à la table des thèmes
  (la copie divergerait à la première retouche), au troisième jumeau
  (E6 groupes), ou à la prochaine retouche du contrat de résolution de
  fil (le RED « jamais la tête » devrait être porté deux fois).
- **ROUVERTE le 2026-08-30 (PLAN-HORIZON-NETTOYAGE, revue)** : le
  Nettoyage de printemps est le jumeau annoncé — `Nettoyage.svelte`
  recopie du Portier le menu ⋯ entier (markup, `ouvrirMini` et ses
  bornes 250/170 en dur, cartes `BOITE_DE`/`TOAST_NON`, CSS
  `.btn-portier`/`.mini`/`.menu`), 4e copie du dessin. Non factorisée
  dans le chantier (trois surfaces validées au STOP visuel, même
  raison qu'au report) — **à traiter en dette dédiée** : un
  `MenuVerdict.svelte` partagé Portier/Nettoyage, et les classes
  communes en `systeme.css` (le précédent `.entete-vue`). S'y ajoute
  la paire de styles `select` née en deux copies (GuichetCompte 40 px
  / Réglages 32 px).

### D-48 · La liste ne suit pas une écriture externe

- **Fait (revue RETOURS-13, 2026-08-30)** : la Réception ne se
  recharge qu'au battement d'une génération de relève ou par ses
  propres handlers de gestes. Un `retirer_routage` (ou toute écriture
  hors des chemins de la Liste — second poste, rejeu, commande e2e)
  laisse la liste périmée jusqu'à une navigation manuelle. Le pas e2e
  de `organized-mode.spec.js` qui « passait » vivait d'une recharge
  FORTUITE de la sonde — il ressert désormais par l'aller-retour de
  dossier, honnêtement. Même famille que D-44 (`connectes` sans cycle
  de rafraîchissement).
- **Raison du report** : le correctif propre est un signal
  d'invalidation générique (bump de génération à toute écriture du
  cœur qui change une vue), pas un `liste.recharger()` de plus câblé
  par surface — un chantier, pas une retouche.
- **Rouvre si** : un constat terrain « la liste ne bouge pas » sur un
  geste hors-Liste, ou au chantier multi-fenêtres/second poste.

### D-55 · The database, the disk files, the `prefs` keys and the localStorage keys stay French

- **Finding** (PLAN-BASCULE-ANGLAIS, Chief Engineer decision D3 of 2026-09-02;
  GLOSSARY §1.6): the SQLite schema (26 tables, ~30 French columns),
  the six `prefs` keys, the files on disk (`wind.db`, `wind.log`,
  `maj.log`, `telemetry.json`, `discovery.db`) and the browser
  `localStorage` keys (`wind-theme`, `wind-volets`, `wind-largeurs`,
  `wind-espacement`, `wind-accueil-*`) keep their French names while
  every identifier around them is English. The PLAN and the glossary
  cite this debt as “D-54” — that number was already the flaky spec
  above; it is D-55 from here.
- **Why deferred**: renaming a column is a migration on every tester's
  database; renaming a storage key silently resets their layout. Not a
  behavior change the switch may embed (§5).
- **Done on the way (E5b, 2026-09-03)**: two persisted VALUES did move,
  with a read-side legacy map and no reset — the pane width shape
  (`{ nav, liste }` → `{ nav, list }`) and the row spacing levels
  (`faible|moyen|eleve` → `low|medium|high`). The keys themselves are
  untouched.
- **Reopen if**: a schema migration is scheduled for another reason
  (rename the columns in the same migration), or the storage keys get a
  versioned envelope.

### D-56 · Shell-composed text stays French while the UI may be English

Opened on 2026-09-03 (PLAN-BASCULE-ANGLAIS E5, CE decision D17). The size
units of `human_size` (`o`, `Ko`, `Mo` — attachments, drafts, the outbox),
the two native dialogs of `main.rs` (second instance, failed relocation)
and the one shell error string a spec asserts are composed by the shell in
French, marked `lang:fr`, whatever the UI language. The clean fix is a
behavior change the switch refuses to embed (§5): send bytes on the wire
and format in the UI per language; give the dialogs an English text when
`prefs.lang` is `en`. A small dedicated job once the switch is closed.

**Seen in the field on 2026-09-03 (E6b)**: in the English interface the
compose weight reads `2.8 Mo / 25 MB` — the total from `human_size`
(shell, French), the limit from the catalogue (English). The spec
asserts it as shipped (`redesign-screen02.spec.js`).

### D-57 · The onboarding illustrations are French screenshots inside an English default UI

Opened on 2026-09-03 (PLAN-BASCULE-ANGLAIS E6b, Chief Engineer decision
D28). `assets/accueil/disposition-{1,2,3}.png` are screenshots of the
French interface, captured by `e2e/capture-onboarding.mjs` (pinned to
`lang: 'fr'` so a replay does not change a visible asset without a
decision). The rule the Chief Engineer set: **every screenshot shown to
the user is in the language the user chose** — one set per language,
selected with the catalogue. To do at the next onboarding job: capture
both sets, select per `lang`, unpin the script.
